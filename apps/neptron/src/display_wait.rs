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
use neptron_kms::{ConnectorClass, ConnectorFacts, choose};
use neptron_platform::{ConnectorStatus, Platform};

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
    /// A stop request ended the wait.
    Stopped {
        /// The time waited.
        waited_ns: u64,
    },
}

/// True when a connector of `platform` qualifies for the `auto` output by
/// the rule of [`neptron_kms::choose`]: connected, not a writeback
/// connector, and not an analog connector that reports no EDID, which is
/// used only when the settings name it.
///
/// The status in sysfs is the result of the last probe. A digital
/// connector that reads `unknown` has had no probe yet: it counts as
/// connected here, and the forced probe of the display open decides.
pub fn display_ready(platform: &Platform) -> bool {
    let facts: Vec<ConnectorFacts> = platform
        .connectors
        .iter()
        .map(|c| {
            let class = connector_class(&c.kind);
            ConnectorFacts {
                name: c.name.clone(),
                class,
                connected: match c.status {
                    ConnectorStatus::Connected => true,
                    ConnectorStatus::Unknown => class == ConnectorClass::Digital,
                    ConnectorStatus::Disconnected => false,
                },
                has_edid: c.edid.is_some(),
            }
        })
        .collect();
    choose(&facts, None, false).is_ok()
}

/// The class of a connector by its sysfs type name (`VGA`, `HDMI-A`), as
/// [`ConnectorClass::of_type`] gives it for the type number. sysfs gives
/// only the name.
pub fn connector_class(kind: &str) -> ConnectorClass {
    match kind {
        "Writeback" => ConnectorClass::Writeback,
        "VGA" | "DVI-I" | "DVI-A" | "Composite" | "SVIDEO" | "Component" | "DIN" | "TV" | "DPI" => {
            ConnectorClass::Analog
        }
        _ => ConnectorClass::Digital,
    }
}

/// Calls `ready` until it gives true, with `pause(until_ns)` between two
/// calls, for at most [`DISPLAY_WAIT_MAX_NS`] on `clock`, or until `stop`
/// gives true. `pause` returns at `until_ns` or later; the device sleeps
/// there, a test moves its simulated clock.
pub fn wait_for_display(
    clock: &impl Clock,
    mut pause: impl FnMut(u64),
    stop: impl Fn() -> bool,
    mut ready: impl FnMut() -> bool,
) -> Waited {
    let start_ns = clock.now_ns();
    loop {
        let waited_ns = clock.now_ns().saturating_sub(start_ns);
        if stop() {
            return Waited::Stopped { waited_ns };
        }
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

/// The log line for the end of a wait, with no prefix; `None` when a
/// display was there at once.
pub fn wait_line(waited: Waited) -> Option<String> {
    match waited {
        Waited::Ready { waited_ns: 0 } => None,
        Waited::Ready { waited_ns } => Some(format!(
            "a display answered after {} ms",
            waited_ns / 1_000_000
        )),
        Waited::TimedOut { waited_ns } => Some(format!(
            "no connected display after {} ms",
            waited_ns / 1_000_000
        )),
        Waited::Stopped { waited_ns } => Some(format!(
            "a stop request ended the wait for a display after {} ms",
            waited_ns / 1_000_000
        )),
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
        let waited = wait_for_display(&clock, |_| pauses += 1, || false, || true);
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
            || false,
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
            || false,
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
            || false,
        );
        let Waited::TimedOut { waited_ns } = waited else {
            panic!("{waited:?}");
        };
        assert!(waited_ns >= DISPLAY_WAIT_MAX_NS);
        assert!(waited_ns < DISPLAY_WAIT_MAX_NS + 4_000_000_000 + DISPLAY_POLL_NS);
    }

    #[test]
    fn a_digital_connector_whose_status_is_unknown_is_ready() {
        use ConnectorStatus::Unknown;
        for kind in ["DP", "HDMI-A", "DVI-D", "Virtual"] {
            let p = platform(vec![connector("X-1", kind, Unknown, false)]);
            assert!(display_ready(&p), "{kind}");
        }
    }

    #[test]
    fn a_writeback_connector_is_never_a_display() {
        use ConnectorStatus::{Connected, Unknown};
        for status in [Connected, Unknown] {
            let p = platform(vec![connector("Writeback-1", "Writeback", status, true)]);
            assert!(!display_ready(&p), "{status:?}");
        }
    }

    /// The DRM connector type numbers and the names that sysfs gives them
    /// (`drm_connector_enum_list` of the kernel).
    const DRM_CONNECTOR_TYPES: [(u32, &str); 21] = [
        (0, "Unknown"),
        (1, "VGA"),
        (2, "DVI-I"),
        (3, "DVI-D"),
        (4, "DVI-A"),
        (5, "Composite"),
        (6, "SVIDEO"),
        (7, "LVDS"),
        (8, "Component"),
        (9, "DIN"),
        (10, "DP"),
        (11, "HDMI-A"),
        (12, "HDMI-B"),
        (13, "TV"),
        (14, "eDP"),
        (15, "Virtual"),
        (16, "DSI"),
        (17, "DPI"),
        (18, "Writeback"),
        (19, "SPI"),
        (20, "USB"),
    ];

    #[test]
    fn the_class_of_a_sysfs_name_is_the_class_of_its_type_in_neptron_kms() {
        for (number, name) in DRM_CONNECTOR_TYPES {
            assert_eq!(
                connector_class(name),
                neptron_kms::ConnectorClass::of_type(number),
                "{name}"
            );
        }
    }

    #[test]
    fn a_stop_request_ends_the_wait() {
        let clock = SimClock::new();
        let mut looks = 0;
        let waited = wait_for_display(
            &clock,
            |until_ns| clock.advance_to(until_ns).expect("forward"),
            || clock.now_ns() >= 3 * DISPLAY_POLL_NS,
            || {
                looks += 1;
                false
            },
        );
        assert_eq!(
            waited,
            Waited::Stopped {
                waited_ns: 3 * DISPLAY_POLL_NS
            }
        );
        assert_eq!(looks, 3);
    }

    #[test]
    fn each_end_of_the_wait_has_its_own_log_line() {
        assert_eq!(wait_line(Waited::Ready { waited_ns: 0 }), None);
        assert_eq!(
            wait_line(Waited::Ready {
                waited_ns: 1_250_000_000
            })
            .as_deref(),
            Some("a display answered after 1250 ms")
        );
        assert_eq!(
            wait_line(Waited::TimedOut {
                waited_ns: DISPLAY_WAIT_MAX_NS
            })
            .as_deref(),
            Some("no connected display after 15000 ms")
        );
        assert_eq!(
            wait_line(Waited::Stopped {
                waited_ns: 500_000_000
            })
            .as_deref(),
            Some("a stop request ended the wait for a display after 500 ms")
        );
    }
}
