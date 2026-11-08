//! The `mal` binary: a stepA Make-a-Lisp interpreter assembled from the
//! comparison rows, plus a listing of everything available for comparison.
//!
//! By default every component is its `std` baseline row. One row at a time
//! can be swapped in with `--<component> <name>` (see `mal --list`), which
//! is how the conformance tests run the mal test suite against each row.
//!
//! Argument parsing is done by hand so the binary itself contributes nothing
//! to the size and build-time columns of the comparison tables.

use std::io;
use std::process::ExitCode;

use line_editor_reedline::ReedlineEditor;
use line_editor_rustyline::RustylineEditor;
use mal::baseline::StdinLineEditor;
use mal::registry;
use mal::select::{self, Selection};
use mal_traits::{Component, Interpreter, LineEditor, ReplOutcome, run_repl};

const USAGE: &str = "\
Usage: mal [ROWS] [FILE [ARGS...]]
       mal --list

With no FILE, start the REPL. With FILE, run it with ARGS as *ARGV* and exit.

Rows (default std; at most one of reader/printer/env/eval/core may differ):
      --reader NAME       --printer NAME      --env NAME
      --eval NAME         --core NAME         --line-editor NAME
      --read-print        step1 mode: read and print without evaluating

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
        ["-l" | "--list"] => {
            print!("{}", registry::render_table());
            return ExitCode::SUCCESS;
        }
        ["-h" | "--help"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        ["-V" | "--version"] => {
            println!("mal {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    match parse(&args) {
        Ok((selection, script)) => run(&selection, script),
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The row flags, then an optional script and its arguments.
fn parse(args: &[String]) -> Result<(Selection, &[String]), String> {
    let mut selection = Selection::default();
    let mut rest = args;
    while let [flag, tail @ ..] = rest {
        let Some(slug) = flag.strip_prefix("--") else {
            break;
        };
        if slug == "read-print" {
            selection.set_read_print(true);
            rest = tail;
            continue;
        }
        let component = Component::ALL
            .into_iter()
            .find(|c| c.slug() == slug)
            .ok_or_else(|| format!("unrecognised option `{flag}`"))?;
        let [name, tail @ ..] = tail else {
            return Err(format!("`{flag}` needs an implementation name"));
        };
        selection.set(component, name);
        rest = tail;
    }
    Ok((selection, rest))
}

fn run(selection: &Selection, script: &[String]) -> ExitCode {
    let argv = script.get(1..).unwrap_or_default();
    let mut interpreter = match select::build(selection, argv) {
        Ok(interpreter) => interpreter,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(file) = script.first() {
        return run_file(interpreter.as_mut(), file);
    }
    match selection.get(Component::LineEditor) {
        "std" => repl(&mut StdinLineEditor::default(), interpreter.as_mut()),
        "rustyline" => match RustylineEditor::new() {
            Ok(mut editor) => repl(&mut editor, interpreter.as_mut()),
            Err(e) => {
                eprintln!("error: cannot start rustyline: {e}");
                ExitCode::FAILURE
            }
        },
        "reedline" => repl(&mut ReedlineEditor::new(), interpreter.as_mut()),
        other => {
            eprintln!(
                "error: unknown line-editor implementation `{other}`; `mal --list` shows what is available"
            );
            ExitCode::from(2)
        }
    }
}

fn run_file(interpreter: &mut (impl Interpreter + ?Sized), file: &str) -> ExitCode {
    let escaped = file.replace('\\', "\\\\").replace('"', "\\\"");
    match interpreter.rep(&format!("(load-file \"{escaped}\")")) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn repl<L: LineEditor>(editor: &mut L, interpreter: &mut (impl Interpreter + ?Sized)) -> ExitCode {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match run_repl(editor, interpreter, &mut out) {
        Ok(ReplOutcome::EndOfInput) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
