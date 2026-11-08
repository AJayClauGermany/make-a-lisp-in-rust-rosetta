# Architecture

The workspace has two permanent crates and a growing set of comparison
crates.

```text
crates/mal-traits      traits, Component/Step vocabulary, Implementation metadata, run_repl
crates/mal             the REPL binary; std-only baselines; the implementation registry;
                       benches/components.rs, the list of what gets benchmarked
crates/mal-bench       the benchmark harness: fixed workloads, one Criterion bench per trait
comparisons/<c>/<x>    one crate per component/crate pair, depends only on mal-traits
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

## `mal`

The binary, plus a small library the binary and the benchmarks share. It
parses its few flags by hand so it contributes nothing to the size and
build-time columns, ships the `std` baselines (`StdinLineEditor`,
`EchoInterpreter`), and keeps the registry that `mal --list` and the report
generator read. Comparison crates are wired in here, and timed from here:
`benches/components.rs` is one line per implementation, calling the matching
harness function. Criterion and `mal-bench` are dev-dependencies only, so
they never reach the binary.

## `mal-bench` and `xtask`

`mal-bench` holds the fixed workloads (`corpus`) and one generic Criterion
benchmark per component trait, each recording under the group
`<component>/<implementation>` so results can be joined back to the registry.
`xtask` is the runner: `cargo xtask bench` records a run as
`runs/<id>/results.json`, `cargo xtask report` renders the newest run and the
registry into `docs/src/results.md`, and `cargo xtask compare` diffs two
runs. These are the only crates with third-party dependencies. See
[Benchmarking](benchmarking.md).

## Comparison crates

Each depends on `mal-traits` and the crate under test, implements one trait,
and exports its `Implementation`. See [Contributing](contributing.md).
