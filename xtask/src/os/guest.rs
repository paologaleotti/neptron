//! Starts QEMU and follows the guest console until the command ends.
//!
//! QEMU never outlives xtask on purpose: on SIGTERM, SIGHUP or SIGINT xtask
//! stops QEMU before it exits. The guest also enforces its own deadline, so
//! it powers off in time even when xtask dies from SIGKILL.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use super::OsError;
use super::qemu::QemuPlan;
use super::transcript::Transcript;

/// Set when xtask receives SIGTERM, SIGHUP or SIGINT.
#[derive(Debug, Clone)]
pub struct StopFlag(Arc<AtomicBool>);

impl StopFlag {
    /// Registers the flag for SIGTERM, SIGHUP and SIGINT. The handlers only
    /// set the flag; the guest loops read it and stop QEMU.
    pub fn register() -> Result<Self, OsError> {
        let flag = Arc::new(AtomicBool::new(false));
        for signal in [
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
            signal_hook::consts::SIGINT,
        ] {
            signal_hook::flag::register(signal, Arc::clone(&flag))
                .map_err(|source| OsError::Signal { signal, source })?;
        }
        Ok(Self(flag))
    }

    /// Whether a stop signal has arrived.
    pub fn raised(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Boots the guest for an interactive session and waits for QEMU to exit.
pub fn interactive(qemu: &mut Command, stop: &StopFlag) -> Result<ExitCode, OsError> {
    eprintln!("xtask: interactive guest; press Ctrl-A then X to stop QEMU");
    let mut child = super::spawn(qemu)?;
    loop {
        if stop.raised() {
            stop_child(&mut child)?;
            return Err(OsError::Stopped);
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(super::exit_code(status.success())),
            Ok(None) => std::thread::sleep(POLL),
            Err(source) => {
                kill(&mut child);
                return Err(OsError::Wait { source });
            }
        }
    }
}

/// Runs `qemu`, writes the command output to `out`, writes the whole
/// console to `log`, and returns the exit code of the guest command. QEMU
/// is stopped after `deadline` or on a stop signal.
pub fn run(
    qemu: &mut Command,
    plan: &QemuPlan,
    nonce: &str,
    deadline: Duration,
    log: &Path,
    stop: &StopFlag,
    out: &mut dyn Write,
) -> Result<i32, OsError> {
    qemu.stdin(Stdio::null()).stdout(Stdio::piped());
    let start = Instant::now();
    let mut child = super::spawn(qemu)?;
    let Some(stdout) = child.stdout.take() else {
        kill(&mut child);
        return Err(OsError::NoConsole);
    };
    let (tx, rx) = mpsc::sync_channel::<String>(CONSOLE_QUEUE_LINES);
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut transcript = Transcript::new(nonce);
    let end = start + deadline;
    let mut announced = false;
    let ending = loop {
        if stop.raised() {
            break Ending::Stopped;
        }
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break Ending::TimedOut;
        }
        match rx.recv_timeout(left.min(POLL)) {
            Ok(line) => {
                if let Some(output) = transcript.feed(&line, start.elapsed()) {
                    // A closed output must not stop the guest run.
                    let _ = writeln!(out, "{output}");
                }
                if !announced && transcript.ready_after.is_some() {
                    announce_ready(&transcript, plan);
                    announced = true;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break Ending::Exited,
        }
    };
    if ending != Ending::Exited {
        kill(&mut child);
    }
    let waited = child.wait();
    // The reader ends when QEMU closes its standard output, which the wait
    // above guarantees; a panic in it only loses console lines.
    let _ = reader.join();
    write_log(log, &transcript)?;
    waited.map_err(|source| OsError::Wait { source })?;

    match (ending, transcript.exit_code) {
        (Ending::Stopped, _) => Err(OsError::Stopped),
        (Ending::TimedOut, _) => Err(OsError::Timeout {
            secs: deadline.as_secs(),
            log: log.to_path_buf(),
            tail: transcript.tail(TAIL_LINES),
        }),
        (Ending::Exited, Some(exit_code)) => Ok(exit_code),
        (Ending::Exited, None) => Err(OsError::NoResult {
            log: log.to_path_buf(),
            tail: transcript.tail(TAIL_LINES),
        }),
    }
}

/// The console log of the last run of `arch` in the checkout's `target/`.
pub fn log_path(target_dir: &Path, arch: &str) -> PathBuf {
    target_dir.join("qemu").join(format!("{arch}-console.log"))
}

/// Enough lines to show a kernel panic and its cause.
const TAIL_LINES: usize = 30;
/// The reader waits when the main loop falls this far behind.
const CONSOLE_QUEUE_LINES: usize = 1024;
/// How often the loops look at the stop flag.
const POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    Exited,
    TimedOut,
    Stopped,
}

fn announce_ready(transcript: &Transcript, plan: &QemuPlan) {
    if let Some(ready) = transcript.ready_after {
        let uptime = transcript
            .guest_uptime_ms
            .map_or_else(|| "unknown".to_owned(), |ms| format!("{ms} ms"));
        eprintln!(
            "xtask: {} guest ({}) ready after {} ms; guest uptime {uptime}",
            plan.arch.name(),
            plan.accel.name(),
            ready.as_millis(),
        );
    }
}

fn stop_child(child: &mut Child) -> Result<(), OsError> {
    kill(child);
    child
        .wait()
        .map(|_| ())
        .map_err(|source| OsError::Wait { source })
}

fn kill(child: &mut Child) {
    // QEMU may have exited already; then there is nothing to stop.
    let _ = child.kill();
}

fn write_log(log: &Path, transcript: &Transcript) -> Result<(), OsError> {
    let io_err = |source| OsError::Io {
        path: log.to_path_buf(),
        source,
    };
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir).map_err(io_err)?;
    }
    let mut text = transcript.lines.join("\n");
    text.push('\n');
    std::fs::write(log, text).map_err(io_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stop_flag_is_raised_by_sigterm() {
        let stop = StopFlag::register().unwrap();
        assert!(!stop.raised());
        signal_hook::low_level::raise(signal_hook::consts::SIGTERM).unwrap();
        assert!(stop.raised());
    }
}
