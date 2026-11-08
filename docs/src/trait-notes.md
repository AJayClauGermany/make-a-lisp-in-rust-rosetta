# Trait notes

Every row is written the way a user of its crate would write it. Where the
traits in `mal-traits` (or the shared value type in `mal-types`) forced a row
away from that, the friction is recorded here, with the rows it affects, so
the traits can be refined and the rows updated together afterwards.

The traits themselves were changed in one place only while the first rows
were written; everything else below is worked around in the rows and listed
as a candidate refinement.

## Changes made

| change | why |
|--------|-----|
| `run_repl` accepts `I: Interpreter + ?Sized` | The binary chooses its rows at run time and holds the interpreter as `Box<dyn Interpreter<Error = _>>`. Backwards compatible. |
| New crate `mal-types` (not a trait change) | The traits are generic over `Value`, so rows only interoperate if they agree on one. `mal-types` is that agreement: an `Rc`-based `Value`, `MalError`, equality and function application. Dependency-free, like `mal-traits`. |
| New `--read-print` mode (wiring, not a trait change) | `step1_read_print.mal` expects symbols to print back unevaluated, so reader and printer rows are checked against it without an evaluator. |

## Proposed refinements

Ordered by how many rows they would simplify. Numbers refer to the notes
below.

| # | refinement | affects |
|---|------------|---------|
| 1 | A reader error contract: an owned error with an `is_incomplete()` (or `UnexpectedEof`) case and a byte offset, instead of "the message must contain `EOF`". Also what a REPL needs for multi-line input. | all 9 readers |
| 2 | `read_str` returns the form *and* where it ended, `(ReadOutcome<V>, usize)`, so grammars need not be bent into "parse one form, ignore the rest", and `load-file` can read form by form instead of wrapping the file in `(do ...)`. | pest, peg, lalrpop, winnow |
| 3 | Give the core namespace the components it calls: `install(&self, env, ctx)` with a context offering read, print and apply/eval, or make `CoreNamespace` generic over them explicitly. | core, eval, the wiring |
| 4 | `Evaluator::eval(&self, ...)` rather than `&mut self`: built-ins (`eval`, `apply`, `map`, `swap!`) and macros re-enter the evaluator, so it cannot hold mutable state anyway. | eval |
| 5 | An owned symbol type for `Env` (`type Symbol`, or `Rc<str>`) instead of `&str`, so binding a parameter does not allocate a new key on every call. | all 8 envs |
| 6 | Separate "define in this scope" from "make a child scope with these bindings" (`fn bind(&self, pairs) -> Self`). Function calls and `let*` only need the latter, which a persistent map can implement with a structurally shared snapshot. | imbl, rpds |
| 7 | `Printer` writes to any `fmt::Write` (or is generic over the output buffer) instead of `&mut String`. | printer candidates `compact_str`, `smol_str` |
| 8 | A value-builder (serde-style visitor) between readers and values, so an evaluator with its own representation (`Arc`, arena, tracing GC) can reuse every reader row. | future eval rows |
| 9 | Let `run_repl` render errors through the printer, or have `Interpreter` return rendered errors, so thrown values print as mal values without a pre-rendered `String` error type. | the wiring |
| 10 | A number representation that the core row can choose (an associated type, or a `Value` parameter). | future core rows |
| 11 | A lifecycle hook on `LineEditor` (`fn finish(&mut self) -> Result<...>`) for saving history, and a fallible `add_history`. | rustyline, reedline |
| 12 | Require `Env: Default` (or add `fn new()`), since every caller needs a root environment. | the wiring, benchmarks |

## Notes by component

### Shared value type

1. **No shared value type.** Each trait has its own associated `Value`, so
   without an agreed type a reader row could not feed a printer row, and
   rows of one component could not be benchmarked on the same fixtures.
   Solved with `mal-types` (above). The cost: evaluators that compare memory
   management (`Arc`, arenas, `gc`) cannot use it and would have to bring a
   reader and printer of their own, which is why the evaluator table has
   only the `std` row so far. Refinement 8.
2. **Closures and the environment type.** A closure captures an
   environment, but the value type is shared by every environment row. The
   `std` evaluator stores the environment as `Rc<dyn Any>` with an `apply`
   function pointer and downcasts it again. The alternatives were worse:
   making `Value` generic over the environment puts a phantom parameter on
   every reader and printer, and forces every environment row to be a
   non-generic newtype to break the recursive type.
3. **`Value::map_from_pairs` returns `MalError`.** Readers then need a
   `From<MalError>` conversion that flattens it to a string, and because
   `MalError::Thrown` holds an `Rc`, it is not `Send + Sync`, so winnow
   cannot carry it as a `ContextError` cause and peg can only report a
   static label. A small `Send + Sync` map error would fix both.
4. **Strings are copied twice.** Every reader that unescapes builds a
   `String` and then copies it into an `Rc<str>`. This is inherent to
   `Rc<str>` (the reference counts live in the same allocation), so not a
   trait issue, but it is part of every reader row's cost.

### Reader

5. **The EOF requirement is a message convention.** The mal test suite
   matches `.*(EOF|end of input|unbalanced).*`. Of the crates, only lalrpop
   (`UnrecognizedEof`) and chumsky (`found end of input`) say so on their
   own. nom and winnow get it from a hand-written `Display`; pest and peg
   decide "ran out of input" by checking whether the error position is the
   input length (pest also when it is at a `"`, since it reports failures
   in atomic rules at the rule's start); logos maps an unmatched character
   to "unterminated string" because that is the only way it can happen;
   regex re-checks string tokens because the guide's tokenizer accepts an
   unterminated string. logos and regex copied the `std` row's error enum
   wholesale. Refinement 1.
6. **`Reader::Error` cannot borrow the input.** winnow's `ParseError`,
   chumsky's `Rich<'src>`, lalrpop's `ParseError` tokens and pest's errors
   all borrow, so every row converts to an owned error at the boundary
   (winnow loses its caret rendering, lalrpop allocates a string per
   expected token, pest's error is boxed because it is large). Refinement 1
   (owned, with an offset) or a generic associated `Error<'a>`.
7. **"First form only, ignore the rest" fights grammar entry rules.** pest
   needs `SOI ~ (form | EOI)` without a trailing `EOI`; peg's `pub rule`
   must consume everything, so the entry rule ends in `[_]*` and walks the
   rest of the input on every read; lalrpop's LR parser must reach EOF, so
   the grammar accepts `<Form> Token*` and has an extra token for
   unterminated strings in the trailing input; winnow's `parse` needs
   `terminated(form, rest)`. Refinement 2.
8. **`&mut self` and `Default` are mostly unused.** Seven readers are unit
   structs. lalrpop keeps its generated parser (which compiles a regex set
   on construction) and needs a manual `Debug`. chumsky cannot keep its
   parser at all: its type borrows the input lifetime, so `read_str`
   rebuilds the grammar on every call, and the chumsky row's timings include
   that.
9. **Overflow and bad maps need hard failures.** In nom, winnow and peg a
   failed integer conversion backtracks, and the token would silently read
   as a symbol; each needs a cut, a checkpoint reset or a `!integer()`
   guard.

### Printer

10. **`&mut String` output.** The trait appends to a `String`, which rules
    out the small-string candidates on the printer page (`compact_str`,
    `smol_str`) as output buffers and writing straight to `io::Write`.
    Refinement 7. `ryu` is not a row because `Value` has no floats.

### Environment

11. **`set` takes `&str`.** Every binding allocates an owned key, including
    every parameter of every function call, although the value already
    holds the symbol as an `Rc<str>`. hashbrown's `entry_ref` avoids the
    allocation only when rebinding. Refinement 5.
12. **Persistent maps cannot be used as persistent maps.** The idiomatic
    environment on `imbl` or `rpds` gives each child scope a structurally
    shared snapshot of its parent, so lookups hit one map. `Env::set`
    through a parent must be visible to children created before it
    (recursive `def!` after `fn*` captured the scope relies on it), which
    snapshots break. So both rows keep the scope chain and a `RefCell`, and
    the persistent map is just a map. Refinement 6.
13. **No constructor in the trait.** The wiring and the benchmarks need
    `E: Default`. Refinement 12.

### Evaluator

14. **`eval(&mut self)` versus re-entry.** `eval`, `apply`, `map`, `swap!`
    and macro expansion all call back into the evaluator from inside a
    built-in or closure, so the evaluator has to be stateless and `Clone`,
    and the `eval` built-in clones it per call. Refinement 4.
15. **`DEBUG-EVAL` cannot be implemented.** The optional tests that bind
    `DEBUG-EVAL` expect the evaluator to print each form, and the evaluator
    has no printer. These are soft failures (17 across steps 3, 7 and 8) in
    every row.
16. **Reference cycles leak.** Every closure that captures the scope it is
    defined in (every recursive `def!`) and the `eval` built-in, which
    captures the top-level environment, form `Rc` cycles. This is the cost
    of the `std` row's memory-management strategy rather than a trait issue,
    and what a tracing-GC row would be measured against.

### Core namespace

17. **The core needs other components.** `read-string` needs a reader,
    `pr-str`/`str`/`prn`/`println` a printer, and `eval`, `apply`, `map` and
    `swap!` the evaluator; `install` only receives the environment. The
    `std` row is generic over a reader and a printer, calls functions through
    `Value::apply`, and `eval` is installed by the wiring. Refinement 3.
18. **`readline` cannot use the line-editor row.** It reads standard input
    directly.
19. **Representation is fixed.** The numeric candidates (`num-bigint`,
    `rug`) need a different number type in `Value`, and the core is the only
    row that would change it. Refinement 10. Until then the core table has
    only the `std` row.
20. **The benchmark only times `install`.** Timing the built-ins themselves
    would make a second core row worth comparing.

### Line editor

21. **"Show the prompt" when piped.** rustyline stops printing the prompt
    when standard input is not a terminal, and the mal test protocol waits
    for it, so the row writes the prompt itself in that case. reedline cannot
    run without a terminal at all; the row reads piped input with `std::io`,
    as nushell does, so the step column for reedline is reached through that
    fallback.
22. **History.** `add_history` returns `()`, so rustyline's error is
    dropped; reedline records history itself, so the call is a no-op. Neither
    can save a history file without a lifecycle hook. Refinement 11.

### Interpreter and wiring

23. **Run-time selection multiplies code.** Every row is generic, so every
    combination the binary can run is a monomorphised copy. The binary allows
    one row to differ from `std` at a time, which keeps the copies linear.
24. **Rendered errors.** `run_repl` prints `Error: {e}` with `Display`, which
    cannot print a thrown mal value with the printer row, so the wiring's
    error type is a pre-rendered `String`. Refinement 9.

## Dependency findings

Not trait issues, but found while writing the rows and relevant to the
MSRV (1.85) and the version column:

- `lalrpop` 0.23 needs Rust 1.86; the row uses 0.22.2.
- `reedline` 0.52 needs 1.95; the resolver chose 0.49.0.
- `rustyline` 18 and 19 depend on `home` 0.5.12, which needs 1.88 without
  `rustyline` declaring a `rust-version`; the row uses 17.0.2 with `home`
  pinned to 0.5.11 in `Cargo.lock`.
- `psm` 0.1.27 and later (used by pest's stack guard and chumsky's
  `stacker`) build-depend on `ar_archive_writer`, which uses let chains
  (1.88); `Cargo.lock` pins `psm` 0.1.26.

A plain `cargo update` can undo the last two pins; `cargo +1.85 check
--workspace --all-targets` (the MSRV job) catches it.
