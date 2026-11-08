//! The `mal` binary: an interactive REPL over whichever component
//! implementations are wired in, plus a listing of everything available for
//! comparison.
//!
//! Argument parsing is done by hand so the binary itself contributes nothing
//! to the size and build-time columns of the comparison tables.

use std::io;
use std::process::ExitCode;

use mal::{baseline, registry};

const USAGE: &str = "\
Usage: mal [OPTIONS]

Options:
  -l, --list      List the implementations available for comparison
  -h, --help      Print this help
  -V, --version   Print the version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => repl(),
        ["-l" | "--list"] => {
            print!("{}", registry::render_table());
            ExitCode::SUCCESS
        }
        ["-h" | "--help"] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["-V" | "--version"] => {
            println!("mal {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        other => {
            eprintln!(
                "error: unrecognised arguments: {}\n\n{USAGE}",
                other.join(" ")
            );
            ExitCode::from(2)
        }
    }
}

fn repl() -> ExitCode {
    let mut editor = baseline::StdinLineEditor::default();
    let mut interpreter = baseline::EchoInterpreter;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match mal_traits::run_repl(&mut editor, &mut interpreter, &mut out) {
        Ok(mal_traits::ReplOutcome::EndOfInput) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
