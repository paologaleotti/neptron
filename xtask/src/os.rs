//! The OS images: the development container, the Buildroot build of the QEMU
//! targets, and the QEMU guest that tier T3 runs in.
//!
//! `os build` runs Buildroot in the container and copies the images to the
//! machine cache. `qemu` boots those images, runs one command in the guest
//! and returns its exit code, or gives an interactive console.

mod arch;
mod build;
mod cross;
#[cfg(test)]
mod defconfig;
mod guest;
mod host;
mod qemu;
mod transcript;

pub use arch::Arch;

use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::{Child, Command, ExitCode};

use crate::load::LoadPolicy;
use crate::repo::{self, Repo, RepoError};
use build::{BuildArgs, BuildPlan};
use host::{Accel, Cache, MachineLock};
use qemu::{GuestJob, Job, QemuArgs, QemuPlan};

/// An error of `os build` or `qemu`.
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
    /// Waiting for a child process failed.
    #[error("cannot wait for QEMU: {source}")]
    Wait {
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
    /// The images directory lacks a file that QEMU needs.
    #[error(
        "{missing} does not exist; build the image with `cargo xtask os build --arch {arch}`, \
         or set NEPTRON_OS_IMAGES (or --images) to a directory with {kernel} and rootfs.ext4"
    )]
    NoImage {
        /// The missing file.
        missing: PathBuf,
        /// The target.
        arch: &'static str,
        /// The kernel file name of the target.
        kernel: &'static str,
    },
    /// A signal handler could not be installed.
    #[error("cannot handle signal {signal}: {source}")]
    Signal {
        /// The signal number.
        signal: i32,
        /// The error of the operating system.
        source: io::Error,
    },
    /// A stop signal arrived. QEMU is stopped.
    #[error("stopped by a signal; QEMU is stopped")]
    Stopped,
    /// QEMU gave no console output stream.
    #[error("QEMU has no console output")]
    NoConsole,
    /// The guest did not finish in time. QEMU is stopped.
    #[error("the guest did not stop in {secs} s; console log in {log}\n{tail}")]
    Timeout {
        /// The time limit.
        secs: u64,
        /// The console log.
        log: PathBuf,
        /// The last console lines.
        tail: String,
    },
    /// QEMU exited before the guest reported an exit code.
    #[error("the guest stopped before the command ended; console log in {log}\n{tail}")]
    NoResult {
        /// The console log.
        log: PathBuf,
        /// The last console lines.
        tail: String,
    },
    /// A repository path or a git command failed.
    #[error(transparent)]
    Repo(#[from] RepoError),
}

/// The usage text of `os build`, `qemu` and `gles-compare`.
pub const USAGE: &str = "\
usage: cargo xtask os build [--arch aarch64|x86_64] [--jobs N] [-- make targets...]
       cargo xtask qemu [--arch A] [--images DIR] [--timeout SECS] [--share DIR]
                        [--script FILE | -- command...]
       cargo xtask gles-compare [--arch A] [--timeout SECS]";

/// `cargo xtask os <subcommand>`.
pub fn run_os(args: &[&str]) -> Result<ExitCode, OsError> {
    match args.split_first() {
        Some((&"build", rest)) => build(rest),
        _ => Err(OsError::Usage("unknown os subcommand".to_owned())),
    }
}

/// `cargo xtask qemu`: boots the image of the target and runs the job.
pub fn run_qemu(args: &[&str]) -> Result<ExitCode, OsError> {
    let args = QemuArgs::parse(args, Arch::default_for(std::env::consts::ARCH))?;
    let mut out = std::io::stdout().lock();
    boot(&args, &mut out).map(|code| match code {
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX)),
        None => ExitCode::SUCCESS,
    })
}

/// Boots the image of the target and runs the job. The command output goes
/// to `out`. Returns the exit code of the guest command, or `None` for an
/// interactive session that ended well.
fn boot(args: &QemuArgs, out: &mut dyn std::io::Write) -> Result<Option<i32>, OsError> {
    let repo = Repo::discover()?;
    let cache = Cache::from_env()?;
    let images = match (&args.images, std::env::var_os(host::IMAGES_ENV)) {
        (Some(dir), _) => dir.clone(),
        (None, Some(dir)) if !dir.is_empty() => PathBuf::from(dir),
        _ => cache.images(args.arch),
    };
    let accel = Accel::choose(
        args.arch,
        std::env::consts::OS,
        std::env::consts::ARCH,
        host::kvm_available(),
    );
    let required = [
        images.join(args.arch.kernel_image()),
        images.join(qemu::ROOTFS),
    ];
    for file in required {
        if !file.is_file() {
            return Err(OsError::NoImage {
                missing: file,
                arch: args.arch.name(),
                kernel: args.arch.kernel_image(),
            });
        }
    }
    host::require_tool(
        args.arch.qemu_binary(),
        "install QEMU (macOS: brew install qemu; Debian: apt install qemu-system)",
    )?;
    if accel == Accel::Tcg {
        eprintln!(
            "xtask: no hardware acceleration for an {} guest on this host; \
             QEMU emulates the CPU and the run is slow",
            args.arch.name()
        );
    }

    let _lock = MachineLock::take(&cache.qemu_lock(), "QEMU guest")?;
    let run_dir = repo.root.join("target").join("qemu");
    remove_stale_scripts(&run_dir);
    let mut written = None;
    let job = match &args.job {
        Job::Interactive => None,
        Job::Script(path) => Some(path.clone()),
        Job::Command(words) => {
            let script = RunScript::write(&run_dir, &qemu::command_script(words))?;
            let path = script.path.clone();
            written = Some(script);
            Some(path)
        }
    }
    .map(|script| GuestJob {
        script,
        nonce: transcript::new_nonce(),
        timeout_secs: args.timeout.as_secs(),
    });
    let plan = QemuPlan {
        arch: args.arch,
        accel,
        images,
        job,
        share: args.share.clone(),
    };
    let policy = LoadPolicy::from_env().map_err(|e| OsError::Usage(e.to_string()))?;
    let mut cmd = under_load_policy(&policy, args.arch.qemu_binary());
    cmd.args(plan.args());
    let stop = guest::StopFlag::register()?;
    let result = match &plan.job {
        None => {
            guest::interactive(&mut cmd, &stop).map(|code| (code != ExitCode::SUCCESS).then_some(1))
        }
        Some(job) => {
            let log = guest::log_path(&repo.root.join("target"), args.arch.name());
            let deadline = args.timeout + qemu::HOST_MARGIN;
            guest::run(&mut cmd, &plan, &job.nonce, deadline, &log, &stop, out).map(|code| {
                eprintln!("xtask: guest command exited with {code}");
                Some(code)
            })
        }
    };
    drop(written);
    result
}

/// `cargo xtask gles-compare`: builds the `gles_compare` example of
/// `neptron-gfx` for the guest in the development container, then runs it
/// in the guest. The exit code is the one of the example.
pub fn run_gles_compare(args: &[&str]) -> Result<ExitCode, OsError> {
    let (arch, timeout) = parse_compare_args(args, Arch::default_for(std::env::consts::ARCH))?;
    let repo = Repo::discover()?;
    let cache = Cache::from_env()?;
    host::require_tool(
        "docker",
        "install Docker (macOS: Docker Desktop) to build for the guest",
    )?;
    check_docker_daemon()?;
    let dockerfile = repo.root.join("Dockerfile");
    let text = std::fs::read_to_string(&dockerfile).map_err(|source| OsError::Io {
        path: dockerfile.clone(),
        source,
    })?;
    let export = repo.root.join("target").join("gles-compare");
    let program = export.join(GLES_COMPARE_EXAMPLE);
    std::fs::create_dir_all(&export).map_err(|source| OsError::Io {
        path: export.clone(),
        source,
    })?;
    if let Err(source) = std::fs::remove_file(&program)
        && source.kind() != ErrorKind::NotFound
    {
        return Err(OsError::Io {
            path: program,
            source,
        });
    }
    let plan = cross::CrossPlan {
        arch,
        container: build::container_image(&text),
        repo: repo.root.clone(),
        export: export.clone(),
        package: "neptron-gfx".to_owned(),
        features: "gles".to_owned(),
        example: GLES_COMPARE_EXAMPLE.to_owned(),
    };
    {
        // The build reads the toolchains in the Buildroot output, which an
        // os build changes.
        let _lock = MachineLock::take(&cache.build_lock(), "Buildroot build")?;
        build::build_container(&build::container_build(&plan.container), &dockerfile)?;
        for step in plan.steps() {
            run_step(Command::new("docker").args(step))?;
        }
    }
    let share = export
        .to_str()
        .ok_or_else(|| OsError::Usage(format!("the path {} is not UTF-8", export.display())))?;
    let timeout = timeout.to_string();
    let guest_program = format!("/mnt/host/{GLES_COMPARE_EXAMPLE}");
    run_qemu(&[
        "--arch",
        arch.name(),
        "--timeout",
        &timeout,
        "--share",
        share,
        "--",
        &guest_program,
    ])
}

const GLES_COMPARE_EXAMPLE: &str = "gles_compare";
/// Software GL draws the largest scenes in seconds each.
const GLES_COMPARE_TIMEOUT_SECS: u64 = 600;

fn parse_compare_args(args: &[&str], default_arch: Arch) -> Result<(Arch, u64), OsError> {
    let mut arch = default_arch;
    let mut timeout = GLES_COMPARE_TIMEOUT_SECS;
    let mut it = args.iter();
    while let Some(&arg) = it.next() {
        let value = it
            .next()
            .copied()
            .ok_or_else(|| OsError::Usage(format!("{arg} needs a value")))?;
        match arg {
            "--arch" => arch = Arch::parse(value)?,
            "--timeout" => {
                timeout = match value.parse::<u64>() {
                    Ok(secs) if secs > 0 => secs,
                    _ => {
                        return Err(OsError::BadNumber {
                            flag: "--timeout",
                            value: value.to_owned(),
                        });
                    }
                }
            }
            other => return Err(OsError::Usage(format!("unknown argument {other:?}"))),
        }
    }
    Ok((arch, timeout))
}

/// A command script in `target/qemu` that is removed when it drops.
#[derive(Debug)]
struct RunScript {
    path: PathBuf,
}

impl RunScript {
    fn write(run_dir: &std::path::Path, text: &str) -> Result<Self, OsError> {
        let path = run_dir.join(format!(
            "{RUN_SCRIPT_PREFIX}{}{RUN_SCRIPT_SUFFIX}",
            std::process::id()
        ));
        write_file(&path, text)?;
        Ok(Self { path })
    }
}

impl Drop for RunScript {
    fn drop(&mut self) {
        // The file is in target/ and the next run removes it anyway.
        let _ = std::fs::remove_file(&self.path);
    }
}

const RUN_SCRIPT_PREFIX: &str = "run-";
const RUN_SCRIPT_SUFFIX: &str = ".sh";

/// Removes the command scripts that runs killed by SIGKILL left. The caller
/// holds the QEMU lock, so no other run uses them.
fn remove_stale_scripts(run_dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(run_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(RUN_SCRIPT_PREFIX) && name.ends_with(RUN_SCRIPT_SUFFIX) {
            // A file that cannot be removed now is tried again on the next run.
            let _ = std::fs::remove_file(entry.path());
        }
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

/// A command for `program` under the machine-load rules: `taskpolicy -c
/// utility` on macOS.
fn under_load_policy(policy: &LoadPolicy, program: &str) -> Command {
    let mut cmd = if policy.taskpolicy {
        let mut c = Command::new("taskpolicy");
        c.args(["-c", "utility", program]);
        c
    } else {
        Command::new(program)
    };
    repo::without_git_location(&mut cmd);
    cmd
}

fn spawn(cmd: &mut Command) -> Result<Child, OsError> {
    cmd.spawn().map_err(|source| spawn_error(cmd, source))
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

fn write_file(path: &std::path::Path, text: &str) -> Result<(), OsError> {
    let io_err = |source| OsError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_err)?;
    }
    std::fs::write(path, text).map_err(io_err)
}

fn exit_code(success: bool) -> ExitCode {
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qemu_runs_under_taskpolicy_on_macos() {
        let policy = LoadPolicy::resolve(true, None, None).unwrap();
        let cmd = under_load_policy(&policy, "qemu-system-aarch64");
        assert_eq!(cmd.get_program(), "taskpolicy");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, ["-c", "utility", "qemu-system-aarch64"]);
    }

    #[test]
    fn qemu_runs_directly_elsewhere() {
        let policy = LoadPolicy::resolve(false, None, None).unwrap();
        let cmd = under_load_policy(&policy, "qemu-system-x86_64");
        assert_eq!(cmd.get_program(), "qemu-system-x86_64");
    }

    #[test]
    fn a_missing_program_is_reported_as_missing() {
        let mut cmd = Command::new("neptron-no-such-program");
        let err = run_step(&mut cmd).unwrap_err();
        assert!(
            matches!(&err, OsError::ToolMissing { tool, .. } if tool == "neptron-no-such-program"),
            "{err}"
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_run_script_is_removed_when_it_drops() {
        let dir = temp_dir("run-script");
        let script = RunScript::write(&dir, "true\n").unwrap();
        let path = script.path.clone();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "true\n");
        drop(script);
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stale_run_scripts_are_removed_and_other_files_kept() {
        let dir = temp_dir("stale");
        for name in ["run-1.sh", "run-22.sh", "aarch64-console.log", "notes.sh"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        remove_stale_scripts(&dir);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["aarch64-console.log", "notes.sh"]);
        remove_stale_scripts(&dir.join("missing"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn gles_compare_arguments_parse() {
        assert_eq!(
            parse_compare_args(&[], Arch::Aarch64).unwrap(),
            (Arch::Aarch64, 600)
        );
        assert_eq!(
            parse_compare_args(&["--arch", "x86_64", "--timeout", "30"], Arch::Aarch64).unwrap(),
            (Arch::X86_64, 30)
        );
        for bad in [&["--timeout", "0"][..], &["--arch"], &["--images", "x"]] {
            assert!(parse_compare_args(bad, Arch::Aarch64).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn unknown_os_subcommand_is_a_usage_error() {
        assert!(matches!(run_os(&["flash"]), Err(OsError::Usage(_))));
        assert!(matches!(run_os(&[]), Err(OsError::Usage(_))));
    }
}
