//! Repository tasks, invoked as `cargo xtask <command>` through the alias in
//! `.cargo/config.toml`.
//!
//! * `bench`: run the Criterion benchmarks, store the results as one run
//!   under `runs/`, and regenerate the results page.
//! * `report`: regenerate `docs/src/results.md` from the newest run (or
//!   check that it is current, for CI).
//! * `compare`: print two recorded runs side by side.
//!
//! The binary and library crates of the comparison are kept dependency-free;
//! this crate is where the tooling dependencies (serde, `serde_json`) live.

mod bench;
mod report;
mod run;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Any failure: the message is printed and the process exits non-zero.
pub type Error = Box<dyn std::error::Error>;
/// Result alias for task functions.
pub type Result<T> = std::result::Result<T, Error>;

const USAGE: &str = "\
Usage: cargo xtask <COMMAND>

Commands:
  bench [--host NAME] [--no-report] [-- <criterion args>]
          Run `cargo bench -p mal`, record the results under runs/<id>/,
          and regenerate docs/src/results.md
  report [--check]
          Regenerate docs/src/results.md from the newest run under runs/.
          With --check, exit 1 instead if the page would change
  compare <RUN> <RUN>
          Print the benchmarks two runs have in common, with the change
  help    Print this help";

/// The repository root: the parent of this crate's directory.
///
/// # Panics
///
/// If the crate has been moved out of its `xtask/` directory under the
/// workspace root, which the alias in `.cargo/config.toml` relies on anyway.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("bench") => bench::run(&args[1..]),
        Some("report") => report::run(&args[1..]),
        Some("compare") => report::compare(&args[1..]),
        Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}").into()),
        None => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
