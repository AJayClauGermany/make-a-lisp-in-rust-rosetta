//! The `peg` reader row: a PEG grammar in a `peg::parser!` block.
//!
//! The grammar, trivia included, lives in the same file as the rest of the
//! row, and each alternative carries its own Rust action, so the parser
//! builds [`Value`]s directly with no intermediate tree. String escapes are
//! decoded by the grammar as well.
//!
//! Errors are rust-peg's own [`ParseError`], which reports the furthest
//! position reached and the set of tokens expected there. An error at the very
//! end of the input means the parser wanted more, and is reported as
//! [`ReadError::UnexpectedEof`] for the mal test suite's sake.

use std::fmt;

use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::Value;
use peg::error::ParseError;
use peg::str::LineCol;

/// Table entry for [`PegReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "peg",
    component: Component::Reader,
    crates: &[CrateRef {
        name: "peg",
        version: "0.8.6",
        url: "https://github.com/kevinmehall/rust-peg",
    }],
    highest_step: Step::Mal,
    notes: "PEG; grammar and actions in a peg::parser! proc-macro block, builds values directly",
};

/// What a `{? }` action reports when an integer literal overflows `i64`.
const INTEGER: &str = "an integer that fits in 64 bits";
/// What a `{? }` action reports for a malformed hash-map literal.
const MAP: &str = "a hash-map with an even number of string or keyword keys";

peg::parser! {
    /// The mal token grammar.
    grammar mal() for str {
        /// The first form, or `None` for blank input. The rest of the input
        /// is skipped with `[_]*`: a `pub rule` must consume all its input.
        pub rule first_form() -> Option<Value>
            = _ f:form() [_]* { Some(f) }
            / _ ![_] { None }

        rule form() -> Value
            = "(" v:forms() ")" { Value::list(v) }
            / "[" v:forms() "]" { Value::vector(v) }
            / "{" v:forms() "}" {? Value::map_from_pairs(v).or(Err(MAP)) }
            / "'" _ f:form() { Value::wrap("quote", f) }
            / "`" _ f:form() { Value::wrap("quasiquote", f) }
            / "~@" _ f:form() { Value::wrap("splice-unquote", f) }
            / "~" _ f:form() { Value::wrap("unquote", f) }
            / "@" _ f:form() { Value::wrap("deref", f) }
            / "^" _ meta:form() _ f:form() {
                Value::list(vec![Value::symbol("with-meta"), f, meta])
            }
            / "\"" s:string_char()* "\"" { Value::string(&s.into_iter().collect::<String>()) }
            / atom()

        rule forms() -> Vec<Value> = _ v:(form() ** _) _ { v }

        /// `\n` is a newline; any other escaped character stands for itself.
        rule string_char() -> char
            = "\\n" { '\n' }
            / "\\" c:[_] { c }
            / [^ '"' | '\\']

        rule atom() -> Value
            = n:$(integer()) {? n.parse().map(Value::Int).or(Err(INTEGER)) }
            / "nil" !symbol_char() { Value::Nil }
            / "true" !symbol_char() { Value::Bool(true) }
            / "false" !symbol_char() { Value::Bool(false) }
            / ":" k:$(symbol_char()*) { Value::keyword(k) }
            / !(integer() / ['~' | '@' | '^']) s:$(symbol_char()+) { Value::symbol(s) }

        rule integer() = "-"? ['0'..='9']+ !symbol_char()

        rule symbol_char()
            = [c if !c.is_whitespace() && !"[]{}()'\"`,;".contains(c)]

        /// Whitespace, commas and `;` comments.
        rule _() = quiet!{ ([c if c.is_whitespace() || c == ','] / ";" [^ '\n']*)* }
    }
}

/// Reads mal source with a rust-peg grammar.
#[derive(Debug, Default, Clone, Copy)]
pub struct PegReader;

impl Reader for PegReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        match mal::first_form(input) {
            Ok(Some(form)) => Ok(ReadOutcome::Form(form)),
            Ok(None) => Ok(ReadOutcome::Empty),
            Err(e) => Err(ReadError::new(e, input)),
        }
    }
}

/// Why a form could not be read. Both variants carry rust-peg's own error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The input ended inside a list, vector, map, string or after a reader
    /// macro.
    UnexpectedEof(ParseError<LineCol>),
    /// Any other syntax error, an integer that does not fit in 64 bits or a
    /// malformed hash-map.
    Syntax(ParseError<LineCol>),
}

impl ReadError {
    /// Classify a rust-peg error. A semantic failure from a `{? }` action is
    /// also positioned after the text it rejected, which can be the end of
    /// the input, so those are told apart by their expected-set label.
    fn new(error: ParseError<LineCol>, input: &str) -> Self {
        let semantic = error.expected.tokens().any(|t| t == INTEGER || t == MAP);
        if error.location.offset == input.len() && !semantic {
            ReadError::UnexpectedEof(error)
        } else {
            ReadError::Syntax(error)
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::UnexpectedEof(e) => write!(f, "unbalanced input: unexpected EOF ({e})"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_reader(PegReader::default);
    }

    #[test]
    fn errors_say_why() {
        let err = PegReader.read_str("(1 [2").unwrap_err();
        assert!(matches!(err, ReadError::UnexpectedEof(_)), "{err}");
        let err = PegReader.read_str(")").unwrap_err();
        assert!(matches!(err, ReadError::Syntax(_)), "{err}");
        let err = PegReader.read_str("99999999999999999999").unwrap_err();
        assert!(err.to_string().contains(INTEGER), "{err}");
        let err = PegReader.read_str("{1}").unwrap_err();
        assert!(err.to_string().contains(MAP), "{err}");
        let err = PegReader.read_str("@").unwrap_err();
        assert!(matches!(err, ReadError::UnexpectedEof(_)), "{err}");
    }

    #[test]
    fn trailing_input_is_ignored() {
        assert_eq!(
            PegReader.read_str("1 \"abc").unwrap(),
            ReadOutcome::Form(Value::Int(1))
        );
    }
}
