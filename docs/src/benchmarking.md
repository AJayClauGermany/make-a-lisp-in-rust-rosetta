# Benchmarking

Every implementation is timed on the same fixed inputs with
[Criterion](https://github.com/bheisler/criterion.rs), each run is committed
as data under `runs/`, and the [Results](results.md) page is generated from
the newest run. Nothing on that page is typed by hand.

## The pieces

| piece | where | job |
|-------|-------|-----|
| workloads | `crates/mal-bench/src/corpus.rs` | the fixed mal forms, the evaluation `PROGRAMS`, the REPL session and symbol lists every row is measured on |
| harness | `crates/mal-bench/src/lib.rs` | one generic Criterion bench per component trait: `reader`, `printer`, `env`, `evaluator`, `core_namespace`, `interpreter` |
| bench list | `crates/mal/benches/components.rs` | one line per implementation: which rows get measured |
| runner | `xtask/` | `cargo xtask bench` records a run; `cargo xtask report` renders it; `cargo xtask compare` diffs two runs |
| data | `runs/<id>/results.json` | one file per run: host, toolchain, commit, command and every benchmark's statistics |
| page | `docs/src/results.md` | generated; CI fails if it is stale |
| charts | `xtask/src/chart.rs` | inline SVG drawn from the same rows as each table: geometric-mean ratio to `std`, and a per-workload heatmap |

The harness is a dev-dependency of `mal` and the runner a separate crate,
so neither reaches the `mal` binary. The comparison crates do: the binary
currently links every row so any of them can be selected at run time (see
the [methodology](methodology.md#the-baseline) for what that means for the
planned binary-size column).

Interpreter benchmarks run `rep` on every corpus form, and check once,
before timing, that each succeeds, so an error path is never what gets
measured. Interpreters that evaluate run `corpus::BINDINGS` first, which
defines the free symbols the forms mention (`a`, `counter`,
`swap-and-reset!`).

## Adding an implementation to the benchmarks

After the row exists and is registered (see [Contributing](contributing.md)),
add one line to the function for its component in
`crates/mal/benches/components.rs`:

```rust
fn readers(c: &mut Criterion) {
    mal_bench::reader(c, &reader_nom::IMPLEMENTATION, reader_nom::NomReader::default);
}
```

Each harness function takes the row's `Implementation` constant, which fixes
the Criterion group name to `<component>/<name>` so the report can join the
numbers back to the registry, and a constructor. Components whose value type
belongs to the implementation (printer, environment, evaluator, core
namespace) also take the fixtures to run on, typically the corpus forms
parsed with the row's own reader:

```rust
mal_bench::env(c, &env_im::IMPLEMENTATION, env_im::ImEnv::default, |i| Value::Int(i as i64));
mal_bench::printer(c, &printer_std::IMPLEMENTATION, &printer_std::StdPrinter, &values);
mal_bench::evaluator(c, &eval_rc::IMPLEMENTATION, eval_rc::setup, &forms);
mal_bench::core_namespace(c, &core_std::IMPLEMENTATION, core_std::Core::default, Env::default);
```

Environment rows are timed twice: `mal_bench::env` on the trait methods,
and `mal_bench::evaluator` running the `PROGRAMS` workloads (recursive
`fib`, a tail-call loop, closures through `map`) with the `std` evaluator on
that environment, recorded under the environment's own group as
`env/<name>/eval/<program>`. The second is what an interpreter actually
does with an environment.

Line editors are not timed: their run time is the terminal's, and the
comparison for that component is binary-size overhead. They still appear on
the results page as *not measured*, because the page lists every registered
implementation.

## Recording a run

```bash
cargo xtask bench                       # record under runs/, regenerate the page
cargo xtask bench -- eval/              # pass a Criterion filter through
cargo xtask bench --host desk --no-report
cargo xtask report                      # regenerate docs/src/results.md
cargo xtask report --check              # what CI runs
cargo xtask compare <run-a> <run-b>     # side-by-side change in mean
```

`cargo xtask bench` clears `target/criterion/` first so the run contains
exactly what that invocation measured, runs `cargo bench -p mal`, reads
Criterion's `benchmark.json` and `estimates.json` for every benchmark, and
writes `runs/<yyyymmdd>-<hhmmss>-<host>/results.json`. Run ids sort
chronologically, so "the newest run" is the last directory.

Record on a quiet machine, commit the run directory together with the
regenerated page, and say which machine it was in the pull request. A run
from a different host is still worth committing; the page names the host
and marks the *vs previous* column as only meaningful between runs from the
same one.

## The run file

`results.json` is self-describing so that two runs can be compared with
their differences in view:

```json
{
  "schema": 1,
  "id": "20261108-192541-cloud-sandbox",
  "recorded_at": "2026-11-08T19:25:41Z",
  "host": { "name": "cloud-sandbox", "os": "linux", "arch": "x86_64", "cpus": 4, "cpu_model": "..." },
  "toolchain": { "rustc": "rustc 1.97.0 (2d8144b78 2026-07-07)" },
  "commit": "d814ce5f7de3",
  "dirty": true,
  "command": ["cargo", "bench", "--package", "mal", "--bench", "components", "--", "--noplot"],
  "benchmarks": [
    {
      "id": "eval/echo/rep/atom",
      "component": "eval",
      "implementation": "echo",
      "workload": "rep/atom",
      "mean_ns": 11.04,
      "median_ns": 10.43,
      "std_dev_ns": 1.77,
      "throughput": { "bytes": 2 }
    }
  ]
}
```

`schema` is bumped when the layout changes incompatibly; `cargo xtask` then
refuses to read older runs rather than misreporting them. The statistics are
Criterion's point estimates in nanoseconds per iteration.

## What is not measured yet

The methodology also calls for binary-size overhead, debug build time and
run time on the mal test suite per row. Those are whole-binary measurements
rather than Criterion benchmarks; they will be added to `cargo xtask bench`
as further sections of the run file once the first comparison crates exist
to be built in and out of the binary.
