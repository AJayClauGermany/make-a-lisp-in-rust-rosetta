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
use mal::interpreter::Mal;
use mal_bench::corpus::{self, Form};
use mal_traits::{CoreNamespace, Env, ReadOutcome, Reader};
use mal_types::Value;

use core_std::StdCore;
use env_std::StdEnv;
use eval_std::StdEvaluator;
use printer_std::StdPrinter;
use reader_std::StdReader;

/// `forms` read with the `std` reader, for the components that take values.
fn parse(forms: &[Form]) -> Vec<(&'static str, Value)> {
    forms
        .iter()
        .map(|form| match StdReader.read_str(form.source) {
            Ok(ReadOutcome::Form(value)) => (form.name, value),
            other => panic!("corpus form {} does not read: {other:?}", form.name),
        })
        .collect()
}

/// The `std` evaluator and an environment of type `E` with the `std` core
/// namespace installed: what the evaluator and environment rows run
/// programs in.
fn setup<E: Env<Value = Value> + Default + 'static>() -> (StdEvaluator<E>, E) {
    let env = E::default();
    StdCore::<E, StdReader, StdPrinter>::new(StdPrinter).install(&env);
    (StdEvaluator::new(), env)
}

fn readers(c: &mut Criterion) {
    mal_bench::reader(c, &reader_std::IMPLEMENTATION, || StdReader);
    mal_bench::reader(c, &reader_nom::IMPLEMENTATION, || reader_nom::NomReader);
    mal_bench::reader(c, &reader_winnow::IMPLEMENTATION, || {
        reader_winnow::WinnowReader
    });
    mal_bench::reader(c, &reader_chumsky::IMPLEMENTATION, || {
        reader_chumsky::ChumskyReader
    });
    mal_bench::reader(c, &reader_pest::IMPLEMENTATION, || reader_pest::PestReader);
    mal_bench::reader(c, &reader_peg::IMPLEMENTATION, || reader_peg::PegReader);
    mal_bench::reader(
        c,
        &reader_lalrpop::IMPLEMENTATION,
        reader_lalrpop::LalrpopReader::default,
    );
    mal_bench::reader(c, &reader_logos::IMPLEMENTATION, || {
        reader_logos::LogosReader
    });
    mal_bench::reader(c, &reader_regex::IMPLEMENTATION, || {
        reader_regex::RegexReader
    });
}

fn printers(c: &mut Criterion) {
    let values = parse(corpus::FORMS);
    mal_bench::printer(c, &printer_std::IMPLEMENTATION, &StdPrinter, &values);
    mal_bench::printer(
        c,
        &printer_itoa::IMPLEMENTATION,
        &printer_itoa::ItoaPrinter,
        &values,
    );
    mal_bench::printer(
        c,
        &printer_memchr::IMPLEMENTATION,
        &printer_memchr::MemchrPrinter,
        &values,
    );
    mal_bench::printer(
        c,
        &printer_ufmt::IMPLEMENTATION,
        &printer_ufmt::UfmtPrinter,
        &values,
    );
}

fn envs(c: &mut Criterion) {
    // Each row twice: the trait methods on their own, then the `std`
    // evaluator running whole programs on that environment.
    #[allow(clippy::cast_possible_wrap)] // Indices are below ENV_BINDINGS.
    let value = |i: usize| Value::Int(i as i64);
    let programs = parse(corpus::PROGRAMS);
    macro_rules! env_row {
        ($row:ident, $env:ty) => {
            mal_bench::env(c, &$row::IMPLEMENTATION, <$env>::new, value);
            mal_bench::evaluator(c, &$row::IMPLEMENTATION, setup::<$env>, &programs);
        };
    }
    env_row!(env_std, StdEnv<Value>);
    env_row!(env_hashbrown, env_hashbrown::HashbrownEnv<Value>);
    env_row!(env_indexmap, env_indexmap::IndexMapEnv<Value>);
    env_row!(env_rustc_hash, env_rustc_hash::FxEnv<Value>);
    env_row!(env_ahash, env_ahash::AHashEnv<Value>);
    env_row!(env_foldhash, env_foldhash::FoldHashEnv<Value>);
    env_row!(env_imbl, env_imbl::ImblEnv<Value>);
    env_row!(env_rpds, env_rpds::RpdsEnv<Value>);
}

fn evaluators(c: &mut Criterion) {
    mal_bench::interpreter(c, &baseline::ECHO, || baseline::EchoInterpreter);
    let programs = parse(corpus::PROGRAMS);
    mal_bench::evaluator(
        c,
        &eval_std::IMPLEMENTATION,
        setup::<StdEnv<Value>>,
        &programs,
    );
    // The whole std stack, read-eval-print, as the REPL runs it.
    mal_bench::interpreter(c, &eval_std::IMPLEMENTATION, || {
        Mal::new(
            StdReader,
            StdPrinter,
            StdEvaluator::new(),
            StdEnv::new(),
            &StdCore::<_, StdReader, _>::new(StdPrinter),
            &[],
        )
    });
}

fn cores(c: &mut Criterion) {
    mal_bench::core_namespace(
        c,
        &core_std::IMPLEMENTATION,
        || StdCore::<StdEnv<Value>, StdReader, StdPrinter>::new(StdPrinter),
        StdEnv::new,
    );
}

criterion_group!(benches, readers, printers, envs, evaluators, cores);
criterion_main!(benches);
