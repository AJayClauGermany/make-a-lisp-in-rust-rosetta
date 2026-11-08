//! The benchmark list.
//!
//! One function per component; each line in a function benchmarks one
//! registered implementation on the shared workloads in `mal_bench::corpus`.
//! To benchmark a new row, add one line to the function for its component,
//! passing its `IMPLEMENTATION` constant and a constructor (plus fixtures for
//! the components whose value type belongs to the implementation). Then run
//! `cargo xtask bench` to record a run and regenerate the results page.
//!
//! Line editors are not timed; see `mal_bench` for why.

// `criterion_group!` expands to an undocumented public function.
#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use mal::baseline;

fn readers(c: &mut Criterion) {
    // mal_bench::reader(c, &reader_nom::IMPLEMENTATION, reader_nom::NomReader::default);
    let _ = c;
}

fn printers(c: &mut Criterion) {
    // let values: Vec<_> = parse_corpus_with(reader_std::StdReader::default());
    // mal_bench::printer(c, &printer_itoa::IMPLEMENTATION, &printer_itoa::ItoaPrinter, &values);
    let _ = c;
}

fn envs(c: &mut Criterion) {
    // mal_bench::env(c, &env_im::IMPLEMENTATION, env_im::ImEnv::default, |i| Value::Int(i as i64));
    let _ = c;
}

fn evaluators(c: &mut Criterion) {
    mal_bench::interpreter(c, &baseline::ECHO, || baseline::EchoInterpreter);
    // mal_bench::evaluator(c, &eval_rc::IMPLEMENTATION, eval_rc::setup, &forms);
}

fn cores(c: &mut Criterion) {
    // mal_bench::core_namespace(c, &core_std::IMPLEMENTATION, core_std::Core::default, Env::default);
    let _ = c;
}

criterion_group!(benches, readers, printers, envs, evaluators, cores);
criterion_main!(benches);
