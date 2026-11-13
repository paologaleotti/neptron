//! When the frontend starts again after it exits: the backoff.

/// The exit code with which the frontend asks to be started again at once,
/// for a planned relaunch: into a game, or back to the game list. The
/// frontend has the same number in `neptron::RELAUNCH_EXIT_CODE`. 75 is
/// `EX_TEMPFAIL` of `sysexits.h`.
pub const RELAUNCH_EXIT_CODE: i32 = 75;

/// The exit code of a frontend that found no connected display after its
/// own bounded wait. The frontend has the same number in
/// `neptron::display_wait::NO_DISPLAY_EXIT_CODE`. 69 is `EX_UNAVAILABLE`
/// of `sysexits.h`.
pub const NO_DISPLAY_EXIT_CODE: i32 = 69;

/// The wait before a restart after [`NO_DISPLAY_EXIT_CODE`], in
/// nanoseconds. The frontend has waited for the display already, so the
/// wait is short and does not grow: a monitor plugged in later gets a
/// picture within one wait of the frontend.
pub const NO_DISPLAY_WAIT_NS: u64 = 1_000_000_000;

/// The wait before the first restart after a crash, in nanoseconds.
pub const BACKOFF_FIRST_NS: u64 = 250_000_000;

/// The longest wait before a restart, in nanoseconds. The wait doubles
/// with each crash in a row up to this time.
pub const BACKOFF_MAX_NS: u64 = 8_000_000_000;

/// A run at least this long, in nanoseconds, ends a series of crashes: the
/// next crash waits [`BACKOFF_FIRST_NS`] again.
pub const STABLE_RUN_NS: u64 = 30_000_000_000;

/// The window in which relaunches are counted, in nanoseconds.
pub const RELAUNCH_WINDOW_NS: u64 = 10_000_000_000;

/// The most relaunches in [`RELAUNCH_WINDOW_NS`] that start at once. A
/// frontend that relaunches more often is in a loop, and each further
/// relaunch counts as a crash with its wait.
pub const RELAUNCHES_MAX: usize = 10;

/// How one run of the frontend ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Exit code 0: the frontend stopped on request. The launcher stops too.
    Quit,
    /// [`RELAUNCH_EXIT_CODE`]: start the frontend again at once.
    Relaunch,
    /// [`NO_DISPLAY_EXIT_CODE`]: no display answered. Start the frontend
    /// again after [`NO_DISPLAY_WAIT_NS`]. It is no crash and no relaunch.
    NoDisplay,
    /// Any other exit code, or a signal.
    Crash,
}

/// What the launcher does after a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Start the frontend again after this wait, in nanoseconds.
    Restart {
        /// The wait.
        after_ns: u64,
    },
    /// Stop the launcher.
    Stop,
}

/// The restart policy. It counts crashes in a row and nothing else: the
/// failed boots and the safe mode belong to the frontend's guard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Backoff {
    crashes: u32,
    /// The end times of the relaunches in the window, oldest first.
    relaunches: std::collections::VecDeque<u64>,
}

impl Exit {
    /// The exit of a process with this exit code, `None` for a signal.
    pub fn of_code(code: Option<i32>) -> Self {
        match code {
            Some(0) => Self::Quit,
            Some(RELAUNCH_EXIT_CODE) => Self::Relaunch,
            Some(NO_DISPLAY_EXIT_CODE) => Self::NoDisplay,
            _ => Self::Crash,
        }
    }
}

impl Backoff {
    /// A policy with no crash yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Crashes in a row since the last stable run.
    pub fn crashes(&self) -> u32 {
        self.crashes
    }

    /// What follows a run that ended with `exit` at `now_ns` after
    /// `ran_ns`.
    pub fn after(&mut self, exit: Exit, ran_ns: u64, now_ns: u64) -> Next {
        if ran_ns >= STABLE_RUN_NS {
            self.crashes = 0;
        }
        let exit = if exit == Exit::Relaunch && !self.relaunch_fits(now_ns) {
            Exit::Crash
        } else {
            exit
        };
        match exit {
            Exit::Quit => Next::Stop,
            Exit::Relaunch => Next::Restart { after_ns: 0 },
            Exit::NoDisplay => Next::Restart {
                after_ns: NO_DISPLAY_WAIT_NS,
            },
            Exit::Crash => {
                let doublings = self.crashes.min(BACKOFF_MAX_NS.ilog2());
                self.crashes = self.crashes.saturating_add(1);
                let after_ns = BACKOFF_FIRST_NS
                    .saturating_mul(1 << doublings)
                    .min(BACKOFF_MAX_NS);
                Next::Restart { after_ns }
            }
        }
    }
}

impl Backoff {
    /// Counts a relaunch at `now_ns`: true while the window holds at most
    /// [`RELAUNCHES_MAX`] of them.
    fn relaunch_fits(&mut self, now_ns: u64) -> bool {
        while self
            .relaunches
            .front()
            .is_some_and(|t| now_ns.saturating_sub(*t) >= RELAUNCH_WINDOW_NS)
        {
            self.relaunches.pop_front();
        }
        self.relaunches.push_back(now_ns);
        if self.relaunches.len() > RELAUNCHES_MAX {
            self.relaunches.pop_front();
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_map_to_exits() {
        assert_eq!(Exit::of_code(Some(0)), Exit::Quit);
        assert_eq!(Exit::of_code(Some(RELAUNCH_EXIT_CODE)), Exit::Relaunch);
        assert_eq!(Exit::of_code(Some(1)), Exit::Crash);
        assert_eq!(Exit::of_code(Some(101)), Exit::Crash, "a Rust panic");
        assert_eq!(Exit::of_code(None), Exit::Crash, "a signal");
    }

    #[test]
    fn crashes_in_a_row_double_the_wait_up_to_the_limit() {
        let mut backoff = Backoff::new();
        let waits: Vec<u64> = (0..10)
            .map(|_| match backoff.after(Exit::Crash, 1_000_000, 0) {
                Next::Restart { after_ns } => after_ns,
                Next::Stop => panic!("a crash restarts"),
            })
            .collect();
        assert_eq!(
            waits,
            [
                250_000_000,
                500_000_000,
                1_000_000_000,
                2_000_000_000,
                4_000_000_000,
                8_000_000_000,
                8_000_000_000,
                8_000_000_000,
                8_000_000_000,
                8_000_000_000,
            ]
        );
        assert_eq!(backoff.crashes(), 10);
    }

    #[test]
    fn a_stable_run_starts_the_series_again() {
        let mut backoff = Backoff::new();
        for _ in 0..4 {
            let _ = backoff.after(Exit::Crash, 0, 0);
        }
        assert_eq!(
            backoff.after(Exit::Crash, STABLE_RUN_NS, 0),
            Next::Restart {
                after_ns: BACKOFF_FIRST_NS
            }
        );
        assert_eq!(backoff.crashes(), 1);
    }

    #[test]
    fn a_relaunch_restarts_at_once_and_keeps_the_count() {
        let mut backoff = Backoff::new();
        let _ = backoff.after(Exit::Crash, 0, 0);
        assert_eq!(
            backoff.after(Exit::Relaunch, 0, 0),
            Next::Restart { after_ns: 0 }
        );
        assert_eq!(backoff.crashes(), 1, "a relaunch is no crash");
        assert_eq!(
            backoff.after(Exit::Crash, 0, 0),
            Next::Restart {
                after_ns: 2 * BACKOFF_FIRST_NS
            }
        );
    }

    #[test]
    fn more_relaunches_than_the_window_allows_back_off() {
        let mut backoff = Backoff::new();
        let ms = 1_000_000;
        for i in 0..RELAUNCHES_MAX as u64 {
            assert_eq!(
                backoff.after(Exit::Relaunch, ms, i * ms),
                Next::Restart { after_ns: 0 }
            );
        }
        assert_eq!(
            backoff.after(Exit::Relaunch, ms, 20 * ms),
            Next::Restart {
                after_ns: BACKOFF_FIRST_NS
            },
            "the eleventh relaunch in 10 s is a crash"
        );
        assert_eq!(
            backoff.after(Exit::Relaunch, ms, 21 * ms),
            Next::Restart {
                after_ns: 2 * BACKOFF_FIRST_NS
            }
        );
        assert_eq!(
            backoff.after(Exit::Relaunch, ms, 21 * ms + RELAUNCH_WINDOW_NS),
            Next::Restart { after_ns: 0 },
            "the window has passed"
        );
    }

    #[test]
    fn no_display_restarts_after_a_fixed_wait_and_counts_nothing() {
        assert_eq!(Exit::of_code(Some(NO_DISPLAY_EXIT_CODE)), Exit::NoDisplay);
        let mut backoff = Backoff::new();
        let _ = backoff.after(Exit::Crash, 0, 0);
        for i in 0..3 * RELAUNCHES_MAX as u64 {
            assert_eq!(
                backoff.after(Exit::NoDisplay, 15_000_000_000, i),
                Next::Restart {
                    after_ns: NO_DISPLAY_WAIT_NS
                }
            );
        }
        assert_eq!(backoff.crashes(), 1, "no display is no crash");
        for i in 0..RELAUNCHES_MAX as u64 {
            assert_eq!(
                backoff.after(Exit::Relaunch, 0, 100 + i),
                Next::Restart { after_ns: 0 },
                "no display uses up no relaunch"
            );
        }
    }

    #[test]
    fn a_quit_stops() {
        assert_eq!(Backoff::new().after(Exit::Quit, 0, 0), Next::Stop);
    }
}
