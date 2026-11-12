//! When the frontend starts again after it exits: the backoff.

/// The exit code with which the frontend asks to be started again at once,
/// for a planned relaunch: into a game, or back to the game list. The
/// frontend has the same number in `neptron::RELAUNCH_EXIT_CODE`. 75 is
/// `EX_TEMPFAIL` of `sysexits.h`.
pub const RELAUNCH_EXIT_CODE: i32 = 75;

/// The wait before the first restart after a crash, in nanoseconds.
pub const BACKOFF_FIRST_NS: u64 = 250_000_000;

/// The longest wait before a restart, in nanoseconds. The wait doubles
/// with each crash in a row up to this time.
pub const BACKOFF_MAX_NS: u64 = 8_000_000_000;

/// A run at least this long, in nanoseconds, ends a series of crashes: the
/// next crash waits [`BACKOFF_FIRST_NS`] again.
pub const STABLE_RUN_NS: u64 = 30_000_000_000;

/// How one run of the frontend ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Exit code 0: the frontend stopped on request. The launcher stops too.
    Quit,
    /// [`RELAUNCH_EXIT_CODE`]: start the frontend again at once.
    Relaunch,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Backoff {
    crashes: u32,
}

impl Exit {
    /// The exit of a process with this exit code, `None` for a signal.
    pub fn of_code(code: Option<i32>) -> Self {
        match code {
            Some(0) => Self::Quit,
            Some(RELAUNCH_EXIT_CODE) => Self::Relaunch,
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

    /// What follows a run that ended with `exit` after `ran_ns`.
    pub fn after(&mut self, exit: Exit, ran_ns: u64) -> Next {
        if ran_ns >= STABLE_RUN_NS {
            self.crashes = 0;
        }
        match exit {
            Exit::Quit => Next::Stop,
            Exit::Relaunch => Next::Restart { after_ns: 0 },
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
            .map(|_| match backoff.after(Exit::Crash, 1_000_000) {
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
            let _ = backoff.after(Exit::Crash, 0);
        }
        assert_eq!(
            backoff.after(Exit::Crash, STABLE_RUN_NS),
            Next::Restart {
                after_ns: BACKOFF_FIRST_NS
            }
        );
        assert_eq!(backoff.crashes(), 1);
    }

    #[test]
    fn a_relaunch_restarts_at_once_and_keeps_the_count() {
        let mut backoff = Backoff::new();
        let _ = backoff.after(Exit::Crash, 0);
        assert_eq!(
            backoff.after(Exit::Relaunch, 0),
            Next::Restart { after_ns: 0 }
        );
        assert_eq!(backoff.crashes(), 1, "a relaunch is no crash");
        assert_eq!(
            backoff.after(Exit::Crash, 0),
            Next::Restart {
                after_ns: 2 * BACKOFF_FIRST_NS
            }
        );
    }

    #[test]
    fn a_quit_stops() {
        assert_eq!(Backoff::new().after(Exit::Quit, 0), Next::Stop);
    }
}
