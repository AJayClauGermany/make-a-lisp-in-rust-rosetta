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
point for the overhead columns. The `mal` binary ships these baselines so
that the runner itself adds no third-party code to the measurements.

## Columns

### Qualitative

Filled in by hand when a row is added, from the crate's documentation and the
experience of writing the implementation. They differ per component; for
example the reader table records parser type, where the action code lives
and how the crate integrates (library, build script, proc macro).

### Measured

Produced by the benchmark harness and committed under `runs/`.

| column | how it is measured |
|--------|--------------------|
| Overhead (release) | Release binary size of `mal` with this row wired in, minus the size with the `std` baseline. Built with the workspace `release` profile (fat LTO, one codegen unit, symbols stripped). |
| Build (debug) | Wall-clock time of a from-scratch `cargo build` of the row's package in the `dev` profile. |
| Run (release) | Wall-clock time of the mal test suite for the row's highest step, release build. |
| Step reached | The highest `stepN` of the mal test suite the row passes. Steps are cumulative. |
| Downloads | crates.io recent downloads badge for the headline crate. |
| Version | The exact version compiled against. |

The host, toolchain and `-j` setting for every run are recorded alongside
the numbers.

## Correctness

A row is only compared at the steps it passes the mal test suite for. The
"step reached" column is therefore also a correctness statement, and an
implementation that stops at `step4_if_fn_do` is reported as such rather than
excluded.
