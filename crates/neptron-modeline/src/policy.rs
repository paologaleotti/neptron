//! The ODREN policy: from the source, the effective settings, the monitor
//! profile and the GPU, the modes to try in order, each with its reason,
//! and the game's place in each. A TV profile takes its own short walk
//! over the fixed modes of its standard, with no Switchres. Pure: no I/O, no clock, no global state
//! except the lock that serialises the Switchres calls.
//!
//! The decision procedure, one function per step. The functions without a
//! link are private, in the child modules:
//!
//! 1. `take_source`: the source, axes swapped for a turned game.
//! 2. `read_settings`: the effective settings; the horizontal strategy
//!    defaults to the GPU's ([`HorizontalStrategy::default_for`]).
//! 3. [`walk_ladder`]: the rungs the picture style walks.
//! 4. `refresh_targets` and `shape`: per candidate, the lines (height
//!    times the factor), the refresh (the game's rate, twice it, or the
//!    forced rate) and the scan.
//! 5. `solve`: the width by the horizontal strategy, and the modeline from
//!    Switchres.
//! 6. [`choose`]: the first proposal that the validator accepts.
//! 7. [`switch_for`]: scale and crop only, or a modeset.
//! 8. [`notice`]: the notice for a fitted rung, a changed rate, double
//!    refresh that the native style took, or a fall to another rung.
//!
//! [`decide`] runs steps 1 to 5. The validator is not a dependency of this
//! crate: the engine runs step 6 with `neptron_safety::validate`.

mod choose;
mod clock;
mod crt;
mod ladder;
mod padding;
mod panel;
mod placement;
mod plan;
mod reason;
mod refresh;
mod settings;
mod shape;
mod solve;
mod source;
mod tv;
mod units;

use std::fmt;

pub use choose::{
    Choice, Notice, Refused, StyleRow, Switch, Winner, choose, notice, style_rows, switch_for,
};
pub use ladder::{Rung, walk_ladder};
pub use padding::{PADDED_FILL_MIN_PERCENT, Padding};
pub use placement::{HorizontalScale, Placement, ScaleClaim, UI_CANVAS};
pub use plan::{Considered, Plan, Proposal, StrategyReplaced, Timing};
pub use reason::Dropped;
pub use refresh::{
    FORCE_50_MHZ, FORCE_60_MHZ, REFRESH_MATCH_MHZ, RefreshKind, RefreshTarget, SkippedTarget,
};
pub use settings::{
    FIXED_SUPER_WIDTHS, HorizontalStrategy, PictureStyle, PolicySettings, RefreshPolicy, TvWidth,
};
pub use shape::{BORDER_LINES, OVERSCAN_CROP_MAX_LINES_PER_EDGE, OverscanCrop};
pub use source::OrientedSource;
pub use tv::{
    TV_CONSOLE_LINES, TV_CONSOLE_WIDTH_SAMPLES, TV_SAFE_WIDTH_SAMPLES, TV_SPEED_SILENT_PPM,
    TV_WHOLE_FACTOR_TOLERANCE_PERCENT,
};

use neptron_types::{GpuCaps, MonitorKind, MonitorProfile, SourceGeometry};

use crate::ModelineError;

/// Why the policy gives no plan.
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyError {
    /// The source has no width, no height or no refresh.
    EmptySource {
        /// Width of the source, in pixels.
        width: u16,
        /// Height of the source, in lines.
        height: u16,
        /// Refresh of the source, in millihertz.
        refresh_mhz: u32,
    },
    /// The profile has no range or no listed mode.
    EmptyProfile {
        /// Id of the profile.
        profile: String,
    },
    /// The solver refuses the profile or the settings.
    Solver(ModelineError),
}

/// Steps 1 to 5 of the decision procedure: the plan for a source.
///
/// On a CRT profile, each rung of the picture style's walk at each refresh
/// target, then a last resort; on a flat panel, each listed mode; on a TV,
/// the modes of the standard's table in the order of the TV rungs. The
/// engine walks the proposals with the validator ([`choose`]); the Display
/// panel calls the same function for each style ([`style_rows`]).
pub fn decide(
    source: &SourceGeometry,
    settings: &PolicySettings,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Result<Plan, PolicyError> {
    let source = source::take_source(source)?;
    let settings = settings::read_settings(settings, caps);
    let empty = || PolicyError::EmptyProfile {
        profile: profile.id.clone(),
    };
    match &profile.kind {
        MonitorKind::Crt { ranges } if ranges.is_empty() => Err(empty()),
        MonitorKind::FlatPanel { modes } if modes.is_empty() => Err(empty()),
        MonitorKind::Tv { standard } => Ok(tv::tv_plan(&source, &settings, *standard, caps)),
        MonitorKind::Crt { ranges } => crt::crt_plan(&crt::Crt {
            source,
            settings,
            profile,
            ranges,
            caps,
        }),
        MonitorKind::FlatPanel { modes } => Ok(panel::panel_plan(&source, &settings, modes)),
    }
}

impl From<ModelineError> for PolicyError {
    fn from(error: ModelineError) -> Self {
        Self::Solver(error)
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySource {
                width,
                height,
                refresh_mhz,
            } => write!(
                f,
                "source {width}x{height} at {refresh_mhz} mHz: every value must be above 0"
            ),
            Self::EmptyProfile { profile } => {
                write!(f, "profile {profile} has no range and no mode")
            }
            Self::Solver(error) => write!(f, "solver: {error}"),
        }
    }
}

impl std::error::Error for PolicyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Solver(error) => Some(error),
            _ => None,
        }
    }
}
