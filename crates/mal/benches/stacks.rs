//! The stack benchmarks: whole interpreters built from combinations of rows.
//!
//! Which stacks are measured is chosen at run time by `MAL_STACKS` (see
//! `mal::stack::StackSet` for the spec): unset, it is the sweep (the `std`
//! stack and every row swapped into it alone); otherwise the named
//! combinations, plus the single-row stacks they are made of, so the report
//! can say whether rows interact. Combinations of more than one non-`std`
//! row need the `matrix` feature:
//!
//! ```text
//! cargo bench -p mal --bench stacks
//! MAL_STACKS='reader=nom,winnow env=*' cargo bench -p mal --bench stacks --features matrix
//! ```
//!
//! `cargo xtask bench --stacks <SPEC>` does this and records the run.

// `criterion_group!` expands to an undocumented public function.
#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use mal::stack::{self, CoreRow, EnvRow, EvalRow, PrinterRow, ReaderRow, StackVisitor};
use mal_bench::corpus;
use mal_traits::Interpreter;

struct BenchStack<'a>(&'a mut Criterion);

impl StackVisitor for BenchStack<'_> {
    type Output = ();

    fn visit<R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow, C: CoreRow>(self) {
        let label = stack::label_of::<R, P, E, Ev, C>();
        mal_bench::stack(self.0, &label, || {
            let mut mal = stack::build::<R, P, E, Ev, C>(&[]);
            for binding in corpus::BINDINGS {
                if let Err(e) = mal.rep(binding) {
                    panic!("stack {label}: corpus binding {binding} fails: {e}");
                }
            }
            mal
        });
    }
}

fn stacks(c: &mut Criterion) {
    let set = match stack::from_env() {
        Ok(set) => set.with_singles(),
        Err(e) => panic!("{e}"),
    };
    assert!(
        !set.needs_matrix() || stack::MATRIX,
        "MAL_STACKS names combinations of more than one non-std row; \
         run with `--features matrix`"
    );
    for selection in set.stacks() {
        if let Err(e) = stack::visit(selection, BenchStack(c)) {
            panic!("{}: {e}", stack::label(selection));
        }
    }
}

criterion_group!(benches, stacks);
criterion_main!(benches);
