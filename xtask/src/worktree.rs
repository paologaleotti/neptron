//! Task worktrees with a cloned build cache (run rule 12).
//!
//! `worktree new <task>` makes `.worktrees/<task>` on branch `mvp/<task>` from
//! `main`, and gives it a copy-on-write clone of the main checkout's `target/`.
//! `worktree cache-check` proves that a build in such a clone compiles no
//! dependency again.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

use serde_json::Value;

use crate::load::{self, LoadError, LoadPolicy};
use crate::repo::{self, Repo, RepoError};

/// An error of a worktree command.
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    /// The arguments are wrong.
    #[error("usage: cargo xtask worktree new <task> | worktree cache-check [--from <checkout>]")]
    Usage,
    /// The task id cannot be a directory and branch name.
    #[error("task {0:?} must be letters, digits, '-', '_' or '.', such as T05-modeline")]
    BadTask(String),
    /// The worktree directory exists already.
    #[error("{0} exists already")]
    Exists(PathBuf),
    /// The checkout builds into a target directory outside itself, so a clone
    /// of `<checkout>/target` would not be its build cache.
    #[error(
        "{checkout} builds into {target}; unset CARGO_TARGET_DIR (or build.target-dir) \
         so that the build cache is {checkout}/target"
    )]
    TargetOutside {
        /// The checkout.
        checkout: PathBuf,
        /// The target directory that Cargo reports.
        target: PathBuf,
    },
    /// The clone of `target/` failed. The new worktree and branch are removed.
    #[error("cannot clone the build cache into {path}: {source}")]
    Clone {
        /// The new worktree.
        path: PathBuf,
        /// The error of `cp`.
        source: RepoError,
    },
    /// A probe build failed.
    #[error("{cmd} failed in {dir}")]
    Probe {
        /// The command line.
        cmd: String,
        /// The directory it ran in.
        dir: PathBuf,
    },
    /// The output of `cargo metadata` is not usable.
    #[error("cannot read the target directory from cargo metadata in {0}")]
    Metadata(PathBuf),
    /// A git or cargo command failed.
    #[error(transparent)]
    Repo(#[from] RepoError),
    /// The load policy or the build slot failed.
    #[error(transparent)]
    Load(#[from] LoadError),
}

/// Runs `worktree <subcommand>`.
pub fn run(args: &[&str]) -> Result<ExitCode, WorktreeError> {
    match args {
        ["new", task] => new(task),
        ["cache-check"] => cache_check(None),
        ["cache-check", "--from", dir] => cache_check(Some(Path::new(dir))),
        _ => Err(WorktreeError::Usage),
    }
}

/// The builds that the cache check repeats in the clone: the test build that
/// nextest runs, and clippy.
const PROBES: &[&[&str]] = &[
    &[
        "test",
        "--workspace",
        "--no-run",
        "--message-format=json-render-diagnostics",
    ],
    &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--message-format=json",
        "--",
        "-D",
        "warnings",
    ],
];

const CACHE_CHECK_PREFIX: &str = "cache-check-";

fn new(task: &str) -> Result<ExitCode, WorktreeError> {
    validate_task(task)?;
    let repo = Repo::discover()?;
    let policy = LoadPolicy::from_env()?;
    let path = create(&repo.main_root()?, task, &policy)?;
    println!("worktree {} on branch mvp/{task}", path.display());
    Ok(ExitCode::SUCCESS)
}

/// Makes the worktree and branch of `task` in `main_root` and clones the
/// build cache into it. On a failed clone it removes both again.
fn create(main_root: &Path, task: &str, policy: &LoadPolicy) -> Result<PathBuf, WorktreeError> {
    let main_root = main_root.to_path_buf();
    let path = task_path(&main_root, task);
    if path.exists() {
        return Err(WorktreeError::Exists(path));
    }
    let source = main_root.join("target");
    let warm = source.is_dir();
    if warm {
        check_target_dir(&main_root, policy)?;
    }
    let branch = format!("mvp/{task}");
    repo::output(
        Command::new("git")
            .arg("-C")
            .arg(&main_root)
            .args(["worktree", "add", "-b", &branch])
            .arg(&path)
            .arg("main"),
    )?;
    if warm {
        let start = Instant::now();
        if let Err(source) = repo::output(&mut clone_command(&source, &path.join("target"))) {
            remove_worktree(&main_root, &path);
            delete_branch(&main_root, &branch);
            return Err(WorktreeError::Clone { path, source });
        }
        println!(
            "cloned {} in {:.1} s",
            source.display(),
            start.elapsed().as_secs_f64()
        );
    } else {
        println!(
            "warning: {} does not exist; the new worktree starts cold",
            source.display()
        );
    }
    Ok(path)
}

fn cache_check(from: Option<&Path>) -> Result<ExitCode, WorktreeError> {
    let here = Repo::discover()?;
    let main_root = here.main_root()?;
    let source = match from {
        Some(dir) => Repo::at(dir)?.root,
        None => main_root.clone(),
    };
    let policy = LoadPolicy::from_env()?;
    check_target_dir(&source, &policy)?;
    remove_stale_clones(&source, &main_root.join(".worktrees"));
    let _slot = policy.acquire(&load::lock_dir(&here.git_common_dir))?;

    println!("cache-check: warming {}", source.display());
    let start = Instant::now();
    let warm = probe(&policy, &source)?;
    println!(
        "cache-check: warm-up took {:.1} s ({} dependency artifacts rebuilt)",
        start.elapsed().as_secs_f64(),
        warm.deps_rebuilt.len()
    );

    let clone = TempWorktree::add(
        &source,
        main_root
            .join(".worktrees")
            .join(format!("{CACHE_CHECK_PREFIX}{}", std::process::id())),
    )?;
    let counts = check_in_clone(&policy, &source, &clone.path)?;
    drop(clone);

    println!(
        "cache-check: dependencies {} fresh, {} rebuilt; workspace crates {} fresh, {} rebuilt",
        counts.deps_fresh,
        counts.deps_rebuilt.len(),
        counts.workspace_fresh,
        counts.workspace_rebuilt
    );
    for d in &counts.deps_rebuilt {
        println!("  rebuilt: {d}");
    }
    if counts.deps_rebuilt.is_empty() {
        println!("cache-check: PASS: the clone compiles no dependency again");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("cache-check: FAIL: the clone does not keep dependencies fresh");
        Ok(ExitCode::FAILURE)
    }
}

/// A detached worktree that is removed when the value drops, also when the
/// check fails or panics.
struct TempWorktree {
    source: PathBuf,
    path: PathBuf,
}

impl TempWorktree {
    fn add(source: &Path, path: PathBuf) -> Result<Self, WorktreeError> {
        repo::output(
            Command::new("git")
                .arg("-C")
                .arg(source)
                .args(["worktree", "add", "--detach"])
                .arg(&path)
                .arg("HEAD"),
        )?;
        Ok(Self {
            source: source.to_path_buf(),
            path,
        })
    }
}

impl Drop for TempWorktree {
    fn drop(&mut self) {
        remove_worktree(&self.source, &self.path);
    }
}

/// What one probe build compiled.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    deps_fresh: usize,
    deps_rebuilt: Vec<String>,
    workspace_fresh: usize,
    workspace_rebuilt: usize,
}

impl Counts {
    fn add(&mut self, other: Counts) {
        self.deps_fresh += other.deps_fresh;
        self.deps_rebuilt.extend(other.deps_rebuilt);
        self.workspace_fresh += other.workspace_fresh;
        self.workspace_rebuilt += other.workspace_rebuilt;
    }
}

/// A task id becomes a directory and a branch name, so it stays plain.
fn validate_task(task: &str) -> Result<(), WorktreeError> {
    let plain = !task.is_empty()
        && task
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !task.starts_with(['-', '.'])
        && !task.contains("..");
    if plain {
        Ok(())
    } else {
        Err(WorktreeError::BadTask(task.to_owned()))
    }
}

/// Path of the worktree directory for a task.
fn task_path(main_root: &Path, task: &str) -> PathBuf {
    main_root.join(".worktrees").join(task)
}

/// Fails when Cargo builds `checkout` into a directory other than
/// `<checkout>/target`, for example because `CARGO_TARGET_DIR` is set.
fn check_target_dir(checkout: &Path, policy: &LoadPolicy) -> Result<(), WorktreeError> {
    let json = repo::output(
        policy
            .cargo(&["metadata", "--no-deps", "--format-version", "1"])
            .current_dir(checkout),
    )?;
    let target = serde_json::from_str::<Value>(&json)
        .ok()
        .and_then(|m| m["target_directory"].as_str().map(PathBuf::from))
        .ok_or_else(|| WorktreeError::Metadata(checkout.to_path_buf()))?;
    if same_path(&target, &checkout.join("target")) {
        Ok(())
    } else {
        Err(WorktreeError::TargetOutside {
            checkout: checkout.to_path_buf(),
            target,
        })
    }
}

/// Compares two paths after resolving symbolic links where they exist, so
/// that `/tmp` and `/private/tmp` on macOS are equal.
fn same_path(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}

/// `cp -cRp` makes APFS clones on macOS. Elsewhere `--reflink=auto` clones
/// where the file system can and copies where it cannot.
fn clone_command(from: &Path, to: &Path) -> Command {
    let mut cmd = Command::new("cp");
    if cfg!(target_os = "macos") {
        cmd.arg("-cRp");
    } else {
        cmd.args(["-a", "--reflink=auto"]);
    }
    cmd.arg(from).arg(to);
    cmd
}

/// Counts fresh and rebuilt artifacts in the JSON messages of a Cargo build.
/// A package from a path source is a workspace crate; any other source is a
/// dependency.
fn count_artifacts(messages: &str) -> Counts {
    let mut c = Counts::default();
    for line in messages.lines() {
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if msg["reason"] != "compiler-artifact" {
            continue;
        }
        let id = msg["package_id"].as_str().unwrap_or_default();
        let fresh = msg["fresh"].as_bool().unwrap_or(false);
        match (id.starts_with("path+"), fresh) {
            (true, true) => c.workspace_fresh += 1,
            (true, false) => c.workspace_rebuilt += 1,
            (false, true) => c.deps_fresh += 1,
            (false, false) => c.deps_rebuilt.push(format!(
                "{} ({})",
                id,
                msg["target"]["name"].as_str().unwrap_or("?")
            )),
        }
    }
    c
}

fn probe(policy: &LoadPolicy, dir: &Path) -> Result<Counts, WorktreeError> {
    let mut total = Counts::default();
    for args in PROBES {
        let mut cmd = policy.cargo(args);
        cmd.current_dir(dir).stderr(Stdio::inherit());
        let out = cmd.output().map_err(|source| RepoError::Spawn {
            cmd: repo::describe(&cmd),
            source,
        })?;
        if !out.status.success() {
            return Err(WorktreeError::Probe {
                cmd: repo::describe(&cmd),
                dir: dir.to_path_buf(),
            });
        }
        total.add(count_artifacts(&String::from_utf8_lossy(&out.stdout)));
    }
    Ok(total)
}

fn check_in_clone(
    policy: &LoadPolicy,
    source: &Path,
    clone: &Path,
) -> Result<Counts, WorktreeError> {
    let target = source.join("target");
    let start = Instant::now();
    repo::output(&mut clone_command(&target, &clone.join("target")))?;
    println!(
        "cache-check: cloned {} in {:.1} s",
        target.display(),
        start.elapsed().as_secs_f64()
    );
    let start = Instant::now();
    let counts = probe(policy, clone)?;
    println!(
        "cache-check: build in the clone took {:.1} s",
        start.elapsed().as_secs_f64()
    );
    Ok(counts)
}

/// Removes the clones of earlier checks whose process no longer runs, for
/// example after an interrupt. A drop guard cannot run on a signal.
fn remove_stale_clones(source: &Path, worktrees: &Path) {
    let Ok(entries) = std::fs::read_dir(worktrees) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|n| n.strip_prefix(CACHE_CHECK_PREFIX))
        else {
            continue;
        };
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !alive {
            println!("cache-check: removing stale {}", entry.path().display());
            remove_worktree(source, &entry.path());
        }
    }
}

/// Cleanup is best effort: a failure is reported and does not hide the
/// result of the command that ran before it.
fn remove_worktree(source: &Path, path: &Path) {
    let removed = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["worktree", "remove", "--force"])
        .arg(path)
        .output();
    if !matches!(removed, Ok(ref o) if o.status.success()) {
        eprintln!(
            "xtask: git could not remove the worktree {}",
            path.display()
        );
    }
    if path.exists()
        && let Err(e) = std::fs::remove_dir_all(path)
    {
        eprintln!("xtask: cannot delete {}: {e}", path.display());
    }
    if let Err(e) = repo::output(
        Command::new("git")
            .arg("-C")
            .arg(source)
            .args(["worktree", "prune"]),
    ) {
        eprintln!("xtask: {e}");
    }
}

fn delete_branch(main_root: &Path, branch: &str) {
    if let Err(e) = repo::output(
        Command::new("git")
            .arg("-C")
            .arg(main_root)
            .args(["branch", "-D", branch]),
    ) {
        eprintln!("xtask: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_task_ids_are_accepted() {
        for t in ["T05", "T05-modeline", "T19_gles", "cache.v2"] {
            assert!(validate_task(t).is_ok(), "{t}");
        }
    }

    #[test]
    fn unsafe_task_ids_are_rejected() {
        for t in ["", "-x", ".hidden", "a/b", "a..b", "T05 x", "../up"] {
            assert!(
                matches!(validate_task(t), Err(WorktreeError::BadTask(_))),
                "{t:?}"
            );
        }
    }

    #[test]
    fn worktrees_live_under_dot_worktrees() {
        assert_eq!(
            task_path(Path::new("/repo"), "T05-modeline"),
            PathBuf::from("/repo/.worktrees/T05-modeline")
        );
    }

    #[test]
    fn clone_uses_copy_on_write() {
        let cmd = clone_command(Path::new("/a/target"), Path::new("/b/target"));
        let args: Vec<_> = cmd.get_args().collect();
        if cfg!(target_os = "macos") {
            assert_eq!(args, ["-cRp", "/a/target", "/b/target"]);
        } else {
            assert_eq!(args, ["-a", "--reflink=auto", "/a/target", "/b/target"]);
        }
    }

    #[test]
    fn this_checkout_builds_into_its_own_target() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let root = std::fs::canonicalize(root).unwrap();
        let policy = LoadPolicy::resolve(false, None, None).unwrap();
        let outside = std::env::var_os("CARGO_TARGET_DIR").is_some();
        let result = check_target_dir(&root, &policy);
        if outside {
            assert!(matches!(result, Err(WorktreeError::TargetOutside { .. })));
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        repo::output(
            Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args),
        )
        .unwrap();
    }

    /// A tiny repository with one crate on branch `main` and a warm-looking
    /// `target/` that `cp` cannot read, so the clone fails.
    #[cfg(unix)]
    fn repo_with_unreadable_target(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/lib.rs"), "").unwrap();
        std::fs::write(dir.join(".gitignore"), "/target/\n/.worktrees/\n").unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", "init"]);
        let locked = dir.join("target/locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::write(locked.join("file"), "x").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_clone_removes_the_worktree_and_the_branch() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("xtask-wt-{}", std::process::id()));
        let dir = {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::canonicalize(&dir).unwrap()
        };
        repo_with_unreadable_target(&dir);
        let policy = LoadPolicy::resolve(false, None, None).unwrap();
        let result = create(&dir, "T99-x", &policy);
        std::fs::set_permissions(
            dir.join("target/locked"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(
            matches!(result, Err(WorktreeError::Clone { .. })),
            "{result:?}"
        );
        assert!(!dir.join(".worktrees/T99-x").exists());
        let branches =
            repo::output(Command::new("git").arg("-C").arg(&dir).args(["branch"])).unwrap();
        assert!(!branches.contains("mvp/T99-x"), "{branches}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn same_path_resolves_links() {
        let tmp = std::env::temp_dir();
        assert!(same_path(&tmp, &tmp.join(".")));
        assert!(!same_path(&tmp, &tmp.join("no-such-dir")));
    }

    #[test]
    fn artifacts_are_split_into_dependencies_and_workspace() {
        let msgs = [
            r#"{"reason":"compiler-artifact","package_id":"registry+https://github.com/rust-lang/crates.io-index#serde_json@1.0.154","target":{"name":"serde_json"},"fresh":true}"#,
            r#"{"reason":"compiler-artifact","package_id":"registry+https://github.com/rust-lang/crates.io-index#itoa@1.0.19","target":{"name":"itoa"},"fresh":false}"#,
            r#"{"reason":"compiler-artifact","package_id":"path+file:///ws/xtask#0.1.0","target":{"name":"xtask"},"fresh":false}"#,
            r#"{"reason":"compiler-artifact","package_id":"path+file:///ws/crates/neptron-types#0.1.0","target":{"name":"neptron_types"},"fresh":true}"#,
            r#"{"reason":"build-script-executed","package_id":"registry+x#serde@1"}"#,
            r#"{"reason":"build-finished","success":true}"#,
            "not json",
        ]
        .join("\n");
        let c = count_artifacts(&msgs);
        assert_eq!(c.deps_fresh, 1);
        assert_eq!(c.deps_rebuilt.len(), 1);
        assert!(c.deps_rebuilt[0].contains("itoa"));
        assert_eq!(c.workspace_fresh, 1);
        assert_eq!(c.workspace_rebuilt, 1);
    }
}
