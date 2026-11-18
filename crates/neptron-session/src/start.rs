//! Why a session that the core stopped ended: a game that did not start,
//! or the end of a game that ran. Pure code, with no clock and no host.

use neptron_libretro::{LogLevel, LogRecord, LogSource};

use crate::EndReason;

/// The time after the launch in which a core stop counts as a failed start
/// when the core said something at the launch (an error line or a
/// message), in nanoseconds of the session clock: 10 s.
///
/// A core that cannot run a game shows its own error screen and stops at
/// the first button press (FinalBurn Neo does). A player who sees that
/// screen presses a button within a few seconds; 10 s leaves room for a
/// slow reader. A game that runs is seldom stopped by its core so soon,
/// and a stop after the window gives the notice of a stopped game, so a
/// wrong label never hides the stop.
pub const START_WINDOW_NS: u64 = 10_000_000_000;

/// The most error-level core log lines that a session keeps from its
/// launch. FinalBurn Neo logs three when its driver does not start, and
/// sends the same three lines as messages.
pub const START_ERRORS_MAX: usize = 8;

/// What the session knows about the start of the game when the core asks
/// to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartFacts {
    /// A loop frame ran the core with a picture of the core to show. A
    /// mode that the inputs refuse hides the picture, and the output off
    /// shows nothing, but the game has started all the same.
    pub picture: bool,
    /// The core logged an error line or sent a message at the launch.
    pub said_at_launch: bool,
    /// The session clock at the end of the launch, in nanoseconds.
    pub started_ns: u64,
}

/// Why the session ends when the core asks to stop at `now_ns`: a failed
/// start before any picture, or within [`START_WINDOW_NS`] of a launch at
/// which the core said something; else the end of a game that ran.
pub fn end_reason(facts: StartFacts, now_ns: u64) -> EndReason {
    let soon = now_ns.saturating_sub(facts.started_ns) < START_WINDOW_NS;
    if !facts.picture || (facts.said_at_launch && soon) {
        EndReason::FailedStart
    } else {
        EndReason::CoreShutdown
    }
}

/// Adds the error-level lines of the core in `logs` to `out`, up to
/// [`START_ERRORS_MAX`] lines in all. Lines of other levels and lines of
/// the host are left out.
pub(crate) fn collect_errors(out: &mut Vec<String>, logs: &[LogRecord]) {
    let errors = logs
        .iter()
        .filter(|r| r.level == LogLevel::Error && r.source == LogSource::Core);
    for record in errors {
        if out.len() >= START_ERRORS_MAX {
            return;
        }
        out.push(record.text.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAUNCH_NS: u64 = 5_000_000;

    fn facts(picture: bool, said_at_launch: bool) -> StartFacts {
        StartFacts {
            picture,
            said_at_launch,
            started_ns: LAUNCH_NS,
        }
    }

    #[test]
    fn a_stop_before_any_picture_is_a_failed_start_at_any_time() {
        for now in [LAUNCH_NS, LAUNCH_NS + START_WINDOW_NS * 100] {
            assert_eq!(end_reason(facts(false, false), now), EndReason::FailedStart);
        }
    }

    #[test]
    fn words_at_the_launch_make_a_failed_start_only_within_the_window() {
        let f = facts(true, true);
        assert_eq!(end_reason(f, LAUNCH_NS), EndReason::FailedStart);
        assert_eq!(
            end_reason(f, LAUNCH_NS + START_WINDOW_NS - 1),
            EndReason::FailedStart
        );
        assert_eq!(
            end_reason(f, LAUNCH_NS + START_WINDOW_NS),
            EndReason::CoreShutdown
        );
        assert_eq!(
            end_reason(f, LAUNCH_NS + 3_600 * 1_000_000_000),
            EndReason::CoreShutdown,
            "after an hour of play"
        );
    }

    #[test]
    fn a_silent_launch_with_a_picture_ends_a_game_that_ran() {
        assert_eq!(
            end_reason(facts(true, false), LAUNCH_NS),
            EndReason::CoreShutdown
        );
    }

    #[test]
    fn a_clock_before_the_launch_counts_as_within_the_window() {
        assert_eq!(end_reason(facts(true, true), 0), EndReason::FailedStart);
    }

    fn record(level: LogLevel, source: LogSource, text: &str) -> LogRecord {
        LogRecord {
            level,
            source,
            text: text.to_owned(),
        }
    }

    #[test]
    fn only_error_lines_of_the_core_count() {
        let logs = [
            record(LogLevel::Debug, LogSource::Core, "debug"),
            record(LogLevel::Info, LogSource::Core, "info"),
            record(LogLevel::Warn, LogSource::Core, "warn"),
            record(LogLevel::Error, LogSource::Host, "host error"),
            record(LogLevel::Error, LogSource::Core, "core error"),
        ];
        let mut out = Vec::new();
        collect_errors(&mut out, &logs);
        assert_eq!(out, ["core error"]);
    }

    #[test]
    fn the_error_lines_stop_at_the_cap() {
        let logs: Vec<LogRecord> = (0..START_ERRORS_MAX + 4)
            .map(|i| record(LogLevel::Error, LogSource::Core, &format!("e{i}")))
            .collect();
        let mut out = vec!["earlier".to_owned()];
        collect_errors(&mut out, &logs);
        assert_eq!(out.len(), START_ERRORS_MAX);
        assert_eq!(out[0], "earlier");
        assert_eq!(out[1], "e0");
        collect_errors(&mut out, &logs);
        assert_eq!(out.len(), START_ERRORS_MAX, "a full list takes no more");
    }
}
