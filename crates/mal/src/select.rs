//! Choosing rows at run time: `mal --reader nom` runs the full interpreter
//! with the `nom` reader row and `std` everywhere else.
//!
//! The names are turned into types by [`crate::stack`], which also decides
//! which combinations this build can run: by default, stacks with at most
//! one non-`std` row (each row measured against the baseline with
//! everything else held fixed); with the `matrix` feature, any combination.

use mal_traits::{Component, Implementation, Interpreter};

use crate::baseline::EchoInterpreter;
use crate::interpreter::{ReadPrint, ReplError};
use crate::stack::{self, CoreRow, EnvRow, EvalRow, PrinterRow, ReaderRow, StackVisitor};

/// An interpreter chosen at run time.
pub type DynInterpreter = Box<dyn Interpreter<Error = ReplError>>;

/// Which row to use for each component, by implementation name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    names: [String; Component::ALL.len()],
    read_print: bool,
}

impl Default for Selection {
    /// `std` for every component.
    fn default() -> Self {
        Self {
            names: Component::ALL.map(|_| String::from("std")),
            read_print: false,
        }
    }
}

impl Selection {
    /// The selection that runs `imp` with `std` rows for everything else.
    pub fn for_implementation(imp: &Implementation) -> Self {
        let mut selection = Self::default();
        selection.set(imp.component, imp.name);
        selection
    }

    /// Use the row called `name` for `component`.
    pub fn set(&mut self, component: Component, name: &str) {
        name.clone_into(&mut self.names[index(component)]);
    }

    /// The row chosen for `component`.
    pub fn get(&self, component: Component) -> &str {
        &self.names[index(component)]
    }

    /// Run as `step1_read_print`: read and print, without evaluating. The
    /// step1 tests check the reader and printer alone, so this composes
    /// with any reader or printer row but not with another evaluator.
    pub fn set_read_print(&mut self, read_print: bool) {
        self.read_print = read_print;
    }

    /// Whether [`set_read_print`](Self::set_read_print) is on.
    pub fn read_print(&self) -> bool {
        self.read_print
    }

    /// The command-line flags that reproduce this selection, e.g.
    /// `["--reader", "nom"]`. `std` rows need no flag.
    pub fn to_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Component::ALL
            .iter()
            .filter(|c| self.get(**c) != "std")
            .flat_map(|c| [format!("--{}", c.slug()), self.get(*c).to_owned()])
            .collect();
        if self.read_print {
            args.push("--read-print".to_owned());
        }
        args
    }
}

fn index(component: Component) -> usize {
    Component::ALL
        .iter()
        .position(|c| *c == component)
        .expect("Component::ALL lists every component")
}

/// Build the interpreter for `selection`, with `argv` as `*ARGV*`. The line
/// editor is not part of the interpreter; see the binary.
pub fn build(selection: &Selection, argv: &[String]) -> Result<DynInterpreter, String> {
    let evaluator = selection.get(Component::Evaluator);
    if evaluator == "echo" {
        let others: Vec<String> = stack::changed(selection)
            .into_iter()
            .filter(|c| *c != Component::Evaluator)
            .map(|c| format!("--{} {}", c.slug(), selection.get(c)))
            .collect();
        if !others.is_empty() || selection.read_print {
            return Err(format!(
                "the echo evaluator reads, evaluates and prints nothing, so it takes no other rows; got {}",
                others.join(" ")
                    + if selection.read_print {
                        " --read-print"
                    } else {
                        ""
                    }
            ));
        }
        return Ok(Box::new(Echo));
    }
    if selection.read_print && evaluator != "std" {
        return Err(format!(
            "--read-print replaces the evaluator, so it cannot be combined with --eval {evaluator}"
        ));
    }
    stack::visit(
        selection,
        Build {
            argv,
            read_print: selection.read_print,
        },
    )
    .map_err(|e| e.to_string())
}

/// Builds the interpreter for the rows it is visited with.
struct Build<'a> {
    argv: &'a [String],
    read_print: bool,
}

impl StackVisitor for Build<'_> {
    type Output = DynInterpreter;

    fn visit<R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow, C: CoreRow>(
        self,
    ) -> DynInterpreter {
        if self.read_print {
            Box::new(ReadPrint::new(R::default(), P::default()))
        } else {
            Box::new(stack::build::<R, P, E, Ev, C>(self.argv))
        }
    }
}

/// [`EchoInterpreter`] with the error type the other interpreters share.
struct Echo;

impl Interpreter for Echo {
    type Error = ReplError;

    fn rep(&mut self, line: &str) -> Result<Option<String>, ReplError> {
        match EchoInterpreter.rep(line) {
            Ok(printed) => Ok(printed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;

    #[test]
    fn every_registered_row_can_be_built() {
        for imp in registry::all() {
            let selection = Selection::for_implementation(imp);
            if imp.component == Component::LineEditor {
                continue;
            }
            assert!(
                build(&selection, &[]).is_ok(),
                "{}/{} is registered but cannot be selected",
                imp.component,
                imp.name
            );
        }
    }

    #[test]
    fn combinations_need_the_matrix_feature() {
        let mut selection = Selection::default();
        selection.set(Component::Reader, "nom");
        selection.set(Component::Env, "fnv");
        assert_eq!(selection.to_args(), ["--reader", "nom", "--env", "fnv"]);
        let built = build(&selection, &[]);
        assert_eq!(built.is_ok(), stack::MATRIX);
        match built {
            Ok(mut mal) => assert_eq!(mal.rep("(+ 1 2)").unwrap().as_deref(), Some("3")),
            Err(err) => assert!(err.contains("--reader nom --env fnv"), "{err}"),
        }
    }

    #[test]
    fn echo_takes_no_other_rows() {
        let mut selection = Selection::default();
        selection.set(Component::Evaluator, "echo");
        assert!(build(&selection, &[]).is_ok());
        selection.set(Component::Reader, "nom");
        let err = build(&selection, &[]).err().unwrap();
        assert!(err.contains("--reader nom"), "{err}");
    }

    #[test]
    fn unknown_rows_are_reported() {
        let mut selection = Selection::default();
        selection.set(Component::Printer, "nope");
        let err = build(&selection, &[]).err().unwrap();
        assert_eq!(
            err,
            "unknown printer implementation `nope`; `mal --list` shows what is available"
        );
    }
}
