//! Component traits for the Make-a-Lisp rosetta comparison.
//!
//! [Make-a-Lisp][mal] (mal) splits a Lisp interpreter into a handful of
//! well-defined components (reader, printer, environment, evaluator, core
//! namespace, REPL) and a numbered ladder of steps that each add a feature.
//! That makes it a good yardstick for comparing Rust crates: every component
//! can be implemented several times with different crates, and the shared
//! interface in this crate is what lets the implementations be swapped and
//! measured against each other.
//!
//! This crate contains **no implementations and no dependencies**. It
//! defines:
//!
//! * the vocabulary of the comparison: [`Component`] and [`Step`];
//! * one trait per component: [`Reader`], [`Printer`], [`Env`],
//!   [`Evaluator`], [`CoreNamespace`], [`LineEditor`] and [`Interpreter`];
//! * [`Implementation`] metadata so a binary or a report generator can list
//!   what has been implemented, with which crates, and how far up the step
//!   ladder it gets;
//! * [`run_repl`], the one piece of shared behaviour: the read-eval-print
//!   loop driver, generic over a line editor and an interpreter.
//!
//! Comparison implementations live in their own crates under
//! `comparisons/<component>/<crate>/` in the repository and depend only on
//! this crate.
//!
//! [mal]: https://github.com/kanaka/mal

mod component;
mod core;
mod env;
mod eval;
mod printer;
mod reader;
mod registry;
mod repl;

pub use component::{Component, Step};
pub use core::CoreNamespace;
pub use env::Env;
pub use eval::Evaluator;
pub use printer::Printer;
pub use reader::{ReadOutcome, Reader};
pub use registry::{CrateRef, Implementation};
pub use repl::{Interpreter, LineEditor, ReplOutcome, run_repl};
