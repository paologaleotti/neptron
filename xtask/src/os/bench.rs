//! The bench commands for a PC or a Raspberry Pi that runs the development
//! variant of its image: `os push` builds the frontend, the launcher and
//! `neptronctl` in the bench profile and puts them in the hot folder of the
//! machine over SSH, `os logs` reads the log, and `os ssh` runs a command
//! there.
//!
//! Every connection uses the SSH agent and key of the user, never a
//! password (`BatchMode=yes`), and a known-hosts file of its own in the
//! cache directory, so the user's own file does not change.

use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::Instant;

use super::host::{self, Cache, MachineLock};
use super::{Board, OsError, build, cross};
use crate::repo::Repo;

/// The folder of the hot binaries on the PC.
pub const HOT_BIN: &str = "/state/hot/bin";
/// The folder of the hot data files on the PC.
pub const HOT_DATA: &str = "/state/hot/share/neptron";
/// The current log file that svlogd keeps on the PC.
pub const LOG: &str = "/state/log/neptron/current";
/// The syslog file on the PC, in a RAM folder. The storage script writes
/// its lines there with the tag `neptron-storage`; the console of a PC is
/// not kept.
pub const SYSLOG: &str = "/var/log/messages";
/// The storage log on the PC: the lines of the storage script of the last
/// boots, which it writes to the state partition.
pub const STORAGE_LOG: &str = "/state/log/storage.log";
/// The known-hosts file of the bench, in the cache directory.
pub const KNOWN_HOSTS: &str = "bench_known_hosts";

/// The binaries of a push: package, features, binary.
pub const PUSHED: [(&str, &str, &str); 3] = [
    ("neptron", "kms,control", "neptron"),
    ("neptron-launch", "", "neptron-launch"),
    ("neptronctl", "kms", "neptronctl"),
];

/// The data folders of `--data`, from `data/` in the checkout.
pub const DATA_DIRS: [&str; 5] = ["monitors", "cores", "systems", "skins", "hardware"];

/// The PC and how to reach it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Host name or address.
    pub host: String,
    /// The user on the PC.
    pub user: String,
    /// The SSH port.
    pub port: u16,
}

/// A bench command with its options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bench {
    /// `os push`.
    Push {
        /// The PC or the Pi.
        target: Target,
        /// The image on the machine: its architecture is the target of
        /// the build.
        board: Board,
        /// The Buildroot output whose toolchain and sysroot the build
        /// uses: the output of the development image on the machine.
        output: String,
        /// Also replace the data files.
        data: bool,
        /// Restart the launcher and the frontend after the copy.
        restart: bool,
    },
    /// `os logs`.
    Logs {
        /// The PC.
        target: Target,
        /// Follow the log as it grows.
        follow: bool,
    },
    /// `os ssh`.
    Ssh {
        /// The PC.
        target: Target,
        /// The command and its words.
        command: Vec<String>,
    },
}

impl Bench {
    /// Parses the arguments of `os push`, `os logs` or `os ssh`.
    pub fn parse(subcommand: &str, args: &[&str]) -> Result<Self, OsError> {
        let mut host = None;
        let mut user = DEFAULT_USER.to_owned();
        let mut port = DEFAULT_PORT;
        let mut data = false;
        let mut restart = true;
        let mut follow = false;
        let mut board = Board::Pc;
        let mut output = None;
        let mut command = Vec::new();
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            let mut value = || {
                it.next()
                    .copied()
                    .ok_or_else(|| OsError::Usage(format!("{arg} needs a value")))
            };
            match (subcommand, arg) {
                (_, "--host") => host = Some(value()?.to_owned()),
                (_, "--user") => user = value()?.to_owned(),
                (_, "--port") => {
                    let text = value()?;
                    port = match text.parse::<u16>() {
                        Ok(p) if p > 0 => p,
                        _ => {
                            return Err(OsError::BadNumber {
                                flag: "--port",
                                value: text.to_owned(),
                            });
                        }
                    };
                }
                (_, "--arch") => {
                    board = Board::parse(value()?)?;
                    if board == Board::QemuAarch64 {
                        return Err(OsError::Usage(
                            "the bench commands are for the x86_64 PC image and the rpi-arm64 \
                             Pi image"
                                .to_owned(),
                        ));
                    }
                }
                ("push", "--output") => output = Some(value()?.to_owned()),
                ("push", "--data") => data = true,
                ("push", "--no-restart") => restart = false,
                ("logs", "--follow") => follow = true,
                ("ssh", "--") => {
                    command = it.map(|s| (*s).to_owned()).collect();
                    break;
                }
                _ => return Err(OsError::Usage(format!("unknown argument {arg:?}"))),
            }
        }
        let host = host.ok_or_else(|| OsError::Usage("--host is needed".to_owned()))?;
        if host.starts_with('-') || user.starts_with('-') || user.contains('@') {
            return Err(OsError::Usage(format!(
                "the host {host:?} or the user {user:?} is not a name"
            )));
        }
        let target = Target { host, user, port };
        match subcommand {
            "push" => Ok(Self::Push {
                target,
                board,
                output: output.unwrap_or_else(|| board.push_output()),
                data,
                restart,
            }),
            "logs" => Ok(Self::Logs { target, follow }),
            "ssh" if command.is_empty() => {
                Err(OsError::Usage("give the command after --".to_owned()))
            }
            "ssh" => Ok(Self::Ssh { target, command }),
            other => Err(OsError::Usage(format!("unknown os subcommand {other:?}"))),
        }
    }
}

/// The options of every `ssh` call: key or agent only, and the
/// known-hosts file of the bench.
pub fn ssh_options(known_hosts: &Path) -> Vec<String> {
    [
        "-o",
        "BatchMode=yes",
        "-o",
        "PasswordAuthentication=no",
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .chain([format!("UserKnownHostsFile={}", known_hosts.display())])
    .collect()
}

/// The `ssh` arguments that run `remote` on the PC.
pub fn ssh_args(target: &Target, known_hosts: &Path, remote: &str) -> Vec<String> {
    let mut args = ssh_options(known_hosts);
    args.extend([
        "-p".to_owned(),
        target.port.to_string(),
        format!("{}@{}", target.user, target.host),
        "--".to_owned(),
        remote.to_owned(),
    ]);
    args
}

/// The shell command on the PC that writes its standard input to `path`.
/// Files go over the SSH connection itself: Dropbear has no SFTP server,
/// which the `scp` of current OpenSSH needs.
pub fn receive_file(path: &str) -> String {
    format!("cat > {}", super::qemu::shell_quote(path))
}

/// The shell command on the PC that unpacks a tar stream on its standard
/// input into `dir`.
pub fn receive_tree(dir: &str) -> String {
    format!("tar -x -f - -C {}", super::qemu::shell_quote(dir))
}

/// The shell script on the PC that makes the folders of a push.
pub fn prepare_script(data: bool) -> String {
    let mut script = format!("set -e; mkdir -p {HOT_BIN}");
    if data {
        script.push_str(&format!("; rm -rf {HOT_DATA}.new; mkdir -p {HOT_DATA}.new"));
    }
    script
}

/// The shell script on the PC that puts the copied files in place. Each
/// binary was copied as `.<name>.new` and is renamed over the old one, so a
/// launcher that starts during the push sees the old file or the new one,
/// never half a file.
pub fn install_script(data: bool) -> String {
    let mut script = format!("set -e; cd {HOT_BIN}");
    for (_, _, bin) in PUSHED {
        script.push_str(&format!("; chmod 755 .{bin}.new; mv -f .{bin}.new {bin}"));
    }
    if data {
        script.push_str(&format!(
            "; rm -rf {HOT_DATA}.old; if [ -d {HOT_DATA} ]; then mv {HOT_DATA} {HOT_DATA}.old; fi; \
             mv {HOT_DATA}.new {HOT_DATA}; rm -rf {HOT_DATA}.old"
        ));
    }
    script.push_str(
        "; sync; if [ -e /state/hot/DISABLED ]; then \
         echo 'note: /state/hot/DISABLED exists, so the image runs its own binaries'; fi",
    );
    script
}

/// The shell script on the PC that restarts the launcher and the frontend
/// with the hold file: the launch script waits while the file exists, so
/// init cannot start the old launcher in the gap, and the next start reads
/// the hot folder again. The launcher forwards SIGTERM to the frontend,
/// which ends its run as a quit. A frontend still there after 10 s gets
/// SIGKILL as a fallback, with a line that says so.
pub const RESTART_SCRIPT: &str = "touch /run/neptron/hold; \
    kill $(pidof neptron-launch) 2>/dev/null; \
    n=0; while pidof neptron-launch > /dev/null && [ $n -lt 100 ]; do usleep 100000; n=$((n + 1)); done; \
    if pidof neptron > /dev/null; then \
    echo 'fallback: the frontend did not end on SIGTERM in 10 s; SIGKILL'; kill -KILL $(pidof neptron); \
    n=0; while pidof neptron-launch > /dev/null && [ $n -lt 50 ]; do usleep 100000; n=$((n + 1)); done; fi; \
    rm -f /run/neptron/hold; \
    if pidof neptron-launch > /dev/null; then echo 'the launcher did not stop'; exit 1; fi; \
    echo 'the launcher and the frontend start again'";

/// The shell script on the PC that prints the log.
pub fn logs_script(follow: bool) -> String {
    if follow {
        format!("tail -n 200 -f {LOG}")
    } else {
        format!(
            "echo '== {LOG}'; tail -n 400 {LOG}; \
             echo '== storage lines of {SYSLOG}'; grep neptron-storage {SYSLOG}; \
             echo '== {STORAGE_LOG}'; tail -n 100 {STORAGE_LOG}; \
             echo '== /run/neptron/storage.toml'; cat /run/neptron/storage.toml; \
             echo '== dmesg'; dmesg | tail -n 100; \
             echo '== /state/hot'; ls -l {HOT_BIN} 2>/dev/null; ls /state/hot 2>/dev/null"
        )
    }
}

/// `cargo xtask os push | logs | ssh`.
pub fn run(subcommand: &str, args: &[&str]) -> Result<ExitCode, OsError> {
    let bench = Bench::parse(subcommand, args)?;
    let cache = Cache::from_env()?;
    std::fs::create_dir_all(&cache.root).map_err(|source| OsError::Io {
        path: cache.root.clone(),
        source,
    })?;
    let known_hosts = cache.root.join(KNOWN_HOSTS);
    host::require_tool("ssh", "install OpenSSH")?;
    match bench {
        Bench::Ssh { target, command } => {
            let remote = command
                .iter()
                .map(|w| super::qemu::shell_quote(w))
                .collect::<Vec<_>>()
                .join(" ");
            remote_run(&target, &known_hosts, &remote)
        }
        Bench::Logs { target, follow } => remote_run(&target, &known_hosts, &logs_script(follow)),
        Bench::Push {
            target,
            board,
            output,
            data,
            restart,
        } => push(&target, board, &output, &known_hosts, &cache, data, restart),
    }
}

fn push(
    target: &Target,
    board: Board,
    output: &str,
    known_hosts: &Path,
    cache: &Cache,
    data: bool,
    restart: bool,
) -> Result<ExitCode, OsError> {
    let repo = Repo::discover()?;
    host::require_tool("docker", "install Docker (macOS: Docker Desktop)")?;
    super::check_docker_daemon()?;
    let dockerfile = repo.root.join("Dockerfile");
    let text = std::fs::read_to_string(&dockerfile).map_err(|source| OsError::Io {
        path: dockerfile.clone(),
        source,
    })?;
    let export = repo.root.join("target").join("os-push");
    std::fs::create_dir_all(&export).map_err(|source| OsError::Io {
        path: export.clone(),
        source,
    })?;
    let container = build::container_image(&text);
    let start = Instant::now();
    {
        // The build reads the toolchains in the Buildroot output, which an
        // os build changes.
        let _lock = MachineLock::take(&cache.build_lock(), "Buildroot build")?;
        build::build_container(&build::container_build(&container), &dockerfile)?;
        for (package, features, bin) in PUSHED {
            let plan = cross::CrossPlan {
                arch: board.arch(),
                output: output.to_owned(),
                container: container.clone(),
                repo: repo.root.clone(),
                export: export.clone(),
                package: package.to_owned(),
                features: features.to_owned(),
                example: String::new(),
                lint: false,
                filtered_linker: true,
            };
            for step in plan.bench_steps(bin) {
                super::run_step(Command::new("docker").args(step))?;
            }
        }
    }
    eprintln!(
        "xtask: the bench build took {} s",
        start.elapsed().as_secs()
    );
    remote_step(target, known_hosts, &prepare_script(data))?;
    for (_, _, bin) in PUSHED {
        let local = export.join(bin);
        let remote = format!("{HOT_BIN}/.{bin}.new");
        let file = std::fs::File::open(&local).map_err(|source| OsError::Io {
            path: local.clone(),
            source,
        })?;
        let mut ssh = Command::new("ssh");
        ssh.args(ssh_args(target, known_hosts, &receive_file(&remote)))
            .stdin(file);
        super::run_step(&mut ssh)?;
    }
    if data {
        send_data(target, known_hosts, &repo.root.join("data"))?;
    }
    remote_step(target, known_hosts, &install_script(data))?;
    if restart {
        remote_step(target, known_hosts, RESTART_SCRIPT)?;
    }
    eprintln!(
        "xtask: pushed {} to {}@{}:{HOT_BIN} in {} s",
        PUSHED.map(|(_, _, b)| b).join(", "),
        target.user,
        target.host,
        start.elapsed().as_secs()
    );
    Ok(ExitCode::SUCCESS)
}

/// Sends the data folders as a tar stream into the new hot data folder.
fn send_data(target: &Target, known_hosts: &Path, data: &Path) -> Result<(), OsError> {
    let mut tar = Command::new("tar");
    tar.arg("-c")
        .arg("-f")
        .arg("-")
        .arg("-C")
        .arg(data)
        .args(DATA_DIRS)
        // macOS tar adds AppleDouble files for extended attributes unless
        // this is set.
        .env("COPYFILE_DISABLE", "1")
        .stdout(std::process::Stdio::piped());
    let mut child = super::spawn(&mut tar)?;
    let Some(stream) = child.stdout.take() else {
        return Err(OsError::Usage("tar gave no output stream".to_owned()));
    };
    let remote = receive_tree(&format!("{HOT_DATA}.new"));
    let mut ssh = Command::new("ssh");
    ssh.args(ssh_args(target, known_hosts, &remote))
        .stdin(stream);
    let sent = super::run_step(&mut ssh);
    let packed = child.wait().map_err(|source| OsError::Wait { source })?;
    sent?;
    if packed.success() {
        Ok(())
    } else {
        Err(OsError::StepFailed {
            cmd: crate::repo::describe(&tar),
            status: packed,
        })
    }
}

fn remote_step(target: &Target, known_hosts: &Path, remote: &str) -> Result<(), OsError> {
    super::run_step(Command::new("ssh").args(ssh_args(target, known_hosts, remote)))
}

fn remote_run(target: &Target, known_hosts: &Path, remote: &str) -> Result<ExitCode, OsError> {
    let mut cmd = Command::new("ssh");
    cmd.args(ssh_args(target, known_hosts, remote));
    let status = cmd
        .status()
        .map_err(|source| super::spawn_error(&cmd, source))?;
    Ok(ExitCode::from(
        u8::try_from(status.code().unwrap_or(1)).unwrap_or(u8::MAX),
    ))
}

const DEFAULT_USER: &str = "root";
const DEFAULT_PORT: u16 = 22;

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            host: "192.168.1.50".to_owned(),
            user: "root".to_owned(),
            port: 22,
        }
    }

    #[test]
    fn push_needs_a_host_and_takes_its_options() {
        assert!(Bench::parse("push", &[]).is_err());
        assert_eq!(
            Bench::parse("push", &["--host", "pc"]).unwrap(),
            Bench::Push {
                target: Target {
                    host: "pc".to_owned(),
                    user: "root".to_owned(),
                    port: 22
                },
                board: Board::Pc,
                output: "x86_64".to_owned(),
                data: false,
                restart: true,
            }
        );
        assert_eq!(
            Bench::parse(
                "push",
                &[
                    "--host",
                    "pc",
                    "--port",
                    "2222",
                    "--user",
                    "me",
                    "--data",
                    "--no-restart",
                    "--arch",
                    "x86_64"
                ]
            )
            .unwrap(),
            Bench::Push {
                target: Target {
                    host: "pc".to_owned(),
                    user: "me".to_owned(),
                    port: 2222
                },
                board: Board::Pc,
                output: "x86_64".to_owned(),
                data: true,
                restart: false,
            }
        );
        for bad in [
            &["--host", "pc", "--follow"][..],
            &["--host", "pc", "--arch", "aarch64"],
            &["--host", "pc", "--port", "0"],
            &["--host", "-oProxyCommand=x"],
            &["--host", "pc", "--user", "a@b"],
        ] {
            assert!(Bench::parse("push", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_push_to_a_pi_builds_with_the_toolchain_of_the_pi_output() {
        let Bench::Push { board, output, .. } =
            Bench::parse("push", &["--host", "pi", "--arch", "rpi-arm64"]).unwrap()
        else {
            panic!("not push");
        };
        assert_eq!((board, output.as_str()), (Board::RpiArm64, "rpi-arm64-dev"));
        let Bench::Push { output, .. } = Bench::parse(
            "push",
            &[
                "--host",
                "pi",
                "--arch",
                "rpi-arm64",
                "--output",
                "rpi-arm64",
            ],
        )
        .unwrap() else {
            panic!("not push");
        };
        assert_eq!(output, "rpi-arm64");
        assert!(Bench::parse("logs", &["--host", "pi", "--arch", "rpi-arm64"]).is_ok());
        assert!(Bench::parse("logs", &["--host", "pi", "--output", "x"]).is_err());
    }

    #[test]
    fn logs_and_ssh_parse() {
        assert_eq!(
            Bench::parse("logs", &["--host", "pc", "--follow"]).unwrap(),
            Bench::Logs {
                target: Target {
                    host: "pc".to_owned(),
                    user: "root".to_owned(),
                    port: 22
                },
                follow: true
            }
        );
        assert!(Bench::parse("ssh", &["--host", "pc"]).is_err());
        let Bench::Ssh { command, .. } =
            Bench::parse("ssh", &["--host", "pc", "--", "ls", "-l", "/state"]).unwrap()
        else {
            panic!("not ssh");
        };
        assert_eq!(command, ["ls", "-l", "/state"]);
    }

    #[test]
    fn every_connection_refuses_passwords_and_keeps_its_own_known_hosts() {
        let known = Path::new("/c/bench_known_hosts");
        let ssh = ssh_args(&target(), known, "uptime").join(" ");
        assert_eq!(
            ssh,
            "-o BatchMode=yes -o PasswordAuthentication=no -o StrictHostKeyChecking=accept-new \
             -o UserKnownHostsFile=/c/bench_known_hosts -p 22 root@192.168.1.50 -- uptime"
        );
        assert_eq!(
            receive_file("/state/hot/bin/.neptron.new"),
            "cat > /state/hot/bin/.neptron.new"
        );
        assert_eq!(receive_file("/a b"), "cat > '/a b'");
        assert_eq!(
            receive_tree("/state/hot/share/neptron.new"),
            "tar -x -f - -C /state/hot/share/neptron.new"
        );
    }

    #[test]
    fn the_install_renames_each_binary_over_the_old_one() {
        let script = install_script(false);
        for bin in ["neptron", "neptron-launch", "neptronctl"] {
            assert!(
                script.contains(&format!("chmod 755 .{bin}.new; mv -f .{bin}.new {bin}")),
                "{script}"
            );
        }
        assert!(script.starts_with("set -e; cd /state/hot/bin"), "{script}");
        assert!(!script.contains("share/neptron"), "{script}");
        let with_data = install_script(true);
        assert!(
            with_data.contains("mv /state/hot/share/neptron.new /state/hot/share/neptron"),
            "{with_data}"
        );
        assert!(prepare_script(true).contains("mkdir -p /state/hot/share/neptron.new"));
    }

    #[test]
    fn the_restart_holds_the_launcher_until_the_old_one_is_gone() {
        let hold = RESTART_SCRIPT.find("touch /run/neptron/hold").unwrap();
        let kill = RESTART_SCRIPT.find("kill $(pidof neptron-launch)").unwrap();
        let release = RESTART_SCRIPT.find("rm -f /run/neptron/hold").unwrap();
        let force = RESTART_SCRIPT.find("kill -KILL $(pidof neptron)").unwrap();
        assert!(
            hold < kill && kill < force && force < release,
            "{RESTART_SCRIPT}"
        );
        assert!(RESTART_SCRIPT.ends_with("start again'"), "{RESTART_SCRIPT}");
        assert!(logs_script(true).contains("tail -n 200 -f /state/log/neptron/current"));
        assert!(logs_script(false).contains("dmesg"));
        assert!(logs_script(false).contains("grep neptron-storage /var/log/messages"));
        assert!(logs_script(false).contains("cat /run/neptron/storage.toml"));
        assert!(logs_script(false).contains("tail -n 100 /state/log/storage.log"));
    }
}
