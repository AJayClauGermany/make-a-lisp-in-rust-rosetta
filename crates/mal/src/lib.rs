//! Library half of the `mal` package.
//!
//! The binary in `main.rs` and the benchmarks in `benches/` both need the
//! dependency-free baselines and the implementation registry, so those live
//! here. Nothing in this crate pulls in a third-party dependency: the
//! benchmark harness (`mal-bench`, Criterion) is a dev-dependency only and
//! never reaches the `mal` binary.

pub mod baseline;
pub mod registry;
