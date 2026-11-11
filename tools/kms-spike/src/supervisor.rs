//! The two processes of a confirmed run, so that Ctrl-C and SIGTERM end
//! with a restored display, without a signal handler.
//!
//! The process the user starts is the supervisor. It starts the worker (the
//! same program with `--worker`) in a new process group, so that Ctrl-C at
//! the terminal reaches only the supervisor. The worker writes every line,
//! and a heartbeat line every few frames, to a pipe that the supervisor
//! reads. When the supervisor dies (Ctrl-C, SIGTERM, SIGHUP, SIGKILL), the
//! pipe has no reader, the worker's next write fails, and the worker
//! restores the display and exits. Rust ignores SIGPIPE, so the failed write
//! is an error, not a signal.
//!
//! The supervisor also enforces the hard time limit from outside: a worker
//! that runs past it is killed, and the kernel then removes its
//! framebuffers.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::BorrowedFd;
use std::os::unix::fs::FileTypeExt as _;
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

/// The line the worker writes to show it is alive. The supervisor does not
/// print it.
pub const HEARTBEAT: &str = "kms-spike-heartbeat";

/// The worker's side of the pipe: every line, and a heartbeat, go to the
/// supervisor. A failed write means the supervisor is gone.
#[derive(Debug)]
pub struct Link<W: Write> {
    out: W,
    last_beat: Option<Instant>,
}

/// The supervisor no longer reads the worker's output.
#[derive(Debug)]
pub struct SupervisorGone(pub io::Error);

/// How often the worker writes a heartbeat. It bounds how long the worker
/// keeps a mode on the tube after the supervisor dies.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(100);

impl<W: Write> Link<W> {
    /// A link that writes to `out`.
    pub fn new(out: W) -> Link<W> {
        Link {
            out,
            last_beat: None,
        }
    }

    /// Writes one line.
    pub fn line(&mut self, text: &str) -> Result<(), SupervisorGone> {
        writeln!(self.out, "{text}")
            .and_then(|()| self.out.flush())
            .map_err(SupervisorGone)
    }

    /// Writes a heartbeat when the last one is at least
    /// [`HEARTBEAT_INTERVAL`] before `now`.
    pub fn heartbeat(&mut self, now: Instant) -> Result<(), SupervisorGone> {
        if self
            .last_beat
            .is_some_and(|t| now.saturating_duration_since(t) < HEARTBEAT_INTERVAL)
        {
            return Ok(());
        }
        self.last_beat = Some(now);
        self.line(HEARTBEAT)
    }
}

impl std::fmt::Display for SupervisorGone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the supervisor process is gone ({})", self.0)
    }
}

/// Writes a line to standard error. The worker uses it instead of
/// `eprintln!`, which panics when the write fails: after a hangup the
/// terminal is gone, and a panic there could come during the restore.
pub fn warn(text: &str) {
    // Nothing is left to report a failed write to.
    let _ = writeln!(io::stderr(), "{text}");
}

/// True when `fd` is a pipe. The worker's output must be the supervisor's
/// pipe: without it no failed write tells the worker that Ctrl-C ended
/// the run.
pub fn is_pipe(fd: BorrowedFd<'_>) -> io::Result<bool> {
    let file = File::from(fd.try_clone_to_owned()?);
    Ok(file.metadata()?.file_type().is_fifo())
}

/// How the supervised worker ended.
#[derive(Debug)]
pub enum Ended {
    /// The worker exited by itself.
    Exited(ExitStatus),
    /// The worker ran past the hard limit and was killed.
    Killed(ExitStatus),
}

/// Starts `command` as the worker in its own process group, forwards its
/// output lines to `out` (heartbeats left out) and waits for it. A worker
/// still running at `hard_limit` is killed.
pub fn supervise(
    mut command: Command,
    hard_limit: Duration,
    out: &mut dyn Write,
) -> io::Result<Ended> {
    let start = Instant::now();
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("the worker has no output pipe"))?;
    let (tx, rx) = mpsc::sync_channel::<String>(LINE_QUEUE);
    let reader = thread::spawn(move || forward(BufReader::new(stdout), &tx));
    let ended = loop {
        match rx.recv_timeout(CHECK_INTERVAL) {
            Ok(line) => writeln!(out, "{line}")?,
            Err(RecvTimeoutError::Timeout) => {}
            // The worker closed its output and is about to exit. The
            // supervisor needs no signal handling, so a plain wait is fine.
            Err(RecvTimeoutError::Disconnected) => thread::sleep(CHECK_INTERVAL),
        }
        if let Some(status) = child.try_wait()? {
            break Ended::Exited(status);
        }
        if start.elapsed() > hard_limit {
            break Ended::Killed(kill(&mut child)?);
        }
    };
    // The worker has exited, so the pipe reaches end of file and the reader
    // thread returns.
    let read = reader
        .join()
        .map_err(|_| io::Error::other("the output reader panicked"))?;
    for line in rx.try_iter() {
        writeln!(out, "{line}")?;
    }
    read?;
    Ok(ended)
}

/// How often the supervisor checks the worker and the clock.
const CHECK_INTERVAL: Duration = Duration::from_millis(100);

/// Lines in flight between the reader thread and the supervisor. The worker
/// writes a few lines per mode, so the queue never fills in practice; when
/// it does, the reader waits and the worker's pipe buffer takes the rest.
const LINE_QUEUE: usize = 256;

fn forward(input: impl BufRead, tx: &SyncSender<String>) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line == HEARTBEAT {
            continue;
        }
        if tx.send(line).is_err() {
            break;
        }
    }
    Ok(())
}

fn kill(child: &mut Child) -> io::Result<ExitStatus> {
    child.kill()?;
    child.wait()
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsFd as _;

    use super::*;

    fn sh(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    }

    struct Broken;

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_pipe_is_told_from_a_file() {
        let (reader, writer) = io::pipe().unwrap();
        assert!(is_pipe(writer.as_fd()).unwrap());
        assert!(is_pipe(reader.as_fd()).unwrap());
        let path = std::env::temp_dir().join(format!("kms-spike-pipe-{}", std::process::id()));
        let file = File::create(&path).unwrap();
        assert!(!is_pipe(file.as_fd()).unwrap());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn heartbeats_are_spaced() {
        let start = Instant::now();
        let mut link = Link::new(Vec::new());
        link.heartbeat(start).unwrap();
        link.heartbeat(start + HEARTBEAT_INTERVAL / 2).unwrap();
        link.line("text").unwrap();
        link.heartbeat(start + HEARTBEAT_INTERVAL).unwrap();
        assert_eq!(
            String::from_utf8(link.out).unwrap(),
            "kms-spike-heartbeat\ntext\nkms-spike-heartbeat\n"
        );
    }

    #[test]
    fn a_broken_pipe_means_the_supervisor_is_gone() {
        let mut link = Link::new(Broken);
        assert!(link.line("x").is_err());
        assert!(link.heartbeat(Instant::now()).is_err());
    }

    #[test]
    fn output_is_forwarded_without_heartbeats() {
        let mut out = Vec::new();
        let ended = supervise(
            sh("echo one; echo kms-spike-heartbeat; echo two; exit 3"),
            Duration::from_secs(60),
            &mut out,
        )
        .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "one\ntwo\n");
        match ended {
            Ended::Exited(s) => assert_eq!(s.code(), Some(3)),
            Ended::Killed(_) => panic!("the worker was killed"),
        }
    }

    #[test]
    fn a_worker_past_the_hard_limit_is_killed() {
        let mut out = Vec::new();
        let ended = supervise(
            sh("echo started; exec sleep 30"),
            Duration::from_millis(300),
            &mut out,
        )
        .unwrap();
        assert!(matches!(ended, Ended::Killed(_)), "{ended:?}");
        assert_eq!(String::from_utf8(out).unwrap(), "started\n");
    }
}
