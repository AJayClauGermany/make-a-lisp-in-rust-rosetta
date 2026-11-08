//! The `pest` reader row: a PEG grammar in its own `.pest` file.
//!
//! `src/mal.pest` holds the whole token and form grammar, trivia included;
//! `pest_derive` turns it into a parser at compile time. The parser produces
//! an untyped tree of [`Pair`]s tagged with [`Rule`]s, and the Rust side
//! walks that tree to build [`Value`]s, so the grammar and the action code
//! live in two different files.
//!
//! Syntax errors are pest's own [`pest::error::Error`], which renders the
//! offending line with a caret. pest has no notion of "ran out of input", so
//! an error positioned at the very end of the input (or at the opening quote
//! of an unterminated string) is reported as [`ReadError::UnexpectedEof`] for
//! the mal test suite's sake.

use std::fmt;

use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::Value;
use pest::Parser as _;
use pest::Span;
use pest::error::{Error, ErrorVariant, InputLocation};
use pest::iterators::Pair;

/// Table entry for [`PestReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "pest",
    component: Component::Reader,
    crates: &[
        CrateRef {
            name: "pest",
            version: "2.9.5",
            url: "https://github.com/pest-parser/pest",
        },
        CrateRef {
            name: "pest_derive",
            version: "2.9.5",
            url: "https://github.com/pest-parser/pest",
        },
    ],
    highest_step: Step::Mal,
    notes: "PEG; grammar in a .pest file via derive macro, actions walk an untyped pair tree in Rust",
};

/// The parser `pest_derive` generates from `src/mal.pest`.
mod grammar {
    // The generated `Rule` enum has a variant per grammar rule, including
    // the silent helpers, which carry no doc comments.
    #![allow(missing_docs)]

    #[derive(pest_derive::Parser)]
    #[grammar = "mal.pest"]
    pub struct MalParser;
}

use grammar::MalParser;
pub use grammar::Rule;

/// Reads mal source with a pest grammar.
#[derive(Debug, Default, Clone, Copy)]
pub struct PestReader;

impl Reader for PestReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let mut pairs =
            MalParser::parse(Rule::input, input).map_err(|e| ReadError::new(e, input))?;
        match pairs.next() {
            Some(pair) if pair.as_rule() != Rule::EOI => to_value(pair)
                .map(ReadOutcome::Form)
                .map_err(|e| ReadError::new(e, input)),
            _ => Ok(ReadOutcome::Empty),
        }
    }
}

/// Why a form could not be read. Both variants carry pest's own error.
#[derive(Debug)]
pub enum ReadError {
    /// The input ended inside a list, vector, map, string or after a reader
    /// macro.
    UnexpectedEof(Box<Error<Rule>>),
    /// Any other syntax error, an integer that does not fit in 64 bits or a
    /// malformed hash-map.
    Syntax(Box<Error<Rule>>),
}

impl ReadError {
    /// Classify a pest error. pest reports a failure inside the atomic
    /// `string` rule at the string's opening quote rather than where the
    /// input ran out, and the only way a string can fail once it has started
    /// is by running out of input, so that counts as EOF too.
    fn new(error: Error<Rule>, input: &str) -> Self {
        let eof = match (&error.variant, &error.location) {
            (ErrorVariant::ParsingError { .. }, InputLocation::Pos(pos)) => {
                *pos == input.len() || input[*pos..].starts_with('"')
            }
            _ => false,
        };
        if eof {
            ReadError::UnexpectedEof(Box::new(error))
        } else {
            ReadError::Syntax(Box::new(error))
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::UnexpectedEof(e) => write!(f, "unbalanced input: unexpected EOF\n{e}"),
            ReadError::Syntax(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReadError::UnexpectedEof(e) | ReadError::Syntax(e) => Some(e),
        }
    }
}

/// A semantic error reported the way pest reports syntax errors.
fn custom(span: Span<'_>, message: impl Into<String>) -> Error<Rule> {
    Error::new_from_span(
        ErrorVariant::CustomError {
            message: message.into(),
        },
        span,
    )
}

/// Build a value from one `form` pair.
fn to_value(pair: Pair<'_, Rule>) -> Result<Value, Error<Rule>> {
    let span = pair.as_span();
    Ok(match pair.as_rule() {
        Rule::list => Value::list(children(pair)?),
        Rule::vector => Value::vector(children(pair)?),
        Rule::map => {
            Value::map_from_pairs(children(pair)?).map_err(|e| custom(span, e.to_string()))?
        }
        Rule::quote => wrap("quote", pair)?,
        Rule::quasiquote => wrap("quasiquote", pair)?,
        Rule::unquote => wrap("unquote", pair)?,
        Rule::splice_unquote => wrap("splice-unquote", pair)?,
        Rule::deref => wrap("deref", pair)?,
        Rule::with_meta => {
            let mut inner = pair.into_inner();
            let meta = to_value(inner.next().expect("grammar: ^ has a meta form"))?;
            let form = to_value(inner.next().expect("grammar: ^ has a target form"))?;
            Value::list(vec![Value::symbol("with-meta"), form, meta])
        }
        Rule::string => {
            let inner = pair.into_inner().next().expect("grammar: string has inner");
            Value::string(&unescape(inner.as_str()))
        }
        Rule::integer => pair
            .as_str()
            .parse()
            .map(Value::Int)
            .map_err(|_| custom(span, "integer out of range"))?,
        Rule::nil => Value::Nil,
        Rule::boolean => Value::Bool(pair.as_str() == "true"),
        Rule::keyword => Value::keyword(&pair.as_str()[1..]),
        Rule::symbol => Value::symbol(pair.as_str()),
        rule => unreachable!("grammar: {rule:?} is not a form"),
    })
}

fn children(pair: Pair<'_, Rule>) -> Result<Vec<Value>, Error<Rule>> {
    pair.into_inner().map(to_value).collect()
}

fn wrap(name: &str, pair: Pair<'_, Rule>) -> Result<Value, Error<Rule>> {
    let form = pair.into_inner().next().expect("grammar: macro has a form");
    Ok(Value::wrap(name, to_value(form)?))
}

/// Resolve `\n`; any other escaped character stands for itself.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(PestReader::default);
    }

    #[test]
    fn errors_say_why() {
        let err = PestReader.read_str("(1 [2").unwrap_err();
        assert!(matches!(err, ReadError::UnexpectedEof(_)), "{err}");
        let err = PestReader.read_str(")").unwrap_err();
        assert!(matches!(err, ReadError::Syntax(_)), "{err}");
        let err = PestReader.read_str("99999999999999999999").unwrap_err();
        assert!(err.to_string().contains("out of range"), "{err}");
        let err = PestReader.read_str("@").unwrap_err();
        assert!(matches!(err, ReadError::UnexpectedEof(_)), "{err}");
    }

    #[test]
    fn trailing_input_is_ignored() {
        assert_eq!(
            PestReader.read_str("1 \"abc").unwrap(),
            ReadOutcome::Form(Value::Int(1))
        );
    }
}
