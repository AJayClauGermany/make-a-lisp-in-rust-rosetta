//! The `combine` reader row: LL(1) parser combinators, generic over the
//! input stream.
//!
//! combine commits to a branch as soon as it has consumed input: `choice`
//! only tries the next alternative when the previous one failed *without*
//! consuming anything. That is exactly what the mal grammar needs after an
//! opening delimiter, a reader macro or a quote, so no `cut` is written
//! anywhere; a missing `)` fails at the end of input instead of backtracking
//! into another branch, and `~` followed by nothing does not fall back to
//! reading `~` as a symbol.
//!
//! The grammar is written generically over any `Stream<Token = char>`, as
//! combine's documentation does, with the recursive `form` declared through
//! the `parser!` macro. `read_str` runs it on a position-tracking stream
//! with combine's `easy` errors and turns those, which borrow the input,
//! into an owned one-line message.

use std::fmt;

use combine::error::StreamError;
use combine::parser::char::char;
use combine::stream::{StreamErrorFor, easy, position};
use combine::{
    EasyParser, Parser, Stream, any, between, choice, eof, many, many1, none_of, optional, parser,
    satisfy, skip_many, skip_many1,
};
use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::Value;

/// Table entry for [`CombineReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "combine",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "combine",
        version: "4.6.9",
        url: "https://github.com/Marwes/combine",
    }],
    highest_step: Step::Mal,
    notes: "LL(1) combinators generic over the stream; commit-on-consume replaces cut; easy errors made owned",
};

impl mal_traits::Row for CombineReader {
    const IMPLEMENTATION: &'static Implementation = &IMPLEMENTATION;
}

/// Reads mal source with combine parser combinators.
#[derive(Debug, Default, Clone, Copy)]
pub struct CombineReader;

impl Reader for CombineReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let mut top = trivia().with(choice((
            eof().map(|()| ReadOutcome::Empty),
            form().map(ReadOutcome::Form),
        )));
        match top.easy_parse(position::Stream::new(input)) {
            // Trailing input after the first form is ignored.
            Ok((outcome, _rest)) => Ok(outcome),
            Err(errors) => Err(ReadError::from_easy(&errors)),
        }
    }
}

/// Why a form could not be read: combine's errors, rendered on one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError {
    /// Whether the parser ran out of input.
    pub eof: bool,
    /// Line and column the parser stopped at.
    pub position: (i32, i32),
    /// combine's unexpected/expected/message entries, joined.
    pub message: String,
}

impl ReadError {
    fn from_easy(errors: &easy::Errors<char, &str, position::SourcePosition>) -> Self {
        let eof = errors
            .errors
            .iter()
            .any(|e| *e == easy::Error::end_of_input());
        let message = errors
            .errors
            .iter()
            .map(ToString::to_string)
            .map(|line| line.trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("; ");
        Self {
            eof,
            position: (errors.position.line, errors.position.column),
            message,
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line, column) = self.position;
        if self.eof {
            f.write_str("unbalanced input: ")?;
        }
        write!(f, "{} (line {line}, column {column})", self.message)
    }
}

impl std::error::Error for ReadError {}

/// Characters that end a symbol, number or keyword.
fn is_delimiter(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '[' | ']' | '{' | '}' | '(' | ')' | '\'' | '"' | '`' | ',' | ';'
        )
}

/// Whitespace, commas and `;` comments.
fn trivia<Input>() -> impl Parser<Input, Output = ()>
where
    Input: Stream<Token = char>,
{
    let comment = char(';').with(skip_many(satisfy(|c| c != '\n')));
    skip_many(choice((
        skip_many1(satisfy(|c: char| c.is_whitespace() || c == ',')),
        comment,
    )))
}

fn string<Input>() -> impl Parser<Input, Output = Value>
where
    Input: Stream<Token = char>,
{
    let escape = char('\\')
        .with(any())
        .map(|c| if c == 'n' { '\n' } else { c });
    between(
        char('"'),
        char('"'),
        many(choice((escape, none_of(['"', '\\'])))),
    )
    .map(|s: String| Value::string(&s))
}

fn atom<Input>() -> impl Parser<Input, Output = Value>
where
    Input: Stream<Token = char>,
{
    many1(satisfy(|c| !is_delimiter(c))).and_then(|token: String| {
        Ok::<_, StreamErrorFor<Input>>(match token.as_str() {
            "nil" => Value::Nil,
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ if is_integer(&token) => Value::Int(token.parse().map_err(|_| {
                StreamErrorFor::<Input>::message_format(format!("integer out of range: {token}"))
            })?),
            _ => match token.strip_prefix(':') {
                Some(keyword) => Value::keyword(keyword),
                None => Value::symbol(&token),
            },
        })
    })
}

/// The forms up to `close`, after the opener has been consumed.
fn items<Input>(close: char) -> impl Parser<Input, Output = Vec<Value>>
where
    Input: Stream<Token = char>,
{
    many(form().skip(trivia())).skip(char(close))
}

parser! {
    /// One form, with leading trivia.
    fn form[Input]()(Input) -> Value
    where [Input: Stream<Token = char>]
    {
        let macro_form = |name: &'static str| form().map(move |f| Value::wrap(name, f));
        trivia().with(choice((
            char('(').skip(trivia()).with(items(')')).map(Value::list),
            char('[').skip(trivia()).with(items(']')).map(Value::vector),
            char('{')
                .skip(trivia())
                .with(items('}'))
                .and_then(|pairs| {
                    Value::map_from_pairs(pairs)
                        .map_err(StreamErrorFor::<Input>::message_format)
                }),
            char('\'').with(macro_form("quote")),
            char('`').with(macro_form("quasiquote")),
            char('~').with(optional(char('@'))).then(move |at| {
                macro_form(if at.is_some() { "splice-unquote" } else { "unquote" })
            }),
            char('@').with(macro_form("deref")),
            char('^').with((form(), form())).map(|(meta, form)| {
                Value::list(vec![Value::symbol("with-meta"), form, meta])
            }),
            string(),
            atom(),
        )))
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
        mal_testkit::check_reader(CombineReader::default);
    }

    #[test]
    fn errors_say_where() {
        let err = CombineReader.read_str("(1\n [2").unwrap_err();
        assert!(err.eof);
        assert_eq!(err.position, (2, 4));
        assert!(err.to_string().starts_with("unbalanced input: "), "{err}");
        let err = CombineReader.read_str(")").unwrap_err();
        assert!(!err.eof, "{err}");
        let err = CombineReader.read_str("99999999999999999999").unwrap_err();
        assert!(err.message.contains("integer out of range"), "{err}");
    }
}
