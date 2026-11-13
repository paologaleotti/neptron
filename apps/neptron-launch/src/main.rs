//! The launcher. It holds the DRM descriptor, starts the frontend, passes
//! the descriptor in `NEPTRON_DRM_FD` and respawns the frontend with a
//! backoff.

#![forbid(unsafe_code)]

mod args;
mod backoff;
mod device;
mod power;
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
use power::{POWER_ENV, POWER_ENV_ON, Power};
use stopping::{STOP_WAIT_NS, StopAction, Stopping};
use supervise::{Ended, Frontend, Time, supervise};

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
        power: options.power,
        stop: Arc::clone(&stop),
        stop_wait: Duration::from_nanos(STOP_WAIT_NS),
    };
    // The device stays open until the launcher exits, also during a power
    // request: the last validated mode stays on the wire until init stops
    // the launcher.
    match supervise(&mut frontend, &mut time, &mut Backoff::new()) {
        Ok(Ended { runs, power: None }) => {
            say!("neptron-launch: the frontend ended after {runs} runs");
            ExitCode::SUCCESS
        }
        Ok(Ended {
            power: Some(power), ..
        }) => carry_out(power, options.power, power::run_command, &mut time, &stop),
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

/// Carries out the reboot or the power-off that the frontend asked for,
/// when the launcher has power control (`control`): it asks init with
/// `run`, then waits for the stop signal of the shutdown. With no power
/// control the launcher stops as after a quit.
///
/// A request that fails leaves the machine running. The launcher then
/// exits with an error, so init starts it again and the menus come back.
fn carry_out(
    power: Power,
    control: bool,
    run: impl FnOnce(&'static str) -> io::Result<Option<i32>>,
    time: &mut impl Time,
    stop: &AtomicBool,
) -> ExitCode {
    if !control {
        say!(
            "neptron-launch: the frontend asks for {}, and the launcher has no --power; it stops",
            power.name()
        );
        return ExitCode::SUCCESS;
    }
    say!(
        "neptron-launch: the frontend asks for {}; {}",
        power.name(),
        power.command()
    );
    match power::request(power, run, time, || stop.load(Ordering::Relaxed)) {
        Ok(()) => {
            say!(
                "neptron-launch: init stops the launcher for {}",
                power.name()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            say!("neptron-launch: {e}; exit 1, init starts the launcher again");
            ExitCode::FAILURE
        }
    }
}

/// How often the launcher looks at its child and at the stop signal.
const CHILD_POLL: Duration = Duration::from_millis(20);

/// The wait after a failed open of the DRM device before the launcher
/// exits, in nanoseconds. Init starts the launcher again at once, so with
/// no wait a missing `/dev/dri/card0` (a driver that has not bound yet)
/// would start it in a tight loop.
const DEVICE_MISSING_WAIT_NS: u64 = 2_000_000_000;

/// Waits `ns` with `sleep`, in steps of [`CHILD_POLL`], and ends at once
/// when `stop` is set: init must not wait for the backoff to stop the
/// launcher. The steps add up to `ns` with no stop.
fn wait_or_stop(ns: u64, stop: &AtomicBool, mut sleep: impl FnMut(u64)) {
    let step_ns = u64::try_from(CHILD_POLL.as_nanos()).unwrap_or(u64::MAX);
    let mut left = ns;
    while left > 0 && !stop.load(Ordering::Relaxed) {
        let step = left.min(step_ns);
        sleep(step);
        left -= step;
    }
}

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
    /// True when the launcher has power control: the frontend gets
    /// [`POWER_ENV`].
    power: bool,
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
        if self.power {
            command.env(POWER_ENV, POWER_ENV_ON);
        } else {
            command.env_remove(POWER_ENV);
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

    fn wait_ns(&mut self, ns: u64) {
        wait_or_stop(ns, &self.stop, |step| {
            std::thread::sleep(Duration::from_nanos(step));
        });
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
            power: false,
            stop: Arc::new(AtomicBool::new(false)),
            stop_wait: Duration::from_nanos(STOP_WAIT_NS),
        }
    }

    #[test]
    fn the_frontend_learns_of_power_control_from_its_environment() {
        let check = format!("test \"${POWER_ENV}\" = {POWER_ENV_ON}");
        let mut on = shell(&check, None);
        on.power = true;
        assert_eq!(on.run().expect("run"), Some(0));
        let check_unset = format!("test -z \"${{{POWER_ENV}+set}}\"");
        assert_eq!(shell(&check_unset, None).run().expect("run"), Some(0));
    }

    #[test]
    fn a_power_request_runs_the_command_once_and_exits_cleanly_on_the_stop() {
        let stop = AtomicBool::new(false);
        let mut runs = Vec::new();
        let mut waits = StopInWait {
            stop: &stop,
            waits: Vec::new(),
        };
        let code = carry_out(
            Power::PowerOff,
            true,
            |c| {
                runs.push(c);
                Ok(Some(0))
            },
            &mut waits,
            &stop,
        );
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(runs, [power::POWER_OFF_COMMAND]);
        assert_eq!(waits.waits, [power::POWER_STOP_WAIT_NS]);
    }

    #[test]
    fn a_failed_power_request_exits_with_an_error_so_init_restarts_the_launcher() {
        let stop = AtomicBool::new(false);
        let mut clock = Waits(Vec::new());
        let failed = carry_out(Power::Reboot, true, |_| Ok(Some(1)), &mut clock, &stop);
        assert_eq!(failed, ExitCode::FAILURE);
        let missing = carry_out(
            Power::Reboot,
            true,
            |_| Err(io::Error::from(io::ErrorKind::NotFound)),
            &mut clock,
            &stop,
        );
        assert_eq!(missing, ExitCode::FAILURE);
        assert!(clock.0.is_empty(), "no wait after a failed command");
        let no_stop = carry_out(Power::Reboot, true, |_| Ok(Some(0)), &mut clock, &stop);
        assert_eq!(
            no_stop,
            ExitCode::FAILURE,
            "init never stopped the launcher"
        );
        assert_eq!(clock.0, [power::POWER_STOP_WAIT_NS]);
    }

    #[test]
    fn with_no_power_control_a_request_runs_nothing_and_stops() {
        let stop = AtomicBool::new(false);
        let mut clock = Waits(Vec::new());
        let code = carry_out(
            Power::PowerOff,
            false,
            |c| panic!("{c} must not run"),
            &mut clock,
            &stop,
        );
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(clock.0.is_empty());
    }

    /// A clock whose wait ends with the stop flag set, as the SIGTERM of
    /// the stop script does.
    struct StopInWait<'a> {
        stop: &'a AtomicBool,
        waits: Vec<u64>,
    }

    impl Time for StopInWait<'_> {
        fn now_ns(&self) -> u64 {
            0
        }

        fn wait_ns(&mut self, ns: u64) {
            self.waits.push(ns);
            self.stop.store(true, Ordering::Relaxed);
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

    /// Runs `wait_or_stop` for `ns` on a simulated clock, with the stop
    /// flag set when the clock reaches `stop_at_ns`. Returns the time the
    /// wait took on that clock.
    fn simulated_wait(ns: u64, stop_at_ns: Option<u64>) -> u64 {
        let stop = AtomicBool::new(false);
        let mut clock = 0;
        wait_or_stop(ns, &stop, |step| {
            clock += step;
            if stop_at_ns.is_some_and(|at| clock >= at) {
                stop.store(true, Ordering::Relaxed);
            }
        });
        clock
    }

    #[test]
    fn the_backoff_wait_runs_its_time_with_no_stop() {
        assert_eq!(simulated_wait(8_000_000_000, None), 8_000_000_000);
        assert_eq!(simulated_wait(5_000_000, None), 5_000_000);
    }

    #[test]
    fn a_stop_during_the_backoff_wait_ends_it_within_one_poll() {
        let poll = u64::try_from(CHILD_POLL.as_nanos()).unwrap();
        let waited = simulated_wait(8_000_000_000, Some(1_000_000_000));
        assert!(
            waited >= 1_000_000_000 && waited < 1_000_000_000 + poll,
            "{waited}"
        );
        let stop = AtomicBool::new(true);
        let mut slept = 0;
        wait_or_stop(8_000_000_000, &stop, |step| slept += step);
        assert_eq!(slept, 0, "a stop before the wait ends it at once");
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
