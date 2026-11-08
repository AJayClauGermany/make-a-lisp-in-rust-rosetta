//! Dependency-free baseline implementations.
//!
//! These are the `null` row of the comparison tables: the smallest thing
//! that satisfies each trait using only the standard library, so that every
//! crate under `comparisons/` has a floor to be measured against.

use std::convert::Infallible;
use std::io::{self, BufRead, Write};

use mal_traits::{Component, Implementation, Interpreter, LineEditor, Step};

/// Reads lines from standard input with no editing or history.
#[derive(Debug, Default)]
pub struct StdinLineEditor {
    buf: String,
}

impl LineEditor for StdinLineEditor {
    type Error = io::Error;

    fn read_line(&mut self, prompt: &str) -> io::Result<Option<String>> {
        let mut stdout = io::stdout().lock();
        stdout.write_all(prompt.as_bytes())?;
        stdout.flush()?;

        self.buf.clear();
        let read = io::stdin().lock().read_line(&mut self.buf)?;
        if read == 0 {
            // EOF: the mal test runner expects a trailing newline here so the
            // next shell prompt starts on its own line.
            stdout.write_all(b"\n")?;
            return Ok(None);
        }
        Ok(Some(self.buf.trim_end_matches(['\n', '\r']).to_owned()))
    }
}

/// `step0_repl`: READ, EVAL and PRINT are all the identity function.
#[derive(Debug, Default, Clone, Copy)]
pub struct EchoInterpreter;

impl Interpreter for EchoInterpreter {
    type Error = Infallible;

    fn rep(&mut self, line: &str) -> Result<Option<String>, Infallible> {
        Ok(Some(line.to_owned()))
    }
}

/// Table entry for [`StdinLineEditor`].
pub const STDIN_LINE_EDITOR: Implementation = Implementation {
    name: "std",
    component: Component::LineEditor,
    crates: &[],
    highest_step: Step::Mal,
    notes: "std::io::stdin; no editing, no history",
};

/// Table entry for [`EchoInterpreter`].
pub const ECHO: Implementation = Implementation {
    name: "echo",
    component: Component::Evaluator,
    crates: &[],
    highest_step: Step::Repl,
    notes: "step0: prints each line back unchanged",
};

/// Table entries for the baselines.
pub const IMPLEMENTATIONS: &[Implementation] = &[STDIN_LINE_EDITOR, ECHO];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_is_step0() {
        let mut echo = EchoInterpreter;
        assert_eq!(echo.rep("(+ 1 2)").unwrap().as_deref(), Some("(+ 1 2)"));
        assert_eq!(echo.rep("").unwrap().as_deref(), Some(""));
        assert_eq!(echo.host_language(), "rust");
    }

    #[test]
    fn baselines_use_no_crates() {
        for imp in IMPLEMENTATIONS {
            assert!(imp.is_baseline(), "{} should be a baseline", imp.name);
        }
    }
}
