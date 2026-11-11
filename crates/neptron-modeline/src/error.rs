use std::fmt;

use neptron_switchres_sys::{BlankingFault, CallError};

/// Why the solver gives no list of modelines.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelineError {
    /// The profile is a flat panel. It lists its modes, and Switchres does
    /// not calculate for it.
    FlatPanel {
        /// Id of the profile.
        profile: String,
    },
    /// The profile has no range.
    NoRanges {
        /// Id of the profile.
        profile: String,
    },
    /// The profile has more ranges than Switchres holds.
    TooManyRanges {
        /// Id of the profile.
        profile: String,
        /// Ranges in the profile.
        count: usize,
        /// Most ranges Switchres holds.
        max: usize,
    },
    /// Switchres refuses a range of the profile.
    InvalidRange {
        /// Id of the profile.
        profile: String,
        /// Index of the range in the profile.
        index: usize,
    },
    /// The blanking of a range is outside the limits that keep Switchres
    /// from looping forever or dividing by zero (see
    /// `neptron_switchres_sys::HBLANK_MAX_FRACTION` and its siblings).
    /// Switchres itself accepts such a range.
    Blanking {
        /// Id of the profile.
        profile: String,
        /// Index of the range in the profile.
        index: usize,
        /// Which limit the range breaks.
        fault: BlankingFault,
        /// The blanking of the range, in nanoseconds.
        blanking_ns: u64,
        /// The limit, in nanoseconds.
        limit_ns: u64,
    },
    /// The source has no width, no height or no refresh.
    EmptySource {
        /// Width of the source, in pixels.
        width: u16,
        /// Height of the source, in lines.
        height: u16,
        /// Refresh of the source, in millihertz.
        refresh_mhz: u32,
    },
    /// The monitor aspect of the settings has a zero part, or is narrower
    /// than 1:4 or wider than 4:1.
    AspectOutOfRange {
        /// Width part of the ratio.
        width: u16,
        /// Height part of the ratio.
        height: u16,
    },
    /// The call into Switchres failed.
    Switchres {
        /// Id of the profile.
        profile: String,
        /// What the call reported.
        error: CallError,
    },
    /// Switchres gave a value that does not fit the contract types.
    Convert {
        /// Id of the profile.
        profile: String,
        /// The value and the limit.
        error: ConvertError,
    },
}

/// A value from Switchres that does not fit the integer contract types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConvertError {
    /// A timing is negative or above the largest value of its field.
    Timing {
        /// Name of the field.
        field: &'static str,
        /// The value from Switchres.
        value: i64,
        /// Largest value of the field.
        max: u64,
    },
    /// A score is not finite, or out of the range of its field after the
    /// conversion.
    Score {
        /// Name of the candidate field.
        field: &'static str,
        /// The value from Switchres.
        value: f64,
    },
}

// The dependency rules allow neptron-types and neptron-switchres-sys only, so
// these impls are written by hand and not derived with thiserror.
impl fmt::Display for ModelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FlatPanel { profile } => write!(
                f,
                "profile {profile} is a flat panel: Switchres calculates for CRT ranges only"
            ),
            Self::NoRanges { profile } => write!(f, "profile {profile} has no CRT range"),
            Self::TooManyRanges {
                profile,
                count,
                max,
            } => write!(
                f,
                "profile {profile} has {count} ranges, but Switchres holds at most {max}"
            ),
            Self::InvalidRange { profile, index } => {
                write!(f, "Switchres refuses range {index} of profile {profile}")
            }
            Self::Blanking {
                profile,
                index,
                fault,
                blanking_ns,
                limit_ns,
            } => {
                let (what, relation) = match fault {
                    BlankingFault::HorizontalTooLong => ("horizontal", "below"),
                    BlankingFault::VerticalTooLong => ("vertical", "below"),
                    BlankingFault::VerticalTooShort => ("vertical", "at least"),
                };
                write!(
                    f,
                    "range {index} of profile {profile}: {what} blanking {blanking_ns} ns, \
                     must be {relation} {limit_ns} ns"
                )
            }
            Self::EmptySource {
                width,
                height,
                refresh_mhz,
            } => write!(
                f,
                "source {width}x{height} at {refresh_mhz} mHz: each value must be above 0"
            ),
            Self::AspectOutOfRange { width, height } => write!(
                f,
                "monitor aspect {width}:{height}: must be from 1:4 to 4:1"
            ),
            Self::Switchres { profile, error } => write!(f, "profile {profile}: {error}"),
            Self::Convert { profile, error } => write!(f, "profile {profile}: {error}"),
        }
    }
}

impl std::error::Error for ModelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Switchres { error, .. } => Some(error),
            Self::Convert { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timing { field, value, max } => {
                write!(f, "Switchres gave {field} {value}, outside 0 to {max}")
            }
            Self::Score { field, value } => write!(
                f,
                "Switchres gave {field} {value}, which does not fit the field"
            ),
        }
    }
}

impl std::error::Error for ConvertError {}
