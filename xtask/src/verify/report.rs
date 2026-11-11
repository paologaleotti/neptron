//! The JSON report of a `verify` run and its printed summary.

use std::io;
use std::path::Path;

use serde_json::{Value, json};

use crate::load::LoadPolicy;

/// The result of one step.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Name of the step.
    pub name: &'static str,
    /// Tier the step belongs to.
    pub tier: u8,
    /// The step compiles code, so its time counts as build time.
    pub builds: bool,
    /// The step passed.
    pub ok: bool,
    /// Wall-clock time of the step in seconds.
    pub seconds: f64,
}

/// Builds the report. `budgets` holds the time budget in seconds of each
/// tier, by tier. A run over budget is reported, not failed: a cold build is
/// always slower. `load` is the policy the steps ran under, if one was read.
/// `error` is the reason when the steps could not start.
pub fn build(
    tier: u8,
    outcomes: &[Outcome],
    budgets: &[f64],
    load: Option<&LoadPolicy>,
    error: Option<&str>,
) -> Value {
    let pass = error.is_none() && outcomes.iter().all(|o| o.ok);
    let total = seconds_of(outcomes.iter());
    let build = seconds_of(outcomes.iter().filter(|o| o.builds));
    let budgets: Vec<Value> = (0..=tier)
        .map(|t| {
            let seconds = seconds_of(outcomes.iter().filter(|o| o.tier == t));
            let limit = budgets.get(usize::from(t)).copied();
            json!({
                "tier": t,
                "seconds": round(seconds),
                "limit_seconds": limit,
                "within": limit.is_none_or(|l| seconds <= l),
            })
        })
        .collect();
    let steps: Vec<Value> = outcomes
        .iter()
        .map(|o| {
            json!({
                "name": o.name,
                "tier": o.tier,
                "result": if o.ok { "pass" } else { "fail" },
                "seconds": round(o.seconds),
            })
        })
        .collect();
    json!({
        "tier": tier,
        "result": if pass { "pass" } else { "fail" },
        "error": error,
        "host": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "total_seconds": round(total),
        "build_seconds": round(build),
        "budgets": budgets,
        "steps": steps,
        "load": load.map(|p| json!({
            "slots": p.slots,
            "jobs": p.jobs,
            "taskpolicy": p.taskpolicy,
        })),
    })
}

/// The summary that `verify` prints.
pub fn summary(report: &Value) -> String {
    let mut s = format!(
        "verify tier {}: {} in {:.1} s (build {:.1} s) on {}\n",
        report["tier"],
        report["result"].as_str().unwrap_or("?").to_uppercase(),
        report["total_seconds"].as_f64().unwrap_or(0.0),
        report["build_seconds"].as_f64().unwrap_or(0.0),
        report["host"].as_str().unwrap_or("?"),
    );
    if let Some(error) = report["error"].as_str() {
        s += &format!("  error: {error}\n");
    }
    for b in report["budgets"].as_array().into_iter().flatten() {
        s += &format!(
            "  tier {}: {:.1} s of {} s budget{}\n",
            b["tier"],
            b["seconds"].as_f64().unwrap_or(0.0),
            b["limit_seconds"],
            if b["within"] == true {
                ""
            } else {
                " (over budget)"
            },
        );
    }
    for st in report["steps"].as_array().into_iter().flatten() {
        s += &format!(
            "  {:<4}  {:<12} {:>6.1} s\n",
            st["result"].as_str().unwrap_or("?"),
            st["name"].as_str().unwrap_or("?"),
            st["seconds"].as_f64().unwrap_or(0.0),
        );
    }
    s
}

/// Writes the report as pretty JSON, and makes its directory first.
pub fn write(path: &Path, report: &Value) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(report).map_err(io::Error::other)?;
    std::fs::write(path, text + "\n")
}

/// Adds from `0.0`, because `Sum` for `f64` starts at `-0.0` and an empty
/// sum would print as "-0.0 s".
fn seconds_of<'a>(outcomes: impl Iterator<Item = &'a Outcome>) -> f64 {
    outcomes.fold(0.0, |sum, o| sum + o.seconds)
}

fn round(seconds: f64) -> f64 {
    (seconds * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(name: &'static str, tier: u8, builds: bool, ok: bool, seconds: f64) -> Outcome {
        Outcome {
            name,
            tier,
            builds,
            ok,
            seconds,
        }
    }

    #[test]
    fn report_sums_build_time_and_budgets() {
        let r = build(
            1,
            &[
                outcome("fmt", 0, false, true, 1.0),
                outcome("clippy", 0, true, true, 70.0),
                outcome("build-tests", 1, true, true, 20.0),
                outcome("test", 1, false, true, 2.0),
            ],
            &[60.0, 120.0],
            LoadPolicy::resolve(true, None, None).ok().as_ref(),
            None,
        );
        assert_eq!(r["result"], "pass");
        assert_eq!(r["total_seconds"], 93.0);
        assert_eq!(r["build_seconds"], 90.0);
        assert_eq!(r["budgets"][0]["within"], false);
        assert_eq!(r["budgets"][1]["seconds"], 22.0);
        assert_eq!(r["load"]["jobs"], 5);
        let text = summary(&r);
        assert!(text.starts_with("verify tier 1: PASS in 93.0 s (build 90.0 s)"));
        assert!(text.contains("tier 0: 71.0 s of 60.0 s budget (over budget)"));
    }

    #[test]
    fn one_failed_step_fails_the_run() {
        let r = build(
            0,
            &[
                outcome("fmt", 0, false, false, 0.1),
                outcome("rules", 0, false, true, 0.1),
            ],
            &[60.0],
            None,
            None,
        );
        assert_eq!(r["result"], "fail");
        assert_eq!(r["steps"][0]["result"], "fail");
    }

    #[test]
    fn a_setup_error_fails_the_run_and_is_shown() {
        let r = build(0, &[], &[60.0], None, Some("no build slot"));
        assert_eq!(r["result"], "fail");
        let text = summary(&r);
        assert!(text.contains("error: no build slot"));
        assert!(text.contains("(build 0.0 s)"), "{text}");
    }

    #[test]
    fn a_missing_budget_is_null_not_a_panic() {
        let r = build(1, &[outcome("x", 1, false, true, 1.0)], &[60.0], None, None);
        assert!(r["budgets"][1]["limit_seconds"].is_null());
        assert_eq!(r["budgets"][1]["within"], true);
    }

    #[test]
    fn the_report_is_written() {
        let dir = std::env::temp_dir().join(format!("xtask-report-{}", std::process::id()));
        let path = dir.join("verify").join("report.json");
        write(&path, &json!({"result": "pass"})).unwrap();
        let back: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back["result"], "pass");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
