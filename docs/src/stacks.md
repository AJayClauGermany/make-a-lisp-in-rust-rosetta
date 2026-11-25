# Stacks: combining rows

The component tables measure one row at a time against `std`. A real
interpreter is one reader, one printer, one environment, one evaluator and
one core namespace *together*, and the compiler optimises them together:
every row is generic, so each combination is its own monomorphised copy
of the interpreter, built with fat LTO. A reader can be inlined into the
`read-string` built-in; an environment's lookup can be folded into the
evaluator's loop; two rows that allocate the same way can share warm
allocator caches. So a combination can be faster, or slower, than its rows
suggest on their own. A *stack* is such a combination. This page explains
how to build, check and measure stacks.

## The pieces

| piece | where | job |
|-------|-------|-----|
| `Row` trait | `mal-traits` | every row type carries its own `Implementation`, so generic code can name the rows it was built from |
| row lists | `crates/mal/src/stack.rs` | the single list of reader, printer, env, eval and core rows; the registry, the binary, the benchmarks and the tests all read it |
| `StackVisitor` | `crates/mal/src/stack.rs` | turns row *names* (a `Selection`) into a call with the row *types*: the way to "loop over" combinations of types |
| `StackSet` | `crates/mal/src/stack.rs` | a set of combinations, parsed from a spec (below) |
| `matrix` feature | `crates/mal/Cargo.toml` | compiles every combination instead of only the one-swap stacks |
| `benches/stacks.rs` | `crates/mal` | times whole stacks: the evaluation programs and the REPL session |
| `tests/stacks.rs` | `crates/mal` | checks every stack prints exactly what the `std` stack prints |
| `MAL_CONFORMANCE_STACKS` | `tests/conformance.rs` | runs the mal test suite against stacks |
| stacks section | [Results](results.md#stacks) | ratios to the `std` stack, and the interaction of each combination |

### Generic rows

Readers, printers and environments are plain types. Evaluators and core
namespaces are generic over the rows they call (`StdEvaluator<E>`,
`StdCore<E, R, P>`), so their rows are *families*: a marker type with a
generic associated type that picks the member for the other rows of the
stack (`EvalRow::For<E>`, `CoreRow::For<E, R, P>`). A new evaluator or core
row adds a two-line marker next to `StdEvalRow` / `StdCoreRow`.

## Choosing stacks: the spec

One spec syntax is used by the benchmarks (`MAL_STACKS`), the consistency
test (`MAL_STACKS`), the conformance test (`MAL_CONFORMANCE_STACKS`) and
`cargo xtask bench --stacks`. Components are separated by spaces or `;`
and rows by `,`. A component left out stays `std`. `*` means every row of
that component that reaches `stepA_mal`.

```text
sweep                        std, then every row swapped into it alone
reader=nom,winnow env=*      2 readers × 13 environments
printer=* ; env=rustc-hash   every printer on the rustc-hash environment
reader=* printer=* env=*     every combination of those three
all                          every combination of every component
```

To vary one component and hold the rest fixed at something other than
`std`, name the fixed rows once and use `*` for the one that varies:
`reader=logos env=rustc-hash printer=*`.

The benchmarks also add the `std` stack and the stack for each non-`std`
row on its own, because the [interaction](#reading-the-results) of a
combination is measured against them.

## Default build versus `matrix`

A default build compiles only the stacks with at most one non-`std` row:
one copy of the interpreter per row, a little over thirty. `mal --reader nom`
works; `mal --reader nom --env fnv` explains that it needs the `matrix`
feature. With `--features matrix` every combination is compiled (about
nine hundred today). That costs about 12 s extra in a debug build, but
about ten minutes (on 4 CPUs) for the stack benchmarks: release builds use fat LTO and
one codegen unit, which is what lets the optimiser see across rows. So it
is off unless asked for:

```bash
cargo run -p mal --features matrix -- --reader winnow --printer itoa --env rapidhash
cargo test -p mal --features matrix --test stacks          # every combination vs std
MAL_STACKS='reader=nom,winnow env=*' cargo test -p mal --features matrix --test stacks
MAL_CONFORMANCE_STACKS='reader=nom printer=*' cargo test -p mal --features matrix --test conformance
```

## Measuring

```bash
cargo xtask bench --stacks sweep                    # no matrix needed
cargo xtask bench --stacks 'reader=* env=*'         # adds --features matrix itself
cargo xtask bench --stacks all -- --warm-up-time 1 --measurement-time 2
```

Each stack is timed on four workloads (the three `PROGRAMS` through `rep`,
and the REPL session), recorded as `stack/<label>/<workload>`. The label
lists the non-`std` rows, e.g. `stack/reader=nom,env=fnv`, or is `std`. At
Criterion's defaults that is about 30 seconds a stack. The sweep is about
30 stacks; `reader=* env=*` is about 140; `all` is about 860, so it is
only practical with shorter Criterion times as above. A stack run is
recorded like any other. The results page takes its component tables from
the newest run that measured components and its stacks section from the
newest run that measured stacks, so the two kinds of run do not hide each
other.

## Reading the results

For each stack the [results page](results.md#stacks) shows:

- **ratio**: the geometric mean, over the workloads, of the stack's time
  divided by the `std` stack's time;
- **predicted**: for a stack with several non-`std` rows, the product of
  the ratios of the stacks that swap in each of those rows alone. This is
  what the combination would cost if the rows did not affect each other;
- **interaction**: ratio divided by predicted. Below 1× the rows do better
  together than their separate results predict, above 1× worse. Values
  within 5% are marked as noise.

An interaction is the thing to look for. "The best reader plus the best
environment" is only the best stack if the interaction is near 1×. When it
is not, the combination is the finding. A typical case would be a reader
whose symbols share allocations with an environment's keys, or two rows
that each inline well alone but together push the evaluator loop past an
inlining threshold.

## Rows that would make combinations more interesting

The component pages list candidates for each component. The crates below
cut *across* components, which makes them the most likely to produce
interactions. Most need a change somewhere other than one row: a build
axis, or a refinement in the [trait notes](trait-notes.md). Versions were
checked on crates.io on 2026-11-25.

| crate(s) | what it changes | why combinations matter | needs |
|----------|-----------------|-------------------------|-------|
| [mimalloc] 0.1.52, [tikv-jemallocator] 0.7.0, [snmalloc-rs] 0.7.6 | the global allocator | every row allocates (`Rc` values, scopes, strings); an allocator can favour one row's pattern over another's | a build axis (cargo feature per allocator; one binary per allocator), not a trait |
| [dhat] 0.3.3, [stats_alloc] 0.1.10 | counts allocations | separates "allocates less" from "computes faster" in a combination | a counting allocator in the bench binary |
| [ustr] 1.1.0, [string-interner] 0.20.0, [symbol_table] 0.5.0 + [nohash-hasher] 0.2.0 | interned symbols with precomputed hashes | the reader interns once; the env hashes nothing. Only pays off if both rows use it, which is an interaction by construction | refinement 5 (an owned symbol type) |
| [compact_str] 0.10.0, [smol_str] 0.3.7, [ecow] 0.3.1, [arcstr] 1.2.0, [hipstr] 0.8.0 | small or shared strings for `Str`/`Symbol` | changes allocation in the reader, key cost in the env, and copies in the printer | a `Value` representation choice |
| [triomphe] 0.1.17, [rclite] 0.4.2, [archery] 1.2.3 | `Rc` without weak counts, smaller counts, or `Rc`/`Arc` behind one type | every clone in every row | a `Value` representation choice |
| [rapidhash] 4.5.1 (now a row), [gxhash] 3.5.0, [xxhash-rust] 0.8.21, [highway] 1.3.0 | hashers | the same hasher behaves differently on short symbol keys inside different tables | env rows (drop-in); gxhash needs AES target features |
| [bumpalo] 3.20.4, [bump-scope] 2.4.1 | arena allocation per REPL line | reader output, eval temporaries and printer buffers in one arena | refinement 8 (readers build any value type) |
| [jrsonnet-gcmodule] 0.5.0, [boa_gc] 0.22.0, [dumpster] 2.1.0 | cycle collection / tracing GC | changes the cost of every closure and scope, so the env rows rank differently | a new evaluator row with its own value type (refinement 8) |
| [thin-vec] 0.2.21 | `Rc<ThinVec<Value>>` lists | one less indirection for every row that walks lists | a `Value` representation choice |

Avoid, according to RustSec as of 2026-11: `chrono` (unmaintained, use
`jiff` or `time`), `im`/`im-rc` (unmaintained, use `imbl`), `smartstring`
(unmaintained), `fxhash` (use `rustc-hash`), `generational-arena`, and
`internment` 0.8.7 (malicious release; pin 0.8.6 or earlier).

The cheapest next axis to add is the global allocator. It needs no trait
change: give `mal` one cargo feature per allocator, have
`cargo xtask bench --allocator <name>` pass that feature through, and
record the allocator in the run file next to the toolchain.

[mimalloc]: https://crates.io/crates/mimalloc
[tikv-jemallocator]: https://crates.io/crates/tikv-jemallocator
[snmalloc-rs]: https://crates.io/crates/snmalloc-rs
[dhat]: https://crates.io/crates/dhat
[stats_alloc]: https://crates.io/crates/stats_alloc
[ustr]: https://crates.io/crates/ustr
[string-interner]: https://crates.io/crates/string-interner
[symbol_table]: https://crates.io/crates/symbol_table
[nohash-hasher]: https://crates.io/crates/nohash-hasher
[compact_str]: https://crates.io/crates/compact_str
[smol_str]: https://crates.io/crates/smol_str
[ecow]: https://crates.io/crates/ecow
[arcstr]: https://crates.io/crates/arcstr
[hipstr]: https://crates.io/crates/hipstr
[triomphe]: https://crates.io/crates/triomphe
[rclite]: https://crates.io/crates/rclite
[archery]: https://crates.io/crates/archery
[rapidhash]: https://crates.io/crates/rapidhash
[gxhash]: https://crates.io/crates/gxhash
[xxhash-rust]: https://crates.io/crates/xxhash-rust
[highway]: https://crates.io/crates/highway
[bumpalo]: https://crates.io/crates/bumpalo
[bump-scope]: https://crates.io/crates/bump-scope
[jrsonnet-gcmodule]: https://crates.io/crates/jrsonnet-gcmodule
[boa_gc]: https://crates.io/crates/boa_gc
[dumpster]: https://crates.io/crates/dumpster
[thin-vec]: https://crates.io/crates/thin-vec
