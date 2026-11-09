//! The `std` reader row: a hand-written recursive-descent parser.
//!
//! No tokenizer pass: the parser walks the input once, by byte offset, and
//! builds values as it goes. This is the baseline every other reader row is
//! measured against, and the zero point for the build-time and binary-size
//! columns.

use std::fmt;

use mal_traits::{Component, Implementation, ReadOutcome, Reader, Step};
use mal_types::{MalError, Value};

/// Table entry for [`StdReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "std",
    component: Component::Reader,
    crates: &[],
    highest_step: Step::Mal,
    notes: "hand-written recursive descent over byte offsets; no token pass",
};

/// Reads mal source with nothing but the standard library.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdReader;

impl Reader for StdReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let mut parser = Parser { input, pos: 0 };
        parser.skip_trivia();
        if parser.peek().is_none() {
            return Ok(ReadOutcome::Empty);
        }
        parser.read_form().map(ReadOutcome::Form)
    }
}

/// Why a form could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// Input ended inside a list, vector or map, or after a reader macro.
    UnexpectedEof {
        /// What the reader was waiting for.
        expected: &'static str,
    },
    /// A closing delimiter with no matching opener.
    UnexpectedClose(char),
    /// A hash-map literal with an odd number of forms or a bad key.
    BadMap(String),
    /// An integer literal that does not fit in 64 bits.
    IntegerOverflow(String),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::UnexpectedEof { expected } => {
                write!(f, "unbalanced input: expected {expected}, got EOF")
            }
            ReadError::UnexpectedClose(c) => write!(f, "unexpected '{c}'"),
            ReadError::BadMap(message) => f.write_str(message),
            ReadError::IntegerOverflow(token) => write!(f, "integer out of range: {token}"),
        }
    }
}

impl std::error::Error for ReadError {}

impl From<MalError> for ReadError {
    fn from(e: MalError) -> Self {
        ReadError::BadMap(e.to_string())
    }
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

/// Characters that end a symbol, number or keyword.
fn is_delimiter(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '[' | ']' | '{' | '}' | '(' | ')' | '\'' | '"' | '`' | ',' | ';'
        )
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Skip whitespace, commas and `;` comments.
    fn skip_trivia(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() || c == ',' {
                self.bump();
            } else if c == ';' {
                let rest = &self.input[self.pos..];
                self.pos += rest.find('\n').unwrap_or(rest.len());
            } else {
                break;
            }
        }
    }

    fn read_form(&mut self) -> Result<Value, ReadError> {
        self.skip_trivia();
        let Some(c) = self.bump() else {
            return Err(ReadError::UnexpectedEof { expected: "a form" });
        };
        match c {
            '(' => self.read_seq(')').map(Value::list),
            '[' => self.read_seq(']').map(Value::vector),
            '{' => Ok(Value::map_from_pairs(self.read_seq('}')?)?),
            ')' | ']' | '}' => Err(ReadError::UnexpectedClose(c)),
            '\'' => self.read_macro("quote"),
            '`' => self.read_macro("quasiquote"),
            '~' if self.peek() == Some('@') => {
                self.bump();
                self.read_macro("splice-unquote")
            }
            '~' => self.read_macro("unquote"),
            '@' => self.read_macro("deref"),
            '^' => {
                let meta = self.read_form()?;
                let form = self.read_form()?;
                Ok(Value::list(vec![Value::symbol("with-meta"), form, meta]))
            }
            '"' => self.read_string(),
            _ => self.read_atom(self.pos - c.len_utf8()),
        }
    }

    fn read_seq(&mut self, close: char) -> Result<Vec<Value>, ReadError> {
        let mut items = Vec::new();
        loop {
            self.skip_trivia();
            match self.peek() {
                None => {
                    return Err(ReadError::UnexpectedEof {
                        expected: match close {
                            ')' => "')'",
                            ']' => "']'",
                            _ => "'}'",
                        },
                    });
                }
                Some(c) if c == close => {
                    self.bump();
                    return Ok(items);
                }
                Some(_) => items.push(self.read_form()?),
            }
        }
    }

    fn read_macro(&mut self, name: &str) -> Result<Value, ReadError> {
        Ok(Value::wrap(name, self.read_form()?))
    }

    fn read_string(&mut self) -> Result<Value, ReadError> {
        let mut s = String::new();
        loop {
            match self.bump() {
                None => return Err(ReadError::UnexpectedEof { expected: "'\"'" }),
                Some('"') => return Ok(Value::string(&s)),
                Some('\\') => match self.bump() {
                    None => return Err(ReadError::UnexpectedEof { expected: "'\"'" }),
                    Some('n') => s.push('\n'),
                    // `\"`, `\\` and anything else unknown stand for themselves.
                    Some(other) => s.push(other),
                },
                Some(c) => s.push(c),
            }
        }
    }

    fn read_atom(&mut self, start: usize) -> Result<Value, ReadError> {
        while self.peek().is_some_and(|c| !is_delimiter(c)) {
            self.bump();
        }
        let token = &self.input[start..self.pos];
        Ok(match token {
            "nil" => Value::Nil,
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ if is_integer(token) => token
                .parse()
                .map(Value::Int)
                .map_err(|_| ReadError::IntegerOverflow(token.to_owned()))?,
            _ => match token.strip_prefix(':') {
                Some(keyword) => Value::keyword(keyword),
                None => Value::symbol(token),
            },
        })
    }
}

/// `-?[0-9]+`
fn is_integer(token: &str) -> bool {
    let digits = token.strip_prefix('-').unwrap_or(token);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(StdReader::default);
    }

    #[test]
    fn errors_name_the_missing_delimiter() {
        let err = StdReader.read_str("(1 [2").unwrap_err();
        assert_eq!(err.to_string(), "unbalanced input: expected ']', got EOF");
        assert_eq!(
            StdReader.read_str(")").unwrap_err(),
            ReadError::UnexpectedClose(')')
        );
    }
}
