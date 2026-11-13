//! The wait at start for a display that answers.
//!
//! A monitor can answer some seconds after the GPU driver loads: a
//! DisplayPort monitor that wakes from standby, for example. The frontend
//! looks at the connectors in sysfs until one qualifies, for a bounded
//! time, before it opens the display. The display open takes the launcher's
//! descriptor and cannot be tried again, so the wait comes first. A start
//! that still finds no display exits with [`NO_DISPLAY_EXIT_CODE`]: the
//! launcher starts it again after a short fixed wait, with no backoff, and
//! the guard counts no failed boot, because its record is not written yet.

use neptron_backend::Clock;
use neptron_platform::{ANALOG_CONNECTOR_KINDS, ConnectorStatus, Platform};

/// The longest wait at start for a connected display, in nanoseconds.
pub const DISPLAY_WAIT_MAX_NS: u64 = 15_000_000_000;

/// The time between two looks at the connectors, in nanoseconds.
pub const DISPLAY_POLL_NS: u64 = 250_000_000;

/// The exit code of a start that found no connected display. The launcher
/// has the same number in `NO_DISPLAY_EXIT_CODE`. 69 is `EX_UNAVAILABLE`
/// of `sysexits.h`.
pub const NO_DISPLAY_EXIT_CODE: u8 = 69;

/// How the wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    /// A display qualified after this time, in nanoseconds.
    Ready {
        /// The time waited.
        waited_ns: u64,
    },
    /// No display qualified in [`DISPLAY_WAIT_MAX_NS`].
    TimedOut {
        /// The time waited.
        waited_ns: u64,
    },
}

/// True when a connector of `platform` qualifies for the `auto` output by
/// the rule of `neptron_kms::choose`: connected, and not an analog
/// connector that reports no EDID. Such a connector is used only when the
/// settings name it.
pub fn display_ready(platform: &Platform) -> bool {
    platform.connectors.iter().any(|c| {
        let analog = ANALOG_CONNECTOR_KINDS.contains(&c.kind.as_str()) || c.kind == "DVI-I";
        c.status == ConnectorStatus::Connected && !(analog && c.edid.is_none())
    })
}

/// Calls `ready` until it gives true, with `pause(until_ns)` between two
/// calls, for at most [`DISPLAY_WAIT_MAX_NS`] on `clock`. `pause` returns
/// at `until_ns` or later; the device sleeps there, a test moves its
/// simulated clock.
pub fn wait_for_display(
    clock: &impl Clock,
    mut pause: impl FnMut(u64),
    mut ready: impl FnMut() -> bool,
) -> Waited {
    let start_ns = clock.now_ns();
    loop {
        let waited_ns = clock.now_ns().saturating_sub(start_ns);
        if ready() {
            return Waited::Ready { waited_ns };
        }
        if waited_ns >= DISPLAY_WAIT_MAX_NS {
            return Waited::TimedOut { waited_ns };
        }
        let next_ns = waited_ns
            .saturating_add(DISPLAY_POLL_NS)
            .min(DISPLAY_WAIT_MAX_NS);
        pause(start_ns.saturating_add(next_ns));
    }
}

#[cfg(test)]
mod tests {
    use neptron_platform::Connector;
    use neptron_testkit::SimClock;

    use super::*;

    fn connector(name: &str, kind: &str, status: ConnectorStatus, edid: bool) -> Connector {
        Connector {
            card: "card0".to_owned(),
            name: name.to_owned(),
            kind: kind.to_owned(),
            status,
            edid: edid.then(|| vec![0; 128]),
        }
    }

    fn platform(connectors: Vec<Connector>) -> Platform {
        Platform {
            gpus: Vec::new(),
            connectors,
        }
    }

    #[test]
    fn a_connected_digital_display_is_ready_with_or_without_edid() {
        use ConnectorStatus::{Connected, Disconnected};
        for edid in [true, false] {
            let p = platform(vec![
                connector("VGA-1", "VGA", Disconnected, false),
                connector("DP-1", "DP", Connected, edid),
            ]);
            assert!(display_ready(&p), "edid {edid}");
        }
    }

    #[test]
    fn an_analog_connector_with_no_edid_is_never_ready() {
        use ConnectorStatus::{Connected, Disconnected, Unknown};
        for kind in ["VGA", "DVI-I", "DVI-A", "Composite", "DPI"] {
            for status in [Connected, Unknown] {
                let p = platform(vec![connector("X-1", kind, status, false)]);
                assert!(!display_ready(&p), "{kind} {status:?}");
            }
            let p = platform(vec![connector("X-1", kind, Connected, true)]);
            assert!(display_ready(&p), "{kind} with an EDID");
        }
        let p = platform(vec![connector("DP-1", "DP", Disconnected, true)]);
        assert!(!display_ready(&p), "a disconnected digital connector");
        assert!(!display_ready(&Platform::default()), "no connector");
    }

    #[test]
    fn a_display_that_is_there_needs_no_wait() {
        let clock = SimClock::new();
        let mut pauses = 0;
        let waited = wait_for_display(&clock, |_| pauses += 1, || true);
        assert_eq!(waited, Waited::Ready { waited_ns: 0 });
        assert_eq!(pauses, 0);
    }

    #[test]
    fn a_display_that_answers_late_ends_the_wait() {
        let clock = SimClock::starting_at(1_000);
        let mut looks = 0;
        let waited = wait_for_display(
            &clock,
            |until_ns| clock.advance_to(until_ns).expect("forward"),
            || {
                looks += 1;
                looks == 9
            },
        );
        assert_eq!(
            waited,
            Waited::Ready {
                waited_ns: 8 * DISPLAY_POLL_NS
            }
        );
    }

    #[test]
    fn no_display_ends_the_wait_at_the_limit() {
        let clock = SimClock::new();
        let mut looks = 0u64;
        let waited = wait_for_display(
            &clock,
            |until_ns| clock.advance_to(until_ns).expect("forward"),
            || {
                looks += 1;
                false
            },
        );
        assert_eq!(
            waited,
            Waited::TimedOut {
                waited_ns: DISPLAY_WAIT_MAX_NS
            }
        );
        assert_eq!(looks, DISPLAY_WAIT_MAX_NS / DISPLAY_POLL_NS + 1);
    }

    #[test]
    fn a_pause_that_returns_late_does_not_extend_the_limit() {
        let clock = SimClock::new();
        let waited = wait_for_display(
            &clock,
            |until_ns| clock.advance_to(until_ns + 4_000_000_000).expect("forward"),
            || false,
        );
        let Waited::TimedOut { waited_ns } = waited else {
            panic!("{waited:?}");
        };
        assert!(waited_ns >= DISPLAY_WAIT_MAX_NS);
        assert!(waited_ns < DISPLAY_WAIT_MAX_NS + 4_000_000_000 + DISPLAY_POLL_NS);
    }
}
