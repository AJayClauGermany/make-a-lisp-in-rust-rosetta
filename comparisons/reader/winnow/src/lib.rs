//! The `winnow` reader row: parser combinators over `&mut &str`.
//!
//! Each piece of the grammar is a `fn(&mut &str) -> ModalResult<..>`, and
//! the recursive `form` parser branches on the next character with
//! [`dispatch!`](winnow::combinator::dispatch) rather than trying every
//! alternative in turn. Everything after an opening delimiter, a reader
//! macro or an opening quote is wrapped in [`cut_err`], so a missing `)` or
//! `"` is a hard failure at the end of input rather than a backtrack.
//!
//! Errors are winnow's own [`ContextError`], labelled with
//! [`StrContext`]s, and the top-level parser is run with [`Parser::parse`],
//! which reports the failing offset. That offset being the end of the input
//! is what makes an error an EOF error.

use std::fmt;

use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::Value;
use winnow::ascii::{digit1, escaped};
use winnow::combinator::{
    alt, cut_err, dispatch, eof, fail, not, opt, peek, preceded, repeat, terminated,
};
use winnow::error::{AddContext, ContextError, ErrMode, FromExternalError, ParseError, StrContext};
use winnow::prelude::*;
use winnow::stream::Stream;
use winnow::token::{any, one_of, rest, take_till, take_while};

/// Table entry for [`WinnowReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "winnow",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "winnow",
        version: "1.0.5",
        url: "https://github.com/winnow-rs/winnow",
    }],
    highest_step: Step::Mal,
    notes: "winnow combinator fns, dispatch! on the first char, cut_err + ContextError labels",
};

/// Reads mal source with winnow parser combinators.
#[derive(Debug, Default, Clone, Copy)]
pub struct WinnowReader;

impl Reader for WinnowReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let empty = (trivia, eof).value(ReadOutcome::Empty);
        // `Parser::parse` insists on consuming everything, so the trailing
        // input the reader ignores is swallowed explicitly with `rest`.
        terminated(alt((empty, form.map(ReadOutcome::Form))), rest)
            .parse(input)
            .map_err(ReadError::from)
    }
}

/// Why a form could not be read.
///
/// An owned summary of winnow's [`ParseError`], which borrows the input.
#[derive(Debug)]
pub struct ReadError {
    offset: usize,
    at_eof: bool,
    context: ContextError,
}

impl ReadError {
    /// The byte offset in the input where reading failed.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Whether the input ended inside a list, vector, map or string, or
    /// after a reader macro.
    #[must_use]
    pub fn is_eof(&self) -> bool {
        self.at_eof
    }

    /// What the parser expected, as winnow's error context.
    #[must_use]
    pub fn context(&self) -> &ContextError {
        &self.context
    }
}

impl From<ParseError<&str, ContextError>> for ReadError {
    fn from(e: ParseError<&str, ContextError>) -> Self {
        ReadError {
            offset: e.offset(),
            at_eof: e.offset() == e.input().len(),
            context: e.into_inner(),
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `ContextError` puts each kind of context on its own line.
        let detail = self.context.to_string().replace('\n', "; ");
        match (self.at_eof, detail.is_empty()) {
            (true, true) => f.write_str("unbalanced input: got EOF"),
            (true, false) => write!(f, "unbalanced input: {detail}, got EOF"),
            (false, true) => write!(f, "syntax error at byte {}", self.offset),
            (false, false) => write!(f, "{detail} at byte {}", self.offset),
        }
    }
}

impl std::error::Error for ReadError {}

/// Characters that may appear in a symbol, number or keyword.
fn is_symbol_char(c: char) -> bool {
    !(c.is_whitespace()
        || matches!(
            c,
            '[' | ']' | '{' | '}' | '(' | ')' | '\'' | '"' | '`' | ',' | ';'
        ))
}

/// Whitespace, commas and `;` comments.
fn trivia(input: &mut &str) -> ModalResult<()> {
    repeat(
        0..,
        alt((
            take_while(1.., |c: char| c.is_whitespace() || c == ',').void(),
            (';', take_till(0.., '\n')).void(),
        )),
    )
    .parse_next(input)
}

/// One form, after any leading trivia.
fn form(input: &mut &str) -> ModalResult<Value> {
    trivia.parse_next(input)?;
    dispatch! {peek(any);
        '(' => seq('(', ')').map(Value::list),
        '[' => seq('[', ']').map(Value::vector),
        '{' => cut_err(
            seq('{', '}')
                .verify_map(|items| Value::map_from_pairs(items).ok())
                .context(StrContext::Label("hash-map")),
        ),
        ')' | ']' | '}' => fail.context(StrContext::Label("closing delimiter")),
        '\'' => reader_macro("'", "quote"),
        '`' => reader_macro("`", "quasiquote"),
        '~' => alt((
            reader_macro("~@", "splice-unquote"),
            reader_macro("~", "unquote"),
        )),
        '@' => reader_macro("@", "deref"),
        '^' => with_meta,
        '"' => string.map(|s| Value::string(&s)),
        _ => atom,
    }
    .parse_next(input)
}

/// `open form* close`, committed once `open` matches.
fn seq<'i>(open: char, close: char) -> impl ModalParser<&'i str, Vec<Value>, ContextError> {
    preceded(
        open,
        cut_err(terminated(
            repeat(0.., form),
            (trivia, close.context(StrContext::Expected(close.into()))),
        )),
    )
}

/// `'x` and friends: `prefix form` reads as `(name form)`.
fn reader_macro<'i>(
    prefix: &'static str,
    name: &'static str,
) -> impl ModalParser<&'i str, Value, ContextError> {
    preceded(prefix, cut_err(form)).map(move |form| Value::wrap(name, form))
}

/// `^meta form` reads as `(with-meta form meta)`.
fn with_meta(input: &mut &str) -> ModalResult<Value> {
    preceded('^', cut_err((form, form)))
        .map(|(meta, form)| Value::list(vec![Value::symbol("with-meta"), form, meta]))
        .parse_next(input)
}

/// A string literal with `\n`, `\"` and `\\` escapes; any other escaped
/// character stands for itself.
fn string(input: &mut &str) -> ModalResult<String> {
    let body = escaped(
        take_till(1.., ['"', '\\']),
        '\\',
        alt(('n'.value('\n'), any)),
    );
    preceded(
        '"',
        cut_err(terminated(
            body,
            '"'.context(StrContext::Expected('"'.into())),
        )),
    )
    .parse_next(input)
}

/// An integer, keyword, `nil`, `true`, `false` or symbol.
fn atom(input: &mut &str) -> ModalResult<Value> {
    alt((
        integer.map(Value::Int),
        preceded(':', take_while(0.., is_symbol_char)).map(Value::keyword),
        take_while(1.., is_symbol_char).map(|token| match token {
            "nil" => Value::Nil,
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::symbol(token),
        }),
    ))
    .parse_next(input)
}

/// `-?[0-9]+` not followed by another symbol character.
fn integer(input: &mut &str) -> ModalResult<i64> {
    let start = input.checkpoint();
    let token =
        terminated((opt('-'), digit1).take(), not(one_of(is_symbol_char))).parse_next(input)?;
    // The token is definitely an integer now, so overflow must not backtrack
    // into the symbol branch.
    token.parse().map_err(|e| {
        input.reset(&start);
        let error = ContextError::from_external_error(input, e);
        ErrMode::Cut(error.add_context(input, &start, StrContext::Label("integer")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(WinnowReader::default);
    }

    #[test]
    fn errors_name_the_missing_delimiter() {
        let err = WinnowReader.read_str("(1 [2").unwrap_err();
        assert_eq!(err.to_string(), "unbalanced input: expected `]`, got EOF");
        let err = WinnowReader.read_str(")").unwrap_err();
        assert!(!err.is_eof());
        assert_eq!(err.offset(), 0);
        assert_eq!(err.to_string(), "invalid closing delimiter at byte 0");
        let err = WinnowReader.read_str("99999999999999999999").unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
        let err = WinnowReader.read_str("{1 2}").unwrap_err();
        assert!(err.to_string().contains("hash-map"), "{err}");
    }
}
