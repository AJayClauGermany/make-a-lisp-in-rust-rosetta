//! The recorded form of one benchmark run: `runs/<id>/results.json`.
//!
//! A run is self-describing (host, toolchain, commit, command) so that two
//! runs can be compared with their differences in view. The benchmark ids
//! follow `mal_bench::group_id`: `<component>/<implementation>/<workload>`,
//! which is what lets the report join numbers back to the registry.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

/// Bumped when the JSON layout changes incompatibly; old runs are then
/// migrated or dropped rather than silently misread.
pub const SCHEMA: u32 = 1;

/// The directory under the workspace root that holds the runs.
pub const RUNS_DIR: &str = "runs";

/// One recorded benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    /// Layout version, see [`SCHEMA`].
    pub schema: u32,
    /// Directory name under `runs/`: `<yyyymmdd>-<hhmmss>-<host>`. Sorting
    /// ids lexicographically sorts runs chronologically.
    pub id: String,
    /// When the run was recorded, RFC 3339 in UTC.
    pub recorded_at: String,
    /// The machine the run was recorded on.
    pub host: Host,
    /// The compiler used.
    pub toolchain: Toolchain,
    /// The workspace commit, if the run was made from a git checkout.
    pub commit: Option<String>,
    /// Whether the checkout had uncommitted changes.
    pub dirty: bool,
    /// The benchmark command, for reproduction.
    pub command: Vec<String>,
    /// Every measurement, sorted by id.
    pub benchmarks: Vec<Benchmark>,
}

/// Where a run was recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    /// Host name, sanitised for use in the run id.
    pub name: String,
    /// `std::env::consts::OS`.
    pub os: String,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// Logical CPUs available to the process.
    pub cpus: usize,
    /// CPU model string, when the platform exposes one.
    pub cpu_model: Option<String>,
}

/// The compiler a run was built with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toolchain {
    /// First line of `rustc -vV`, e.g. `rustc 1.85.0 (4d91de4e4 2025-02-17)`.
    pub rustc: String,
}

/// One Criterion benchmark's summary statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Benchmark {
    /// Full Criterion id: `<component>/<implementation>/<workload>`.
    pub id: String,
    /// `Component::slug()` of the row.
    pub component: String,
    /// `Implementation::name` of the row.
    pub implementation: String,
    /// What was measured, e.g. `read_str/nested` or `repl-session`.
    pub workload: String,
    /// Point estimate of the mean time per iteration, in nanoseconds.
    pub mean_ns: f64,
    /// Point estimate of the median time per iteration, in nanoseconds.
    pub median_ns: f64,
    /// Point estimate of the standard deviation, in nanoseconds.
    pub std_dev_ns: f64,
    /// Input size per iteration, when the benchmark declared one.
    pub throughput: Option<Throughput>,
}

/// The amount of input one iteration processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Throughput {
    /// Bytes per iteration.
    Bytes(u64),
    /// Items per iteration.
    Elements(u64),
}

impl Run {
    /// Load every run under `<root>/runs/`, newest first.
    ///
    /// Directories without a `results.json`, such as the `README.md` entry,
    /// are skipped; a malformed or newer-schema file is an error so it is
    /// never silently left out of the report.
    pub fn load_all(root: &Path) -> Result<Vec<Run>> {
        let dir = root.join(RUNS_DIR);
        let mut runs = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| format!("reading {}: {e}", dir.display()))? {
            let path = entry?.path().join("results.json");
            if path.is_file() {
                runs.push(Run::load(&path)?);
            }
        }
        runs.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(runs)
    }

    /// Load one run from its `results.json`.
    pub fn load(path: &Path) -> Result<Run> {
        let text =
            fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let run: Run =
            serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
        if run.schema != SCHEMA {
            return Err(format!(
                "{} has schema {}, this xtask reads schema {SCHEMA}",
                path.display(),
                run.schema
            )
            .into());
        }
        Ok(run)
    }

    /// Write this run to `<root>/runs/<id>/results.json` and return the path.
    pub fn save(&self, root: &Path) -> Result<PathBuf> {
        let dir = root.join(RUNS_DIR).join(&self.id);
        fs::create_dir_all(&dir)?;
        let path = dir.join("results.json");
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        fs::write(&path, text)?;
        Ok(path)
    }

    /// The benchmark with this id, if the run has it.
    pub fn benchmark(&self, id: &str) -> Option<&Benchmark> {
        self.benchmarks.iter().find(|b| b.id == id)
    }
}

/// Collect every benchmark Criterion wrote under `<target>/criterion/`.
///
/// Criterion stores each benchmark as `<dir>/new/benchmark.json` (the ids)
/// next to `<dir>/new/estimates.json` (the statistics). Only `new/` is read:
/// `base/` holds saved baselines from earlier invocations.
pub fn collect_criterion(criterion_dir: &Path) -> Result<Vec<Benchmark>> {
    let mut found = Vec::new();
    walk(criterion_dir, &mut found)?;
    let mut benchmarks = Vec::with_capacity(found.len());
    for dir in found {
        benchmarks.push(read_benchmark(&dir)?);
    }
    benchmarks.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(benchmarks)
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    if dir.join("new").join("benchmark.json").is_file() {
        found.push(dir.join("new"));
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() && path.file_name().is_some_and(|n| n != "report") {
            walk(&path, found)?;
        }
    }
    Ok(())
}

/// The parts of Criterion's `benchmark.json` the report needs.
#[derive(Deserialize)]
struct CriterionId {
    group_id: String,
    function_id: Option<String>,
    value_str: Option<String>,
    full_id: String,
    throughput: Option<serde_json::Value>,
}

/// The parts of Criterion's `estimates.json` the report needs.
#[derive(Deserialize)]
struct CriterionEstimates {
    mean: Estimate,
    median: Estimate,
    std_dev: Estimate,
}

#[derive(Deserialize)]
struct Estimate {
    point_estimate: f64,
}

fn read_benchmark(new_dir: &Path) -> Result<Benchmark> {
    let id_path = new_dir.join("benchmark.json");
    let id: CriterionId = serde_json::from_str(&fs::read_to_string(&id_path)?)
        .map_err(|e| format!("parsing {}: {e}", id_path.display()))?;
    let est_path = new_dir.join("estimates.json");
    let est: CriterionEstimates = serde_json::from_str(&fs::read_to_string(&est_path)?)
        .map_err(|e| format!("parsing {}: {e}", est_path.display()))?;

    let (component, implementation) = id.group_id.split_once('/').ok_or_else(|| {
        format!(
            "benchmark group `{}` is not `<component>/<implementation>`; use mal_bench::group_id",
            id.group_id
        )
    })?;
    let workload = match (&id.function_id, &id.value_str) {
        (Some(f), Some(v)) => format!("{f}/{v}"),
        (Some(f), None) => f.clone(),
        (None, Some(v)) => v.clone(),
        (None, None) => return Err(format!("benchmark `{}` has no function id", id.full_id).into()),
    };
    Ok(Benchmark {
        id: id.full_id,
        component: component.to_owned(),
        implementation: implementation.to_owned(),
        workload,
        mean_ns: est.mean.point_estimate,
        median_ns: est.median.point_estimate,
        std_dev_ns: est.std_dev.point_estimate,
        throughput: id.throughput.as_ref().and_then(parse_throughput),
    })
}

/// Criterion serialises its `Throughput` enum as `{"Bytes": n}`,
/// `{"BytesDecimal": n}` or `{"Elements": n}`.
fn parse_throughput(value: &serde_json::Value) -> Option<Throughput> {
    let object = value.as_object()?;
    let (variant, amount) = object.iter().next()?;
    let amount = amount.as_u64()?;
    match variant.as_str() {
        "Bytes" | "BytesDecimal" => Some(Throughput::Bytes(amount)),
        "Elements" => Some(Throughput::Elements(amount)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn collects_criterion_layout() {
        let dir = std::env::temp_dir().join(format!("xtask-criterion-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let bench = dir.join("eval_echo").join("rep").join("atom").join("new");
        write(
            &bench.join("benchmark.json"),
            r#"{"group_id":"eval/echo","function_id":"rep","value_str":"atom","throughput":{"Bytes":2},"full_id":"eval/echo/rep/atom","directory_name":"eval_echo/rep/atom","title":"eval/echo/rep/atom"}"#,
        );
        write(
            &bench.join("estimates.json"),
            r#"{"mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":3.0},"point_estimate":2.0,"standard_error":0.1},"median":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":3.0},"point_estimate":1.9,"standard_error":0.1},"median_abs_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":0.0,"upper_bound":1.0},"point_estimate":0.2,"standard_error":0.1},"slope":null,"std_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":0.0,"upper_bound":1.0},"point_estimate":0.3,"standard_error":0.1}}"#,
        );
        // A saved baseline and the report directory must be ignored.
        write(
            &dir.join("eval_echo")
                .join("rep")
                .join("atom")
                .join("base")
                .join("benchmark.json"),
            "{}",
        );
        write(&dir.join("report").join("index.html"), "");

        let benchmarks = collect_criterion(&dir).unwrap();
        assert_eq!(
            benchmarks,
            vec![Benchmark {
                id: "eval/echo/rep/atom".into(),
                component: "eval".into(),
                implementation: "echo".into(),
                workload: "rep/atom".into(),
                mean_ns: 2.0,
                median_ns: 1.9,
                std_dev_ns: 0.3,
                throughput: Some(Throughput::Bytes(2)),
            }]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn run_round_trips_through_json() {
        let run = Run {
            schema: SCHEMA,
            id: "20260101-120000-host".into(),
            recorded_at: "2026-01-01T12:00:00Z".into(),
            host: Host {
                name: "host".into(),
                os: "linux".into(),
                arch: "x86_64".into(),
                cpus: 4,
                cpu_model: None,
            },
            toolchain: Toolchain {
                rustc: "rustc 1.85.0".into(),
            },
            commit: Some("abc".into()),
            dirty: false,
            command: vec!["cargo".into(), "bench".into()],
            benchmarks: vec![],
        };
        let text = serde_json::to_string(&run).unwrap();
        let back: Run = serde_json::from_str(&text).unwrap();
        assert_eq!(back.id, run.id);
        assert!(text.contains(r#""schema":1"#));
    }
}
