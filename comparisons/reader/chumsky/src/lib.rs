//! The `chumsky` reader row: a declarative combinator grammar.
//!
//! The whole grammar is one value built by a `parser` function, with the
//! recursive `form` rule tied with [`recursive`]. chumsky has no `cut`: alternatives
//! are tried in order and the error furthest into the input wins, which is
//! enough to report a missing `)` or `"` at the end of the input. Semantic
//! errors (integer overflow, a bad hash-map) are emitted with
//! [`Parser::validate`] so parsing carries on, as chumsky recommends.
//!
//! Errors are chumsky's own [`Rich`] errors, which already describe the end
//! of input as "end of input".

use std::fmt;

use chumsky::prelude::*;
use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::Value;

/// Table entry for [`ChumskyReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "chumsky",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "chumsky",
        version: "0.13.0",
        url: "https://github.com/zesterer/chumsky",
    }],
    highest_step: Step::Mal,
    notes: "chumsky combinator grammar with recursive(); Rich errors, rebuilt per read_str call",
};

/// Reads mal source with a chumsky grammar.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChumskyReader;

impl Reader for ChumskyReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        // The parser's type is tied to the input lifetime, so it cannot be
        // stored in `self` and is built again for every call.
        parser()
            .parse(input)
            .into_result()
            .map_err(|errors| ReadError {
                errors: errors.into_iter().map(Rich::into_owned).collect(),
            })
    }
}

/// Why a form could not be read: the errors chumsky reported, detached from
/// the input they point into.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadError {
    errors: Vec<Rich<'static, char>>,
}

impl ReadError {
    /// The individual errors, with spans into the input.
    #[must_use]
    pub fn errors(&self) -> &[Rich<'static, char>] {
        &self.errors
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, error) in self.errors.iter().enumerate() {
            if i > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ReadError {}

type Extra<'src> = extra::Err<Rich<'src, char>>;

/// Characters that may appear in a symbol, number or keyword.
fn is_symbol_char(c: char) -> bool {
    !(c.is_whitespace()
        || matches!(
            c,
            '[' | ']' | '{' | '}' | '(' | ')' | '\'' | '"' | '`' | ',' | ';'
        ))
}

/// The reader grammar: one form, or nothing but trivia.
fn parser<'src>() -> impl Parser<'src, &'src str, ReadOutcome<Value>, Extra<'src>> {
    let symbol_char = any()
        .filter(|c: &char| is_symbol_char(*c))
        .labelled("symbol character");
    let comment = just(';').then(none_of('\n').repeated()).ignored();
    let blank = any()
        .filter(|c: &char| c.is_whitespace() || *c == ',')
        .ignored();
    let trivia = blank.or(comment).labelled("whitespace").repeated();

    let form = recursive(|form| {
        let seq = |open, close| {
            form.clone()
                .repeated()
                .collect::<Vec<_>>()
                .then_ignore(trivia)
                .delimited_by(just(open), just(close))
        };
        let reader_macro = |prefix, name| {
            just(prefix)
                .ignore_then(form.clone())
                .map(move |form| Value::wrap(name, form))
        };

        let map = seq('{', '}').validate(|items, e, emitter| {
            Value::map_from_pairs(items).unwrap_or_else(|err| {
                emitter.emit(Rich::custom(e.span(), err.to_string()));
                Value::Nil
            })
        });
        let with_meta = just('^')
            .ignore_then(form.clone().then(form.clone()))
            .map(|(meta, form)| Value::list(vec![Value::symbol("with-meta"), form, meta]));

        let escape = just('\\').ignore_then(just('n').to('\n').or(any()));
        let string = none_of("\\\"")
            .labelled("string character")
            .or(escape)
            .repeated()
            .collect::<String>()
            .delimited_by(just('"'), just('"'))
            .map(|s| Value::string(&s));

        let integer = just('-')
            .or_not()
            .then(text::digits(10))
            .to_slice()
            .then_ignore(symbol_char.not())
            .validate(|token: &str, e, emitter| {
                token.parse().map_or_else(
                    |_| {
                        emitter.emit(Rich::custom(e.span(), "integer out of range"));
                        Value::Nil
                    },
                    Value::Int,
                )
            });
        let keyword = just(':')
            .ignore_then(symbol_char.repeated().to_slice())
            .map(Value::keyword);
        // `~`, `@` and `^` start reader macros; without the guard, a macro
        // with no form after it would backtrack into a symbol.
        let symbol = symbol_char
            .and_is(one_of("~@^").not())
            .then(symbol_char.repeated())
            .to_slice()
            .map(|token| match token {
                "nil" => Value::Nil,
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                _ => Value::symbol(token),
            });

        trivia.ignore_then(
            choice((
                seq('(', ')').map(Value::list),
                seq('[', ']').map(Value::vector),
                map,
                reader_macro("'", "quote"),
                reader_macro("`", "quasiquote"),
                reader_macro("~@", "splice-unquote"),
                reader_macro("~", "unquote"),
                reader_macro("@", "deref"),
                with_meta,
                string,
                integer,
                keyword,
                symbol,
            ))
            .labelled("form"),
        )
    });

    choice((
        trivia.then(end()).to(ReadOutcome::Empty),
        form.map(ReadOutcome::Form),
    ))
    // Trailing input after the first form is ignored.
    .lazy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(ChumskyReader::default);
    }

    #[test]
    fn errors_describe_the_problem() {
        let err = ChumskyReader.read_str("(1 [2").unwrap_err();
        assert_eq!(err.errors().len(), 1);
        assert_eq!(err.errors()[0].found(), None);
        assert!(err.to_string().contains("end of input"), "{err}");
        let err = ChumskyReader.read_str(")").unwrap_err();
        assert_eq!(err.errors()[0].found(), Some(&')'));
        let err = ChumskyReader.read_str("99999999999999999999").unwrap_err();
        assert_eq!(err.to_string(), "integer out of range");
        let err = ChumskyReader.read_str("{1 2}").unwrap_err();
        assert!(err.to_string().contains("hash-map"), "{err}");
    }
}
