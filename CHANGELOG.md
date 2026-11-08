# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Cargo workspace with `mal-traits` (component traits, step and component
  vocabulary, `Implementation` metadata, REPL driver) and `mal` (the REPL
  binary with a dependency-free step0 baseline and `--list`).
- Documentation site scaffold: mdBook under `docs/` and oranda configuration.
- CI: format, clippy, test, docs, `cargo deny`, and the oranda site build.
- Benchmarking: `mal-bench` (fixed workloads and one generic Criterion
  benchmark per component trait), a bench list in `crates/mal/benches/`
  that takes one line per implementation, and `cargo xtask bench | report |
  compare` to record runs as `runs/<id>/results.json` and generate
  `docs/src/results.md` from the newest one. CI checks the page is current
  and runs the benchmarks once as a smoke test. First run recorded for the
  `echo` baseline.
- `Component::title()` for headings; `mal` now also exposes its baselines
  and registry as a library for the benchmarks.
