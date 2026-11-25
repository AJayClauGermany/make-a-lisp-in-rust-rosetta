//! Stacks: any combination of rows wired into one interpreter.
//!
//! A *stack* is one reader, printer, environment, evaluator and core row
//! assembled into a [`Mal`] interpreter. Every row is generic code, so every
//! stack is its own monomorphised copy of the interpreter, and the compiler
//! optimises each combination as a whole: a reader can be inlined into the
//! `read-string` built-in, an environment into the evaluator's lookup loop.
//! That is what makes combinations worth measuring: two rows can do better
//! (or worse) together than their separate results predict.
//!
//! This module is the single list of rows the interpreter can be built from
//! ([`READERS`], [`PRINTERS`], [`ENVS`], [`EVALS`], [`CORES`]); the registry,
//! the `mal` binary's flags, the benchmarks and the consistency tests all
//! read it. Adding a row means adding its type to one list below.
//!
//! Choosing rows at run time goes through visitors: [`visit`] turns a
//! [`Selection`] (names) into a call of [`StackVisitor::visit`] with the
//! matching types. By default only the stacks with at most one non-`std` row
//! are compiled, so the number of copies grows with the sum of the rows. The
//! `matrix` cargo feature compiles every combination instead (the product of
//! the rows), which is slow to build and only needed to run or benchmark
//! arbitrary combinations; see the stacks page of the documentation.

use std::fmt;

use mal_traits::{
    Component, CoreNamespace, Env, Evaluator, Implementation, Printer, Reader, Row, Step,
};
use mal_types::{MalError, Value};

use crate::interpreter::Mal;
use crate::select::Selection;

/// The components a stack is made of: every component but the line editor,
/// which drives an interpreter rather than being part of one.
pub const COMPONENTS: [Component; 5] = [
    Component::Reader,
    Component::Printer,
    Component::Env,
    Component::Evaluator,
    Component::Core,
];

/// A reader row that can be part of a stack.
pub trait ReaderRow: Reader<Value = Value> + Row + Default + 'static {}
impl<T: Reader<Value = Value> + Row + Default + 'static> ReaderRow for T {}

/// A printer row that can be part of a stack. `Clone`, because the core
/// namespace keeps its own copy for `pr-str` and friends.
pub trait PrinterRow: Printer<Value = Value> + Row + Default + Clone + 'static {}
impl<T: Printer<Value = Value> + Row + Default + Clone + 'static> PrinterRow for T {}

/// An environment row that can be part of a stack.
pub trait EnvRow: Env<Value = Value> + Row + Default + 'static {}
impl<T: Env<Value = Value> + Row + Default + 'static> EnvRow for T {}

/// An evaluator row. Evaluators are generic over the environment they bind
/// symbols in, so the row is a *family* of evaluator types, one per
/// environment row; [`For`](Self::For) picks the member.
pub trait EvalRow: 'static {
    /// The row's table entry.
    const IMPLEMENTATION: &'static Implementation;
    /// The evaluator for environment row `E`.
    type For<E: EnvRow>: Evaluator<Value = Value, Env = E, Error = MalError>
        + Clone
        + Default
        + 'static;
}

/// A core-namespace row. Like evaluators, core namespaces are generic over
/// the rows they call (the environment they install into, the reader behind
/// `read-string`, the printer behind `pr-str`), so the row is a family.
pub trait CoreRow: 'static {
    /// The row's table entry.
    const IMPLEMENTATION: &'static Implementation;
    /// The namespace over environment `E`, reader `R` and printer `P`.
    type For<E: EnvRow, R: ReaderRow, P: PrinterRow>: CoreNamespace<Env = E>;
    /// A namespace whose printing built-ins use `printer`.
    fn new<E: EnvRow, R: ReaderRow, P: PrinterRow>(printer: P) -> Self::For<E, R, P>;
}

/// The `std` evaluator row, [`eval_std::StdEvaluator`], as a family.
#[derive(Debug, Clone, Copy)]
pub enum StdEvalRow {}

impl EvalRow for StdEvalRow {
    const IMPLEMENTATION: &'static Implementation = &eval_std::IMPLEMENTATION;
    type For<E: EnvRow> = eval_std::StdEvaluator<E>;
}

/// The `std` core row, [`core_std::StdCore`], as a family.
#[derive(Debug, Clone, Copy)]
pub enum StdCoreRow {}

impl CoreRow for StdCoreRow {
    const IMPLEMENTATION: &'static Implementation = &core_std::IMPLEMENTATION;
    type For<E: EnvRow, R: ReaderRow, P: PrinterRow> = core_std::StdCore<E, R, P>;
    fn new<E: EnvRow, R: ReaderRow, P: PrinterRow>(printer: P) -> Self::For<E, R, P> {
        core_std::StdCore::new(printer)
    }
}

// The `std` row of each component: the baseline every stack is compared with.
pub use printer_std::StdPrinter;
pub use reader_std::StdReader;
/// The `std` environment row over the shared value type.
pub type StdEnv = env_std::StdEnv<Value>;

/// The interpreter a stack of rows builds.
pub type StackInterpreter<R, P, E, Ev> = Mal<R, P, <Ev as EvalRow>::For<E>>;

/// Build the interpreter for one stack, with `argv` as `*ARGV*`.
///
/// The result is a concrete type, not a trait object, so a caller that
/// names the rows statically (a benchmark) gets the fully monomorphised
/// interpreter.
pub fn build<R, P, E, Ev, C>(argv: &[String]) -> StackInterpreter<R, P, E, Ev>
where
    R: ReaderRow,
    P: PrinterRow,
    E: EnvRow,
    Ev: EvalRow,
    C: CoreRow,
{
    let printer = P::default();
    let core = C::new::<E, R, P>(printer.clone());
    Mal::new(
        R::default(),
        printer,
        Ev::For::<E>::default(),
        E::default(),
        &core,
        argv,
    )
}

// ---------------------------------------------------------------------------
// The row lists. One entry per row; everything else reads these.
// ---------------------------------------------------------------------------

/// Defines, for one component, the list of its rows' table entries and a
/// function that calls a visitor with the row type of a given name.
macro_rules! rows {
    (
        $(#[$list_doc:meta])* $list:ident,
        $(#[$pick_doc:meta])* $pick:ident($visitor:ident :: $method:ident),
        $row_trait:ident,
        [$($ty:ty),+ $(,)?]
    ) => {
        $(#[$list_doc])*
        pub const $list: &[&Implementation] = &[$(<$ty as $row_trait>::IMPLEMENTATION),+];

        $(#[$pick_doc])*
        pub fn $pick<V: $visitor>(name: &str, visitor: V) -> Option<V::Output> {
            $(
                if name == <$ty as $row_trait>::IMPLEMENTATION.name {
                    return Some(visitor.$method::<$ty>());
                }
            )+
            None
        }
    };
}

/// Something to do with a reader row chosen by name; see [`pick_reader`].
pub trait PickReader {
    /// What the visitor returns.
    type Output;
    /// Called with the chosen row's type.
    fn reader<R: ReaderRow>(self) -> Self::Output;
}

/// Something to do with a printer row chosen by name; see [`pick_printer`].
pub trait PickPrinter {
    /// What the visitor returns.
    type Output;
    /// Called with the chosen row's type.
    fn printer<P: PrinterRow>(self) -> Self::Output;
}

/// Something to do with an environment row chosen by name; see [`pick_env`].
pub trait PickEnv {
    /// What the visitor returns.
    type Output;
    /// Called with the chosen row's type.
    fn env<E: EnvRow>(self) -> Self::Output;
}

/// Something to do with an evaluator row chosen by name; see [`pick_eval`].
pub trait PickEval {
    /// What the visitor returns.
    type Output;
    /// Called with the chosen row's family.
    fn eval<Ev: EvalRow>(self) -> Self::Output;
}

/// Something to do with a core row chosen by name; see [`pick_core`].
pub trait PickCore {
    /// What the visitor returns.
    type Output;
    /// Called with the chosen row's family.
    fn core<C: CoreRow>(self) -> Self::Output;
}

rows!(
    /// Every reader row, `std` first.
    READERS,
    /// Call `visitor` with the reader row called `name`, if there is one.
    pick_reader(PickReader::reader),
    Row,
    [
        reader_std::StdReader,
        reader_nom::NomReader,
        reader_winnow::WinnowReader,
        reader_chumsky::ChumskyReader,
        reader_pest::PestReader,
        reader_peg::PegReader,
        reader_lalrpop::LalrpopReader,
        reader_logos::LogosReader,
        reader_regex::RegexReader,
        reader_stacker::StackerReader,
        reader_combine::CombineReader,
        reader_lexpr::LexprReader,
    ]
);

rows!(
    /// Every printer row, `std` first.
    PRINTERS,
    /// Call `visitor` with the printer row called `name`, if there is one.
    pick_printer(PickPrinter::printer),
    Row,
    [
        printer_std::StdPrinter,
        printer_itoa::ItoaPrinter,
        printer_memchr::MemchrPrinter,
        printer_ufmt::UfmtPrinter,
        printer_pretty::PrettyPrinter,
        printer_lexical::LexicalPrinter,
    ]
);

rows!(
    /// Every environment row, `std` first.
    ENVS,
    /// Call `visitor` with the environment row called `name`, if there is one.
    pick_env(PickEnv::env),
    Row,
    [
        env_std::StdEnv<Value>,
        env_hashbrown::HashbrownEnv<Value>,
        env_indexmap::IndexMapEnv<Value>,
        env_rustc_hash::FxEnv<Value>,
        env_ahash::AHashEnv<Value>,
        env_foldhash::FoldHashEnv<Value>,
        env_imbl::ImblEnv<Value>,
        env_rpds::RpdsEnv<Value>,
        env_fnv::FnvEnv<Value>,
        env_smallvec::SmallVecEnv<Value>,
        env_lasso::LassoEnv<Value>,
        env_slotmap::SlotMapEnv<Value>,
        env_rapidhash::RapidHashEnv<Value>,
    ]
);

rows!(
    /// Every evaluator row that can be part of a stack, `std` first. The
    /// `echo` evaluator is not one: it evaluates nothing, so it has no use for
    /// the other rows (see [`crate::baseline`]).
    EVALS,
    /// Call `visitor` with the evaluator row called `name`, if there is one.
    pick_eval(PickEval::eval),
    EvalRow,
    [StdEvalRow]
);

rows!(
    /// Every core row, `std` first.
    CORES,
    /// Call `visitor` with the core row called `name`, if there is one.
    pick_core(PickCore::core),
    CoreRow,
    [StdCoreRow]
);

/// The rows of `component` that can be part of a stack.
///
/// # Panics
///
/// For [`Component::LineEditor`], which is not part of a stack.
pub fn rows(component: Component) -> &'static [&'static Implementation] {
    match component {
        Component::Reader => READERS,
        Component::Printer => PRINTERS,
        Component::Env => ENVS,
        Component::Evaluator => EVALS,
        Component::Core => CORES,
        other => panic!("{other} rows are not part of a stack"),
    }
}

// ---------------------------------------------------------------------------
// Visiting a whole stack.
// ---------------------------------------------------------------------------

/// Something to do with a whole stack chosen at run time; see [`visit`].
///
/// Rust has no generic closures, so this is the closure: a type whose
/// generic method receives the five row types.
pub trait StackVisitor {
    /// What the visitor returns.
    type Output;
    /// Called with the chosen rows.
    fn visit<R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow, C: CoreRow>(self)
    -> Self::Output;
}

/// Why a selection cannot be turned into a stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackError {
    /// A component names a row that does not exist.
    Unknown {
        /// The component.
        component: Component,
        /// The name given.
        name: String,
    },
    /// More than one row differs from `std`, and this build only compiled
    /// the stacks where at most one does.
    NeedsMatrix(String),
}

impl fmt::Display for StackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StackError::Unknown { component, name } => write!(
                f,
                "unknown {component} implementation `{name}`; `mal --list` shows what is available"
            ),
            StackError::NeedsMatrix(rows) => write!(
                f,
                "this build only combines one non-std row with std for the rest; got {rows}. \
                 Rebuild with `--features matrix` to run any combination"
            ),
        }
    }
}

impl std::error::Error for StackError {}

/// Check that every component of `selection` names an existing row.
pub fn check(selection: &Selection) -> Result<(), StackError> {
    for component in COMPONENTS {
        let name = selection.get(component);
        if !rows(component).iter().any(|imp| imp.name == name) {
            return Err(StackError::Unknown {
                component,
                name: name.to_owned(),
            });
        }
    }
    Ok(())
}

/// Whether this build can run any combination of rows (the `matrix`
/// feature), rather than only stacks with at most one non-`std` row.
pub const MATRIX: bool = cfg!(feature = "matrix");

/// The components of `selection` whose row is not `std`.
pub fn changed(selection: &Selection) -> Vec<Component> {
    COMPONENTS
        .into_iter()
        .filter(|c| selection.get(*c) != "std")
        .collect()
}

/// Call `visitor` with the rows `selection` names. The line editor and the
/// read-print flag are not part of a stack and are ignored.
pub fn visit<V: StackVisitor>(selection: &Selection, visitor: V) -> Result<V::Output, StackError> {
    check(selection)?;
    let changed = changed(selection);
    if MATRIX || changed.len() <= 1 {
        Ok(dispatch(selection, &changed, visitor))
    } else {
        Err(StackError::NeedsMatrix(
            changed
                .iter()
                .map(|c| format!("--{} {}", c.slug(), selection.get(*c)))
                .collect::<Vec<_>>()
                .join(" "),
        ))
    }
}

const CHECKED: &str = "row names are checked before dispatch";

/// Every combination: one nested dispatch per component.
#[cfg(feature = "matrix")]
fn dispatch<V: StackVisitor>(
    selection: &Selection,
    _changed: &[Component],
    visitor: V,
) -> V::Output {
    matrix::dispatch(selection, visitor)
}

/// At most one non-`std` row: dispatch on that component only, so only the
/// stacks one swap away from `std` are compiled.
#[cfg(not(feature = "matrix"))]
fn dispatch<V: StackVisitor>(
    selection: &Selection,
    changed: &[Component],
    visitor: V,
) -> V::Output {
    let Some(&component) = changed.first() else {
        return visitor.visit::<StdReader, StdPrinter, StdEnv, StdEvalRow, StdCoreRow>();
    };
    let name = selection.get(component);
    let picked = match component {
        Component::Reader => pick_reader(name, swap::Reader(visitor)),
        Component::Printer => pick_printer(name, swap::Printer(visitor)),
        Component::Env => pick_env(name, swap::Env(visitor)),
        Component::Evaluator => pick_eval(name, swap::Eval(visitor)),
        Component::Core => pick_core(name, swap::Core(visitor)),
        other => unreachable!("{other} is not part of a stack"),
    };
    picked.expect(CHECKED)
}

/// Adapters that swap one row into the `std` stack.
#[cfg(not(feature = "matrix"))]
mod swap {
    use super::{
        CoreRow, EnvRow, EvalRow, PickCore, PickEnv, PickEval, PickPrinter, PickReader, PrinterRow,
        ReaderRow, StackVisitor, StdCoreRow, StdEnv, StdEvalRow, StdPrinter, StdReader,
    };

    pub struct Reader<V>(pub V);
    pub struct Printer<V>(pub V);
    pub struct Env<V>(pub V);
    pub struct Eval<V>(pub V);
    pub struct Core<V>(pub V);

    impl<V: StackVisitor> PickReader for Reader<V> {
        type Output = V::Output;
        fn reader<R: ReaderRow>(self) -> V::Output {
            self.0
                .visit::<R, StdPrinter, StdEnv, StdEvalRow, StdCoreRow>()
        }
    }
    impl<V: StackVisitor> PickPrinter for Printer<V> {
        type Output = V::Output;
        fn printer<P: PrinterRow>(self) -> V::Output {
            self.0
                .visit::<StdReader, P, StdEnv, StdEvalRow, StdCoreRow>()
        }
    }
    impl<V: StackVisitor> PickEnv for Env<V> {
        type Output = V::Output;
        fn env<E: EnvRow>(self) -> V::Output {
            self.0
                .visit::<StdReader, StdPrinter, E, StdEvalRow, StdCoreRow>()
        }
    }
    impl<V: StackVisitor> PickEval for Eval<V> {
        type Output = V::Output;
        fn eval<Ev: EvalRow>(self) -> V::Output {
            self.0
                .visit::<StdReader, StdPrinter, StdEnv, Ev, StdCoreRow>()
        }
    }
    impl<V: StackVisitor> PickCore for Core<V> {
        type Output = V::Output;
        fn core<C: CoreRow>(self) -> V::Output {
            self.0
                .visit::<StdReader, StdPrinter, StdEnv, StdEvalRow, C>()
        }
    }
}

/// Adapters that pick every component in turn, carrying the types chosen so
/// far, so every combination of rows is compiled.
#[cfg(feature = "matrix")]
mod matrix {
    use std::marker::PhantomData;

    use mal_traits::Component;

    use super::{
        CHECKED, CoreRow, EnvRow, EvalRow, PickCore, PickEnv, PickEval, PickPrinter, PickReader,
        PrinterRow, ReaderRow, StackVisitor, pick_core, pick_env, pick_eval, pick_printer,
        pick_reader,
    };
    use crate::select::Selection;

    pub fn dispatch<V: StackVisitor>(selection: &Selection, visitor: V) -> V::Output {
        let start = Chosen::<V, ()> {
            selection,
            visitor,
            rows: PhantomData,
        };
        pick_reader(selection.get(Component::Reader), start).expect(CHECKED)
    }

    /// The visitor, the selection, and the row types `T` chosen so far, as
    /// a tuple that grows by one component per step.
    struct Chosen<'a, V, T> {
        selection: &'a Selection,
        visitor: V,
        rows: PhantomData<fn() -> T>,
    }

    impl<'a, V, T> Chosen<'a, V, T> {
        fn then<U>(self) -> Chosen<'a, V, U> {
            Chosen {
                selection: self.selection,
                visitor: self.visitor,
                rows: PhantomData,
            }
        }
    }

    impl<V: StackVisitor> PickReader for Chosen<'_, V, ()> {
        type Output = V::Output;
        fn reader<R: ReaderRow>(self) -> V::Output {
            let name = self.selection.get(Component::Printer);
            pick_printer(name, self.then::<(R,)>()).expect(CHECKED)
        }
    }
    impl<V: StackVisitor, R: ReaderRow> PickPrinter for Chosen<'_, V, (R,)> {
        type Output = V::Output;
        fn printer<P: PrinterRow>(self) -> V::Output {
            let name = self.selection.get(Component::Env);
            pick_env(name, self.then::<(R, P)>()).expect(CHECKED)
        }
    }
    impl<V: StackVisitor, R: ReaderRow, P: PrinterRow> PickEnv for Chosen<'_, V, (R, P)> {
        type Output = V::Output;
        fn env<E: EnvRow>(self) -> V::Output {
            let name = self.selection.get(Component::Evaluator);
            pick_eval(name, self.then::<(R, P, E)>()).expect(CHECKED)
        }
    }
    impl<V: StackVisitor, R: ReaderRow, P: PrinterRow, E: EnvRow> PickEval
        for Chosen<'_, V, (R, P, E)>
    {
        type Output = V::Output;
        fn eval<Ev: EvalRow>(self) -> V::Output {
            let name = self.selection.get(Component::Core);
            pick_core(name, self.then::<(R, P, E, Ev)>()).expect(CHECKED)
        }
    }
    impl<V: StackVisitor, R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow> PickCore
        for Chosen<'_, V, (R, P, E, Ev)>
    {
        type Output = V::Output;
        fn core<C: CoreRow>(self) -> V::Output {
            self.visitor.visit::<R, P, E, Ev, C>()
        }
    }
}

// ---------------------------------------------------------------------------
// Naming stacks and sets of stacks.
// ---------------------------------------------------------------------------

/// The stable label of a stack: its non-`std` rows as
/// `component=name` pairs joined by `,` in [`COMPONENTS`] order, or `std`
/// when every row is `std`. Used as the benchmark group `stack/<label>`.
pub fn label(selection: &Selection) -> String {
    let parts: Vec<String> = changed(selection)
        .into_iter()
        .map(|c| format!("{}={}", c.slug(), selection.get(c)))
        .collect();
    if parts.is_empty() {
        "std".to_owned()
    } else {
        parts.join(",")
    }
}

/// The selection a [`label`] describes.
pub fn from_label(label: &str) -> Result<Selection, String> {
    let mut selection = Selection::default();
    if label == "std" {
        return Ok(selection);
    }
    for part in label.split(',') {
        let (slug, name) = part
            .split_once('=')
            .ok_or_else(|| format!("`{part}` in stack label `{label}` is not component=name"))?;
        let component = component(slug)?;
        selection.set(component, name);
    }
    Ok(selection)
}

/// The labels of the types a visitor was called with, for code that has the
/// types but not the selection.
pub fn label_of<R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow, C: CoreRow>() -> String {
    label(&selection_of::<R, P, E, Ev, C>())
}

/// The selection naming the given row types.
pub fn selection_of<R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow, C: CoreRow>() -> Selection
{
    let mut selection = Selection::default();
    selection.set(Component::Reader, R::IMPLEMENTATION.name);
    selection.set(Component::Printer, P::IMPLEMENTATION.name);
    selection.set(Component::Env, E::IMPLEMENTATION.name);
    selection.set(Component::Evaluator, Ev::IMPLEMENTATION.name);
    selection.set(Component::Core, C::IMPLEMENTATION.name);
    selection
}

/// The highest step every row of `selection` reaches: what the stack as a
/// whole can be expected to pass. `None` if a row is unknown.
pub fn highest_step(selection: &Selection) -> Option<Step> {
    COMPONENTS
        .into_iter()
        .map(|c| {
            rows(c)
                .iter()
                .find(|imp| imp.name == selection.get(c))
                .map(|imp| imp.highest_step)
        })
        .try_fold(Step::Mal, |lowest, step| Some(lowest.min(step?)))
}

fn component(slug: &str) -> Result<Component, String> {
    component_index(slug).map(|i| COMPONENTS[i])
}

/// The position in [`COMPONENTS`] of the component called `slug`.
fn component_index(slug: &str) -> Result<usize, String> {
    COMPONENTS
        .iter()
        .position(|c| c.slug() == slug)
        .ok_or_else(|| {
            format!(
                "`{slug}` is not a stack component; use one of {}",
                COMPONENTS.map(Component::slug).join(", ")
            )
        })
}

/// A set of stacks: for each component, the rows to combine. The stacks are
/// every combination of one row per component.
///
/// Written as a spec, components are separated by whitespace or `;` and rows
/// by `,`; `*` means every row of the component that reaches `stepA_mal`,
/// and a component left out is `std`:
///
/// ```text
/// reader=nom,winnow env=*        2 readers x 13 environments
/// printer=* ; env=rustc-hash     every printer on the rustc-hash env
/// all                            every combination
/// sweep                          std, and every row swapped in alone
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSet {
    stacks: Vec<Selection>,
}

impl StackSet {
    /// The one-at-a-time sweep: the `std` stack, then every row that reaches
    /// `stepA_mal` swapped into it alone. This is what a default build can
    /// run, and the baseline [`with_singles`](Self::with_singles) adds.
    pub fn sweep() -> Self {
        let mut stacks = vec![Selection::default()];
        for component in COMPONENTS {
            for imp in rows(component) {
                if imp.name != "std" && imp.supports(Step::Mal) {
                    stacks.push(Selection::for_implementation(imp));
                }
            }
        }
        Self { stacks }
    }

    /// Parse a spec (see the type docs).
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        match spec {
            "sweep" => return Ok(Self::sweep()),
            "all" => return Self::parse("reader=* printer=* env=* eval=* core=*"),
            _ => {}
        }
        let mut choices: Vec<Vec<&'static str>> = COMPONENTS.iter().map(|_| vec!["std"]).collect();
        for part in spec.split([' ', '\t', '\n', ';']).filter(|p| !p.is_empty()) {
            let (slug, names) = part
                .split_once('=')
                .ok_or_else(|| format!("`{part}` is not component=row[,row...]"))?;
            let index = component_index(slug)?;
            let component = COMPONENTS[index];
            let available = rows(component);
            let mut chosen = Vec::new();
            for name in names.split(',').filter(|n| !n.is_empty()) {
                if name == "*" {
                    chosen.extend(
                        available
                            .iter()
                            .filter(|imp| imp.supports(Step::Mal))
                            .map(|imp| imp.name),
                    );
                    continue;
                }
                let imp = available
                    .iter()
                    .find(|imp| imp.name == name)
                    .ok_or_else(|| {
                        format!(
                            "unknown {component} row `{name}`; available: {}",
                            available
                                .iter()
                                .map(|i| i.name)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?;
                chosen.push(imp.name);
            }
            chosen.dedup();
            if chosen.is_empty() {
                return Err(format!("`{part}` names no rows"));
            }
            choices[index] = chosen;
        }
        let mut stacks = vec![Selection::default()];
        for (component, names) in COMPONENTS.iter().zip(&choices) {
            stacks = stacks
                .iter()
                .flat_map(|base| {
                    names.iter().map(move |name| {
                        let mut s = base.clone();
                        s.set(*component, name);
                        s
                    })
                })
                .collect();
        }
        let mut set = Self { stacks: Vec::new() };
        set.extend(stacks);
        Ok(set)
    }

    /// Add the `std` stack and, for every non-`std` row any stack uses, the
    /// stack with only that row swapped in. Those are what an interaction is
    /// measured against: how much faster or slower a combination is than its
    /// rows are on their own.
    #[must_use]
    pub fn with_singles(mut self) -> Self {
        let mut singles = vec![Selection::default()];
        for stack in &self.stacks {
            for component in changed(stack) {
                let mut single = Selection::default();
                single.set(component, stack.get(component));
                singles.push(single);
            }
        }
        let rest = std::mem::take(&mut self.stacks);
        self.extend(singles);
        self.extend(rest);
        self
    }

    /// Whether any stack needs the `matrix` feature (more than one non-`std`
    /// row).
    pub fn needs_matrix(&self) -> bool {
        self.stacks.iter().any(|s| changed(s).len() > 1)
    }

    /// The stacks, in a stable order with duplicates removed.
    pub fn stacks(&self) -> &[Selection] {
        &self.stacks
    }

    fn extend(&mut self, stacks: impl IntoIterator<Item = Selection>) {
        for stack in stacks {
            if !self.stacks.contains(&stack) {
                self.stacks.push(stack);
            }
        }
    }
}

/// The set named by the `MAL_STACKS` environment variable, or the sweep
/// when it is unset. Benchmarks and consistency tests use this so one
/// variable chooses the combinations everywhere.
pub fn from_env() -> Result<StackSet, String> {
    match std::env::var("MAL_STACKS") {
        Ok(spec) if !spec.trim().is_empty() => {
            StackSet::parse(&spec).map_err(|e| format!("MAL_STACKS: {e}"))
        }
        _ => Ok(StackSet::sweep()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Label;
    impl StackVisitor for Label {
        type Output = String;
        fn visit<R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow, C: CoreRow>(self) -> String {
            label_of::<R, P, E, Ev, C>()
        }
    }

    #[test]
    fn labels_round_trip() {
        for stack in StackSet::sweep().stacks() {
            let label = label(stack);
            assert_eq!(&from_label(&label).unwrap(), stack, "{label}");
            assert_eq!(visit(stack, Label).unwrap(), label);
        }
        assert_eq!(label(&Selection::default()), "std");
    }

    #[test]
    fn row_names_are_label_safe() {
        for component in COMPONENTS {
            for imp in rows(component) {
                assert!(
                    imp.name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                    "{component}/{} cannot be part of a stack label",
                    imp.name
                );
                assert_eq!(
                    imp.component, component,
                    "{} is listed under {component}",
                    imp.name
                );
            }
        }
    }

    #[test]
    fn specs_expand_to_products() {
        let set = StackSet::parse("reader=nom,winnow; env=std,rustc-hash").unwrap();
        let labels: Vec<String> = set.stacks().iter().map(label).collect();
        assert_eq!(
            labels,
            [
                "reader=nom",
                "reader=nom,env=rustc-hash",
                "reader=winnow",
                "reader=winnow,env=rustc-hash"
            ]
        );
        assert!(set.needs_matrix());
        let with_singles: Vec<String> = set.with_singles().stacks().iter().map(label).collect();
        assert_eq!(
            with_singles,
            [
                "std",
                "reader=nom",
                "env=rustc-hash",
                "reader=winnow",
                "reader=nom,env=rustc-hash",
                "reader=winnow,env=rustc-hash"
            ]
        );
        assert!(
            StackSet::parse("reader=nope")
                .unwrap_err()
                .contains("unknown reader row")
        );
        assert!(StackSet::parse("line-editor=std").is_err());
        assert!(!StackSet::sweep().needs_matrix());
    }

    #[test]
    fn star_skips_rows_below_step_a() {
        let set = StackSet::parse("reader=*").unwrap();
        assert!(
            set.stacks()
                .iter()
                .all(|s| s.get(Component::Reader) != "lexpr")
        );
        assert_eq!(
            highest_step(&Selection::for_implementation(
                &reader_lexpr::IMPLEMENTATION
            )),
            Some(Step::Repl)
        );
    }

    #[test]
    fn combinations_need_the_matrix_feature() {
        let mut selection = Selection::default();
        selection.set(Component::Reader, "nom");
        selection.set(Component::Env, "fnv");
        let result = visit(&selection, Label);
        if MATRIX {
            assert_eq!(result.unwrap(), "reader=nom,env=fnv");
        } else {
            let err = result.unwrap_err().to_string();
            assert!(err.contains("--reader nom --env fnv"), "{err}");
            assert!(err.contains("--features matrix"), "{err}");
        }
    }
}
