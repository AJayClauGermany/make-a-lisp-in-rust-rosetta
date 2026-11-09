//! The `reedline` line-editor row: nushell's line editor, with Emacs
//! keybindings and reedline's own in-memory history.
//!
//! Reedline only drives a terminal: it switches it to raw mode and fails
//! when standard input is piped, so unlike rustyline it does not read plain
//! lines by itself. This row checks `IsTerminal` and reads piped input with
//! `std::io` instead, which is also what nushell does for non-interactive
//! input. This repository's conformance tests pipe their input (like
//! `runtest.py --no-pty`), so the step column for this row is reached
//! through that fallback.
//!
//! Upstream `runtest.py` runs implementations on a pseudo-terminal by
//! default. There reedline does run, and it sends a cursor-position query and
//! a styled prompt before every line; `runtest.py` answers the query by
//! clearing its output buffer, which loses the previous result, so this row
//! is not expected to pass the upstream runner in that mode (checked with a
//! pty driver that answers the query the way `runtest.py` does).

use std::borrow::Cow;
use std::io::{self, BufRead, IsTerminal, Write};

use mal_traits::{Component, CrateRef, Implementation, LineEditor, Step};
use reedline::{Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal};

/// Table entry for [`ReedlineEditor`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "reedline",
    component: Component::LineEditor,
    crates: &[CrateRef {
        name: "reedline",
        version: "0.49.0",
        url: "https://github.com/nushell/reedline",
    }],
    highest_step: Step::Mal,
    notes: "Reedline::create(); Emacs keymap, built-in history; std::io fallback when piped",
};

/// Reads lines with reedline on a terminal, or plainly when piped.
pub struct ReedlineEditor {
    editor: Option<Reedline>,
    buf: String,
}

impl ReedlineEditor {
    /// Reedline's default configuration when standard input is a terminal.
    pub fn new() -> Self {
        Self {
            editor: io::stdin().is_terminal().then(Reedline::create),
            buf: String::new(),
        }
    }
}

impl Default for ReedlineEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ReedlineEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReedlineEditor")
            .field("interactive", &self.editor.is_some())
            .finish_non_exhaustive()
    }
}

/// Reedline renders prompts in segments; mal's `user> ` is one segment with
/// no mode indicator.
struct MalPrompt<'a>(&'a str);

impl Prompt for MalPrompt<'_> {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.0)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        Cow::Owned(format!("(search: {}) ", search.term))
    }
}

impl LineEditor for ReedlineEditor {
    type Error = io::Error;

    fn read_line(&mut self, prompt: &str) -> io::Result<Option<String>> {
        let Some(editor) = &mut self.editor else {
            return read_piped(prompt, &mut self.buf);
        };
        loop {
            match editor.read_line(&MalPrompt(prompt))? {
                Signal::Success(line) => return Ok(Some(line)),
                Signal::CtrlD => return Ok(None),
                // Ctrl-C abandons the line being typed, as in a shell.
                Signal::CtrlC => return Ok(Some(String::new())),
                // Other signals (e.g. host commands) are not used by mal.
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
    }

    // Reedline records every submitted line in its own history, so the
    // REPL's `add_history` call has nothing to do.
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
