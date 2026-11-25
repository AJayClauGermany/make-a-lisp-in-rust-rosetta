# Trait notes

Every row is written the way a user of its crate would write it. Where the
traits in `mal-traits` (or the shared value type in `mal-types`) forced a row
away from that, the friction is recorded here, with the rows it affects, so
the traits can be refined and the rows updated together afterwards.

The traits themselves were changed in one place only while the first rows
were written; everything else below is worked around in the rows and listed
as a candidate refinement.

A second batch of rows (readers `stacker`, `combine`, `lexpr`; printers
`pretty`, `lexical`; environments `fnv`, `smallvec`, `lasso`, `slotmap`;
line editor `dialoguer`) was chosen to test the traits from new directions
rather than to add more of the same: a ready-made parser with its own data
model, a document-building printer, environments that are not one hash map
per scope, and a line editor that is not a readline. None of them needed a
trait change. What they showed is under
[Second batch](#second-batch-what-the-new-rows-showed).

## Changes made

| change | why |
|--------|-----|
| `run_repl` accepts `I: Interpreter + ?Sized` | The binary chooses its rows at run time and holds the interpreter as `Box<dyn Interpreter<Error = _>>`. Backwards compatible. |
| New crate `mal-types` (not a trait change) | The traits are generic over `Value`, so rows only interoperate if they agree on one. `mal-types` is that agreement: an `Rc`-based `Value`, `MalError`, equality and function application. Dependency-free, like `mal-traits`. |
| New `--read-print` mode (wiring, not a trait change) | `step1_read_print.mal` expects symbols to print back unevaluated, so reader and printer rows are checked against it without an evaluator. |

## Proposed refinements

Ordered roughly by how many rows they would simplify.

| # | refinement | affects |
|---|------------|---------|
| 1 | A reader error contract: an owned error with an `is_incomplete()` (or `UnexpectedEof`) case and a byte offset, instead of "the message must contain `EOF`". Also what a REPL needs for multi-line input. lexpr already has exactly this (`Error::is_eof`), and combine's `easy` errors carry `end_of_input()` and a position. | all 12 readers |
| 2 | `read_str` returns the form *and* where it ended, `(ReadOutcome<V>, usize)`, so grammars need not be bent into "parse one form, ignore the rest", and `load-file` can read form by form instead of wrapping the file in `(do ...)`. | pest, peg, lalrpop, winnow |
| 3 | Give the core namespace the components it calls: `install(&self, env, ctx)` with a context offering read, print and apply/eval, or make `CoreNamespace` generic over them explicitly. | core, eval, the wiring |
| 4 | `Evaluator::eval(&self, ...)` rather than `&mut self`: built-ins (`eval`, `apply`, `map`, `swap!`) and macros re-enter the evaluator, so it cannot hold mutable state anyway. | eval |
| 5 | An owned symbol type for `Env` (`type Symbol`, or `Rc<str>`) instead of `&str`, so binding a parameter does not allocate a new key on every call, and an interned symbol is hashed once by the reader instead of on every lookup. | all 12 envs, lasso most |
| 6 | Separate "define in this scope" from "make a child scope with these bindings" (`fn bind(&self, pairs) -> Self`). Function calls and `let*` only need the latter, which a persistent map can implement with a structurally shared snapshot. | imbl, rpds |
| 7 | `Printer` writes to any `fmt::Write` (or is generic over the output buffer) instead of `&mut String`. pretty renders to any `fmt::Write` or `io::Write` already. | printer candidates `compact_str`, `smol_str`, pretty |
| 8 | A value-builder (serde-style visitor) between readers and values, so an evaluator with its own representation (`Arc`, arena, tracing GC) can reuse every reader row. | future eval rows |
| 9 | Let `run_repl` render errors through the printer, or have `Interpreter` return rendered errors, so thrown values print as mal values without a pre-rendered `String` error type. | the wiring |
| 10 | A number representation that the core row can choose (an associated type, or a `Value` parameter). | future core rows |
| 11 | A lifecycle hook on `LineEditor` (`fn finish(&mut self) -> Result<...>`) for saving history, and a fallible `add_history`. | rustyline, reedline |
| 12 | Require `Env: Default` (or add `fn new()`), since every caller needs a root environment. | the wiring, benchmarks |
| 13 | A nesting limit readers (and printers) honour, reported as an error instead of a stack overflow. The stacker row shows that fixing the reader alone is not enough: printing or dropping the value it reads still recurses. | 6 of 12 readers, all printers, `Value`'s `Drop` |
| 14 | Report the fraction of each step's cases a row passes next to the highest unbroken step, so a row that implements most of the grammar is not indistinguishable from one that implements none. | lexpr |
| 15 | Drop trait surface no row uses: `Interpreter::host_language` (the wiring binds `*host-language*` itself), `Implementation::supports`, and `Env::contains` (only the shared checks call it); make `Reader::read_str` take `&self` unless a row needs state. | the traits |

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
10. **Reader macros must not fall back to symbols.** `~`, `@` and `^` are
    symbol characters inside a symbol (`a~b`) but start a reader macro at
    the beginning of a form. A combinator grammar without a cut tries the
    macro, fails when no form follows, and backtracks into reading `~` as a
    symbol: chumsky did exactly that until its symbol rule excluded those
    first characters, as the pest, lalrpop and logos grammars do.
11. **No depth limit in the trait.** Every recursive-descent row overflows
    the stack on deep enough nesting, aborting the process, and the trait
    has no way to report "too deep" as an error. How deep differs a lot.
    Reading an unclosed run of `(` in a debug build of the binary (so only
    the reader recurses), nom aborts at about 340 levels, winnow 1,700, peg
    2,050, logos 2,700, regex 5,000 and `std` 5,300, while chumsky
    (`stacker`), pest (its stack guard) and lalrpop (an explicit LR stack)
    read 200,000. Release builds use smaller frames and get further. The
    printer rows recurse too, so printing a deep enough value overflows
    whichever reader built it. A `max_depth` setting or an `Error` case for
    it would turn the crash into a reported error.
12. **nom's `escaped_transform` misreports a trailing backslash.** It
    reports `"abc\` at the start of the string body rather than as running
    out of input, and rejects an empty body. The row uses `fold_many0` over
    literal runs and escapes instead, as nom's own string example does.

### Printer

13. **`&mut String` output.** The trait appends to a `String`, which rules
    out the small-string candidates on the printer page (`compact_str`,
    `smol_str`) as output buffers and writing straight to `io::Write`.
    Refinement 7. `ryu` is not a row because `Value` has no floats.

### Environment

14. **`set` takes `&str`.** Every binding allocates an owned key, including
    every parameter of every function call, although the value already
    holds the symbol as an `Rc<str>`. hashbrown's `entry_ref` avoids the
    allocation only when rebinding. Refinement 5.
15. **Persistent maps cannot be used as persistent maps.** The idiomatic
    environment on `imbl` or `rpds` gives each child scope a structurally
    shared snapshot of its parent, so lookups hit one map. `Env::set`
    through a parent must be visible to children created before it
    (recursive `def!` after `fn*` captured the scope relies on it), which
    snapshots break. So both rows keep the scope chain and a `RefCell`, and
    the persistent map is just a map. Refinement 6.
16. **No constructor in the trait.** The wiring and the benchmarks need
    `E: Default`. Refinement 12.

### Evaluator

17. **`eval(&mut self)` versus re-entry.** `eval`, `apply`, `map`, `swap!`
    and macro expansion all call back into the evaluator from inside a
    built-in or closure, so the evaluator has to be stateless and `Clone`,
    and the `eval` built-in clones it per call. Refinement 4.
18. **`DEBUG-EVAL` cannot be implemented.** The optional tests that bind
    `DEBUG-EVAL` expect the evaluator to print each form, and the evaluator
    has no printer. These are soft failures (17 across steps 3, 7 and 8) in
    every row.
19. **Reference cycles leak.** Every closure that captures the scope it is
    defined in (every recursive `def!`) and the `eval` built-in, which
    captures the top-level environment, form `Rc` cycles. This is the cost
    of the `std` row's memory-management strategy rather than a trait issue,
    and what a tracing-GC row would be measured against.

### Core namespace

20. **The core needs other components.** `read-string` needs a reader,
    `pr-str`/`str`/`prn`/`println` a printer, and `eval`, `apply`, `map` and
    `swap!` the evaluator; `install` only receives the environment. The
    `std` row is generic over a reader and a printer, calls functions through
    `Value::apply`, and `eval` is installed by the wiring. Refinement 3.
21. **`readline` cannot use the line-editor row.** It reads standard input
    directly.
22. **Representation is fixed.** The numeric candidates (`num-bigint`,
    `rug`) need a different number type in `Value`, and the core is the only
    row that would change it. Refinement 10. Until then the core table has
    only the `std` row.
23. **The benchmark only times `install`.** Timing the built-ins themselves
    would make a second core row worth comparing.

### Line editor

24. **"Show the prompt" when piped.** rustyline reads piped input itself
    but stops printing the prompt, and the newline at end of input, so the
    row writes both; it still applies backspace characters in piped lines,
    which the other rows pass through unchanged. reedline cannot run without
    a terminal at all; the row reads piped input with `std::io`, as nushell
    does, so the step column for reedline is reached through that fallback.
    The conformance tests pipe input (`runtest.py --no-pty`); upstream
    `runtest.py` uses a pseudo-terminal by default, where reedline sends a
    cursor query before every line that the runner does not cope with.
25. **History.** `add_history` returns `()`, so rustyline's error is
    dropped; reedline records history itself, so the call is a no-op. Neither
    can save a history file without a lifecycle hook. Refinement 11.

### Interpreter and wiring

26. **Run-time selection multiplies code.** Every row is generic, so every
    combination the binary can run is a monomorphised copy. A default build
    allows one row to differ from `std` at a time, which keeps the copies
    linear; the `matrix` feature compiles the product, about 900 copies,
    which costs seconds in a debug build and minutes under the release
    profile's fat LTO. The copies are the point: each is optimised as a
    whole, which is what [stack benchmarks](stacks.md) measure. Evaluators
    and core namespaces are generic over other rows, so they are plugged in
    as families (a generic associated type picks the member for the rest of
    the stack). That needed no trait change, only a marker type per row in
    the wiring.
27. **Rendered errors.** `run_repl` prints `Error: {e}` with `Display`, which
    cannot print a thrown mal value with the printer row, so the wiring's
    error type is a pre-rendered `String`. Refinement 9.

## Second batch: what the new rows showed

The question the second batch asked was whether the traits are
over-prescriptive: do they admit crates that do the job in a way the first
rows did not, without adding requirements on top of what Make-a-Lisp itself
asks for? Mostly yes. Every new row was written the way its crate's own
documentation writes things, and none needed a trait change. The friction
below is either already a listed refinement or a consequence of mal's test
suite rather than of the traits.

### What fit without friction

28. **A ready-made parser with its own data model (lexpr).** The `Reader`
    trait is the shape lexpr already has: `Parser::next_value` reads one
    datum and leaves the rest, returns `None` for blank or comment-only
    input, and `Error::is_eof` says whether input ran out. The conversion
    from `lexpr::Value` to the shared `Value` is a 30-line function. What
    lexpr cannot do is read mal (no `{}` maps; `,` is `unquote`, not
    whitespace; no `~`, `@` or `^` reader macros), so the gate it fails is
    the grammar, not the trait. It passes 85% of the suite's cases (all of
    steps 3 to 5) and still reaches only `step0_repl`, because a row's step
    is the last of an unbroken run of fully passing steps. Refinement 14.
29. **Configuration lives in the constructor.** pretty needs a line width
    and lexpr needs its syntax options; neither is a trait parameter and
    neither has to be. `PrettyPrinter::with_width` and
    `LexprReader::default` carry them, the same way lalrpop's row keeps its
    compiled parser. This argues against adding options such as a width to
    `Printer`.
30. **Combinators with commit-on-consume semantics (combine).** combine only
    tries the next alternative of `choice` when the previous one failed
    without consuming input, which is exactly "a missing `)` is an error at
    EOF, and `~` with nothing after it is not a symbol". No `cut` was needed
    (compare nom, winnow and chumsky, notes 9 and 10). The grammar is generic
    over the input stream, as combine's examples are, and only needed one
    type annotation on an `and_then` closure for the generic error type.
31. **Not hashing at all (smallvec), hashing one byte at a time (fnv).**
    Both are drop-in. They are there for the numbers: whether a linear scan
    over a few inline pairs beats hashing for function-call scopes, and how
    far the newer word-at-a-time hashers have moved from FNV.
32. **A readline that is not a readline (dialoguer).** dialoguer's `Input`
    is a form prompt. It fits `read_line(prompt)` once a theme writes the
    prompt verbatim (the default appends `": "`) and the answer echo is
    turned off, and it records history itself, so the default no-op
    `add_history` is right for it, as for reedline.

### Friction

33. **An arena of scopes is reference counting again (slotmap).** `Env`
    handles are `Clone` and the trait says nothing about when a scope is
    dead, so an arena cannot free a function call's scope when the call
    returns: a row that never freed scopes would grow without bound under the
    benchmark loops. The row counts handles per scope and frees a scope, and
    any outer scopes only it kept alive, when the count reaches zero. That is
    `Rc` written by hand, plus a hazard `Rc` hides: freeing a scope drops its
    values, a value can be a closure holding a handle into the same arena,
    and that handle's `Drop` borrows the arena again, so every drop has to
    happen after the arena's `RefCell` borrow is released. A tracing
    collector over the arena (the point of arena-allocated scopes in most
    interpreters) would need a root set the trait cannot provide. This is the
    environment-side half of note 19 and refinement 8: the traits assume
    reference-counted sharing.
34. **Interning stops at the trait boundary (lasso).** Symbols are interned
    once, and scopes are keyed by 32-bit `Spur`s instead of owned strings, so
    binding a parameter no longer allocates. But `get` and `set` take `&str`
    and the shared `Value` stores symbols as `Rc<str>`, so every lookup still
    hashes the symbol's text in the interner before walking the chain with
    the integer key. The one thing interning gives for free is that a symbol
    never bound anywhere is rejected after that single interner lookup.
    Refinement 5.
35. **No end-of-input key (dialoguer).** `LineEditor::read_line` returns
    `Ok(None)` at end of input, and the REPL loop relies on it to exit.
    dialoguer ignores Ctrl-D as a control character, so on a terminal the
    loop can only end through Ctrl-C, which ends the process. Checked on a
    pseudo-terminal: the prompt is written verbatim, the up arrow recalls the
    previous line, Ctrl-D does nothing, Ctrl-C exits with SIGINT. The trait
    is not wrong here; the crate is answering a different question.
36. **Depth safety cannot be one component's job (stacker).** With
    `stacker::maybe_grow` around each nested form, the `std` parser reads
    200,000 levels of nesting instead of aborting at about 5,000. The value
    it returns still cannot be printed (every printer recurses) or dropped
    (`Value`'s derived `Drop` recurses through `Rc<Vec<Value>>`) at that
    depth, so the reader row fixes the crash only for input that is rejected
    or never used. Refinement 13 is a property of the whole stack.
37. **Bytes, not `str` (lexical).** `lexical-write-integer` formats into a
    `&mut [u8]`, so the digits are UTF-8 checked before they can be appended
    to the `String` the trait writes into. With `unsafe_code` denied in the
    workspace the check stays; next to the `itoa` row it shows what that
    costs.

### Trait surface no row needed

38. Across every row and the wiring: `Interpreter::host_language` is never
    called (the wiring binds `*host-language*` directly);
    `Implementation::supports` is never called; `Env::contains` is called
    only by the shared checks; `CoreNamespace::symbols` is used only for the
    benchmark's throughput figure. Ten of twelve readers are unit structs
    and the other two (lalrpop's compiled parser, lexpr's options) only read
    their state, so `read_str(&mut self)` is never used mutably. Refinement
    15.

### Candidates looked at and not implemented

| crate | component | why not (yet) |
|-------|-----------|---------------|
| [pom], [grmtools] (`lrpar`) | reader | Same categories as nom/winnow (PEG combinators) and lalrpop (LR with a build script); worth adding for the numbers, not expected to show anything new about the traits. |
| [tree-sitter] | reader | Needs Rust 1.90 (MSRV is 1.85), generates a C parser, and produces a concrete syntax tree for editors rather than values; a poor fit for "read one form". |
| [sexp] | reader | Same kind of crate as lexpr (ready-made S-expression parser), less configurable. |
| [compact_str], [smol_str] | printer | Blocked by `&mut String` output, refinement 7. |
| [ryu] | printer | `Value` has no floats. |
| [rapidhash] | env | Another hasher; would add a number, not a finding. |
| [elsa] | env | `FrozenMap` never replaces a value (references into it must stay valid), and `Env::set` must replace an existing binding (rebinding with `def!`), so it cannot be a scope's map. |
| [string-interner], [ustr] | env | Same finding as lasso. |
| [num-bigint], [rug] | core | Need a number type other than `Value::Int(i64)`, refinement 10. |
| [gc], [dumpster], [gc-arena], [bumpalo] | eval | Need their own value type, so their own reader and printer, refinement 8. A `stacker` evaluator was also considered: it would have to copy the whole 460-line `std` evaluator to wrap its recursive calls, since the evaluator has no extension points. |
| [inquire], [linefeed] | line editor | inquire is another form-prompt crate (as dialoguer); linefeed has been unmaintained since 2019. |

[pom]: https://github.com/J-F-Liu/pom
[grmtools]: https://github.com/softdevteam/grmtools
[tree-sitter]: https://github.com/tree-sitter/tree-sitter
[sexp]: https://github.com/cgaebel/sexp
[compact_str]: https://github.com/ParkMyCar/compact_str
[smol_str]: https://github.com/rust-analyzer/smol_str
[ryu]: https://github.com/dtolnay/ryu
[rapidhash]: https://github.com/hoxxep/rapidhash
[elsa]: https://github.com/Manishearth/elsa
[string-interner]: https://github.com/robbepop/string-interner
[ustr]: https://github.com/anderslanglands/ustr
[num-bigint]: https://github.com/rust-num/num-bigint
[rug]: https://gitlab.com/tspiteri/rug
[gc]: https://github.com/Manishearth/rust-gc
[dumpster]: https://github.com/claytonwramsey/dumpster
[gc-arena]: https://github.com/kyren/gc-arena
[bumpalo]: https://github.com/fitzgen/bumpalo
[inquire]: https://github.com/mikaelmello/inquire
[linefeed]: https://github.com/murarth/linefeed

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
