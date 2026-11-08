//! The `nom` reader row: parser combinators over `&str`.
//!
//! Each piece of the grammar is a plain `fn(&str) -> IResult<..>` built from
//! nom's `complete` combinators, and the recursive `form` parser calls
//! itself directly. Everything after an opening delimiter, a reader macro or
//! an opening quote is wrapped in [`cut`], so a missing `)` or `"` is a hard
//! failure at the end of input rather than a backtrack into another branch.
//!
//! nom's own [`nom::error::Error`] records only an [`ErrorKind`] and the
//! remaining input, which cannot say "expected `)`" or carry an integer
//! overflow, so this row threads a small custom error type through the
//! parsers (the approach nom's error-management guide recommends) and turns
//! it into the public [`ReadError`] at the end.

use std::fmt;

use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::{MalError, Value};
use nom::branch::alt;
use nom::bytes::complete::{escaped_transform, is_not, tag, take_till, take_while, take_while1};
use nom::character::complete::{anychar, char, digit1, satisfy};
use nom::combinator::{cut, eof, map, map_res, not, opt, recognize, value};
use nom::error::{ErrorKind, FromExternalError, ParseError};
use nom::multi::{many0, many0_count};
use nom::sequence::{pair, preceded, terminated};
use nom::{IResult, Parser};

/// Table entry for [`NomReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "nom",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "nom",
        version: "8.0.0",
        url: "https://github.com/rust-bakery/nom",
    }],
    highest_step: Step::Mal,
    notes: "nom 8 combinator fns with cut after openers; custom ParseError for EOF messages",
};

/// Reads mal source with nom parser combinators.
#[derive(Debug, Default, Clone, Copy)]
pub struct NomReader;

impl Reader for NomReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let empty = value(ReadOutcome::Empty, preceded(trivia, eof));
        match alt((empty, map(form, ReadOutcome::Form))).parse(input) {
            // Trailing input after the first form is ignored.
            Ok((_rest, outcome)) => Ok(outcome),
            Err(nom::Err::Error(e) | nom::Err::Failure(e)) => Err(e.into()),
            // Only streaming parsers report `Incomplete`; the `complete` ones
            // used here fail at the end of input instead.
            Err(nom::Err::Incomplete(_)) => Err(ReadError::UnexpectedEof { expected: None }),
        }
    }
}

/// Why a form could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// Input ended inside a list, vector, map or string, or after a reader
    /// macro.
    UnexpectedEof {
        /// The closing delimiter the reader was waiting for, if it knows.
        expected: Option<char>,
    },
    /// A character that cannot start or continue a form here, such as a
    /// closing delimiter with no matching opener.
    Unexpected(char),
    /// A hash-map literal with an odd number of forms or a bad key.
    BadMap(String),
    /// An integer literal that does not fit in 64 bits.
    IntegerOverflow(String),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::UnexpectedEof {
                expected: Some(close),
            } => write!(f, "unbalanced input: expected '{close}', got EOF"),
            ReadError::UnexpectedEof { expected: None } => {
                f.write_str("unbalanced input: expected a form, got EOF")
            }
            ReadError::Unexpected(c) => write!(f, "unexpected '{c}'"),
            ReadError::BadMap(message) => f.write_str(message),
            ReadError::IntegerOverflow(token) => write!(f, "integer out of range: {token}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// The error type threaded through the parsers: where it happened and why.
#[derive(Debug)]
struct Failure<'a> {
    input: &'a str,
    reason: Reason,
}

#[derive(Debug)]
enum Reason {
    /// A nom combinator failed; which one does not matter for the message.
    Syntax,
    /// [`char`] did not find the character it wanted.
    Expected(char),
    /// [`Value::map_from_pairs`] rejected the map's contents.
    BadMap(MalError),
    /// The digits of an integer do not fit in an `i64`.
    Overflow(String),
}

impl<'a> ParseError<&'a str> for Failure<'a> {
    fn from_error_kind(input: &'a str, _kind: ErrorKind) -> Self {
        Failure {
            input,
            reason: Reason::Syntax,
        }
    }

    fn append(_input: &'a str, _kind: ErrorKind, other: Self) -> Self {
        other
    }

    fn from_char(input: &'a str, c: char) -> Self {
        Failure {
            input,
            reason: Reason::Expected(c),
        }
    }
}

impl<'a> FromExternalError<&'a str, MalError> for Failure<'a> {
    fn from_external_error(input: &'a str, _kind: ErrorKind, e: MalError) -> Self {
        Failure {
            input,
            reason: Reason::BadMap(e),
        }
    }
}

impl From<Failure<'_>> for ReadError {
    fn from(failure: Failure<'_>) -> Self {
        match (failure.reason, failure.input.chars().next()) {
            (Reason::BadMap(e), _) => ReadError::BadMap(e.to_string()),
            (Reason::Overflow(token), _) => ReadError::IntegerOverflow(token),
            (Reason::Expected(c), None) => ReadError::UnexpectedEof { expected: Some(c) },
            (Reason::Syntax, None) => ReadError::UnexpectedEof { expected: None },
            (Reason::Expected(_) | Reason::Syntax, Some(found)) => ReadError::Unexpected(found),
        }
    }
}

type PResult<'a, O> = IResult<&'a str, O, Failure<'a>>;

/// Characters that may appear in a symbol, number or keyword.
fn is_symbol_char(c: char) -> bool {
    !(c.is_whitespace()
        || matches!(
            c,
            '[' | ']' | '{' | '}' | '(' | ')' | '\'' | '"' | '`' | ',' | ';'
        ))
}

/// Whitespace, commas and `;` comments.
fn trivia(input: &str) -> PResult<'_, ()> {
    let blank = take_while1(|c: char| c.is_whitespace() || c == ',');
    let comment = preceded(char(';'), take_till(|c| c == '\n'));
    value((), many0_count(alt((blank, comment)))).parse(input)
}

/// One form, after any leading trivia.
fn form(input: &str) -> PResult<'_, Value> {
    preceded(
        trivia,
        alt((
            map(seq('(', ')'), Value::list),
            map(seq('[', ']'), Value::vector),
            preceded(
                char('{'),
                cut(map_res(seq_tail('}'), Value::map_from_pairs)),
            ),
            reader_macro("'", "quote"),
            reader_macro("`", "quasiquote"),
            reader_macro("~@", "splice-unquote"),
            reader_macro("~", "unquote"),
            reader_macro("@", "deref"),
            with_meta,
            map(string, |s| Value::string(&s)),
            map(integer, Value::Int),
            map(
                preceded(char(':'), take_while(is_symbol_char)),
                Value::keyword,
            ),
            map(take_while1(is_symbol_char), symbol),
        )),
    )
    .parse(input)
}

/// `open form* close`, committed once `open` matches.
fn seq<'a>(
    open: char,
    close: char,
) -> impl Parser<&'a str, Output = Vec<Value>, Error = Failure<'a>> {
    preceded(char(open), cut(seq_tail(close)))
}

/// The forms of a sequence up to and including its closing delimiter.
fn seq_tail<'a>(close: char) -> impl Parser<&'a str, Output = Vec<Value>, Error = Failure<'a>> {
    terminated(many0(form), preceded(trivia, char(close)))
}

/// `'x` and friends: `prefix form` reads as `(name form)`.
fn reader_macro<'a>(
    prefix: &'static str,
    name: &'static str,
) -> impl Parser<&'a str, Output = Value, Error = Failure<'a>> {
    map(preceded(tag(prefix), cut(form)), move |form| {
        Value::wrap(name, form)
    })
}

/// `^meta form` reads as `(with-meta form meta)`.
fn with_meta(input: &str) -> PResult<'_, Value> {
    map(
        preceded(char('^'), cut(pair(form, form))),
        |(meta, form)| Value::list(vec![Value::symbol("with-meta"), form, meta]),
    )
    .parse(input)
}

/// A string literal with `\n`, `\"` and `\\` escapes; any other escaped
/// character stands for itself.
fn string(input: &str) -> PResult<'_, String> {
    let escape = alt((value('\n', char('n')), anychar));
    // `escaped_transform` rejects an empty body, so `""` needs the `opt`.
    let body = map(
        opt(escaped_transform(is_not("\"\\"), '\\', escape)),
        Option::unwrap_or_default,
    );
    preceded(char('"'), cut(terminated(body, char('"')))).parse(input)
}

/// `-?[0-9]+` not followed by another symbol character.
fn integer(input: &str) -> PResult<'_, i64> {
    let digits = recognize(pair(opt(char('-')), digit1));
    let (rest, token) = terminated(digits, not(satisfy(is_symbol_char))).parse(input)?;
    // The token is definitely an integer now, so overflow must not backtrack
    // into the symbol branch.
    let n = token.parse().map_err(|_| {
        nom::Err::Failure(Failure {
            input,
            reason: Reason::Overflow(token.to_owned()),
        })
    })?;
    Ok((rest, n))
}

/// `nil`, `true`, `false` or a symbol.
fn symbol(token: &str) -> Value {
    match token {
        "nil" => Value::Nil,
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::symbol(token),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(NomReader::default);
    }

    #[test]
    fn errors_name_the_missing_delimiter() {
        let err = NomReader.read_str("(1 [2").unwrap_err();
        assert_eq!(err.to_string(), "unbalanced input: expected ']', got EOF");
        assert_eq!(
            NomReader.read_str(")").unwrap_err(),
            ReadError::Unexpected(')')
        );
        assert_eq!(
            NomReader.read_str("99999999999999999999").unwrap_err(),
            ReadError::IntegerOverflow("99999999999999999999".to_owned())
        );
        assert!(matches!(
            NomReader.read_str("{1 2}").unwrap_err(),
            ReadError::BadMap(_)
        ));
    }
}
