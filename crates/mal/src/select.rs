//! Choosing rows at run time: `mal --reader nom` runs the full interpreter
//! with the `nom` reader row and `std` everywhere else.
//!
//! Every row is generic code, so every combination the binary can run is a
//! separate monomorphised copy of the interpreter. Allowing any combination
//! would multiply those copies (readers × printers × environments); allowing
//! one row to differ from `std` at a time adds them instead, which is all the
//! comparison needs: each row is measured against the baseline with
//! everything else held fixed.

use mal_traits::{Component, Env, Implementation, Interpreter, Printer, Reader};
use mal_types::Value;

use core_std::StdCore;
use env_std::StdEnv;
use eval_std::StdEvaluator;
use printer_std::StdPrinter;
use reader_std::StdReader;

use crate::baseline::EchoInterpreter;
use crate::interpreter::{Mal, ReadPrint, ReplError};

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
    let changed: Vec<Component> = Component::ALL
        .into_iter()
        .filter(|c| *c != Component::LineEditor && selection.get(*c) != "std")
        .collect();
    if selection.read_print && selection.get(Component::Evaluator) != "std" {
        return Err(format!(
            "--read-print replaces the evaluator, so it cannot be combined with --eval {}",
            selection.get(Component::Evaluator)
        ));
    }
    let component = match changed.as_slice() {
        [] => {
            return Ok(assemble(
                selection,
                StdReader,
                StdPrinter,
                StdEnv::new(),
                argv,
            ));
        }
        [one] => *one,
        _ => {
            return Err(format!(
                "only one of reader, printer, env, eval and core may differ from std at a time; got {}",
                changed
                    .iter()
                    .map(|c| format!("--{} {}", c.slug(), selection.get(*c)))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    };
    let name = selection.get(component);
    // One row swapped in, `std` for the rest.
    macro_rules! with {
        (reader: $reader:expr) => {
            Some(assemble(
                selection,
                $reader,
                StdPrinter,
                StdEnv::new(),
                argv,
            ))
        };
        (printer: $printer:expr) => {
            Some(assemble(
                selection,
                StdReader,
                $printer,
                StdEnv::new(),
                argv,
            ))
        };
        (env: $env:expr) => {
            Some(assemble(selection, StdReader, StdPrinter, $env, argv))
        };
    }
    let built = match (component, name) {
        (Component::Reader, "nom") => with!(reader: reader_nom::NomReader),
        (Component::Reader, "winnow") => with!(reader: reader_winnow::WinnowReader),
        (Component::Reader, "chumsky") => with!(reader: reader_chumsky::ChumskyReader),
        (Component::Reader, "pest") => with!(reader: reader_pest::PestReader),
        (Component::Reader, "peg") => with!(reader: reader_peg::PegReader),
        (Component::Reader, "lalrpop") => with!(reader: reader_lalrpop::LalrpopReader::default()),
        (Component::Reader, "logos") => with!(reader: reader_logos::LogosReader),
        (Component::Reader, "regex") => with!(reader: reader_regex::RegexReader),
        (Component::Printer, "itoa") => with!(printer: printer_itoa::ItoaPrinter),
        (Component::Printer, "memchr") => with!(printer: printer_memchr::MemchrPrinter),
        (Component::Printer, "ufmt") => with!(printer: printer_ufmt::UfmtPrinter),
        (Component::Env, "hashbrown") => with!(env: env_hashbrown::HashbrownEnv::new()),
        (Component::Env, "indexmap") => with!(env: env_indexmap::IndexMapEnv::new()),
        (Component::Env, "rustc-hash") => with!(env: env_rustc_hash::FxEnv::new()),
        (Component::Env, "ahash") => with!(env: env_ahash::AHashEnv::new()),
        (Component::Env, "foldhash") => with!(env: env_foldhash::FoldHashEnv::new()),
        (Component::Env, "imbl") => with!(env: env_imbl::ImblEnv::new()),
        (Component::Env, "rpds") => with!(env: env_rpds::RpdsEnv::new()),
        (Component::Evaluator, "echo") => Some(Box::new(Echo) as DynInterpreter),
        _ => None,
    };
    built.ok_or_else(|| {
        format!("unknown {component} implementation `{name}`; `mal --list` shows what is available")
    })
}

/// The interpreter over the given reader, printer and environment rows:
/// the read-print interpreter when the selection asks for it, otherwise the
/// `std` evaluator and core namespace.
fn assemble<R, P, E>(
    selection: &Selection,
    reader: R,
    printer: P,
    env: E,
    argv: &[String],
) -> DynInterpreter
where
    R: Reader<Value = Value> + Default + 'static,
    P: Printer<Value = Value> + Clone + 'static,
    E: Env<Value = Value> + 'static,
{
    if selection.read_print {
        Box::new(ReadPrint::new(reader, printer))
    } else {
        stack(reader, printer, env, argv)
    }
}

/// The `std` evaluator and core namespace over the given reader, printer and
/// environment rows.
fn stack<R, P, E>(reader: R, printer: P, env: E, argv: &[String]) -> DynInterpreter
where
    R: Reader<Value = Value> + Default + 'static,
    P: Printer<Value = Value> + Clone + 'static,
    E: Env<Value = Value> + 'static,
{
    let core = StdCore::<E, R, P>::new(printer.clone());
    Box::new(Mal::new(
        reader,
        printer,
        StdEvaluator::<E>::new(),
        env,
        &core,
        argv,
    ))
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
    fn only_one_row_may_differ_from_std() {
        let mut selection = Selection::default();
        selection.set(Component::Reader, "nom");
        selection.set(Component::Env, "im");
        let err = build(&selection, &[]).err().unwrap();
        assert!(err.contains("--reader nom --env im"), "{err}");
        assert_eq!(selection.to_args(), ["--reader", "nom", "--env", "im"]);
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
