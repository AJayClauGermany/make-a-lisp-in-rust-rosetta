# Line editor

**Trait:** `mal_traits::LineEditor`. **Directory:** `comparisons/line-editor/`.

The line editor shows the `user> ` prompt and reads a line for the REPL,
optionally with editing and history. The guide suggests a readline library
from `step0_repl` onward and a `readline` built-in at `stepA_mal`. This is the
component where binary-size overhead is most visible, since a line editor
is a large dependency relative to the interpreter.

## What the job demands

- `Ok(None)` at end of input, so the REPL exits cleanly on Ctrl-D.
- Must work when standard input is not a terminal. Upstream `runtest.py`
  runs implementations on a pseudo-terminal by default (`--no-pty` pipes);
  this repository's conformance tests pipe input, so they check the
  non-terminal path.

## Notes from the rows

rustyline reads piped input itself but stops printing the prompt and the
newline at end of input, so the row prints them; it still applies
backspace characters in piped lines, unlike the other rows. reedline needs a
terminal, so the row reads piped input with `std::io`, and its step column
is reached through that fallback; on a pseudo-terminal it sends a cursor
query before every line, which upstream `runtest.py` does not cope with. See
[Trait notes](../trait-notes.md#line-editor).

## Candidates

| crate | editing | history | step reached | version |
|-------|---------|---------|--------------|---------|
| std   | none | none | stepA_mal | - |
| [rustyline] | Emacs/vi keymaps | in-memory | stepA_mal | 17.0.2 |
| [reedline] | Emacs keymap, menus | built-in, in-memory | stepA_mal (piped input falls back to `std::io`) | 0.49.0 |
| [linefeed] | Emacs keymap | in-memory | | |
| [crossterm] (hand-rolled editor) | custom | custom | | |

[rustyline]: https://github.com/kkawakam/rustyline
[reedline]: https://github.com/nushell/reedline
[linefeed]: https://github.com/murarth/linefeed
[crossterm]: https://github.com/crossterm-rs/crossterm
