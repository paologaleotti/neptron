//! `cargo xtask`: build, verification and worktree automation for NEPTRON.
//!
//! Every Cargo build and test that this tool starts follows the machine-load
//! rules in [`load`].

#![forbid(unsafe_code)]

mod load;
mod repo;
mod rules;
mod verify;

use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command> [args]

commands:
  verify --tier <N>    run tier N of the verification ladder and the tiers
                       below it; writes target/verify/report.json
  check [cargo args]   cargo check, under the machine-load rules
  test [nextest args]  cargo nextest run, under the machine-load rules
  clippy [cargo args]  cargo clippy, under the machine-load rules
  rules                check the dependency rules, the unsafe allow-list
                       and the headless feature set

environment:
  NEPTRON_BUILD_SLOTS  build slots (default on macOS: 2, elsewhere: no lock)
  NEPTRON_BUILD_JOBS   CARGO_BUILD_JOBS per build (default on macOS: 5)
";

/// An error of any xtask command.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("unknown command {0:?}\n{USAGE}")]
    UnknownCommand(String),
    #[error(transparent)]
    Repo(#[from] repo::RepoError),
    #[error(transparent)]
    Load(#[from] load::LoadError),
    #[error(transparent)]
    Rules(#[from] rules::RulesError),
    #[error(transparent)]
    Verify(#[from] verify::VerifyError),
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, Error> {
    let Some((cmd, rest)) = args.split_first() else {
        eprint!("{USAGE}");
        return Ok(ExitCode::FAILURE);
    };
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    match cmd.as_str() {
        "verify" => Ok(verify::run(&rest)?),
        "check" => passthrough(&["check"], &rest),
        "test" => passthrough(&["nextest", "run"], &rest),
        "clippy" => passthrough(&["clippy"], &rest),
        "rules" => {
            let repo = repo::Repo::discover()?;
            let policy = load::LoadPolicy::from_env()?;
            Ok(exit_code(rules::run(&repo, &policy)? == 0))
        }
        "help" | "-h" | "--help" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(Error::UnknownCommand(other.to_owned())),
    }
}

fn passthrough(base: &[&str], rest: &[&str]) -> Result<ExitCode, Error> {
    let repo = repo::Repo::discover()?;
    let policy = load::LoadPolicy::from_env()?;
    let slot = policy.acquire(&load::lock_dir(&repo.git_common_dir))?;
    if let Some(slot) = &slot {
        eprintln!("xtask: holding build slot {}", slot.slot);
    }
    let args: Vec<&str> = base.iter().chain(rest).copied().collect();
    let status = repo::status(policy.cargo(&args).current_dir(&repo.root))?;
    Ok(exit_code(status.success()))
}

fn exit_code(success: bool) -> ExitCode {
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
