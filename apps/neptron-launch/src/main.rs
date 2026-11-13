//! The launcher. It holds the DRM descriptor, starts the frontend, passes
//! the descriptor in `NEPTRON_DRM_FD` and respawns the frontend with a
//! backoff.

#![forbid(unsafe_code)]

mod args;
mod backoff;
mod device;
mod stopping;
mod supervise;

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process};

use backoff::Backoff;
use device::{DRM_FD_ENV, Device, DeviceError};
use stopping::{STOP_WAIT_NS, StopAction, Stopping};
use supervise::{Frontend, Time, supervise};

fn main() -> ExitCode {
    let options = match args::parse(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(e) => {
            say!("neptron-launch: {e}");
            return ExitCode::from(2);
        }
    };
    // The signals are watched before the device opens, so a stop ends
    // the wait after a failed open at once.
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if let Err(e) = signal_hook::flag::register(signal, Arc::clone(&stop)) {
            say!("neptron-launch: cannot watch signal {signal}: {e}");
            return ExitCode::FAILURE;
        }
    }
    let mut time = MonotonicTime {
        start: Instant::now(),
        stop: Arc::clone(&stop),
    };
    let Ok(device) = open_device(options.device.as_deref(), &mut time) else {
        return ExitCode::FAILURE;
    };
    let mut frontend = Process {
        program: options.program,
        args: options.args,
        fd: device.as_ref().map(Device::fd),
        stop,
        stop_wait: Duration::from_nanos(STOP_WAIT_NS),
    };
    match supervise(&mut frontend, &mut time, &mut Backoff::new()) {
        Ok(runs) => {
            say!("neptron-launch: the frontend ended after {runs} runs");
            ExitCode::SUCCESS
        }
        Err(e) => {
            say!("neptron-launch: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Writes a line to standard error. A failed write loses the line: the
/// launcher must not panic or stop over its log, and `eprintln!` panics on
/// a write error.
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        // A line that cannot be written is lost; see above.
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}
use say;

/// How often the launcher looks at its child and at the stop signal.
const CHILD_POLL: Duration = Duration::from_millis(20);

/// The wait after a failed open of the DRM device before the launcher
/// exits, in nanoseconds. Init starts the launcher again at once, so with
/// no wait a missing `/dev/dri/card0` (a driver that has not bound yet)
/// would start it in a tight loop.
const DEVICE_MISSING_WAIT_NS: u64 = 2_000_000_000;

/// Opens the DRM device at `path`, when there is one. A failure is logged
/// in one line and followed by [`DEVICE_MISSING_WAIT_NS`] on `time`.
fn open_device(path: Option<&Path>, time: &mut impl Time) -> Result<Option<Device>, DeviceError> {
    let Some(path) = path else {
        return Ok(None);
    };
    Device::open(path).map(Some).map_err(|e| {
        say!(
            "neptron-launch: {e}; exit in {} ms",
            DEVICE_MISSING_WAIT_NS / 1_000_000
        );
        time.wait_ns(DEVICE_MISSING_WAIT_NS);
        e
    })
}

/// The frontend as a child process with the descriptor number in its
/// environment.
struct Process {
    program: OsString,
    args: Vec<OsString>,
    fd: Option<i32>,
    /// Set by SIGTERM or SIGINT.
    stop: Arc<AtomicBool>,
    /// The wait between the forwarded SIGTERM and SIGKILL.
    stop_wait: Duration,
}

impl Frontend for Process {
    /// Runs the child to its end. A stop signal is forwarded to it as
    /// SIGTERM once, and the launcher waits for it, so no orphan keeps DRM
    /// master. A child still there after the stop wait gets SIGKILL: a
    /// frontend stuck inside a core never reads its stop flag.
    fn run(&mut self) -> io::Result<Option<i32>> {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if let Some(fd) = self.fd {
            command.env(DRM_FD_ENV, fd.to_string());
        }
        let mut child = command.spawn()?;
        let start = Instant::now();
        let wait_ns = u64::try_from(self.stop_wait.as_nanos()).unwrap_or(u64::MAX);
        let mut stopping = Stopping::new(wait_ns);
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status.code());
            }
            let now_ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            match stopping.next(self.stopping(), now_ns) {
                StopAction::Wait => {}
                StopAction::Term => kill_process(Pid::from_child(&child), Signal::TERM)?,
                StopAction::Kill => {
                    // The signal goes first: a log write can block on a
                    // stuck log pipe, and the stop must not wait for it.
                    kill_process(Pid::from_child(&child), Signal::KILL)?;
                    say!(
                        "neptron-launch: the frontend did not end {} ms after SIGTERM; SIGKILL",
                        self.stop_wait.as_millis()
                    );
                }
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
    /// Set by SIGTERM or SIGINT: it ends a wait at once.
    stop: Arc<AtomicBool>,
}

impl Time for MonotonicTime {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    /// Waits `ns`, in steps of [`CHILD_POLL`], and ends at once on a stop
    /// signal: init must not wait for the backoff to stop the launcher.
    fn wait_ns(&mut self, ns: u64) {
        let end = Instant::now() + Duration::from_nanos(ns);
        while !self.stop.load(Ordering::Relaxed) {
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            std::thread::sleep(left.min(CHILD_POLL));
        }
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
            stop_wait: Duration::from_nanos(STOP_WAIT_NS),
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
    fn a_child_that_ignores_sigterm_gets_sigkill_after_the_stop_wait() {
        let ready =
            std::env::temp_dir().join(format!("neptron-launch-ignore-{}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_file(&ready);
        // The shell ignores SIGTERM before it says it is ready, so the stop
        // comes only after the trap is set.
        let mut child = shell(
            &format!("trap '' TERM; : > {}; while :; do :; done", ready.display()),
            None,
        );
        child.stop_wait = Duration::from_millis(300);
        let stop = Arc::clone(&child.stop);
        let runner = std::thread::spawn(move || child.run());
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !ready.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "the child did not start"
            );
            std::thread::yield_now();
        }
        let stopped = std::time::Instant::now();
        stop.store(true, Ordering::Relaxed);
        let code = runner.join().expect("the runner").expect("run");
        assert_eq!(code, None, "SIGKILL ends it");
        let after = stopped.elapsed();
        assert!(
            after >= Duration::from_millis(300) && after < Duration::from_secs(10),
            "SIGTERM was ignored and SIGKILL came after the wait: {after:?}"
        );
        // The file is the test's own; a failed removal leaves a temp file.
        let _ = std::fs::remove_file(&ready);
    }

    #[test]
    fn a_stop_signal_ends_the_backoff_wait_at_once() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut time = MonotonicTime {
            start: Instant::now(),
            stop: Arc::clone(&stop),
        };
        let start = Instant::now();
        time.wait_ns(60_000_000);
        assert!(start.elapsed() >= Duration::from_millis(60), "a full wait");
        stop.store(true, Ordering::Relaxed);
        let start = Instant::now();
        time.wait_ns(8_000_000_000);
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "{:?}",
            start.elapsed()
        );
    }

    /// A clock that records its waits.
    struct Waits(Vec<u64>);

    impl Time for Waits {
        fn now_ns(&self) -> u64 {
            0
        }

        fn wait_ns(&mut self, ns: u64) {
            self.0.push(ns);
        }
    }

    #[test]
    fn a_missing_device_waits_before_the_launcher_exits() {
        let mut waits = Waits(Vec::new());
        let missing = std::path::Path::new("/nonexistent/neptron/dri/card0");
        assert!(open_device(Some(missing), &mut waits).is_err());
        assert_eq!(waits.0, [DEVICE_MISSING_WAIT_NS]);
        let mut none = Waits(Vec::new());
        assert!(matches!(open_device(None, &mut none), Ok(None)));
        assert!(none.0.is_empty());
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
