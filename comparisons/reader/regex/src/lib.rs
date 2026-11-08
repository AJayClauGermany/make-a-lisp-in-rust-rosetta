//! The `regex` reader row: the mal guide's own approach.
//!
//! One regular expression, copied from the guide, splits the input into
//! tokens; a recursive-descent `read_form` / `read_list` / `read_atom` then
//! walks the token list, exactly as step 1 of the guide lays it out. The
//! guide also leans on small regexes to classify atoms, so integers, string
//! literals and string escapes are recognised with regexes too. Every regex
//! is compiled once, on first use, into a `static` [`LazyLock`], as the
//! `regex` documentation recommends.

use std::fmt;
use std::sync::LazyLock;

use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::{MalError, Value};
use regex::Regex;

/// Table entry for [`RegexReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "regex",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "regex",
        version: "1.13.1",
        url: "https://github.com/rust-lang/regex",
    }],
    highest_step: Step::Mal,
    notes: "the guide's single tokenizer regex, then recursive descent over the token list",
};

/// The tokenizer from the mal guide. Group 1 is the token; the leading
/// `[\s,]*` swallows trivia.
static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"[\s,]*(~@|[\[\]{}()'`~^@]|"(?:\\.|[^\\"])*"?|;.*|[^\s\[\]{}('"`,;)]*)"#)
        .expect("tokenizer regex is valid")
});

/// `-?[0-9]+`, as a whole token.
static INTEGER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?[0-9]+$").expect("integer regex is valid"));

/// A string token with its closing quote. The tokenizer's trailing `"?`
/// also lets an unterminated string through as a token, which this rejects.
static STRING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^"(?:\\.|[^\\"])*"$"#).expect("string regex is valid"));

/// A backslash escape inside a string literal.
static ESCAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\(.)").expect("escape regex is valid"));

/// Reads mal source the way the mal guide describes.
#[derive(Debug, Default, Clone, Copy)]
pub struct RegexReader;

impl Reader for RegexReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let mut tokens = Tokens {
            tokens: tokenize(input),
            pos: 0,
        };
        if tokens.peek().is_none() {
            return Ok(ReadOutcome::Empty);
        }
        tokens.read_form().map(ReadOutcome::Form)
    }
}

/// Why a form could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// Input ended inside a list, vector, map or string, or after a reader
    /// macro.
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

/// Split `input` into tokens, dropping comments and the empty match the
/// tokenizer makes at the end of input.
fn tokenize(input: &str) -> Vec<&str> {
    TOKEN
        .captures_iter(input)
        .map(|caps| caps.get(1).map_or("", |m| m.as_str()))
        .filter(|token| !token.is_empty() && !token.starts_with(';'))
        .collect()
}

/// The guide's `Reader` object: the token list and a position in it.
struct Tokens<'a> {
    tokens: Vec<&'a str>,
    pos: usize,
}

impl<'a> Tokens<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.tokens.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<&'a str> {
        let token = self.peek()?;
        self.pos += 1;
        Some(token)
    }

    fn read_form(&mut self) -> Result<Value, ReadError> {
        let Some(token) = self.next() else {
            return Err(ReadError::UnexpectedEof { expected: "a form" });
        };
        match token {
            "(" => self.read_list(")").map(Value::list),
            "[" => self.read_list("]").map(Value::vector),
            "{" => Ok(Value::map_from_pairs(self.read_list("}")?)?),
            ")" => Err(ReadError::UnexpectedClose(')')),
            "]" => Err(ReadError::UnexpectedClose(']')),
            "}" => Err(ReadError::UnexpectedClose('}')),
            "'" => self.read_macro("quote"),
            "`" => self.read_macro("quasiquote"),
            "~" => self.read_macro("unquote"),
            "~@" => self.read_macro("splice-unquote"),
            "@" => self.read_macro("deref"),
            "^" => {
                let meta = self.read_form()?;
                let form = self.read_form()?;
                Ok(Value::list(vec![Value::symbol("with-meta"), form, meta]))
            }
            _ => read_atom(token),
        }
    }

    fn read_list(&mut self, close: &'static str) -> Result<Vec<Value>, ReadError> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None => {
                    return Err(ReadError::UnexpectedEof {
                        expected: match close {
                            ")" => "')'",
                            "]" => "']'",
                            _ => "'}'",
                        },
                    });
                }
                Some(token) if token == close => {
                    self.next();
                    return Ok(items);
                }
                Some(_) => items.push(self.read_form()?),
            }
        }
    }

    fn read_macro(&mut self, name: &str) -> Result<Value, ReadError> {
        Ok(Value::wrap(name, self.read_form()?))
    }
}

fn read_atom(token: &str) -> Result<Value, ReadError> {
    Ok(match token {
        "nil" => Value::Nil,
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ if INTEGER.is_match(token) => token
            .parse()
            .map(Value::Int)
            .map_err(|_| ReadError::IntegerOverflow(token.to_owned()))?,
        _ if STRING.is_match(token) => {
            let body = &token[1..token.len() - 1];
            let unescaped = ESCAPE.replace_all(body, |caps: &regex::Captures<'_>| {
                match &caps[1] {
                    "n" => "\n".to_owned(),
                    // `\"`, `\\` and anything else unknown stand for themselves.
                    other => other.to_owned(),
                }
            });
            Value::string(&unescaped)
        }
        _ if token.starts_with('"') => {
            return Err(ReadError::UnexpectedEof { expected: "'\"'" });
        }
        _ => match token.strip_prefix(':') {
            Some(keyword) => Value::keyword(keyword),
            None => Value::symbol(token),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(RegexReader::default);
    }

    #[test]
    fn errors_name_the_missing_delimiter() {
        let err = RegexReader.read_str("(1 [2").unwrap_err();
        assert_eq!(err.to_string(), "unbalanced input: expected ']', got EOF");
        assert_eq!(
            RegexReader.read_str(")").unwrap_err(),
            ReadError::UnexpectedClose(')')
        );
        assert!(matches!(
            RegexReader.read_str("99999999999999999999"),
            Err(ReadError::IntegerOverflow(_))
        ));
    }

    #[test]
    fn tokenizes_like_the_guide() {
        assert_eq!(
            tokenize("(~@a, @b ^c \"d\\\"e\" ; f\n)"),
            ["(", "~@", "a", "@", "b", "^", "c", "\"d\\\"e\"", ")"]
        );
    }
}
