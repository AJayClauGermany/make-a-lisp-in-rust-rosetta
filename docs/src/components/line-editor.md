# Line editor

**Trait:** `mal_traits::LineEditor`. **Directory:** `comparisons/line-editor/`.

The line editor shows the `user> ` prompt and reads a line for the REPL,
optionally with editing and history. The guide suggests a readline library
from `step0_repl` onward and a `readline` built-in at `stepA_mal`. This is the
component where binary-size overhead is most visible, since a line editor
is a large dependency relative to the interpreter.

## What the job demands

- `Ok(None)` at end of input, so the REPL exits cleanly on Ctrl-D.
- Must work when standard input is not a terminal: the test suite pipes input.

## Candidates

| crate | editing | history | step reached | version |
|-------|---------|---------|--------------|---------|
| std   | none | none | stepA_mal | - |
| [rustyline] | Emacs/vi keymaps | file-backed | | |
| [reedline] | Emacs/vi keymaps, menus | file- or SQLite-backed | | |
| [linefeed] | Emacs keymap | in-memory | | |
| [crossterm] (hand-rolled editor) | custom | custom | | |

[rustyline]: https://github.com/kkawakam/rustyline
[reedline]: https://github.com/nushell/reedline
[linefeed]: https://github.com/murarth/linefeed
[crossterm]: https://github.com/crossterm-rs/crossterm
