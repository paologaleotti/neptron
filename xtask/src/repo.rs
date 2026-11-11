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
    /// The shared git directory has no parent directory.
    #[error("{0} has no parent directory")]
    NoParent(PathBuf),
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

    /// Root of the main checkout: the parent of the shared git directory.
    pub fn main_root(&self) -> Result<PathBuf, RepoError> {
        self.git_common_dir
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| RepoError::NoParent(self.git_common_dir.clone()))
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

/// Returns a `git` command that runs in `dir` and ignores the repository of
/// the caller.
///
/// Every `git` call of xtask starts here. A plain `Command::new("git")`
/// inherits `GIT_DIR` when xtask runs below a git operation, and then acts on
/// the repository of that operation and not on `dir`.
pub fn git(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir);
    without_git_location(&mut cmd);
    cmd
}

/// Removes from `cmd` the variables with which git tells its children where
/// the repository is.
///
/// `git rebase --exec`, hooks and `git bisect run` export them. A child that
/// keeps them can change the repository of the caller, for example a test
/// that runs `git init` in a temporary directory.
pub fn without_git_location(cmd: &mut Command) -> &mut Command {
    for var in GIT_LOCATION_VARS {
        cmd.env_remove(var);
    }
    cmd
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

const GIT_LOCATION_VARS: [&str; 8] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
];

fn git_path(dir: &Path, args: &[&str]) -> Result<PathBuf, RepoError> {
    let out = output(git(dir).args(args))?;
    Ok(PathBuf::from(out.trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn removed(cmd: &Command, var: &str) -> bool {
        cmd.get_envs()
            .any(|(key, value)| key == OsStr::new(var) && value.is_none())
    }

    #[test]
    fn git_removes_every_location_variable() {
        let cmd = git(Path::new("/tmp"));
        for var in GIT_LOCATION_VARS {
            assert!(removed(&cmd, var), "{var} stays in the environment");
        }
    }

    #[test]
    fn git_runs_in_the_given_directory() {
        let cmd = git(Path::new("/some/dir"));
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, [OsStr::new("-C"), OsStr::new("/some/dir")]);
    }

    #[test]
    fn only_this_module_starts_git() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let needle = concat!("Command::new(\"", "git\")");
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name() != Some(OsStr::new("repo.rs")) {
                    let text = std::fs::read_to_string(&path).unwrap();
                    assert!(
                        !text.contains(needle),
                        "{} starts git without repo::git",
                        path.display()
                    );
                }
            }
        }
    }
}
