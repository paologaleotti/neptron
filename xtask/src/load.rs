//! Machine-load rules (run rule 13).
//!
//! On macOS every Cargo command runs under `taskpolicy -c utility`, and a
//! build holds one of a fixed number of build slots, each with a fixed number
//! of Cargo jobs. On other hosts (Linux CI) there is no limit unless the
//! environment sets one.

use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Environment variable that sets the number of build slots.
pub const SLOTS_ENV: &str = "NEPTRON_BUILD_SLOTS";
/// Environment variable that sets `CARGO_BUILD_JOBS` for each build.
pub const JOBS_ENV: &str = "NEPTRON_BUILD_JOBS";

/// An error of the load policy or of the build-slot lock.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// An environment variable holds no positive integer.
    #[error("{name} must be a positive integer, got {value:?}")]
    BadValue {
        /// Name of the variable.
        name: &'static str,
        /// Its value.
        value: String,
    },
    /// A lock file or its directory cannot be made, opened or locked.
    #[error("cannot lock {path}: {source}")]
    Lock {
        /// The lock file or directory.
        path: PathBuf,
        /// The error of the operating system.
        source: std::io::Error,
    },
}

/// How xtask limits the load of the builds it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadPolicy {
    /// Number of build slots, or `None` for no lock.
    pub slots: Option<u32>,
    /// Value of `CARGO_BUILD_JOBS`, or `None` to leave it to Cargo.
    pub jobs: Option<u32>,
    /// Start commands under `taskpolicy -c utility`.
    pub taskpolicy: bool,
}

impl LoadPolicy {
    /// Reads the policy for the current host from the environment.
    pub fn from_env() -> Result<Self, LoadError> {
        Self::resolve(
            cfg!(target_os = "macos"),
            std::env::var(SLOTS_ENV).ok().as_deref(),
            std::env::var(JOBS_ENV).ok().as_deref(),
        )
    }

    /// Builds the policy from the host kind and the raw environment values.
    pub fn resolve(
        macos: bool,
        slots: Option<&str>,
        jobs: Option<&str>,
    ) -> Result<Self, LoadError> {
        let (default_slots, default_jobs) = if macos {
            (Some(DEFAULT_SLOTS_MACOS), Some(DEFAULT_JOBS_MACOS))
        } else {
            (None, None)
        };
        Ok(Self {
            slots: parse_positive(SLOTS_ENV, slots)?.or(default_slots),
            jobs: parse_positive(JOBS_ENV, jobs)?.or(default_jobs),
            taskpolicy: macos,
        })
    }

    /// Returns a `cargo` command with the policy applied.
    pub fn cargo(&self, args: &[&str]) -> Command {
        let mut cmd = if self.taskpolicy {
            let mut c = Command::new("taskpolicy");
            c.args(["-c", "utility", "cargo"]);
            c
        } else {
            Command::new("cargo")
        };
        cmd.args(args);
        if let Some(jobs) = self.jobs {
            cmd.env("CARGO_BUILD_JOBS", jobs.to_string());
        }
        cmd
    }

    /// Waits for a free build slot and holds it until the guard drops.
    ///
    /// The lock files live in the git common directory, so that every
    /// worktree of the repository shares the same slots.
    pub fn acquire(&self, lock_dir: &Path) -> Result<Option<SlotGuard>, LoadError> {
        let Some(slots) = self.slots else {
            return Ok(None);
        };
        fs::create_dir_all(lock_dir).map_err(|source| LoadError::Lock {
            path: lock_dir.to_path_buf(),
            source,
        })?;
        let mut announced = false;
        loop {
            for slot in 0..slots {
                let path = lock_dir.join(format!("slot-{slot}.lock"));
                let file = File::options()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(&path)
                    .map_err(|source| LoadError::Lock {
                        path: path.clone(),
                        source,
                    })?;
                match file.try_lock() {
                    Ok(()) => return Ok(Some(SlotGuard { _file: file, slot })),
                    Err(TryLockError::WouldBlock) => {}
                    Err(TryLockError::Error(source)) => {
                        return Err(LoadError::Lock { path, source });
                    }
                }
            }
            if !announced {
                eprintln!("xtask: all {slots} build slots are busy, waiting");
                announced = true;
            }
            std::thread::sleep(SLOT_POLL);
        }
    }
}

/// A held build slot. The OS releases the lock when the file closes.
#[derive(Debug)]
pub struct SlotGuard {
    _file: File,
    /// Index of the held slot.
    pub slot: u32,
}

/// Directory that holds the build-slot lock files.
pub fn lock_dir(git_common_dir: &Path) -> PathBuf {
    git_common_dir.join("neptron-build-slots")
}

/// Two slots of five jobs fill the 10 cores of the development machine and no more.
const DEFAULT_SLOTS_MACOS: u32 = 2;
const DEFAULT_JOBS_MACOS: u32 = 5;

const SLOT_POLL: Duration = Duration::from_millis(500);

fn parse_positive(name: &'static str, raw: Option<&str>) -> Result<Option<u32>, LoadError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(None),
        Some(text) => match text.parse::<u32>() {
            Ok(0) | Err(_) => Err(LoadError::BadValue {
                name,
                value: text.to_owned(),
            }),
            Ok(n) => Ok(Some(n)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_defaults_are_two_slots_of_five_jobs() {
        let p = LoadPolicy::resolve(true, None, None).unwrap();
        assert_eq!(
            p,
            LoadPolicy {
                slots: Some(2),
                jobs: Some(5),
                taskpolicy: true
            }
        );
    }

    #[test]
    fn linux_has_no_limit_by_default() {
        let p = LoadPolicy::resolve(false, None, None).unwrap();
        assert_eq!(
            p,
            LoadPolicy {
                slots: None,
                jobs: None,
                taskpolicy: false
            }
        );
    }

    #[test]
    fn environment_overrides_defaults() {
        let p = LoadPolicy::resolve(true, Some("1"), Some(" 8 ")).unwrap();
        assert_eq!(p.slots, Some(1));
        assert_eq!(p.jobs, Some(8));
    }

    #[test]
    fn zero_or_garbage_is_rejected() {
        assert!(LoadPolicy::resolve(true, Some("0"), None).is_err());
        assert!(LoadPolicy::resolve(true, None, Some("five")).is_err());
    }

    #[test]
    fn macos_command_runs_under_taskpolicy_with_jobs() {
        let p = LoadPolicy::resolve(true, None, None).unwrap();
        let cmd = p.cargo(&["check", "-p", "neptron-types"]);
        assert_eq!(cmd.get_program(), "taskpolicy");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(
            args,
            ["-c", "utility", "cargo", "check", "-p", "neptron-types"]
        );
        let jobs = cmd
            .get_envs()
            .find(|(k, _)| *k == "CARGO_BUILD_JOBS")
            .and_then(|(_, v)| v);
        assert_eq!(jobs, Some("5".as_ref()));
    }

    #[test]
    fn linux_command_is_plain_cargo() {
        let p = LoadPolicy::resolve(false, None, None).unwrap();
        let cmd = p.cargo(&["check"]);
        assert_eq!(cmd.get_program(), "cargo");
        assert_eq!(cmd.get_envs().count(), 0);
    }

    #[test]
    fn slots_exclude_each_other() {
        let dir = std::env::temp_dir().join(format!("xtask-slots-{}", std::process::id()));
        let p = LoadPolicy::resolve(true, Some("2"), None).unwrap();
        let a = p.acquire(&dir).unwrap().unwrap();
        let b = p.acquire(&dir).unwrap().unwrap();
        assert_ne!(a.slot, b.slot);
        drop(a);
        let c = p.acquire(&dir).unwrap().unwrap();
        assert_ne!(c.slot, b.slot);
        drop((b, c));
        fs::remove_dir_all(&dir).unwrap();
    }
}
