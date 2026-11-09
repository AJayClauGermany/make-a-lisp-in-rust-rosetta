# Architecture

The workspace has a handful of permanent crates and a growing set of
comparison crates.

```text
crates/mal-traits      traits, Component/Step vocabulary, Implementation metadata, run_repl
crates/mal-types       the shared Rc-based Value and MalError rows agree on
crates/mal-testkit     unit checks every reader, printer and env row runs (dev-only)
crates/mal             the stepA binary assembled from rows; run-time row selection;
                       the implementation registry; benches/components.rs, the list of
                       what gets benchmarked; tests/conformance.rs, the mal test suite
crates/mal-bench       the benchmark harness: fixed workloads, one Criterion bench per trait
comparisons/<c>/<x>    one crate per component/crate pair: mal-traits, mal-types and the crate
xtask                  cargo xtask bench | report | compare: runs/ in, docs/src/results.md out
runs/<id>/             one results.json per recorded benchmark run
```

## `mal-traits`

Has no dependencies and contains no implementations. It defines one trait per
component, each generic over the value type through an associated type so
that value representation is itself something a row can vary:

| trait | mal module | job |
|-------|------------|-----|
| `Reader` | `reader` | `read_str`: text to a value, or `Empty` for blank input |
| `Printer` | `printer` | `pr_str`: value to text, readably or not |
| `Env` | `env` | scoped bindings: `child`, `set`, `get` |
| `Evaluator` | `stepN` `EVAL` | evaluate a form in an environment |
| `CoreNamespace` | `core` | install the built-ins into an environment |
| `LineEditor` | REPL input | read one line, optional history |
| `Interpreter` | `rep` | read, eval, print one line |

Alongside the traits:

- `Component` and `Step` enumerate the comparison axes. `Component::slug()`
  is the directory name under `comparisons/`; `Step::name()` is the mal test
  file name.
- `Implementation` and `CrateRef` describe a table row: the crates used with
  exact versions, the highest step reached and one line of notes. Every
  comparison crate exports one `Implementation` constant so the tables are
  generated from the code rather than maintained by hand.
- `run_repl` is the one shared piece of behaviour: the loop that reads a
  line, calls `Interpreter::rep`, prints the result or `Error: ...`, and
  stops at end of input. All interpreters share it so the measurements stay
  about the components.

## `mal-types`

The traits leave the value type to each implementation, but a reader row
can only feed a printer row, and the rows of one component can only be
benchmarked on the same fixtures, if they agree on one. `mal-types` is that
agreement: an `Rc`-based `Value` enum, `MalError`, mal equality, metadata and
`Value::apply`, which lets the core namespace call closures without knowing
the evaluator's environment type (closures store their environment
type-erased). It has no dependencies. Evaluator rows that compare a
different representation cannot use it; see [Trait notes](trait-notes.md).

## `mal`

The binary, plus the library the binary, the benchmarks and the tests
share. It parses its flags by hand so argument parsing adds nothing to the
size and build-time columns, ships the binary-level baselines (`StdinLineEditor`,
`EchoInterpreter`), and keeps the registry that `mal --list` and the report
generator read.

- `interpreter::Mal` is the guide's `rep`: a reader, the evaluator and a
  printer around a top-level environment with the core namespace, the
  `eval` built-in, `*ARGV*`, `*host-language*` and the mal prelude (`not`,
  `load-file`, `cond`). `ReadPrint` is step1: read and print only.
- `select` builds an interpreter from row names: `mal --reader nom` runs
  the `nom` reader with `std` everywhere else. Only one of reader, printer,
  env, eval and core may differ from `std` at a time, so the number of
  monomorphised interpreters grows with the number of rows rather than
  their product. `--line-editor` is chosen separately, and `--read-print`
  composes with any reader or printer row.
- `tests/conformance.rs` runs the vendored mal test suite (`tests/mal/`)
  against every registered row through the binary and fails if a row does
  not reach exactly the step it claims. `step0_repl` runs only for the echo
  evaluator and `step1_read_print` in `--read-print` mode, because those
  files test the shape of their own step.

Comparison crates are timed from here too: `benches/components.rs` is one
line per implementation, calling the matching harness function. Criterion,
`mal-bench` and the test dependencies are dev-dependencies only, so they
never reach the binary.

## `mal-bench` and `xtask`

`mal-bench` holds the fixed workloads (`corpus`) and one generic Criterion
benchmark per component trait, each recording under the group
`<component>/<implementation>` so results can be joined back to the registry.
`xtask` is the runner: `cargo xtask bench` records a run as
`runs/<id>/results.json`, `cargo xtask report` renders the newest run and the
registry into `docs/src/results.md`, and `cargo xtask compare` diffs two
runs. Their third-party dependencies (Criterion, serde) never reach the
`mal` binary. See [Benchmarking](benchmarking.md).

## Comparison crates

Each depends on `mal-traits`, `mal-types` and the crate under test,
implements one trait, and exports its `Implementation`. See
[Contributing](contributing.md).
