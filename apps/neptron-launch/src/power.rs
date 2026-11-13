//! A reboot or a power-off that the frontend asks for. The launcher does
//! not call the system call: it asks init with the BusyBox command, and
//! init runs the shutdown steps of the image (the stop script, `rcK`,
//! `umount -a -r`) before it turns the machine off. The first step stops
//! the launcher with SIGTERM, so after the command the launcher waits for
//! that signal.

use std::io;

use crate::supervise::Time;

/// The environment variable that tells the frontend that this launcher
/// carries out a reboot or a power-off. The frontend has the same name in
/// `neptron::power::POWER_ENV`.
pub const POWER_ENV: &str = "NEPTRON_POWER";

/// The value of [`POWER_ENV`] when the launcher has power control.
pub const POWER_ENV_ON: &str = "1";

/// The BusyBox command that asks init for a reboot. Without `-f` it only
/// signals init, which runs the shutdown steps first.
pub const REBOOT_COMMAND: &str = "/sbin/reboot";

/// The BusyBox command that asks init for a power-off.
pub const POWER_OFF_COMMAND: &str = "/sbin/poweroff";

/// How long the launcher waits for the SIGTERM of the stop script after
/// the command, in nanoseconds. Init runs the stop script as its first
/// shutdown step, at once; a launcher still waiting after this time knows
/// that the shutdown did not start.
pub const POWER_STOP_WAIT_NS: u64 = 30_000_000_000;

/// What the frontend asks the machine to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Start the machine again.
    Reboot,
    /// Turn the machine off.
    PowerOff,
}

/// Why a request to init did not end in a shutdown.
#[derive(Debug, thiserror::Error)]
pub enum PowerError {
    /// The command did not start.
    #[error("cannot run {command}: {source}")]
    Start {
        /// The command.
        command: &'static str,
        /// The error of the system.
        #[source]
        source: io::Error,
    },
    /// The command ended with an error, or by a signal (`None`).
    #[error("{command} failed with exit code {code:?}")]
    Failed {
        /// The command.
        command: &'static str,
        /// Its exit code.
        code: Option<i32>,
    },
    /// Init did not stop the launcher after the command.
    #[error("no stop from init {} ms after {command}", waited_ns / 1_000_000)]
    NoStop {
        /// The command.
        command: &'static str,
        /// The time the launcher waited, in nanoseconds.
        waited_ns: u64,
    },
}

impl Power {
    /// The BusyBox command for this request.
    pub fn command(self) -> &'static str {
        match self {
            Self::Reboot => REBOOT_COMMAND,
            Self::PowerOff => POWER_OFF_COMMAND,
        }
    }

    /// The request in words, for the log.
    pub fn name(self) -> &'static str {
        match self {
            Self::Reboot => "a reboot",
            Self::PowerOff => "a power-off",
        }
    }
}

/// Asks init for `power` with one run of its command, then waits on
/// `time` for the stop signal of the shutdown. `run` runs a command and
/// gives its exit code, `None` for a signal. `stopping` reads the stop
/// flag; the wait of `time` ends at once when it is set.
///
/// # Errors
///
/// [`PowerError`] when the command does not start or fails, or when no
/// stop comes in [`POWER_STOP_WAIT_NS`]. The machine then still runs, and
/// the launcher must exit with an error so that init starts it again and
/// the menus come back.
pub fn request(
    power: Power,
    run: impl FnOnce(&'static str) -> io::Result<Option<i32>>,
    time: &mut impl Time,
    stopping: impl Fn() -> bool,
) -> Result<(), PowerError> {
    let command = power.command();
    match run(command) {
        Err(source) => return Err(PowerError::Start { command, source }),
        Ok(Some(0)) => {}
        Ok(code) => return Err(PowerError::Failed { command, code }),
    }
    let start_ns = time.now_ns();
    time.wait_ns(POWER_STOP_WAIT_NS);
    if stopping() {
        return Ok(());
    }
    Err(PowerError::NoStop {
        command,
        waited_ns: time.now_ns().saturating_sub(start_ns),
    })
}

/// Runs `command` with no arguments and waits for it.
///
/// # Errors
///
/// The command did not start, or the wait failed.
pub fn run_command(command: &str) -> io::Result<Option<i32>> {
    std::process::Command::new(command)
        .status()
        .map(|status| status.code())
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    /// A clock that records its waits; a stop can come during a wait.
    struct FakeTime<'a> {
        now_ns: u64,
        waits: Vec<u64>,
        /// Set during the first wait, when `stop_in_wait` holds.
        stop: &'a Cell<bool>,
        stop_in_wait: bool,
    }

    impl Time for FakeTime<'_> {
        fn now_ns(&self) -> u64 {
            self.now_ns
        }

        fn wait_ns(&mut self, ns: u64) {
            self.waits.push(ns);
            if self.stop_in_wait {
                self.stop.set(true);
                self.now_ns += 1_000_000;
            } else {
                self.now_ns += ns;
            }
        }
    }

    fn time(stop: &Cell<bool>, stop_in_wait: bool) -> FakeTime<'_> {
        FakeTime {
            now_ns: 0,
            waits: Vec::new(),
            stop,
            stop_in_wait,
        }
    }

    #[test]
    fn each_request_runs_its_command_once_and_waits_for_the_stop() {
        for (power, command) in [
            (Power::Reboot, "/sbin/reboot"),
            (Power::PowerOff, "/sbin/poweroff"),
        ] {
            let stop = Cell::new(false);
            let mut clock = time(&stop, true);
            let runs = RefCell::new(Vec::new());
            let result = request(
                power,
                |c| {
                    runs.borrow_mut().push(c);
                    Ok(Some(0))
                },
                &mut clock,
                || stop.get(),
            );
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(*runs.borrow(), [command]);
            assert_eq!(clock.waits, [POWER_STOP_WAIT_NS]);
        }
    }

    #[test]
    fn a_command_that_does_not_start_or_fails_is_an_error_with_no_wait() {
        let stop = Cell::new(false);
        let mut clock = time(&stop, false);
        let missing = request(
            Power::PowerOff,
            |_| Err(io::Error::from(io::ErrorKind::NotFound)),
            &mut clock,
            || stop.get(),
        );
        assert!(
            matches!(
                missing,
                Err(PowerError::Start {
                    command: "/sbin/poweroff",
                    ..
                })
            ),
            "{missing:?}"
        );
        let failed = request(Power::Reboot, |_| Ok(Some(1)), &mut clock, || stop.get());
        assert!(
            matches!(
                failed,
                Err(PowerError::Failed {
                    command: "/sbin/reboot",
                    code: Some(1)
                })
            ),
            "{failed:?}"
        );
        let killed = request(Power::Reboot, |_| Ok(None), &mut clock, || stop.get());
        assert!(matches!(killed, Err(PowerError::Failed { code: None, .. })));
        assert!(clock.waits.is_empty(), "no wait after a failure");
    }

    #[test]
    fn no_stop_after_the_command_is_an_error_after_the_whole_wait() {
        let stop = Cell::new(false);
        let mut clock = time(&stop, false);
        let result = request(Power::PowerOff, |_| Ok(Some(0)), &mut clock, || stop.get());
        let Err(PowerError::NoStop { waited_ns, .. }) = result else {
            panic!("no stop is an error: {result:?}");
        };
        assert_eq!(waited_ns, POWER_STOP_WAIT_NS);
        assert_eq!(
            PowerError::NoStop {
                command: POWER_OFF_COMMAND,
                waited_ns,
            }
            .to_string(),
            "no stop from init 30000 ms after /sbin/poweroff"
        );
    }

    #[test]
    fn a_real_command_reports_its_exit_code() {
        assert_eq!(run_command("/usr/bin/true").expect("true"), Some(0));
        assert_eq!(run_command("/usr/bin/false").expect("false"), Some(1));
        assert!(run_command("/nonexistent/neptron/poweroff").is_err());
    }
}
