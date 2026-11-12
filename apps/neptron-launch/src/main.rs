//! The launcher. It holds the DRM descriptor, starts the frontend, passes
//! the descriptor in `NEPTRON_DRM_FD` and respawns the frontend with a
//! backoff.

#![forbid(unsafe_code)]

mod args;
mod backoff;
mod device;
mod supervise;

use std::ffi::OsString;
use std::io;
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process};

use backoff::Backoff;
use device::{DRM_FD_ENV, Device};
use supervise::{Frontend, Time, supervise};

fn main() -> ExitCode {
    let options = match args::parse(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("neptron-launch: {e}");
            return ExitCode::from(2);
        }
    };
    let device = match options.device.as_deref().map(Device::open).transpose() {
        Ok(device) => device,
        Err(e) => {
            eprintln!("neptron-launch: {e}");
            return ExitCode::FAILURE;
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if let Err(e) = signal_hook::flag::register(signal, Arc::clone(&stop)) {
            eprintln!("neptron-launch: cannot watch signal {signal}: {e}");
            return ExitCode::FAILURE;
        }
    }
    let mut frontend = Process {
        program: options.program,
        args: options.args,
        fd: device.as_ref().map(Device::fd),
        stop,
    };
    let mut time = MonotonicTime {
        start: Instant::now(),
    };
    match supervise(&mut frontend, &mut time, &mut Backoff::new()) {
        Ok(runs) => {
            eprintln!("neptron-launch: the frontend ended after {runs} runs");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("neptron-launch: {e}");
            ExitCode::FAILURE
        }
    }
}

/// How often the launcher looks at its child and at the stop signal.
const CHILD_POLL: Duration = Duration::from_millis(20);

/// The frontend as a child process with the descriptor number in its
/// environment.
struct Process {
    program: OsString,
    args: Vec<OsString>,
    fd: Option<i32>,
    /// Set by SIGTERM or SIGINT.
    stop: Arc<AtomicBool>,
}

impl Frontend for Process {
    /// Runs the child to its end. A stop signal is forwarded to it as
    /// SIGTERM once, and the launcher waits for it, so no orphan keeps DRM
    /// master.
    fn run(&mut self) -> io::Result<Option<i32>> {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if let Some(fd) = self.fd {
            command.env(DRM_FD_ENV, fd.to_string());
        }
        let mut child = command.spawn()?;
        let mut forwarded = false;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status.code());
            }
            if !forwarded && self.stopping() {
                forwarded = true;
                kill_process(Pid::from_child(&child), Signal::TERM)?;
            }
            std::thread::sleep(CHILD_POLL);
        }
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

struct MonotonicTime {
    start: Instant,
}

impl Time for MonotonicTime {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn wait_ns(&mut self, ns: u64) {
        std::thread::sleep(Duration::from_nanos(ns));
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::backoff::RELAUNCH_EXIT_CODE;

    fn shell(script: &str, fd: Option<i32>) -> Process {
        Process {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            fd,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn a_stop_signal_is_forwarded_to_the_child() {
        let mut child = shell("sleep 30", None);
        child.stop.store(true, Ordering::Relaxed);
        let start = std::time::Instant::now();
        assert_eq!(
            child.run().expect("run"),
            None,
            "the child ends by the signal"
        );
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{:?}",
            start.elapsed()
        );
        assert!(child.stopping());
    }

    #[test]
    fn a_child_reports_its_exit_code() {
        let mut child = shell(&format!("exit {RELAUNCH_EXIT_CODE}"), None);
        assert_eq!(child.run().expect("run"), Some(RELAUNCH_EXIT_CODE));
        assert_eq!(shell("exit 0", None).run().expect("run"), Some(0));
        assert_eq!(shell("kill -9 $$", None).run().expect("run"), None);
    }

    #[test]
    fn a_child_gets_the_descriptor_under_its_number() {
        let path =
            std::env::temp_dir().join(format!("neptron-launch-child-{}", std::process::id()));
        std::fs::write(&path, b"held").expect("write");
        let file = std::fs::File::open(&path).expect("open");
        device::inheritable(&file).expect("inheritable");
        let script = format!(
            "test \"${DRM_FD_ENV}\" = {fd} && test \"$(cat <&{fd})\" = held",
            fd = file.as_raw_fd()
        );
        let code = shell(&script, Some(file.as_raw_fd())).run().expect("run");
        assert_eq!(
            code,
            Some(0),
            "the child reads the descriptor of the launcher"
        );
        // The file is the test's own; a failed removal leaves a temp file.
        let _ = std::fs::remove_file(&path);
    }
}
