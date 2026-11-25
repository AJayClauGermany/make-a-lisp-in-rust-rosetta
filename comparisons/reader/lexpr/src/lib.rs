//! The `lexpr` reader row: a ready-made S-expression parser with its own
//! data model, configured as close to mal's syntax as its options allow,
//! and its values converted to mal values afterwards.
//!
//! Every other reader row is a parser *toolkit* given mal's grammar. lexpr
//! is the other kind of crate a Lisp author reaches for: a parser for an
//! existing Lisp syntax (R6RS Scheme by default, Emacs Lisp optionally)
//! that produces its own `lexpr::Value`. It fits the `Reader` trait with no
//! friction at all: `Parser::next_value` reads exactly one datum and leaves
//! the rest, returns `None` for blank or comment-only input, and its error
//! type knows on its own whether it ran out of input (`Error::is_eof`).
//!
//! What it cannot do is read mal. Its options cover brackets (vectors),
//! `:keyword`s and `nil`, but there is no hash-map literal, `,` is
//! `unquote` rather than whitespace, and `~`, `~@`, `@` and `^` are not
//! reader macros. Those forms are read as something else or rejected, so the
//! row stops at the first step that uses them; the conversion maps what it
//! can (`,@` to `splice-unquote`, `#t`/`#f` and the `true`/`false` symbols to
//! booleans) and rejects improper lists, floats and other values mal has no
//! type for.

use std::fmt;

use lexpr::parse::{Brackets, KeywordSyntax, NilSymbol, Options, Parser};
use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::Value;

/// Table entry for [`LexprReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "lexpr",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "lexpr",
        version: "0.2.7",
        url: "https://github.com/rotty/lexpr-rs",
    }],
    highest_step: Step::Repl,
    notes: "ready-made Scheme/Elisp S-expression parser + conversion; fits the trait, not mal's grammar ({}, ~, @, ^)",
};

impl mal_traits::Row for LexprReader {
    const IMPLEMENTATION: &'static Implementation = &IMPLEMENTATION;
}

/// Reads S-expressions with lexpr and converts them to mal values.
#[derive(Debug, Clone, Copy)]
pub struct LexprReader {
    options: Options,
}

impl Default for LexprReader {
    /// The lexpr options closest to mal: `[..]` vectors, `:keywords` and a
    /// distinguished `nil`.
    fn default() -> Self {
        Self {
            options: Options::new()
                .with_keyword_syntax(KeywordSyntax::ColonPrefix)
                .with_brackets(Brackets::Vector)
                .with_nil_symbol(NilSymbol::Special),
        }
    }
}

impl Reader for LexprReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        let mut parser = Parser::from_str_custom(input, self.options);
        match parser.next_value() {
            Ok(Some(value)) => convert(&value).map(ReadOutcome::Form),
            Ok(None) => Ok(ReadOutcome::Empty),
            Err(e) if e.is_eof() => Err(ReadError::UnexpectedEof(e.to_string())),
            Err(e) => Err(ReadError::Syntax(e.to_string())),
        }
    }
}

/// Why a form could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// lexpr ran out of input; its message.
    UnexpectedEof(String),
    /// lexpr rejected the input; its message.
    Syntax(String),
    /// lexpr read a value mal has no type for.
    Unsupported(String),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // lexpr's messages say "EOF while parsing a list" and so on, but
            // the "unbalanced" the test suite also accepts is added for
            // certainty.
            ReadError::UnexpectedEof(message) => write!(f, "unbalanced input: {message}"),
            ReadError::Syntax(message) => f.write_str(message),
            ReadError::Unsupported(what) => write!(f, "mal has no {what}"),
        }
    }
}

impl std::error::Error for ReadError {}

fn convert(value: &lexpr::Value) -> Result<Value, ReadError> {
    use lexpr::Value as L;
    Ok(match value {
        L::Nil => Value::Nil,
        L::Null => Value::list(Vec::new()),
        L::Bool(b) => Value::Bool(*b),
        L::Number(n) => Value::Int(
            n.as_i64()
                .ok_or_else(|| ReadError::Unsupported(format!("number like {n}")))?,
        ),
        L::String(s) => Value::string(s),
        L::Symbol(s) => match &**s {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            // lexpr reads `,@x` as `(unquote-splicing x)`.
            "unquote-splicing" => Value::symbol("splice-unquote"),
            name => Value::symbol(name),
        },
        L::Keyword(k) => Value::keyword(k),
        L::Vector(items) => Value::vector(items.iter().map(convert).collect::<Result<_, _>>()?),
        L::Cons(cons) => {
            let (items, tail) = cons.to_ref_vec();
            if !tail.is_null() {
                return Err(ReadError::Unsupported(String::from("dotted pairs")));
            }
            Value::list(items.into_iter().map(convert).collect::<Result<_, _>>()?)
        }
        L::Char(_) => return Err(ReadError::Unsupported(String::from("characters"))),
        L::Bytes(_) => return Err(ReadError::Unsupported(String::from("byte vectors"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        LexprReader::default().read_str(input)
    }

    fn form(input: &str) -> Value {
        match read(input) {
            Ok(ReadOutcome::Form(value)) => value,
            other => panic!("reading {input:?}: {other:?}"),
        }
    }

    #[test]
    fn reads_the_shared_subset() {
        assert_eq!(
            form("(+ 1 [2 :a \"s\\n\"] nil true)"),
            Value::list(vec![
                Value::symbol("+"),
                Value::Int(1),
                Value::vector(vec![
                    Value::Int(2),
                    Value::keyword("a"),
                    Value::string("s\n")
                ]),
                Value::Nil,
                Value::Bool(true),
            ])
        );
        assert_eq!(form("'a"), Value::wrap("quote", Value::symbol("a")));
        assert_eq!(form("`a"), Value::wrap("quasiquote", Value::symbol("a")));
        // Only the first form is read.
        assert_eq!(form("1 2"), Value::Int(1));
        assert!(matches!(read(" ; only a comment"), Ok(ReadOutcome::Empty)));
    }

    #[test]
    fn running_out_of_input_is_reported_as_such() {
        for input in ["(1 2", "[1", "\"abc", "'"] {
            let err = read(input).unwrap_err();
            assert!(matches!(err, ReadError::UnexpectedEof(_)), "{input}: {err}");
            assert!(err.to_string().contains("unbalanced"));
        }
    }

    /// The mal syntax lexpr does not have, recorded so the row's step
    /// stays explained.
    #[test]
    fn mal_only_syntax_is_not_read_as_mal() {
        // `,` is unquote, not whitespace.
        assert_eq!(
            form("(1, 2)"),
            Value::list(vec![Value::Int(1), Value::wrap("unquote", Value::Int(2))])
        );
        // No hash-map literal and no `@` reader macro.
        assert!(read("{\"a\" 1}").is_err());
        assert_ne!(form("@a"), Value::wrap("deref", Value::symbol("a")));
    }
}
