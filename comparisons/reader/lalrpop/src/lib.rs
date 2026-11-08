//! The `lalrpop` reader row: an LR(1) grammar compiled by a build script.
//!
//! `src/grammar.lalrpop` holds the token definitions for LALRPOP's built-in
//! lexer and the form grammar with its action code; `build.rs` turns it into
//! a table-driven parser in `OUT_DIR` and [`lalrpop_util::lalrpop_mod!`]
//! includes it. The actions build [`Value`]s directly.
//!
//! Errors are LALRPOP's own [`ParseError`], carrying a [`SemanticError`] for
//! what the actions reject. An unbalanced list is the parser's
//! `UnrecognizedEof`, whose message already says "EOF"; an unterminated
//! string is lexed as its own token so the grammar can report it as one.

use std::fmt;

use lalrpop_util::{ParseError, lalrpop_mod};
use mal_traits::{Component, CrateRef, Implementation, ReadOutcome, Reader, Step};
use mal_types::Value;

lalrpop_mod!(
    // Generated code is not held to the workspace's pedantic lints.
    #[allow(clippy::all, clippy::pedantic)]
    grammar
);

/// Table entry for [`LalrpopReader`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "lalrpop",
    component: Component::Reader,
    crates: &[
        CrateRef {
            name: "lalrpop",
            version: "0.22.2",
            url: "https://github.com/lalrpop/lalrpop",
        },
        CrateRef {
            name: "lalrpop-util",
            version: "0.22.2",
            url: "https://github.com/lalrpop/lalrpop",
        },
    ],
    highest_step: Step::Mal,
    notes: "LR(1) + built-in regex lexer; grammar and actions in a .lalrpop file compiled by build.rs",
};

/// Reads mal source with a LALRPOP-generated parser.
///
/// Holds the generated parser, whose constructor compiles the lexer's
/// regular expressions, so it is built once per reader rather than per call.
#[derive(Default)]
pub struct LalrpopReader {
    parser: grammar::FirstParser,
}

/// The generated parser does not implement `Debug`.
impl fmt::Debug for LalrpopReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LalrpopReader").finish_non_exhaustive()
    }
}

impl Reader for LalrpopReader {
    type Value = Value;
    type Error = ReadError;

    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Value>, ReadError> {
        match self.parser.parse(input) {
            Ok(Some(form)) => Ok(ReadOutcome::Form(form)),
            Ok(None) => Ok(ReadOutcome::Empty),
            // Tokens borrow the input; the trait's error type cannot.
            Err(e) => Err(e.map_token(|t| t.to_string())),
        }
    }
}

/// Why a form could not be read: LALRPOP's error, with byte offsets as
/// locations and tokens copied out of the input.
pub type ReadError = ParseError<usize, String, SemanticError>;

/// What the grammar's action code rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticError {
    /// A string literal with no closing quote.
    UnterminatedString,
    /// An integer literal that does not fit in 64 bits.
    IntegerOverflow(String),
    /// A hash-map literal with an odd number of forms or a bad key.
    BadMap(String),
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemanticError::UnterminatedString => {
                f.write_str("unbalanced string: expected '\"', got EOF")
            }
            SemanticError::IntegerOverflow(token) => write!(f, "integer out of range: {token}"),
            SemanticError::BadMap(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for SemanticError {}

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
        mal_testkit::check_reader(LalrpopReader::default);
    }

    #[test]
    fn errors_say_why() {
        let mut reader = LalrpopReader::default();
        let err = reader.read_str("(1 [2").unwrap_err();
        assert!(matches!(err, ParseError::UnrecognizedEof { .. }), "{err}");
        let err = reader.read_str(")").unwrap_err();
        assert!(matches!(err, ParseError::UnrecognizedToken { .. }), "{err}");
        let err = reader.read_str("99999999999999999999").unwrap_err();
        assert!(err.to_string().contains("out of range"), "{err}");
        let err = reader.read_str("@").unwrap_err();
        assert!(matches!(err, ParseError::UnrecognizedEof { .. }), "{err}");
        let err = reader.read_str("\"abc\\").unwrap_err();
        assert!(err.to_string().contains("EOF"), "{err}");
    }

    #[test]
    fn trailing_input_is_ignored() {
        let mut reader = LalrpopReader::default();
        assert_eq!(
            reader.read_str("1 \"abc").unwrap(),
            ReadOutcome::Form(Value::Int(1))
        );
        assert_eq!(
            reader.read_str("a ) ] }").unwrap(),
            ReadOutcome::Form(Value::symbol("a"))
        );
    }
}
