# Methodology

The method follows the rosetta-rs repositories such as
[parse-rosetta-rs](https://github.com/rosetta-rs/parse-rosetta-rs) and
[argparse-rosetta-rs](https://github.com/rosetta-rs/argparse-rosetta-rs).

## One task, many crates

Each component of the interpreter is a task with a fixed interface, defined
as a trait in `mal-traits`. A comparison row is a crate implementing that
trait, in its own package under `comparisons/<component>/<crate>/`, depending
on nothing but `mal-traits` and the crate(s) being compared.

Implementations are written the way a user of that crate would write them,
not tuned for the benchmark. The question each row answers is "what is it
like to do this job with this crate", not "how fast can this crate be made
to go".

## The baseline

Every component has a `std` row: the smallest implementation using only the
standard library. It is the `null` row of the rosetta tables and the zero
point for the overhead columns.

The `mal` binary currently links every row, so that the conformance tests
and `mal --reader nom` can select any of them at run time. The planned
binary-size column needs a `mal` built with only the baselines plus the one
row being measured; that means putting each non-`std` row behind a cargo
feature, which is not done yet.

## Columns

### Qualitative

Filled in by hand when a row is added, from the crate's documentation and the
experience of writing the implementation. They differ per component; for
example the reader table records parser type, where the action code lives
and how the crate integrates (library, build script, proc macro).

### Measured

Produced by `cargo xtask bench` and committed under `runs/`, one directory
per run; the [Results](results.md) page is generated from the newest run.
See [Benchmarking](benchmarking.md) for the mechanics.

| column | how it is measured | status |
|--------|--------------------|--------|
| Run time (micro) | Criterion benchmarks of the row's trait methods on the fixed workloads in `mal_bench::corpus`: mean and standard deviation per iteration, throughput where the workload has a size. One Criterion group per row, `<component>/<name>`. | recorded |
| Overhead (release) | Release binary size of `mal` with only this row wired in, minus the size with only the `std` baselines. Built with the workspace `release` profile (fat LTO, one codegen unit, symbols stripped). Needs per-row cargo features first (see above). | planned |
| Build (debug) | Wall-clock time of a from-scratch `cargo build` of the row's package in the `dev` profile. | planned |
| Run (release) | Wall-clock time of the mal test suite for the row's highest step, release build. | planned |
| Step reached | The highest `stepN` of the mal test suite the row passes. Steps are cumulative. | from the registry |
| Downloads | crates.io recent downloads badge for the headline crate. | planned |
| Version | The exact version compiled against. | from the registry |

The host (name, OS, architecture, CPU model and count), toolchain, commit
and exact benchmark command of every run are recorded alongside the numbers,
and the page says which run it was rendered from. Numbers from different
hosts are presented side by side only with the host named; the change
column between two runs is read as meaningful only when the hosts match.

## Correctness

A row is only compared at the steps it passes the mal test suite for. The
"step reached" column is therefore also a correctness statement, and an
implementation that stops at `step4_if_fn_do` is reported as such rather than
excluded.

The column is checked, not claimed: `cargo test` runs the upstream test
files (vendored under `crates/mal/tests/mal/`) against every registered row
with the same matching rules as mal's `runtest.py`, swapping that one row
into an otherwise all-`std` interpreter, and fails if the step a row reaches
differs from the step its `Implementation` states. Failures in the suite's
optional (`soft`) sections are reported but do not count, as upstream.

Rows also run shared unit checks for their component (`mal-testkit`), and
every reader must read every benchmark form to exactly the value the `std`
reader produces, so the reader table never times an error path.
