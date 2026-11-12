//! `cargo xtask verify --tier N`: runs a tier of the verification ladder and
//! every tier below it, then writes `target/verify/report.json`.
//!
//! Every step runs, also after a failure, so that one report shows all
//! failures. The report is written on every run that gets past the argument
//! check.

mod report;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use crate::load::{self, LoadError, LoadPolicy};
use crate::repo::{self, Repo, RepoError};
use crate::rules;

use report::Outcome;

/// An error that stops `verify` before its steps run.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    /// The arguments are wrong.
    #[error("usage: cargo xtask verify --tier <N>")]
    Usage,
    /// The tier is higher than this xtask can run.
    #[error("tier {tier} does not exist yet: this xtask runs tiers 0 to {max}")]
    UnknownTier {
        /// The tier that was asked for.
        tier: u8,
        /// The highest tier that exists.
        max: u8,
    },
    /// The checkout cannot be found.
    #[error(transparent)]
    Repo(#[from] RepoError),
    /// The load policy or the build slot failed.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// The report cannot be written.
    #[error("cannot write {path}: {source}")]
    WriteReport {
        /// Path of the report.
        path: PathBuf,
        /// The error of the operating system.
        source: std::io::Error,
    },
}

/// Runs `verify` with the given arguments.
pub fn run(args: &[&str]) -> Result<ExitCode, VerifyError> {
    let tier = parse_tier(args)?;
    let steps = plan(tier)?;
    let started = Instant::now();
    let root = Repo::discover().map(|r| r.root);
    let outcome = run_steps(&steps);
    let policy = LoadPolicy::from_env().ok();
    let path = report_path(root.as_deref().ok());
    let report = match &outcome {
        Ok(outcomes) => report::build(tier, outcomes, &TIER_BUDGET_SECONDS, policy.as_ref(), None),
        Err(e) => {
            let setup = Outcome {
                name: "setup",
                tier: 0,
                builds: false,
                ok: false,
                seconds: started.elapsed().as_secs_f64(),
            };
            let error = e.to_string();
            report::build(
                tier,
                &[setup],
                &TIER_BUDGET_SECONDS,
                policy.as_ref(),
                Some(&error),
            )
        }
    };
    report::write(&path, &report).map_err(|source| VerifyError::WriteReport {
        path: path.clone(),
        source,
    })?;
    print!("{}", report::summary(&report));
    println!("report: {}", path.display());
    outcome?;
    Ok(if report["result"] == "pass" {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Highest tier that this version of xtask can run.
const MAX_TIER: u8 = 2;

/// Time budget of each tier in seconds, from the verification ladder. The
/// length follows `MAX_TIER`, so every tier that `plan` accepts has a budget.
const TIER_BUDGET_SECONDS: [f64; MAX_TIER as usize + 1] = [60.0, 120.0, 300.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Cargo(&'static [&'static str]),
    Rules,
}

#[derive(Debug, Clone, Copy)]
struct Step {
    name: &'static str,
    tier: u8,
    action: Action,
    /// The step compiles code, so its time counts as build time.
    builds: bool,
}

/// The steps of every tier. The headless feature set is the default one, so
/// no step passes `--features` or `--all-features`.
const STEPS: &[Step] = &[
    Step {
        name: "fmt",
        tier: 0,
        action: Action::Cargo(&["fmt", "--all", "--check"]),
        builds: false,
    },
    Step {
        name: "rules",
        tier: 0,
        action: Action::Rules,
        builds: false,
    },
    Step {
        name: "deny",
        tier: 0,
        action: Action::Cargo(&["deny", "check", "licenses", "bans", "sources"]),
        builds: false,
    },
    Step {
        name: "clippy",
        tier: 0,
        action: Action::Cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]),
        builds: true,
    },
    Step {
        name: "build-tests",
        tier: 1,
        action: Action::Cargo(&["nextest", "run", "--workspace", "--no-run"]),
        builds: true,
    },
    Step {
        name: "test",
        tier: 1,
        // A stub crate has no tests yet; that is not a failure.
        action: Action::Cargo(&["nextest", "run", "--workspace", "--no-tests=warn"]),
        builds: false,
    },
    Step {
        name: "doctest",
        tier: 1,
        action: Action::Cargo(&["test", "--workspace", "--doc"]),
        builds: true,
    },
    Step {
        name: "app",
        tier: 2,
        // The frontend on the fakes: the end-to-end runs and the UI
        // crawler. They are the ignored tests of the app, so tier 1 skips
        // them; the test binary is built in tier 1.
        action: Action::Cargo(&["nextest", "run", "-p", "neptron", "--run-ignored", "only"]),
        builds: false,
    },
];

fn parse_tier(args: &[&str]) -> Result<u8, VerifyError> {
    match args {
        ["--tier", n] => n.parse().map_err(|_| VerifyError::Usage),
        _ => Err(VerifyError::Usage),
    }
}

fn plan(tier: u8) -> Result<Vec<Step>, VerifyError> {
    if tier > MAX_TIER {
        return Err(VerifyError::UnknownTier {
            tier,
            max: MAX_TIER,
        });
    }
    Ok(STEPS.iter().filter(|s| s.tier <= tier).copied().collect())
}

/// Runs every step under one build slot. Only a failure to find the checkout
/// or to get a slot stops the run; a failed step does not.
fn run_steps(steps: &[Step]) -> Result<Vec<Outcome>, VerifyError> {
    let repo = Repo::discover()?;
    let policy = LoadPolicy::from_env()?;
    let slot = policy.acquire(&load::lock_dir(&repo.git_common_dir))?;
    if let Some(slot) = &slot {
        eprintln!("xtask: holding build slot {}", slot.slot);
    }
    Ok(steps
        .iter()
        .map(|step| run_step(step, &repo, &policy))
        .collect())
}

fn run_step(step: &Step, repo: &Repo, policy: &LoadPolicy) -> Outcome {
    eprintln!("xtask: verify step {} (tier {})", step.name, step.tier);
    let start = Instant::now();
    let ok = match step.action {
        Action::Cargo(args) => match repo::status(policy.cargo(args).current_dir(&repo.root)) {
            Ok(status) => status.success(),
            Err(e) => {
                eprintln!("xtask: step {}: {e}", step.name);
                false
            }
        },
        Action::Rules => match rules::run(repo, policy) {
            Ok(violations) => violations == 0,
            Err(e) => {
                eprintln!("xtask: step {}: {e}", step.name);
                false
            }
        },
    };
    Outcome {
        name: step.name,
        tier: step.tier,
        builds: step.builds,
        ok,
        seconds: start.elapsed().as_secs_f64(),
    }
}

fn report_path(root: Option<&Path>) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.unwrap_or(Path::new(".")).join("target"));
    target.join("verify").join("report.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(steps: &[Step]) -> Vec<&str> {
        steps.iter().map(|s| s.name).collect()
    }

    #[test]
    fn tier_zero_is_static_checks_only() {
        assert_eq!(names(&plan(0).unwrap()), ["fmt", "rules", "deny", "clippy"]);
    }

    #[test]
    fn tier_one_includes_tier_zero() {
        let steps = plan(1).unwrap();
        assert_eq!(&names(&steps)[..4], ["fmt", "rules", "deny", "clippy"]);
        assert!(names(&steps).contains(&"test"));
    }

    #[test]
    fn tier_two_runs_the_app_on_the_fakes_after_tier_one() {
        let steps = plan(2).unwrap();
        assert_eq!(names(&steps).last(), Some(&"app"));
        assert!(names(&steps).contains(&"test"));
        let app = steps.iter().find(|s| s.name == "app").unwrap();
        let Action::Cargo(args) = app.action else {
            panic!("app is a cargo step");
        };
        assert_eq!(args[..2], ["nextest", "run"]);
        assert!(args.contains(&"neptron") && args.contains(&"only"));
    }

    #[test]
    fn missing_tiers_are_an_error() {
        assert!(matches!(
            plan(3),
            Err(VerifyError::UnknownTier { tier: 3, max: 2 })
        ));
    }

    #[test]
    fn every_step_has_a_budget() {
        for s in STEPS {
            assert!(
                usize::from(s.tier) < TIER_BUDGET_SECONDS.len(),
                "{}",
                s.name
            );
        }
    }

    #[test]
    fn tests_run_through_nextest() {
        let test = STEPS.iter().find(|s| s.name == "test").unwrap();
        let Action::Cargo(args) = test.action else {
            panic!("test is a cargo step");
        };
        assert_eq!(args[..2], ["nextest", "run"]);
    }

    #[test]
    fn no_step_turns_on_hardware_features() {
        for s in STEPS {
            if let Action::Cargo(args) = s.action {
                assert!(
                    !args.iter().any(|a| a.contains("features")),
                    "{} changes the feature set",
                    s.name
                );
            }
        }
    }

    #[test]
    fn tier_argument_is_required() {
        assert_eq!(parse_tier(&["--tier", "1"]).unwrap(), 1);
        assert!(parse_tier(&[]).is_err());
        assert!(parse_tier(&["--tier", "x"]).is_err());
    }

    #[test]
    fn a_failed_cargo_step_does_not_stop_the_run() {
        let repo = Repo::discover().unwrap();
        let policy = LoadPolicy::resolve(false, None, None).unwrap();
        let step = Step {
            name: "missing",
            tier: 0,
            action: Action::Cargo(&["no-such-subcommand-xyz"]),
            builds: false,
        };
        let outcome = run_step(&step, &repo, &policy);
        assert!(!outcome.ok);
    }
}
