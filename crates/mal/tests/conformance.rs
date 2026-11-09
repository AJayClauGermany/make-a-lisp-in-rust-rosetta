//! Runs the mal test suite against every registered row and checks that
//! each row reaches exactly the step its `Implementation` claims.
//!
//! This is a port of the parts of mal's `runtest.py` that decide pass or
//! fail: the test-file format, the output matching and soft failures. Each
//! step file is run in a fresh `mal` process started with the row's flags
//! (`mal --reader nom`), forms are written one per line, and the output up
//! to the next prompt is the result. A step passes when it has no hard
//! failures; a row's highest step is the last of an unbroken run of passing
//! steps.
//!
//! `step0_repl.mal` tests an echo REPL, which is what a step0
//! implementation is and what no later step is; upstream only ever runs it
//! against step0. So it is run for the `echo` evaluator only, and every
//! other row starts at `step1_read_print` with step0 counted as reached.
//! Likewise `step1_read_print.mal` expects symbols to print back rather
//! than be looked up, so it runs in `--read-print` mode, which tests the
//! reader and printer rows alone; steps 2 to A run the full interpreter.
//!
//! Set `MAL_CONFORMANCE=reader/nom,env/imbl` to run only some rows, and run
//! with `--nocapture` to see the per-row report even when everything passes.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fancy_regex::Regex;
use mal::registry;
use mal::select::Selection;
use mal_traits::{Component, Implementation, Step};

/// How long one form may take before the step is abandoned.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// One form and what it must produce, as `runtest.py`'s `TestReader` reads
/// them.
#[derive(Debug)]
struct Case {
    line: usize,
    form: String,
    /// Output patterns from `;/` lines, each followed by a newline.
    out: String,
    /// The return value from `;=>`, or empty.
    ret: String,
    /// Whether a failure only counts as soft (`;>>> soft=True` was seen).
    soft: bool,
}

fn parse_cases(text: &str) -> Vec<Case> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut cases = Vec::new();
    let mut soft = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() || line.starts_with(";;") {
            continue;
        }
        if let Some(settings) = line.strip_prefix(";>>> ") {
            if settings.contains("soft=True") {
                soft = true;
            }
            continue;
        }
        assert!(
            !line.starts_with(';'),
            "unexpected comment at line {i}: {line}"
        );
        let mut case = Case {
            line: i,
            form: line.to_owned(),
            out: String::new(),
            ret: String::new(),
            soft,
        };
        while let Some(next) = lines.get(i) {
            if let Some(ret) = next.strip_prefix(";=>") {
                ret.clone_into(&mut case.ret);
                i += 1;
                break;
            } else if let Some(out) = next.strip_prefix(";/") {
                case.out.push_str(out);
                case.out.push('\n');
                i += 1;
            } else {
                break;
            }
        }
        if case.ret.is_empty() && case.out.ends_with('\n') {
            case.out.pop();
        }
        cases.push(case);
    }
    cases
}

impl Case {
    /// `runtest.py` matches against the echoed form, a newline and the
    /// output; the pty echoes the form there, so it is added back here.
    fn matches(&self, output: &str) -> bool {
        if self.ret.is_empty() && self.out.is_empty() {
            return true;
        }
        let res = format!("{}\n{}", self.form, output);
        let ret = fancy_regex::escape(&self.ret);
        [
            format!("(?s).*\n{}{ret}", self.out),
            format!("(?s).*\n.*\n{}{ret}", self.out),
        ]
        .iter()
        .any(|pattern| {
            Regex::new(pattern)
                .unwrap_or_else(|e| panic!("bad pattern at line {}: {e}", self.line))
                .is_match(&res)
                .unwrap_or(false)
        })
    }
}

/// A running `mal` process, read until its next prompt.
struct Session {
    child: Child,
    stdin: ChildStdin,
    output: Receiver<Vec<u8>>,
    buffer: String,
    prompt: Regex,
}

impl Session {
    fn start(args: &[String], dir: &Path) -> Session {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mal"))
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start mal");
        let stdin = child.stdin.take().expect("piped stdin");
        let (tx, output) = mpsc::channel();
        for mut stream in [
            Box::new(child.stdout.take().expect("piped stdout")) as Box<dyn Read + Send>,
            Box::new(child.stderr.take().expect("piped stderr")),
        ] {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut chunk = [0; 4096];
                while let Ok(n @ 1..) = stream.read(&mut chunk) {
                    if tx.send(chunk[..n].to_vec()).is_err() {
                        break;
                    }
                }
            });
        }
        Session {
            child,
            stdin,
            output,
            buffer: String::new(),
            prompt: Regex::new(r"(?:^|\n)[^\s()<>]+> $").expect("prompt pattern"),
        }
    }

    /// Everything printed up to the next prompt, without the newline before
    /// it, or `None` on timeout or exit.
    fn read_to_prompt(&mut self) -> Option<String> {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            if let Ok(Some(found)) = self.prompt.find(&self.buffer) {
                let output = self.buffer[..found.start()].to_owned();
                self.buffer.clear();
                return Some(output);
            }
            match self
                .output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(bytes) => self.buffer.push_str(&String::from_utf8_lossy(&bytes)),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    fn send(&mut self, line: &str) -> bool {
        writeln!(self.stdin, "{line}").is_ok() && self.stdin.flush().is_ok()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug, Default)]
struct StepResult {
    passed: usize,
    soft_failed: usize,
    failures: Vec<String>,
}

fn run_step(args: &[String], dir: &Path, step: Step) -> StepResult {
    let text =
        std::fs::read_to_string(dir.join(format!("{}.mal", step.name()))).expect("read step file");
    let mut result = StepResult::default();
    let mut session = Session::start(args, dir);
    if session.read_to_prompt().is_none() {
        result
            .failures
            .push(format!("no initial prompt; got {:?}", session.buffer));
        return result;
    }
    for case in parse_cases(&text) {
        let output = if session.send(&case.form) {
            session.read_to_prompt()
        } else {
            None
        };
        match output {
            Some(output) if case.matches(&output) => result.passed += 1,
            Some(_) if case.soft => result.soft_failed += 1,
            Some(output) => result.failures.push(format!(
                "line {}: {}\n      expected: {}{}\n      got:      {output:?}",
                case.line, case.form, case.out, case.ret
            )),
            None => {
                result.failures.push(format!(
                    "line {}: {} -> timed out or exited; got {:?}",
                    case.line, case.form, session.buffer
                ));
                break;
            }
        }
    }
    result
}

struct RowReport {
    imp: &'static Implementation,
    steps: Vec<(Step, StepResult)>,
}

impl RowReport {
    fn highest(&self) -> Option<Step> {
        let first = self.steps.first().map(|(step, _)| *step)?;
        let start = if first == Step::Repl {
            None
        } else {
            Some(Step::Repl)
        };
        self.steps
            .iter()
            .take_while(|(_, r)| r.failures.is_empty())
            .map(|(step, _)| *step)
            .last()
            .or(start)
    }
}

/// Every registered row, or the ones `MAL_CONFORMANCE` names.
///
/// # Panics
///
/// If the filter names a row that is not registered, so a typo cannot pass
/// by running nothing.
fn selected_rows() -> Vec<&'static Implementation> {
    let all = registry::all();
    let Ok(filter) = std::env::var("MAL_CONFORMANCE") else {
        return all;
    };
    let id = |imp: &Implementation| format!("{}/{}", imp.component, imp.name);
    let wanted: Vec<&str> = filter
        .split(',')
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    assert!(
        !wanted.is_empty(),
        "MAL_CONFORMANCE is set but names no rows"
    );
    let unknown: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|w| !all.iter().any(|imp| id(imp) == *w))
        .collect();
    assert!(
        unknown.is_empty(),
        "MAL_CONFORMANCE names unregistered rows {unknown:?}; registered rows are {:?}",
        all.iter().map(|imp| id(imp)).collect::<Vec<_>>()
    );
    all.into_iter()
        .filter(|imp| wanted.contains(&id(imp).as_str()))
        .collect()
}

#[test]
fn every_row_reaches_its_claimed_step() {
    let dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "mal", "tests"]
        .iter()
        .collect();
    let rows = selected_rows();
    let jobs: Vec<(usize, Step)> = rows
        .iter()
        .enumerate()
        .flat_map(|(row, imp)| {
            let echo = Selection::for_implementation(imp).get(Component::Evaluator) == "echo";
            Step::ALL
                .into_iter()
                .filter(move |step| echo || *step != Step::Repl)
                .map(move |step| (row, step))
        })
        .collect();
    let jobs = Arc::new(Mutex::new(jobs.into_iter()));
    let results = Arc::new(Mutex::new(Vec::new()));
    let workers = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let (jobs, results, rows, dir) = (&jobs, &results, &rows, &dir);
            scope.spawn(move || {
                loop {
                    let Some((row, step)) = jobs.lock().unwrap().next() else {
                        break;
                    };
                    let mut selection = Selection::for_implementation(rows[row]);
                    // step1 checks reading and printing alone (see the module docs).
                    selection.set_read_print(step == Step::ReadPrint);
                    let args = selection.to_args();
                    let result = run_step(&args, dir, step);
                    results.lock().unwrap().push((row, step, result));
                }
            });
        }
    });

    let mut results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    results.sort_by_key(|(row, step, _)| (*row, *step));
    let mut reports: Vec<RowReport> = rows
        .iter()
        .map(|imp| RowReport {
            imp,
            steps: Vec::new(),
        })
        .collect();
    for (row, step, result) in results {
        reports[row].steps.push((step, result));
    }

    let mut mismatches = Vec::new();
    for report in &reports {
        let imp = report.imp;
        let reached = report.highest();
        let summary: Vec<String> = report
            .steps
            .iter()
            .map(|(step, r)| {
                let mark = if r.failures.is_empty() { "ok" } else { "FAIL" };
                format!("{}:{mark}({}/{}s)", step.label(), r.passed, r.soft_failed)
            })
            .collect();
        eprintln!(
            "{:<12} {:<8} claims {:<16} reaches {:<16} {}",
            imp.component.to_string(),
            imp.name,
            imp.highest_step.name(),
            reached.map_or("nothing", Step::name),
            summary.join(" ")
        );
        if reached != Some(imp.highest_step) {
            let mut detail = format!(
                "{}/{} claims {} but reaches {}",
                imp.component,
                imp.name,
                imp.highest_step,
                reached.map_or("nothing", Step::name)
            );
            if let Some((step, r)) = report.steps.iter().find(|(_, r)| !r.failures.is_empty()) {
                let _ = write!(detail, "; first failing step {step}:");
                for failure in r.failures.iter().take(5) {
                    detail.push_str("\n    ");
                    detail.push_str(failure);
                }
            }
            mismatches.push(detail);
        }
    }
    assert!(mismatches.is_empty(), "\n{}", mismatches.join("\n"));
}

#[test]
fn reader_follows_runtest_rules() {
    let cases = parse_cases(
        ";; comment\n(+ 1 2)\n;=>3\n(prn 1)\n;/1\n;=>nil\n;>>> soft=True\n(foo)\n;/err.*\n",
    );
    assert_eq!(cases.len(), 3);
    assert_eq!(
        (cases[0].form.as_str(), cases[0].ret.as_str()),
        ("(+ 1 2)", "3")
    );
    assert!(cases[0].matches("3"));
    assert!(!cases[0].matches("4"));
    assert_eq!(cases[1].out, "1\n");
    assert!(cases[1].matches("1\nnil"));
    assert!(cases[2].soft);
    assert_eq!(cases[2].out, "err.*");
    assert!(cases[2].matches("err thing"));
    assert!(!cases[2].matches("Error: err thing"));
}
