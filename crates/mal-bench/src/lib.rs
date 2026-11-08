//! Benchmark harness for the Make-a-Lisp rosetta comparison.
//!
//! One function per component trait, each generic over the implementation,
//! so that benchmarking a new row is a single line in
//! `crates/mal/benches/components.rs`:
//!
//! ```ignore
//! mal_bench::reader(c, &reader_nom::IMPLEMENTATION, reader_nom::NomReader::default);
//! ```
//!
//! Every function records its measurements under the Criterion group
//! `<component>/<implementation>` (see [`group_id`]) with one benchmark per
//! workload from [`corpus`], so the report generator in `xtask` can join the
//! numbers back to the [`Implementation`] metadata without any extra
//! bookkeeping. The corpus is fixed; the same inputs are used for every row
//! so the rows are comparable with each other and across runs.
//!
//! Line editors have no time benchmark: they are measured on binary size,
//! and their run time is dominated by the terminal.

pub mod corpus;

use std::convert::Infallible;
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use mal_traits::{
    CoreNamespace, Env, Evaluator, Implementation, Interpreter, LineEditor, Printer, Reader,
    run_repl,
};

/// The Criterion group every benchmark of `imp` is recorded under:
/// `<component slug>/<implementation name>`, e.g. `reader/nom`.
pub fn group_id(imp: &Implementation) -> String {
    format!("{}/{}", imp.component, imp.name)
}

/// Benchmark a [`Reader`]: `read_str` on every form in [`corpus::FORMS`].
///
/// `new` constructs a fresh reader; it is called once per workload, outside
/// the timed region, so readers may hold reusable buffers.
pub fn reader<R, F>(c: &mut Criterion, imp: &Implementation, mut new: F)
where
    R: Reader,
    F: FnMut() -> R,
{
    let mut group = c.benchmark_group(group_id(imp));
    for form in corpus::FORMS {
        group.throughput(Throughput::Bytes(form.source.len() as u64));
        group.bench_function(BenchmarkId::new("read_str", form.name), |b| {
            let mut reader = new();
            b.iter(|| reader.read_str(black_box(form.source)));
        });
    }
    group.finish();
}

/// Benchmark a [`Printer`]: `pr_str` of each named value, readably and raw.
///
/// Values are supplied by the caller because their type belongs to the
/// implementation; typically they are the [`corpus::FORMS`] parsed with the
/// matching reader.
pub fn printer<P>(c: &mut Criterion, imp: &Implementation, printer: &P, values: &[(&str, P::Value)])
where
    P: Printer,
{
    let mut group = c.benchmark_group(group_id(imp));
    for (name, value) in values {
        group.bench_function(BenchmarkId::new("pr_str-readably", name), |b| {
            let mut out = String::new();
            b.iter(|| {
                out.clear();
                printer.pr_str_into(black_box(value), true, &mut out);
            });
        });
        group.bench_function(BenchmarkId::new("pr_str-raw", name), |b| {
            let mut out = String::new();
            b.iter(|| {
                out.clear();
                printer.pr_str_into(black_box(value), false, &mut out);
            });
        });
    }
    group.finish();
}

/// Number of bindings used by the [`env()`] workloads.
pub const ENV_BINDINGS: usize = 64;
/// Number of nested scopes used by the [`env()`] outer-lookup workloads.
pub const ENV_DEPTH: usize = 8;

/// Benchmark an [`Env`]: binding, local lookup, lookup through outer scopes
/// and a lookup that misses every scope.
///
/// `new` constructs an empty top-level environment and `value` produces the
/// value to bind to the `i`th symbol.
pub fn env<E, N, V>(c: &mut Criterion, imp: &Implementation, mut new: N, mut value: V)
where
    E: Env,
    N: FnMut() -> E,
    V: FnMut(usize) -> E::Value,
{
    let symbols = corpus::symbols(ENV_BINDINGS);
    let mut group = c.benchmark_group(group_id(imp));

    group.bench_function(BenchmarkId::new("set", ENV_BINDINGS), |b| {
        b.iter_batched(
            &mut new,
            |env| {
                for (i, symbol) in symbols.iter().enumerate() {
                    env.set(symbol, value(i));
                }
                env
            },
            BatchSize::SmallInput,
        );
    });

    let populated = new();
    for (i, symbol) in symbols.iter().enumerate() {
        populated.set(symbol, value(i));
    }
    group.bench_function(BenchmarkId::new("get-local", ENV_BINDINGS), |b| {
        b.iter(|| {
            for symbol in &symbols {
                black_box(populated.get(black_box(symbol)));
            }
        });
    });

    let mut inner = populated.clone();
    for _ in 0..ENV_DEPTH {
        inner = inner.child();
    }
    group.bench_function(BenchmarkId::new("get-outer", ENV_DEPTH), |b| {
        b.iter(|| {
            for symbol in &symbols {
                black_box(inner.get(black_box(symbol)));
            }
        });
    });
    group.bench_function(BenchmarkId::new("get-miss", ENV_DEPTH), |b| {
        b.iter(|| black_box(inner.get(black_box("unbound-symbol"))));
    });
    group.bench_function("child", |b| {
        b.iter(|| black_box(populated.child()));
    });

    group.finish();
}

/// Benchmark an [`Evaluator`]: `eval` of each named form in a fresh
/// evaluator and top-level environment.
///
/// `setup` builds the evaluator and the environment the forms run in (with
/// the core namespace installed, if the forms need it); it runs once per
/// workload, outside the timed region. The form is cloned outside the timed
/// region too, since `eval` takes it by value.
pub fn evaluator<Ev, S>(
    c: &mut Criterion,
    imp: &Implementation,
    mut setup: S,
    forms: &[(&str, Ev::Value)],
) where
    Ev: Evaluator,
    Ev::Value: Clone,
    S: FnMut() -> (Ev, Ev::Env),
{
    let mut group = c.benchmark_group(group_id(imp));
    for (name, form) in forms {
        group.bench_function(BenchmarkId::new("eval", name), |b| {
            let (mut evaluator, env) = setup();
            b.iter_batched(
                || form.clone(),
                |ast| evaluator.eval(ast, &env),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark a [`CoreNamespace`]: installing every built-in into a fresh
/// environment.
pub fn core_namespace<N, F, G>(
    c: &mut Criterion,
    imp: &Implementation,
    mut new: F,
    mut fresh_env: G,
) where
    N: CoreNamespace,
    F: FnMut() -> N,
    G: FnMut() -> N::Env,
{
    let mut group = c.benchmark_group(group_id(imp));
    let namespace = new();
    group.throughput(Throughput::Elements(namespace.symbols().len() as u64));
    group.bench_function("install", |b| {
        b.iter_batched(
            &mut fresh_env,
            |env| {
                namespace.install(&env);
                env
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// Benchmark an [`Interpreter`]: `rep` on every form in [`corpus::FORMS`],
/// and one whole [`corpus::session`] driven through [`run_repl`] with an
/// in-memory line editor, which is what the mal test runner does.
pub fn interpreter<I, F>(c: &mut Criterion, imp: &Implementation, mut new: F)
where
    I: Interpreter,
    F: FnMut() -> I,
{
    let mut group = c.benchmark_group(group_id(imp));
    for form in corpus::FORMS {
        group.throughput(Throughput::Bytes(form.source.len() as u64));
        group.bench_function(BenchmarkId::new("rep", form.name), |b| {
            let mut interpreter = new();
            b.iter(|| interpreter.rep(black_box(form.source)));
        });
    }

    group.throughput(Throughput::Bytes(corpus::session_bytes()));
    group.bench_function("repl-session", |b| {
        let mut interpreter = new();
        let mut out = Vec::new();
        b.iter(|| {
            out.clear();
            let mut editor = ScriptedLineEditor::new(corpus::session());
            run_repl(&mut editor, &mut interpreter, &mut out)
        });
    });
    group.finish();
}

/// A [`LineEditor`] that replays a fixed list of lines and then reports end
/// of input. Lets [`run_repl`] be driven without a terminal.
#[derive(Debug, Clone)]
pub struct ScriptedLineEditor {
    lines: std::vec::IntoIter<&'static str>,
}

impl ScriptedLineEditor {
    /// Replay `lines` in order.
    pub fn new(lines: Vec<&'static str>) -> Self {
        Self {
            lines: lines.into_iter(),
        }
    }
}

impl LineEditor for ScriptedLineEditor {
    type Error = Infallible;

    fn read_line(&mut self, _prompt: &str) -> Result<Option<String>, Infallible> {
        Ok(self.lines.next().map(str::to_owned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mal_traits::{Component, Step};

    #[test]
    fn group_id_is_component_slash_name() {
        let imp = Implementation {
            name: "nom",
            component: Component::Reader,
            crates: &[],
            highest_step: Step::ReadPrint,
            notes: "",
        };
        assert_eq!(group_id(&imp), "reader/nom");
    }

    #[test]
    fn scripted_editor_replays_then_ends() {
        let mut editor = ScriptedLineEditor::new(vec!["a", "b"]);
        assert_eq!(editor.read_line("> ").unwrap().as_deref(), Some("a"));
        assert_eq!(editor.read_line("> ").unwrap().as_deref(), Some("b"));
        assert_eq!(editor.read_line("> ").unwrap(), None);
    }
}
