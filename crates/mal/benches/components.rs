//! The component benchmarks.
//!
//! One function per component; each benchmarks every row of that component
//! listed in `mal::stack` on the shared workloads in `mal_bench::corpus`,
//! with `std` rows for anything else it needs. A new row is benchmarked as
//! soon as it is listed there. Then run `cargo xtask bench` to record a run
//! and regenerate the results page. Whole stacks are in `benches/stacks.rs`.
//!
//! Line editors are not timed; see `mal_bench` for why.

// `criterion_group!` expands to an undocumented public function.
#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use mal::baseline;
use mal::stack::{
    self, CoreRow, EnvRow, EvalRow, PickCore, PickEnv, PickEval, PickPrinter, PickReader,
    PrinterRow, ReaderRow, StdCoreRow, StdEnv, StdPrinter, StdReader,
};
use mal_bench::corpus::{self, Form};
use mal_traits::{CoreNamespace, Interpreter, ReadOutcome, Reader, Step};
use mal_types::Value;

use eval_std::StdEvaluator;

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
/// namespace installed: what the environment rows run programs in.
fn setup<E: EnvRow>() -> (StdEvaluator<E>, E) {
    let env = E::default();
    StdCoreRow::new::<E, StdReader, StdPrinter>(StdPrinter).install(&env);
    (StdEvaluator::new(), env)
}

struct BenchReader<'a>(&'a mut Criterion);

impl PickReader for BenchReader<'_> {
    type Output = ();
    fn reader<R: ReaderRow>(self) {
        mal_bench::reader(self.0, R::IMPLEMENTATION, R::default);
    }
}

fn readers(c: &mut Criterion) {
    for imp in stack::READERS {
        // The corpus is mal syntax; a row that cannot read mal (lexpr) would
        // only time its error path.
        if imp.supports(Step::ReadPrint) {
            stack::pick_reader(imp.name, BenchReader(c));
        }
    }
}

struct BenchPrinter<'a> {
    c: &'a mut Criterion,
    values: &'a [(&'static str, Value)],
}

impl PickPrinter for BenchPrinter<'_> {
    type Output = ();
    fn printer<P: PrinterRow>(self) {
        mal_bench::printer(self.c, P::IMPLEMENTATION, &P::default(), self.values);
    }
}

fn printers(c: &mut Criterion) {
    let values = parse(corpus::FORMS);
    for imp in stack::PRINTERS {
        stack::pick_printer(imp.name, BenchPrinter { c, values: &values });
    }
}

struct BenchEnv<'a> {
    c: &'a mut Criterion,
    programs: &'a [(&'static str, Value)],
}

impl PickEnv for BenchEnv<'_> {
    type Output = ();
    fn env<E: EnvRow>(self) {
        #[allow(clippy::cast_possible_wrap)] // Indices are below ENV_BINDINGS.
        let value = |i: usize| Value::Int(i as i64);
        // Each row twice: the trait methods on their own, then the `std`
        // evaluator running whole programs on that environment.
        mal_bench::env(self.c, E::IMPLEMENTATION, E::default, value);
        mal_bench::evaluator(self.c, E::IMPLEMENTATION, setup::<E>, self.programs);
    }
}

fn envs(c: &mut Criterion) {
    let programs = parse(corpus::PROGRAMS);
    for imp in stack::ENVS {
        stack::pick_env(
            imp.name,
            BenchEnv {
                c,
                programs: &programs,
            },
        );
    }
}

struct BenchEval<'a> {
    c: &'a mut Criterion,
    programs: &'a [(&'static str, Value)],
}

impl PickEval for BenchEval<'_> {
    type Output = ();
    fn eval<Ev: EvalRow>(self) {
        // The programs on the `std` environment with the `std` core.
        mal_bench::evaluator(
            self.c,
            Ev::IMPLEMENTATION,
            || {
                let env = StdEnv::default();
                StdCoreRow::new::<StdEnv, StdReader, StdPrinter>(StdPrinter).install(&env);
                (Ev::For::<StdEnv>::default(), env)
            },
            self.programs,
        );
        // The whole stack with this evaluator, read-eval-print, as the REPL
        // runs it, with the corpus's free symbols bound so every form
        // evaluates.
        mal_bench::interpreter(self.c, Ev::IMPLEMENTATION, || {
            let mut mal = stack::build::<StdReader, StdPrinter, StdEnv, Ev, StdCoreRow>(&[]);
            for binding in corpus::BINDINGS {
                if let Err(e) = mal.rep(binding) {
                    panic!("corpus binding {binding} fails: {e}");
                }
            }
            mal
        });
    }
}

fn evaluators(c: &mut Criterion) {
    mal_bench::interpreter(c, &baseline::ECHO, || baseline::EchoInterpreter);
    let programs = parse(corpus::PROGRAMS);
    for imp in stack::EVALS {
        stack::pick_eval(
            imp.name,
            BenchEval {
                c,
                programs: &programs,
            },
        );
    }
}

struct BenchCore<'a>(&'a mut Criterion);

impl PickCore for BenchCore<'_> {
    type Output = ();
    fn core<C: CoreRow>(self) {
        mal_bench::core_namespace(
            self.0,
            C::IMPLEMENTATION,
            || C::new::<StdEnv, StdReader, StdPrinter>(StdPrinter),
            StdEnv::new,
        );
    }
}

fn cores(c: &mut Criterion) {
    for imp in stack::CORES {
        stack::pick_core(imp.name, BenchCore(c));
    }
}

criterion_group!(benches, readers, printers, envs, evaluators, cores);
criterion_main!(benches);
