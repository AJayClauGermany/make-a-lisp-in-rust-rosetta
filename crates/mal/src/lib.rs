//! Library half of the `mal` package.
//!
//! The binary in `main.rs`, the benchmarks in `benches/` and the
//! conformance tests in `tests/` share the dependency-free baselines, the
//! implementation registry, the interpreter that wires rows together and the
//! run-time row selection, so those live here. The benchmark harness
//! (`mal-bench`, Criterion) is a dev-dependency only and never reaches the
//! `mal` binary.

pub mod baseline;
pub mod interpreter;
pub mod registry;
pub mod select;
