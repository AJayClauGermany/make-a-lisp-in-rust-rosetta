//! The `logos` reader row: a derived lexer plus a hand-written parser.
//!
//! The whole token grammar lives in one `#[derive(Logos)]` enum: trivia and
//! comments are `skip` patterns, literals are parsed into their payloads by
//! token callbacks, and an unterminated string is the only input the lexer
//! rejects. What logos does not do is structure, so a small recursive-descent
//! parser pulls tokens straight off the [`logos::Lexer`] iterator and builds
//! lists, vectors, maps and reader-macro forms.

use std::fmt;

use logos::{Lexer, Logos};
use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::{MalError, Value};

/// Table entry for [`LogosReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "logos",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "logos",
        version: "0.16.1",
        url: "https://github.com/maciejhirsz/logos",
    }],
    highest_step: Step::Mal,
    notes: "#[derive(Logos)] token enum with literal callbacks; hand-written recursive descent over the lexer",
};

/// Reads mal source with a logos-generated lexer.
#[derive(Debug, Default, Clone, Copy)]
pub struct LogosReader;

impl Reader for LogosReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let mut lexer = Token::lexer(input);
        match lexer.next() {
            None => Ok(ReadOutcome::Empty),
            Some(token) => parse_form(&mut lexer, token?).map(ReadOutcome::Form),
        }
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

/// What the lexer itself can reject.
///
/// Every character starts some token, so the only lexing failure is a
/// string literal with no closing quote; logos still needs a `Default`
/// variant for "no pattern matched".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum LexError {
    /// A `"` with no closing `"` before the end of input.
    #[default]
    UnterminatedString,
    /// An integer literal that does not fit in 64 bits.
    IntegerOverflow(String),
}

impl From<LexError> for ReadError {
    fn from(e: LexError) -> Self {
        match e {
            LexError::UnterminatedString => ReadError::UnexpectedEof { expected: "'\"'" },
            LexError::IntegerOverflow(token) => ReadError::IntegerOverflow(token),
        }
    }
}

/// The mal token grammar.
///
/// Symbols are "anything else": they may not start with a character that
/// begins another token (`~`, `^`, `@`), but may contain one. Integers and
/// keywords also match the symbol pattern, so they carry an explicit
/// priority (logos refuses to compile the ambiguity otherwise); literal
/// `#[token]`s such as `nil` already outrank regexes. Longest match still
/// makes `nilly` and `123abc` symbols.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(error = LexError)]
#[logos(skip r"[\s,]+")]
// A comment runs to the end of the line. logos flags `[^\n]*` as a greedy
// "dot" repetition, but stopping at the newline is exactly the intent.
#[logos(skip(r";[^\n]*", allow_greedy = true))]
enum Token<'src> {
    #[token("(")]
    OpenParen,
    #[token(")")]
    CloseParen,
    #[token("[")]
    OpenBracket,
    #[token("]")]
    CloseBracket,
    #[token("{")]
    OpenBrace,
    #[token("}")]
    CloseBrace,

    #[token("'")]
    Quote,
    #[token("`")]
    Quasiquote,
    #[token("~")]
    Unquote,
    #[token("~@")]
    SpliceUnquote,
    #[token("@")]
    Deref,
    #[token("^")]
    Meta,

    #[token("nil")]
    Nil,
    #[token("true", |_| true)]
    #[token("false", |_| false)]
    Bool(bool),
    #[regex("-?[0-9]+", parse_int, priority = 10)]
    Int(i64),
    #[regex(r#""(?:[^"\\]|\\(?s:.))*""#, unescape)]
    Str(String),
    #[regex(r#":[^\s\[\]{}()'"`,;]*"#, |lex| &lex.slice()[1..], priority = 10)]
    Keyword(&'src str),
    #[regex(r#"[^\s\[\]{}()'"`,;~^@][^\s\[\]{}()'"`,;]*"#)]
    Symbol(&'src str),
}

fn parse_int<'src>(lex: &mut Lexer<'src, Token<'src>>) -> Result<i64, LexError> {
    lex.slice()
        .parse()
        .map_err(|_| LexError::IntegerOverflow(lex.slice().to_owned()))
}

/// Strip the quotes and resolve escapes: `\n` is a newline, and any other
/// escaped character (`\"`, `\\`, ...) stands for itself.
fn unescape<'src>(lex: &mut Lexer<'src, Token<'src>>) -> String {
    let body = &lex.slice()[1..lex.slice().len() - 1];
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            // The regex guarantees a character follows every backslash.
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            },
            c => out.push(c),
        }
    }
    out
}

/// Turn one already-lexed token into a form, reading further tokens as
/// needed.
fn parse_form<'src>(
    lexer: &mut Lexer<'src, Token<'src>>,
    token: Token<'src>,
) -> Result<Value, ReadError> {
    Ok(match token {
        Token::OpenParen => Value::list(parse_seq(lexer, &Token::CloseParen, "')'")?),
        Token::OpenBracket => Value::vector(parse_seq(lexer, &Token::CloseBracket, "']'")?),
        Token::OpenBrace => Value::map_from_pairs(parse_seq(lexer, &Token::CloseBrace, "'}'")?)?,
        Token::CloseParen => return Err(ReadError::UnexpectedClose(')')),
        Token::CloseBracket => return Err(ReadError::UnexpectedClose(']')),
        Token::CloseBrace => return Err(ReadError::UnexpectedClose('}')),
        Token::Quote => Value::wrap("quote", next_form(lexer)?),
        Token::Quasiquote => Value::wrap("quasiquote", next_form(lexer)?),
        Token::Unquote => Value::wrap("unquote", next_form(lexer)?),
        Token::SpliceUnquote => Value::wrap("splice-unquote", next_form(lexer)?),
        Token::Deref => Value::wrap("deref", next_form(lexer)?),
        Token::Meta => {
            let meta = next_form(lexer)?;
            let form = next_form(lexer)?;
            Value::list(vec![Value::symbol("with-meta"), form, meta])
        }
        Token::Nil => Value::Nil,
        Token::Bool(b) => Value::Bool(b),
        Token::Int(i) => Value::Int(i),
        Token::Str(s) => Value::string(&s),
        Token::Keyword(k) => Value::keyword(k),
        Token::Symbol(s) => Value::symbol(s),
    })
}

/// Read the next form, which must exist.
fn next_form<'src>(lexer: &mut Lexer<'src, Token<'src>>) -> Result<Value, ReadError> {
    match lexer.next() {
        None => Err(ReadError::UnexpectedEof { expected: "a form" }),
        Some(token) => parse_form(lexer, token?),
    }
}

/// Read forms up to `close`, which the caller's opener has already consumed.
fn parse_seq<'src>(
    lexer: &mut Lexer<'src, Token<'src>>,
    close: &Token<'src>,
    expected: &'static str,
) -> Result<Vec<Value>, ReadError> {
    let mut items = Vec::new();
    loop {
        match lexer.next() {
            None => return Err(ReadError::UnexpectedEof { expected }),
            Some(token) => {
                let token = token?;
                if token == *close {
                    return Ok(items);
                }
                items.push(parse_form(lexer, token)?);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(LogosReader::default);
    }

    #[test]
    fn errors_name_the_missing_delimiter() {
        let err = LogosReader.read_str("(1 [2").unwrap_err();
        assert_eq!(err.to_string(), "unbalanced input: expected ']', got EOF");
        assert_eq!(
            LogosReader.read_str(")").unwrap_err(),
            ReadError::UnexpectedClose(')')
        );
        assert!(matches!(
            LogosReader.read_str("99999999999999999999"),
            Err(ReadError::IntegerOverflow(_))
        ));
    }

    #[test]
    fn reader_macro_characters_inside_symbols() {
        assert_eq!(
            LogosReader.read_str("a@b").unwrap(),
            ReadOutcome::Form(Value::symbol("a@b"))
        );
    }
}
