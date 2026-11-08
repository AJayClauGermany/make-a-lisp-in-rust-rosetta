//! The read-eval-print loop: line input and the `rep` driver.

use std::fmt;

/// Reads lines of input for the interactive loop.
///
/// Compares readline-style crates on editing features, history and binary
/// size. The baseline reads from standard input with no editing.
pub trait LineEditor {
    /// Raised on I/O failure. Reaching end of input is *not* an error.
    type Error: std::error::Error;

    /// Show `prompt` and read one line, without its trailing newline.
    ///
    /// Returns `Ok(None)` at end of input (EOF, Ctrl-D).
    fn read_line(&mut self, prompt: &str) -> Result<Option<String>, Self::Error>;

    /// Record `line` in the editor's history, if it keeps one.
    fn add_history(&mut self, line: &str) {
        let _ = line;
    }
}

/// Reads, evaluates and prints one line: the guide's `rep` function.
///
/// An interpreter wires a [`Reader`](crate::Reader),
/// [`Evaluator`](crate::Evaluator) and [`Printer`](crate::Printer) together
/// with a top-level environment. Keeping it as its own trait lets the
/// benchmark harness drive any combination of components the same way.
pub trait Interpreter {
    /// Raised by any phase of `rep`. Printed to the user and the loop goes on.
    type Error: std::error::Error;

    /// The name of the host language as reported by `*host-language*`.
    fn host_language(&self) -> &'static str {
        "rust"
    }

    /// Read, evaluate and print `line`.
    ///
    /// Returns `Ok(None)` when there is nothing to print, which is how an
    /// empty or comment-only line is handled.
    fn rep(&mut self, line: &str) -> Result<Option<String>, Self::Error>;
}

/// Why [`run_repl`] returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplOutcome {
    /// The line editor reported end of input.
    EndOfInput,
}

/// Errors from [`run_repl`]: only the line editor can stop the loop.
#[derive(Debug)]
pub enum ReplError<L> {
    /// The line editor failed to read input.
    Input(L),
    /// Writing to the output sink failed.
    Output(std::io::Error),
}

impl<L: fmt::Display> fmt::Display for ReplError<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplError::Input(e) => write!(f, "failed to read input: {e}"),
            ReplError::Output(e) => write!(f, "failed to write output: {e}"),
        }
    }
}

impl<L: std::error::Error + 'static> std::error::Error for ReplError<L> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReplError::Input(e) => Some(e),
            ReplError::Output(e) => Some(e),
        }
    }
}

/// Drive the read-eval-print loop until end of input.
///
/// Each line is passed to [`Interpreter::rep`]; the result, or the error's
/// `Display` form prefixed with `Error: `, is written to `out`. This mirrors
/// the behaviour the mal test runner expects, so every interpreter gets the
/// same loop and the comparison stays about the components.
///
/// `I` may be unsized, so a binary that chooses its components at run time
/// can pass a `Box<dyn Interpreter<Error = E>>`.
pub fn run_repl<L, I, W>(
    editor: &mut L,
    interpreter: &mut I,
    out: &mut W,
) -> Result<ReplOutcome, ReplError<L::Error>>
where
    L: LineEditor,
    I: Interpreter + ?Sized,
    W: std::io::Write,
{
    loop {
        let Some(line) = editor.read_line("user> ").map_err(ReplError::Input)? else {
            return Ok(ReplOutcome::EndOfInput);
        };
        editor.add_history(&line);
        match interpreter.rep(&line) {
            Ok(Some(text)) => writeln!(out, "{text}").map_err(ReplError::Output)?,
            Ok(None) => {}
            Err(e) => writeln!(out, "Error: {e}").map_err(ReplError::Output)?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::convert::Infallible;

    struct Scripted(VecDeque<&'static str>);

    impl LineEditor for Scripted {
        type Error = Infallible;
        fn read_line(&mut self, _prompt: &str) -> Result<Option<String>, Infallible> {
            Ok(self.0.pop_front().map(str::to_owned))
        }
    }

    #[derive(Debug)]
    struct Boom;
    impl fmt::Display for Boom {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("boom")
        }
    }
    impl std::error::Error for Boom {}

    struct Shout;
    impl Interpreter for Shout {
        type Error = Boom;
        fn rep(&mut self, line: &str) -> Result<Option<String>, Boom> {
            match line.trim() {
                "" => Ok(None),
                "fail" => Err(Boom),
                other => Ok(Some(other.to_uppercase())),
            }
        }
    }

    #[test]
    fn loop_prints_results_errors_and_skips_empty_lines() {
        let mut editor = Scripted(VecDeque::from(["hello", "", "fail", "  bye "]));
        let mut out = Vec::new();
        let outcome = run_repl(&mut editor, &mut Shout, &mut out).unwrap();
        assert_eq!(outcome, ReplOutcome::EndOfInput);
        assert_eq!(String::from_utf8(out).unwrap(), "HELLO\nError: boom\nBYE\n");
    }

    #[test]
    fn loop_accepts_trait_objects() {
        let mut editor = Scripted(VecDeque::from(["x"]));
        let mut interpreter: Box<dyn Interpreter<Error = Boom>> = Box::new(Shout);
        let mut out = Vec::new();
        run_repl(&mut editor, interpreter.as_mut(), &mut out).unwrap();
        assert_eq!(out, b"X\n");
    }
}
