//! The OS images: the development container and the Buildroot build of the
//! QEMU targets.
//!
//! `os build` runs Buildroot in the container and copies the images to the
//! machine cache.

mod arch;
mod build;
#[cfg(test)]
mod defconfig;
mod host;

pub use arch::Arch;

use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use crate::repo::{self, Repo, RepoError};
use build::{BuildArgs, BuildPlan};
use host::{Cache, MachineLock};

/// An error of `os build`.
#[derive(Debug, thiserror::Error)]
pub enum OsError {
    /// The arguments are wrong.
    #[error("{0}\n{USAGE}")]
    Usage(String),
    /// The architecture name is not a target.
    #[error("unknown architecture {0:?}; the targets are aarch64 and x86_64")]
    BadArch(String),
    /// A numeric flag holds no positive integer.
    #[error("{flag} needs a positive integer, got {value:?}")]
    BadNumber {
        /// The flag.
        flag: &'static str,
        /// Its value.
        value: String,
    },
    /// More Buildroot jobs than the machine allows.
    #[error("--jobs {jobs} is over the limit of {max} for this machine")]
    TooManyJobs {
        /// The jobs asked for.
        jobs: u32,
        /// The limit.
        max: u32,
    },
    /// A tool is not installed.
    #[error("{tool} is not on the PATH; {hint}")]
    ToolMissing {
        /// The program name.
        tool: String,
        /// How to install it.
        hint: &'static str,
    },
    /// The Docker client cannot reach its daemon.
    #[error("Docker is installed but its daemon does not answer; start Docker Desktop\n{stderr}")]
    DockerDown {
        /// The standard error of `docker info`.
        stderr: String,
    },
    /// A command could not start.
    #[error("cannot start {cmd}: {source}")]
    Spawn {
        /// The command line.
        cmd: String,
        /// The error of the operating system.
        source: io::Error,
    },
    /// A step of the build failed.
    #[error("{cmd} failed with {status}")]
    StepFailed {
        /// The command line.
        cmd: String,
        /// The exit status.
        status: std::process::ExitStatus,
    },
    /// Neither `NEPTRON_CACHE_DIR` nor `HOME` is set.
    #[error("set HOME or NEPTRON_CACHE_DIR to find the cache directory")]
    NoHome,
    /// A file operation failed.
    #[error("{path}: {source}")]
    Io {
        /// The file or directory.
        path: PathBuf,
        /// The error of the operating system.
        source: io::Error,
    },
    /// A repository path or a git command failed.
    #[error(transparent)]
    Repo(#[from] RepoError),
}

/// The usage text of `os build`.
pub const USAGE: &str =
    "usage: cargo xtask os build [--arch aarch64|x86_64] [--jobs N] [-- make targets...]";

/// `cargo xtask os <subcommand>`.
pub fn run_os(args: &[&str]) -> Result<ExitCode, OsError> {
    match args.split_first() {
        Some((&"build", rest)) => build(rest),
        _ => Err(OsError::Usage("unknown os subcommand".to_owned())),
    }
}

fn build(args: &[&str]) -> Result<ExitCode, OsError> {
    let args = BuildArgs::parse(args, Arch::default_for(std::env::consts::ARCH))?;
    let repo = Repo::discover()?;
    let cache = Cache::from_env()?;
    host::require_tool(
        "docker",
        "install Docker (macOS: Docker Desktop) to build the OS images",
    )?;
    check_docker_daemon()?;
    let dockerfile = repo.root.join("Dockerfile");
    let text = std::fs::read_to_string(&dockerfile).map_err(|source| OsError::Io {
        path: dockerfile.clone(),
        source,
    })?;
    let plan = BuildPlan {
        external: repo.root.join("os"),
        images: cache.images(args.arch),
        container: build::container_image(&text),
        args,
    };
    let _lock = MachineLock::take(&cache.build_lock(), "Buildroot build")?;
    let start = std::time::Instant::now();
    build::execute(&plan, &dockerfile)?;
    eprintln!(
        "xtask: {} image built in {} s; images in {}",
        plan.args.arch.name(),
        start.elapsed().as_secs(),
        plan.images.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn check_docker_daemon() -> Result<(), OsError> {
    let mut cmd = Command::new("docker");
    cmd.args(["info", "--format", "{{.ServerVersion}}"]);
    let out = cmd.output().map_err(|source| spawn_error(&cmd, source))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(OsError::DockerDown {
            stderr: String::from_utf8_lossy(&out.stderr).trim_end().to_owned(),
        })
    }
}

fn run_step(cmd: &mut Command) -> Result<(), OsError> {
    eprintln!("xtask: {}", repo::describe(cmd));
    let status = cmd.status().map_err(|source| spawn_error(cmd, source))?;
    if status.success() {
        Ok(())
    } else {
        Err(OsError::StepFailed {
            cmd: repo::describe(cmd),
            status,
        })
    }
}

fn spawn_error(cmd: &Command, source: io::Error) -> OsError {
    if source.kind() == ErrorKind::NotFound {
        OsError::ToolMissing {
            tool: cmd.get_program().to_string_lossy().into_owned(),
            hint: "install it and put it on the PATH",
        }
    } else {
        OsError::Spawn {
            cmd: repo::describe(cmd),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_reported_as_missing() {
        let mut cmd = Command::new("neptron-no-such-program");
        let err = run_step(&mut cmd).unwrap_err();
        assert!(
            matches!(&err, OsError::ToolMissing { tool, .. } if tool == "neptron-no-such-program"),
            "{err}"
        );
    }

    #[test]
    fn unknown_os_subcommand_is_a_usage_error() {
        assert!(matches!(run_os(&["flash"]), Err(OsError::Usage(_))));
        assert!(matches!(run_os(&[]), Err(OsError::Usage(_))));
    }
}
