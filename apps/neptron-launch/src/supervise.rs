//! The supervisor loop: start the frontend, wait for it, start it again.

use std::io;

use crate::backoff::{Backoff, Exit, Next, NoDisplayLog};
use crate::power::Power;

/// The frontend process, as the supervisor sees it.
pub trait Frontend {
    /// Starts a new run and waits for its end. It gives the exit code, or
    /// `None` when a signal ended the process.
    ///
    /// # Errors
    ///
    /// The process could not start, or the wait failed.
    fn run(&mut self) -> io::Result<Option<i32>>;

    /// A stop signal came: the supervisor starts no new run.
    fn stopping(&self) -> bool;
}

/// The time of the supervisor: a monotonic clock and a wait.
pub trait Time {
    /// Monotonic nanoseconds.
    fn now_ns(&self) -> u64;
    /// Waits `ns` nanoseconds.
    fn wait_ns(&mut self, ns: u64);
}

/// How the supervisor ended with no error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ended {
    /// The runs of the frontend.
    pub runs: u64,
    /// The reboot or the power-off that the last run asked for. `None`
    /// after a quit or a stop signal.
    pub power: Option<Power>,
}

/// Why the supervisor stopped.
#[derive(Debug, thiserror::Error)]
pub enum SuperviseError {
    /// The frontend could not start, or the wait for it failed.
    #[error("cannot run the frontend after {runs} runs: {source}")]
    Run {
        /// The runs that started before the failure.
        runs: u64,
        /// The error of the system.
        #[source]
        source: io::Error,
    },
}

/// Runs the frontend until it quits with exit code 0, asks for a reboot or
/// a power-off, or until a stop signal ends the run that holds it. A crash
/// starts it again after the wait of the [`Backoff`]; a relaunch starts it
/// again at once. A stop signal wins over a request of the last run.
///
/// # Errors
///
/// [`SuperviseError::Run`] when the frontend cannot start. The init system
/// then restarts the launcher, which opens the device again.
pub fn supervise<F: Frontend, T: Time>(
    frontend: &mut F,
    time: &mut T,
    backoff: &mut Backoff,
) -> Result<Ended, SuperviseError> {
    let mut runs = 0u64;
    let mut no_display_log = NoDisplayLog::new();
    loop {
        let started_ns = time.now_ns();
        let code = frontend
            .run()
            .map_err(|source| SuperviseError::Run { runs, source })?;
        runs += 1;
        let now_ns = time.now_ns();
        let ran_ns = now_ns.saturating_sub(started_ns);
        let ended = |power| Ok(Ended { runs, power });
        if frontend.stopping() {
            return ended(None);
        }
        let exit = Exit::of_code(code);
        match backoff.after(exit, ran_ns, now_ns) {
            Next::Stop => return ended(None),
            Next::Power(power) => return ended(Some(power)),
            Next::Restart { after_ns } => {
                if exit == Exit::NoDisplay {
                    if no_display_log.due(now_ns) {
                        crate::say!(
                            "neptron-launch: the frontend found no display; it starts again every {} ms. \
                             An analog connector with no EDID (a VGA CRT) is used only when \
                             display.output names it",
                            after_ns / 1_000_000
                        );
                    }
                } else if after_ns > 0 {
                    crate::say!(
                        "neptron-launch: the frontend ended with {code:?} after {} ms, crash {} in a row; start again in {} ms",
                        ran_ns / 1_000_000,
                        backoff.crashes(),
                        after_ns / 1_000_000
                    );
                }
                if after_ns > 0 {
                    time.wait_ns(after_ns);
                }
                if frontend.stopping() {
                    return ended(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backoff::{
        BACKOFF_FIRST_NS, NO_DISPLAY_EXIT_CODE, NO_DISPLAY_WAIT_NS, POWER_OFF_EXIT_CODE,
        REBOOT_EXIT_CODE, RELAUNCH_EXIT_CODE, STABLE_RUN_NS,
    };

    /// A frontend that ends each run with the next scripted result after a
    /// scripted run time on the simulated clock.
    struct FakeFrontend<'a> {
        script: Vec<(u64, io::Result<Option<i32>>)>,
        clock: &'a std::cell::Cell<u64>,
        /// The run after which a stop signal comes.
        stop_after: Option<usize>,
        runs: usize,
    }

    impl Frontend for FakeFrontend<'_> {
        fn run(&mut self) -> io::Result<Option<i32>> {
            assert!(!self.script.is_empty(), "the script ended");
            self.runs += 1;
            let (ran_ns, result) = self.script.remove(0);
            self.clock.set(self.clock.get() + ran_ns);
            result
        }

        fn stopping(&self) -> bool {
            self.stop_after.is_some_and(|n| self.runs >= n)
        }
    }

    struct FakeTime<'a> {
        clock: &'a std::cell::Cell<u64>,
        waits: Vec<u64>,
    }

    impl Time for FakeTime<'_> {
        fn now_ns(&self) -> u64 {
            self.clock.get()
        }

        fn wait_ns(&mut self, ns: u64) {
            self.waits.push(ns);
            self.clock.set(self.clock.get() + ns);
        }
    }

    fn run(
        script: Vec<(u64, io::Result<Option<i32>>)>,
    ) -> (Result<Ended, SuperviseError>, Vec<u64>) {
        run_stopping(script, None)
    }

    fn run_stopping(
        script: Vec<(u64, io::Result<Option<i32>>)>,
        stop_after: Option<usize>,
    ) -> (Result<Ended, SuperviseError>, Vec<u64>) {
        let clock = std::cell::Cell::new(0);
        let mut frontend = FakeFrontend {
            script,
            clock: &clock,
            stop_after,
            runs: 0,
        };
        let mut time = FakeTime {
            clock: &clock,
            waits: Vec::new(),
        };
        let result = supervise(&mut frontend, &mut time, &mut Backoff::new());
        (result, time.waits)
    }

    #[test]
    fn starts_with_no_display_wait_a_fixed_time_and_then_the_run_goes_on() {
        let no_display = || (15_000_000_000, Ok(Some(NO_DISPLAY_EXIT_CODE)));
        let mut script: Vec<_> = (0..5).map(|_| no_display()).collect();
        script.push((1_000, Ok(Some(101))));
        script.push((1_000, Ok(Some(0))));
        let (result, waits) = run(script);
        assert_eq!(result.expect("a quit"), quit(7));
        assert_eq!(
            waits,
            [
                NO_DISPLAY_WAIT_NS,
                NO_DISPLAY_WAIT_NS,
                NO_DISPLAY_WAIT_NS,
                NO_DISPLAY_WAIT_NS,
                NO_DISPLAY_WAIT_NS,
                BACKOFF_FIRST_NS
            ],
            "the first crash after the starts with no display waits the first backoff"
        );
    }

    #[test]
    fn crashes_restart_with_a_growing_wait_until_a_quit() {
        let (result, waits) = run(vec![
            (1_000, Ok(Some(101))),
            (1_000, Ok(None)),
            (1_000, Ok(Some(1))),
            (1_000, Ok(Some(0))),
        ]);
        assert_eq!(result.expect("a quit"), quit(4));
        assert_eq!(
            waits,
            [BACKOFF_FIRST_NS, 2 * BACKOFF_FIRST_NS, 4 * BACKOFF_FIRST_NS]
        );
    }

    #[test]
    fn relaunches_restart_at_once() {
        let (result, waits) = run(vec![
            (1_000, Ok(Some(RELAUNCH_EXIT_CODE))),
            (1_000, Ok(Some(RELAUNCH_EXIT_CODE))),
            (1_000, Ok(Some(0))),
        ]);
        assert_eq!(result.expect("a quit"), quit(3));
        assert!(waits.is_empty());
    }

    #[test]
    fn a_relaunch_loop_backs_off_on_the_simulated_clock() {
        let mut script: Vec<(u64, io::Result<Option<i32>>)> = (0..12)
            .map(|_| (1_000_000, Ok(Some(RELAUNCH_EXIT_CODE))))
            .collect();
        script.push((1_000_000, Ok(Some(0))));
        let (result, waits) = run(script);
        assert_eq!(result.expect("a quit"), quit(13));
        assert_eq!(waits, [BACKOFF_FIRST_NS, 2 * BACKOFF_FIRST_NS]);
    }

    #[test]
    fn a_stop_signal_ends_the_supervisor_after_the_run() {
        let (result, waits) = run_stopping(
            vec![
                (1_000, Ok(Some(1))),
                (1_000, Ok(None)),
                (1_000, Ok(Some(0))),
            ],
            Some(2),
        );
        assert_eq!(
            result.expect("a stop"),
            quit(2),
            "no third run after the signal"
        );
        assert_eq!(waits, [BACKOFF_FIRST_NS]);
    }

    #[test]
    fn a_long_run_resets_the_wait() {
        let (result, waits) = run(vec![
            (1_000, Ok(Some(1))),
            (1_000, Ok(Some(1))),
            (STABLE_RUN_NS, Ok(Some(1))),
            (1_000, Ok(Some(0))),
        ]);
        assert_eq!(result.expect("a quit"), quit(4));
        assert_eq!(
            waits,
            [BACKOFF_FIRST_NS, 2 * BACKOFF_FIRST_NS, BACKOFF_FIRST_NS]
        );
    }

    fn quit(runs: u64) -> Ended {
        Ended { runs, power: None }
    }

    #[test]
    fn a_power_request_ends_the_supervisor_with_no_restart_and_no_wait() {
        for (code, power) in [
            (REBOOT_EXIT_CODE, Power::Reboot),
            (POWER_OFF_EXIT_CODE, Power::PowerOff),
        ] {
            // The script has no run after the request: a restart would
            // panic in the fake frontend.
            let (result, waits) = run(vec![(1_000, Ok(Some(1))), (1_000, Ok(Some(code)))]);
            assert_eq!(
                result.expect("a request"),
                Ended {
                    runs: 2,
                    power: Some(power)
                }
            );
            assert_eq!(waits, [BACKOFF_FIRST_NS], "only the crash waited");
        }
    }

    #[test]
    fn a_stop_signal_wins_over_a_power_request() {
        let (result, _) = run_stopping(vec![(1_000, Ok(Some(POWER_OFF_EXIT_CODE)))], Some(1));
        assert_eq!(result.expect("a stop"), quit(1));
    }

    #[test]
    fn a_frontend_that_cannot_start_stops_the_supervisor() {
        let (result, _) = run(vec![
            (1_000, Ok(Some(1))),
            (0, Err(io::Error::from(io::ErrorKind::NotFound))),
        ]);
        let Err(SuperviseError::Run { runs, source }) = result else {
            panic!("a start failure is an error");
        };
        assert_eq!(runs, 1);
        assert_eq!(source.kind(), io::ErrorKind::NotFound);
    }
}
