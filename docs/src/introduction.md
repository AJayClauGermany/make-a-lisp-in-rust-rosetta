# Introduction

This project compares Rust crates by making each of them do the same job
inside a small Lisp interpreter.

The interpreter is [Make-a-Lisp][mal] (mal), a language-agnostic guide that
builds a Lisp in eleven numbered steps, with a test suite for each step. The
guide splits the work into components with sharp boundaries: a reader, a
printer, an environment, an evaluator, a core namespace and a REPL. Each of
those is a job that several Rust crates can do, which makes each component a
natural [rosetta-rs][rosetta]-style comparison: one task, implemented once per
crate, built and measured identically, presented in a table.

## What you will find here

- **Methodology**: what is measured, how, and what the columns mean.
- **The Make-a-Lisp steps**: the ladder every implementation climbs, and
  which component each step exercises.
- **Architecture**: the traits in `mal-traits` that all implementations are
  written against, and how the `mal` binary wires them together.
- **Benchmarking**: how rows are timed, how runs are recorded under `runs/`,
  and how the results page is generated from them.
- **Components**: one page per component with the candidate crates, the
  qualitative comparison table and notes on what the job demands.
- **Results**: the measured tables from the most recent run.

## Status

The workspace, traits, benchmark harness, run recorder and this site are in
place. No comparison implementations have been written yet, so the tables
contain only the dependency-free baselines, and the results page shows the
baseline interpreter's numbers from the first recorded run.

[mal]: https://github.com/kanaka/mal
[rosetta]: https://github.com/rosetta-rs
