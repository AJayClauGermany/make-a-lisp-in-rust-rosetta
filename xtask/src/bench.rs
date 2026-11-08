//! `cargo xtask bench`: run the benchmarks and record them as a run.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::run::{self, Host, Run, Toolchain};
use crate::{Result, report, workspace_root};

/// Parse the `bench` arguments, run the benchmarks and save the run.
pub fn run(args: &[String]) -> Result<()> {
    let mut host_override = None;
    let mut report_after = true;
    let mut criterion_args = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--host" => {
                host_override = Some(iter.next().ok_or("--host needs a value")?.clone());
            }
            "--no-report" => report_after = false,
            "--" => {
                criterion_args.extend(iter.by_ref().cloned());
            }
            other => return Err(format!("unknown argument `{other}` for bench").into()),
        }
    }

    let root = workspace_root();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let criterion_dir = target_dir(&root, &cargo)?.join("criterion");

    // Only this invocation's results may end up in the run.
    if criterion_dir.is_dir() {
        fs::remove_dir_all(&criterion_dir)?;
    }

    let mut command = vec![
        "cargo".to_owned(),
        "bench".to_owned(),
        "--package".to_owned(),
        "mal".to_owned(),
        "--bench".to_owned(),
        "components".to_owned(),
        "--".to_owned(),
        "--noplot".to_owned(),
    ];
    command.extend(criterion_args);
    eprintln!("$ {}", command.join(" "));
    let status = Command::new(&cargo)
        .args(&command[1..])
        .current_dir(&root)
        .status()
        .map_err(|e| format!("running cargo bench: {e}"))?;
    if !status.success() {
        return Err(format!("cargo bench failed with {status}").into());
    }

    let benchmarks = run::collect_criterion(&criterion_dir)?;
    if benchmarks.is_empty() {
        return Err(format!("no benchmarks found under {}", criterion_dir.display()).into());
    }

    let (date, time, recorded_at) = timestamp();
    let host = host(host_override);
    let run = Run {
        schema: run::SCHEMA,
        id: format!("{date}-{time}-{}", host.name),
        recorded_at,
        host,
        toolchain: Toolchain {
            rustc: rustc_version(),
        },
        commit: git(&root, &["rev-parse", "--short=12", "HEAD"]),
        dirty: git(&root, &["status", "--porcelain"]).is_some_and(|s| !s.is_empty()),
        command,
        benchmarks,
    };
    let path = run.save(&root)?;
    eprintln!(
        "recorded {} benchmarks in {}",
        run.benchmarks.len(),
        path.display()
    );

    if report_after {
        report::run(&[])?;
    }
    Ok(())
}

/// Ask cargo where the target directory is, honouring `CARGO_TARGET_DIR`
/// and `.cargo/config.toml`.
fn target_dir(root: &Path, cargo: &std::ffi::OsStr) -> Result<PathBuf> {
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("running cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed".into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let dir = metadata["target_directory"]
        .as_str()
        .ok_or("cargo metadata has no target_directory")?;
    Ok(PathBuf::from(dir))
}

fn host(name_override: Option<String>) -> Host {
    let raw = name_override
        .or_else(|| {
            Command::new("hostname")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        })
        .or_else(|| {
            fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_owned())
        })
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    Host {
        name: sanitise(&raw),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        cpus: std::thread::available_parallelism().map_or(1, usize::from),
        cpu_model: cpu_model(),
    }
}

/// Lowercase letters, digits and hyphens only, so the name is safe as part
/// of a directory name on every platform.
fn sanitise(name: &str) -> String {
    let mut out: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        "unknown".to_owned()
    } else {
        out.to_owned()
    }
}

fn cpu_model() -> Option<String> {
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        if let Some(line) = cpuinfo.lines().find(|l| l.starts_with("model name")) {
            if let Some((_, model)) = line.split_once(':') {
                return Some(model.trim().to_owned());
            }
        }
    }
    Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn rustc_version() -> String {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    Command::new(rustc)
        .arg("-vV")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

/// The current UTC time as `(yyyymmdd, hhmmss, RFC 3339)`.
fn timestamp() -> (String, String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0));
    let (hh, mm, ss) = (secs % 86_400 / 3600, secs % 3600 / 60, secs % 60);
    (
        format!("{y:04}{m:02}{d:02}"),
        format!("{hh:02}{mm:02}{ss:02}"),
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"),
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_759), (2026, 11, 2));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn host_names_are_path_safe() {
        assert_eq!(sanitise("My Laptop.local"), "my-laptop-local");
        assert_eq!(sanitise("---"), "unknown");
        assert_eq!(sanitise("runner_42"), "runner-42");
    }
}
