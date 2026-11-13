//! A reboot or a power-off from the System panel. The frontend ends its
//! run as a quit and exits with a code that asks the launcher for it; the
//! launcher asks init, and init runs the shutdown steps of the image. The
//! frontend itself never stops the machine.

use std::ffi::OsStr;

use neptron_video::{Phase, SafeReason};

use crate::shell::Outcome;

/// The exit code that asks the launcher for a reboot. It equals
/// `REBOOT_EXIT_CODE` of `neptron-launch`: 80, free in `sysexits.h`.
pub const REBOOT_EXIT_CODE: u8 = 80;

/// The exit code that asks the launcher for a power-off. It equals
/// `POWER_OFF_EXIT_CODE` of `neptron-launch`: 81.
pub const POWER_OFF_EXIT_CODE: u8 = 81;

/// The environment variable that a launcher with power control sets for
/// the frontend. It equals `POWER_ENV` of `neptron-launch`.
pub const POWER_ENV: &str = "NEPTRON_POWER";

/// True when `value`, the value of [`POWER_ENV`], says that the launcher
/// carries out a reboot or a power-off. Only `1` does: a run with no
/// launcher, or a launcher with no power control, has no value.
pub fn control_from(value: Option<&OsStr>) -> bool {
    value.is_some_and(|v| v == "1")
}

/// A reboot or a power-off that the System panel asks the run for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerRequest {
    /// Start the machine again.
    Reboot,
    /// Turn the machine off.
    PowerOff,
}

impl PowerRequest {
    /// The outcome of a run that ends for this request.
    pub fn outcome(self) -> Outcome {
        match self {
            Self::Reboot => Outcome::Reboot,
            Self::PowerOff => Outcome::PowerOff,
        }
    }
}

/// Whether a run takes a power request: only in the game list
/// (`in_list`), whose run holds no game with save RAM that only its own
/// quit writes in full; only with power control from the launcher
/// (`control`); and only while the run has no outcome yet (`ended`).
pub fn takes_request(in_list: bool, control: bool, ended: bool) -> bool {
    in_list && control && !ended
}

/// Whether a power request gives back the failed boot that the guard
/// counted at the start of this boot. The guard counts each start as
/// failed until the boot is good (`boot_good`). A reboot that the player
/// asks for on a boot that the guard does not hold to be failed is no
/// failed boot:
///
/// - `Confirmed`: the confirmed picture shows. The count goes back.
/// - `Safe(NothingConfirmed)`: a first boot whose prompt reverted; no
///   configuration failed. The count goes back.
/// - `Safe(NoValidMode)`: the configuration gave no picture; the guard
///   holds the boot failed. The count stays, so two such boots reach
///   `Safe(FailedBoots)`.
/// - `Safe(FailedBoots)` and `Safe(Recovery)`: the record is in safe mode
///   by its own rules. The count stays.
/// - `Pending`: no decision yet. The count stays; the run takes no
///   request then anyway.
pub fn gives_back_boot(boot_good: bool, phase: Phase) -> bool {
    !boot_good
        && matches!(
            phase,
            Phase::Confirmed | Phase::Safe(SafeReason::NothingConfirmed)
        )
}

/// The exit code of a run that ends with `outcome` for a reboot or a
/// power-off. `None` for a quit or a relaunch.
pub fn exit_code(outcome: &Outcome) -> Option<u8> {
    match outcome {
        Outcome::Reboot => Some(REBOOT_EXIT_CODE),
        Outcome::PowerOff => Some(POWER_OFF_EXIT_CODE),
        Outcome::Quit | Outcome::Relaunch(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_power_outcome_has_its_code_and_the_others_none() {
        assert_eq!(exit_code(&Outcome::Reboot), Some(REBOOT_EXIT_CODE));
        assert_eq!(exit_code(&Outcome::PowerOff), Some(POWER_OFF_EXIT_CODE));
        assert_eq!(exit_code(&Outcome::Quit), None);
        let list = crate::NextRun {
            game: None,
            notice: None,
        };
        assert_eq!(exit_code(&Outcome::Relaunch(list)), None);
    }

    #[test]
    fn a_request_is_taken_only_in_the_list_with_control_and_no_outcome() {
        assert!(takes_request(true, true, false));
        for (in_list, control, ended) in [
            (false, true, false),
            (true, false, false),
            (true, true, true),
            (false, false, false),
            (false, true, true),
        ] {
            assert!(
                !takes_request(in_list, control, ended),
                "{in_list} {control} {ended}"
            );
        }
        assert_eq!(PowerRequest::Reboot.outcome(), Outcome::Reboot);
        assert_eq!(PowerRequest::PowerOff.outcome(), Outcome::PowerOff);
    }

    #[test]
    fn only_a_boot_the_guard_does_not_hold_failed_gets_its_count_back() {
        assert!(gives_back_boot(false, Phase::Confirmed));
        assert!(gives_back_boot(
            false,
            Phase::Safe(SafeReason::NothingConfirmed)
        ));
        for reason in [
            SafeReason::NoValidMode,
            SafeReason::FailedBoots,
            SafeReason::Recovery,
        ] {
            assert!(!gives_back_boot(false, Phase::Safe(reason)), "{reason:?}");
        }
        assert!(
            !gives_back_boot(true, Phase::Confirmed),
            "a good boot counts nothing"
        );
    }

    #[test]
    fn only_the_value_one_gives_power_control() {
        assert!(control_from(Some(OsStr::new("1"))));
        for value in [None, Some(""), Some("0"), Some("yes"), Some("1 ")] {
            assert!(!control_from(value.map(OsStr::new)), "{value:?}");
        }
    }

    #[test]
    fn the_codes_are_new_codes_below_those_of_a_shell() {
        for code in [REBOOT_EXIT_CODE, POWER_OFF_EXIT_CODE] {
            assert!(![0, 1, 2, 69, 75, 101].contains(&code), "{code}");
            assert!(code < 126, "{code}");
        }
        assert_ne!(REBOOT_EXIT_CODE, POWER_OFF_EXIT_CODE);
    }
}
