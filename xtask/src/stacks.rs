//! The stacks section of the results page: whole interpreters built from
//! combinations of rows, and whether rows interact.
//!
//! A stack's *ratio* is the geometric mean, over the workloads both were
//! measured on, of its time divided by the `std` stack's. For a stack with
//! more than one non-`std` row, the *predicted* ratio is the product of the
//! ratios of the stacks that swap in each of those rows alone: what the
//! combination would cost if the rows did not affect each other. The
//! *interaction* is measured over predicted. Below 1 the rows do better
//! together than apart (the compiler found something across them, or they
//! share work); above 1 they get in each other's way.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use mal::stack;

use crate::chart;
use crate::report::{STACK, duration};
use crate::run::{Benchmark, Run};

/// Interactions smaller than this (as a fraction) are reported as noise.
const NOISE: f64 = 0.05;

/// More stacks than this and the per-workload heatmap is left out: it would
/// be wider than the page without saying more than the table.
const MAX_CHARTED: usize = 40;

/// One measured stack, with its numbers relative to the `std` stack.
struct Row<'a> {
    label: &'a str,
    /// The non-`std` rows, as `component=name`.
    parts: Vec<String>,
    ratio: Option<f64>,
    predicted: Option<f64>,
}

/// `ln(stack / std)` per workload, for the workloads both measured.
fn log_ratios(stack: &[&Benchmark], std: &[&Benchmark]) -> BTreeMap<String, f64> {
    stack
        .iter()
        .filter_map(|b| {
            let base = std.iter().find(|x| x.workload == b.workload)?;
            (b.mean_ns > 0.0 && base.mean_ns > 0.0)
                .then(|| (b.workload.clone(), (b.mean_ns / base.mean_ns).ln()))
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)] // A handful of workloads.
fn geo_mean(logs: impl IntoIterator<Item = f64>) -> Option<f64> {
    let logs: Vec<f64> = logs.into_iter().collect();
    (!logs.is_empty()).then(|| (logs.iter().sum::<f64>() / logs.len() as f64).exp())
}

/// Render the section from the newest run that measured stacks.
pub fn render(out: &mut String, run: Option<&Run>) {
    out.push_str("## Stacks\n\n");
    out.push_str(
        "A stack is a whole interpreter built from one row of each component and\n\
         compiled as one unit, so these numbers include what the compiler made of\n\
         the combination. Each stack is timed on the evaluation programs and the\n\
         REPL session, and compared with the all-`std` stack. *Ratio* is the\n\
         geometric mean of its time over the `std` stack's; for a stack of several\n\
         non-`std` rows, *predicted* multiplies the ratios of the stacks that swap\n\
         in each row alone, and *interaction* is ratio over predicted: below 1×\n\
         the rows do better together than apart, above 1× worse. Interactions\n\
         within 5% are marked as noise. See [Stacks](stacks.md) for how to run\n\
         other combinations.\n\n",
    );
    let Some(run) = run else {
        out.push_str(
            "No stack run has been recorded yet. `cargo xtask bench --stacks sweep`\n\
             records the `std` stack and every row swapped into it alone;\n\
             `cargo xtask bench --stacks 'reader=* env=*'` records combinations.\n\n",
        );
        return;
    };
    let _ = writeln!(
        out,
        "From run [`{}`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/blob/main/runs/{}/results.json) on `{}`.\n",
        run.id, run.id, run.host.name
    );

    let mut by_label: BTreeMap<&str, Vec<&Benchmark>> = BTreeMap::new();
    for b in run.benchmarks.iter().filter(|b| b.component == STACK) {
        by_label.entry(&b.implementation).or_default().push(b);
    }
    let rows = measure(&by_label);

    if rows.len() <= MAX_CHARTED {
        let order: Vec<&str> = rows.iter().map(|r| r.label).collect();
        let measured: Vec<&Benchmark> = by_label.values().flatten().copied().collect();
        if let Some(charts) = chart::render("Stacks", &order, &measured) {
            out.push_str(&charts);
            out.push('\n');
        }
    }

    render_table(out, &rows);

    out.push_str("<details><summary>Every stack benchmark</summary>\n\n");
    out.push_str("| stack | workload | mean | std dev |\n|---|---|---:|---:|\n");
    for row in &rows {
        for b in &by_label[row.label] {
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} | {} |",
                row.label,
                b.workload,
                duration(b.mean_ns),
                duration(b.std_dev_ns)
            );
        }
    }
    out.push_str("\n</details>\n\n");
}

/// Each measured stack with its ratio and, for combinations, the ratio its
/// single-row stacks predict; fastest first.
fn measure<'a>(by_label: &BTreeMap<&'a str, Vec<&Benchmark>>) -> Vec<Row<'a>> {
    let std = by_label.get(chart::BASELINE).cloned().unwrap_or_default();
    let logs: BTreeMap<&str, BTreeMap<String, f64>> = by_label
        .iter()
        .map(|(label, benches)| (*label, log_ratios(benches, &std)))
        .collect();

    let mut rows: Vec<Row> = Vec::new();
    for label in by_label.keys() {
        let parts: Vec<String> = match stack::from_label(label) {
            Ok(selection) => stack::changed(&selection)
                .into_iter()
                .map(|c| format!("{}={}", c.slug(), selection.get(c)))
                .collect(),
            Err(_) => vec![(*label).to_owned()],
        };
        let own = &logs[label];
        let ratio = geo_mean(own.values().copied());
        let predicted = if parts.len() > 1 {
            // Per workload: the sum of the single-row log ratios, over the
            // workloads every single was measured on.
            let singles: Option<Vec<&BTreeMap<String, f64>>> =
                parts.iter().map(|p| logs.get(p.as_str())).collect();
            singles.and_then(|singles| {
                geo_mean(
                    own.keys()
                        .filter_map(|w| singles.iter().map(|s| s.get(w)).sum::<Option<f64>>()),
                )
            })
        } else {
            None
        };
        rows.push(Row {
            label,
            parts,
            ratio,
            predicted,
        });
    }
    rows.sort_by(|a, b| {
        let key = |r: &Row| r.ratio.unwrap_or(f64::INFINITY);
        key(a).total_cmp(&key(b)).then_with(|| a.label.cmp(b.label))
    });
    rows
}

/// The ratio / predicted / interaction table.
fn render_table(out: &mut String, rows: &[Row]) {
    out.push_str("| stack | ratio | predicted | interaction |\n|---|---:|---:|---|\n");
    for row in rows {
        let ratio = row
            .ratio
            .map_or_else(|| "-".to_owned(), |r| format!("{r:.3}×"));
        let (predicted, interaction) = match (row.ratio, row.predicted) {
            (Some(r), Some(p)) if p > 0.0 => {
                let i = r / p;
                let verdict = if i < 1.0 - NOISE {
                    "better together"
                } else if i > 1.0 + NOISE {
                    "worse together"
                } else {
                    "noise"
                };
                (format!("{p:.3}×"), format!("{i:.3}× ({verdict})"))
            }
            _ if row.parts.len() > 1 => ("-".to_owned(), "singles not measured".to_owned()),
            _ => ("-".to_owned(), String::new()),
        };
        let _ = writeln!(
            out,
            "| `{}` | {ratio} | {predicted} | {interaction} |",
            row.label
        );
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{Host, Toolchain};

    fn bench(label: &str, workload: &str, mean: f64) -> Benchmark {
        Benchmark {
            id: format!("stack/{label}/{workload}"),
            component: STACK.into(),
            implementation: label.into(),
            workload: workload.into(),
            mean_ns: mean,
            median_ns: mean,
            std_dev_ns: 1.0,
            throughput: None,
        }
    }

    fn run(benchmarks: Vec<Benchmark>) -> Run {
        Run {
            schema: crate::run::SCHEMA,
            id: "20260102-030405-box".into(),
            recorded_at: "2026-01-02T03:04:05Z".into(),
            host: Host {
                name: "box".into(),
                os: "linux".into(),
                arch: "x86_64".into(),
                cpus: 8,
                cpu_model: None,
            },
            toolchain: Toolchain {
                rustc: "rustc 1.85.0".into(),
            },
            commit: None,
            dirty: false,
            command: vec![],
            benchmarks,
        }
    }

    #[test]
    fn interaction_is_measured_over_the_product_of_singles() {
        let run = run(vec![
            bench("std", "rep/fib", 100.0),
            bench("reader=nom", "rep/fib", 110.0),
            bench("env=fnv", "rep/fib", 80.0),
            // Predicted 1.1 * 0.8 = 0.88; measured 0.70: better together.
            bench("reader=nom,env=fnv", "rep/fib", 70.0),
            // No single for printer=itoa.
            bench("printer=itoa,env=fnv", "rep/fib", 75.0),
        ]);
        let mut out = String::new();
        render(&mut out, Some(&run));
        assert!(
            out.contains("| `reader=nom,env=fnv` | 0.700× | 0.880× | 0.795× (better together) |"),
            "{out}"
        );
        assert!(out.contains("| `printer=itoa,env=fnv` | 0.750× | - | singles not measured |"));
        assert!(out.contains("| `std` | 1.000× | - |  |"));
        // Sorted fastest first.
        assert!(out.find("| `reader=nom,env=fnv`").unwrap() < out.find("| `std` |").unwrap());
    }

    #[test]
    fn says_how_to_record_when_nothing_is_measured() {
        let mut out = String::new();
        render(&mut out, None);
        assert!(out.contains("cargo xtask bench --stacks sweep"));
    }
}
