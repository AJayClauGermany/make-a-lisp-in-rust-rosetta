//! Every stack prints exactly what the `std` stack prints.
//!
//! Rows are only interchangeable if any combination of them behaves the
//! same: a reader feeding a printer it was not written against, a core
//! namespace calling back into a different reader for `read-string`, an
//! evaluator binding parameters in a different environment. This runs one
//! transcript (definitions, the benchmark session and programs, errors, and
//! the built-ins that cross components) through each stack in process and
//! compares it, line for line, with the `std` stack's.
//!
//! Which stacks: `MAL_STACKS` if set (see `mal::stack::StackSet`), else
//! every combination in a `--features matrix` build, else the sweep (`std`
//! and every row swapped in alone). Stacks containing a row that does not
//! reach `stepA_mal` are skipped, since the transcript needs all of mal.
//! The full mal test suite runs against stacks in `tests/conformance.rs`.

use mal::stack::{self, CoreRow, EnvRow, EvalRow, PrinterRow, ReaderRow, StackSet, StackVisitor};
use mal_bench::corpus;
use mal_traits::{Interpreter, Step};

/// Forms whose results depend on more than one row at once.
const CROSSING: &[&str] = &[
    // core -> reader -> printer
    r#"(pr-str (read-string "(a [b {:c \"d\\n\"}] ~e ~@f @g ^{:m 1} h)"))"#,
    r#"(str "x" 1 :k nil (list 1 "y") [2] {"z" 3})"#,
    // core -> evaluator -> env
    "(map (fn* [x] (* x x)) [1 2 3])",
    "(apply + 1 2 (list 3 4))",
    "(let* [a (atom 1)] (do (swap! a (fn* [x y] (+ x y)) 41) @a))",
    // macros and quasiquote through the evaluator and env
    "(defmacro! unless (fn* [p a b] `(if ~p ~b ~a)))",
    "(unless false 7 8)",
    // errors, rendered with the printer
    r#"(try* (throw {:k [1 "two"]}) (catch* e (pr-str e)))"#,
    "(throw (list 1 \"x\"))",
    "undefined-symbol",
    "(1 2",
    "\"unterminated",
    "{:a}",
    // deeper scopes than the programs use
    "(let* [a 1] (let* [b 2] (let* [c 3] (let* [d 4] (+ a b c d)))))",
    "(def! deep (fn* [n] (if (= n 0) 0 (+ 1 (deep (- n 1))))))",
    "(deep 200)",
    "(meta (with-meta [1] {:m true}))",
];

fn transcript_lines() -> Vec<&'static str> {
    let mut lines: Vec<&'static str> = corpus::BINDINGS.to_vec();
    lines.extend(corpus::session());
    lines.extend(corpus::PROGRAMS.iter().map(|p| p.source));
    lines.extend(CROSSING);
    lines
}

struct Transcript;

impl StackVisitor for Transcript {
    type Output = Vec<String>;

    fn visit<R: ReaderRow, P: PrinterRow, E: EnvRow, Ev: EvalRow, C: CoreRow>(self) -> Vec<String> {
        let mut mal = stack::build::<R, P, E, Ev, C>(&[]);
        transcript_lines()
            .into_iter()
            .map(|line| match mal.rep(line) {
                Ok(Some(out)) => out,
                Ok(None) => String::new(),
                Err(e) => format!("Error: {e}"),
            })
            .collect()
    }
}

fn selected() -> StackSet {
    match std::env::var("MAL_STACKS") {
        Ok(spec) if !spec.trim().is_empty() => {
            StackSet::parse(&spec).unwrap_or_else(|e| panic!("MAL_STACKS: {e}"))
        }
        _ if stack::MATRIX => StackSet::parse("all").expect("`all` parses"),
        _ => StackSet::sweep(),
    }
}

/// Errors from the reader carry each row's own wording, which the mal
/// suite only checks loosely (`.*(EOF|end of input|unbalanced).*`); compare
/// those by kind, everything else exactly.
fn normalise(line: &str, out: &str) -> String {
    let reader_error = matches!(line, "(1 2" | "\"unterminated" | "{:a}");
    if reader_error && out.starts_with("Error: ") {
        "Error: <reader>".to_owned()
    } else {
        out.to_owned()
    }
}

#[test]
fn every_stack_matches_std() {
    let set = selected();
    assert!(
        !set.needs_matrix() || stack::MATRIX,
        "MAL_STACKS names combinations; run with `--features matrix`"
    );
    let lines = transcript_lines();
    let expected = stack::visit(&mal::select::Selection::default(), Transcript).unwrap();
    for (line, out) in lines.iter().zip(&expected) {
        assert!(
            !out.starts_with("Error:") || CROSSING.contains(line),
            "the std stack fails on {line}: {out}"
        );
    }
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for selection in set.stacks() {
        if stack::highest_step(selection) != Some(Step::Mal) {
            continue;
        }
        let got = stack::visit(selection, Transcript).unwrap();
        checked += 1;
        for ((line, want), have) in lines.iter().zip(&expected).zip(&got) {
            if normalise(line, want) != normalise(line, have) {
                mismatches.push(format!(
                    "{}: {line}\n    std:   {want}\n    stack: {have}",
                    stack::label(selection)
                ));
            }
        }
    }
    eprintln!("checked {checked} stacks against std");
    assert!(checked > 0, "no stack reaches stepA_mal");
    assert!(
        mismatches.is_empty(),
        "{} differences:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
