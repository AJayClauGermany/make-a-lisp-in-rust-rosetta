//! The `dialoguer` line-editor row: `dialoguer::Input`, a text prompt from a
//! crate for interactive command-line *forms* (wizards, confirmations,
//! selections), with its arrow-key history.
//!
//! It is the kind of crate a CLI already depending on dialoguer would try
//! first, and it shows where "read a line" and "ask a question" differ:
//!
//! - Prompts go through a `Theme`, whose default appends `": "`; the row
//!   supplies a theme that writes mal's prompt verbatim, and turns off the
//!   echo of the answer (`report(false)`) that a form would show.
//! - There is no end-of-input key. Ctrl-D is ignored as a control character,
//!   so on a terminal `read_line` never returns `Ok(None)`: the REPL is left
//!   with Ctrl-C, which ends the process.
//! - History belongs to the caller (`history_with(&mut impl History)`), and
//!   dialoguer writes each submitted line to it itself, so the REPL's
//!   `add_history` call has nothing to do.
//! - Like reedline it needs a terminal and reports `NotConnected` otherwise,
//!   so piped input (this repository's conformance tests) is read with
//!   `std::io`, and the step column is reached through that fallback.

use std::fmt;
use std::io::{self, BufRead, IsTerminal, Write};

use dialoguer::console::Term;
use dialoguer::theme::Theme;
use dialoguer::{BasicHistory, Input};
use mal_traits::{Component, CrateRef, Implementation, LineEditor, Step};

/// Table entry for [`DialoguerEditor`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "dialoguer",
    component: Component::LineEditor,
    crates: &[CrateRef {
        name: "dialoguer",
        version: "0.12.0",
        url: "https://github.com/console-rs/dialoguer",
    }],
    highest_step: Step::Mal,
    notes: "Input::interact_text with BasicHistory and a verbatim-prompt Theme; no Ctrl-D; std::io fallback when piped",
};

/// Reads lines with a dialoguer text prompt on a terminal, or plainly when
/// piped.
pub struct DialoguerEditor {
    interactive: bool,
    history: BasicHistory,
    buf: String,
}

impl DialoguerEditor {
    /// A dialoguer prompt with unbounded, de-duplicated history.
    pub fn new() -> Self {
        Self {
            interactive: io::stdin().is_terminal() && Term::stdout().is_term(),
            history: BasicHistory::new().no_duplicates(true),
            buf: String::new(),
        }
    }
}

impl Default for DialoguerEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DialoguerEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DialoguerEditor")
            .field("interactive", &self.interactive)
            .finish_non_exhaustive()
    }
}

/// Writes the prompt exactly as given, where dialoguer's themes add `": "`
/// and styling.
struct Verbatim;

impl Theme for Verbatim {
    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        _default: Option<&str>,
    ) -> fmt::Result {
        f.write_str(prompt)
    }
}

impl LineEditor for DialoguerEditor {
    type Error = dialoguer::Error;

    fn read_line(&mut self, prompt: &str) -> Result<Option<String>, dialoguer::Error> {
        if !self.interactive {
            return Ok(read_piped(prompt, &mut self.buf)?);
        }
        Input::<String>::with_theme(&Verbatim)
            .with_prompt(prompt)
            .allow_empty(true)
            .report(false)
            .history_with(&mut self.history)
            .interact_text_on(&Term::stdout())
            .map(Some)
    }

    // `history_with` makes dialoguer record each submitted line itself.
}

fn read_piped(prompt: &str, buf: &mut String) -> io::Result<Option<String>> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(prompt.as_bytes())?;
    stdout.flush()?;
    buf.clear();
    if io::stdin().lock().read_line(buf)? == 0 {
        stdout.write_all(b"\n")?;
        return Ok(None);
    }
    Ok(Some(buf.trim_end_matches(['\n', '\r']).to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_is_written_verbatim() {
        let mut out = String::new();
        Verbatim
            .format_input_prompt(&mut out, "user> ", None)
            .unwrap();
        assert_eq!(out, "user> ");
    }
}
