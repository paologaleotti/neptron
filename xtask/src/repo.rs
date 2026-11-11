//! Paths of the repository and small process helpers.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

/// The checkout that xtask runs in.
#[derive(Debug, Clone)]
pub struct Repo {
    /// Root of the current checkout (a worktree or the main checkout).
    pub root: PathBuf,
    /// The git directory that all worktrees share.
    pub git_common_dir: PathBuf,
}

/// An error of a child process or of a repository path.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    /// The command could not start.
    #[error("cannot start {cmd}: {source}")]
    Spawn {
        /// The command line.
        cmd: String,
        /// The error of the operating system.
        source: io::Error,
    },
    /// The command exited with a failure.
    #[error("{cmd} failed: {stderr}")]
    Failed {
        /// The command line.
        cmd: String,
        /// The standard error of the command.
        stderr: String,
    },
    /// The command wrote output that is not UTF-8.
    #[error("{cmd} wrote output that is not UTF-8")]
    NotUtf8 {
        /// The command line.
        cmd: String,
    },
    /// The current directory is not readable.
    #[error("cannot read the current directory: {0}")]
    CurrentDir(io::Error),
}

impl Repo {
    /// Finds the checkout from `CARGO_MANIFEST_DIR`, which `cargo run` sets to
    /// the xtask directory, or from the current directory.
    pub fn discover() -> Result<Self, RepoError> {
        let start = match std::env::var_os("CARGO_MANIFEST_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => std::env::current_dir().map_err(RepoError::CurrentDir)?,
        };
        Self::at(&start)
    }

    /// Finds the checkout that contains `dir`.
    pub fn at(dir: &Path) -> Result<Self, RepoError> {
        let root = git_path(dir, &["rev-parse", "--show-toplevel"])?;
        let git_common_dir = git_path(
            dir,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        Ok(Self {
            root,
            git_common_dir,
        })
    }
}

/// Runs a command with inherited output and returns its status.
pub fn status(cmd: &mut Command) -> Result<ExitStatus, RepoError> {
    cmd.status().map_err(|source| RepoError::Spawn {
        cmd: describe(cmd),
        source,
    })
}

/// Runs a command, fails on a non-zero exit, and returns its standard output.
pub fn output(cmd: &mut Command) -> Result<String, RepoError> {
    let out = cmd.output().map_err(|source| RepoError::Spawn {
        cmd: describe(cmd),
        source,
    })?;
    if !out.status.success() {
        return Err(RepoError::Failed {
            cmd: describe(cmd),
            stderr: String::from_utf8_lossy(&out.stderr).trim_end().to_owned(),
        });
    }
    String::from_utf8(out.stdout).map_err(|_| RepoError::NotUtf8 { cmd: describe(cmd) })
}

/// The command line of `cmd`, for messages.
pub fn describe(cmd: &Command) -> String {
    let mut s = cmd.get_program().to_string_lossy().into_owned();
    for a in cmd.get_args() {
        s.push(' ');
        s.push_str(&a.to_string_lossy());
    }
    s
}

fn git_path(dir: &Path, args: &[&str]) -> Result<PathBuf, RepoError> {
    let out = output(Command::new("git").arg("-C").arg(dir).args(args))?;
    Ok(PathBuf::from(out.trim_end()))
}
