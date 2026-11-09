//! The `rustyline` line-editor row: Emacs/vi editing and in-memory history
//! through `rustyline::DefaultEditor`.
//!
//! When standard input is not a terminal (this repository's conformance
//! tests pipe their input), rustyline reads lines itself without a
//! terminal, but it stops showing the prompt and does not end the last
//! prompt's line at end of input. `LineEditor::read_line` promises the
//! prompt, and the mal test protocol waits for it, so in that case the row
//! writes the prompt, and the newline at end of input, itself.
//!
//! Piped lines are still processed by rustyline: each backspace character
//! (U+0008) deletes the character before it, as if it had been typed. The
//! `std` and `reedline` rows pass such a line through unchanged, so a mal
//! program containing a literal backspace reads differently under this row.

use std::io::{self, IsTerminal, Write};

use mal_traits::{Component, CrateRef, Implementation, LineEditor, Step};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

/// Table entry for [`RustylineEditor`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "rustyline",
    component: Component::LineEditor,
    crates: &[CrateRef {
        name: "rustyline",
        version: "17.0.2",
        url: "https://github.com/kkawakam/rustyline",
    }],
    highest_step: Step::Mal,
    notes: "DefaultEditor; Emacs/vi keymaps, in-memory history; prompt written by the row when piped",
};

/// Reads lines with rustyline.
#[derive(Debug)]
pub struct RustylineEditor {
    editor: DefaultEditor,
    interactive: bool,
}

impl RustylineEditor {
    /// A rustyline editor with the default configuration.
    pub fn new() -> rustyline::Result<Self> {
        Ok(Self {
            editor: DefaultEditor::new()?,
            interactive: io::stdin().is_terminal(),
        })
    }
}

impl LineEditor for RustylineEditor {
    type Error = ReadlineError;

    fn read_line(&mut self, prompt: &str) -> Result<Option<String>, ReadlineError> {
        if !self.interactive {
            let mut stdout = io::stdout().lock();
            stdout.write_all(prompt.as_bytes())?;
            stdout.flush()?;
        }
        let prompt = if self.interactive { prompt } else { "" };
        match self.editor.readline(prompt) {
            Ok(line) => Ok(Some(line)),
            Err(ReadlineError::Eof) => {
                // On a terminal rustyline ends the prompt's line itself.
                if !self.interactive {
                    let mut stdout = io::stdout().lock();
                    stdout.write_all(b"\n")?;
                    stdout.flush()?;
                }
                Ok(None)
            }
            // Ctrl-C abandons the line being typed, as in a shell.
            Err(ReadlineError::Interrupted) => Ok(Some(String::new())),
            Err(e) => Err(e),
        }
    }

    fn add_history(&mut self, line: &str) {
        // `LineEditor::add_history` cannot report failure; a history entry
        // that could not be recorded is not worth stopping the REPL for.
        let _ = self.editor.add_history_entry(line);
    }
}
