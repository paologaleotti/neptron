//! The ODREN policy: from the source, the effective settings, the monitor
//! profile and the GPU, the modes to try in order, each with its reason,
//! and the game's place in each. A TV profile takes its own short walk
//! over the fixed modes of its standard, with no Switchres. Pure: no I/O,
//! no clock, no global state except the lock that serialises the Switchres
//! calls.
//!
//! The decision procedure, one function per step. The functions without a
//! link are private, in the child modules:
//!
//! 1. `take_source`: the source, axes swapped for a turned game.
//! 2. `read_settings`: the effective settings; the horizontal strategy
//!    defaults to the GPU's ([`HorizontalStrategy::default_for`]).
//! 3. [`walk_ladder`]: the rungs the picture mode walks.
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

mod cable;
mod choose;
mod clock;
mod crt;
mod fit;
mod ladder;
mod offer;
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

pub use cable::{CableSignal, VGA_LINE_RATE_MHZ, cable_signal};
pub use choose::{
    Choice, Notice, NoticeReason, PANEL_SPEED_NOTICE_MIN_PPM, Refused, SPEED_ALERT_MIN_PPM,
    SoftReason, Switch, Winner, choose, notice, speed_alert, switch_for,
};
pub use fit::FIT_EXACT_MARGIN_PERMILLE;
pub use ladder::{Rung, walk_ladder};
pub use offer::{
    ChoicesWork, DeclineReason, Declined, Offer, Offered, PictureChoices, REFERENCE_SOURCES,
    RowAnswer, RowChoice, ScanlineChoice, ScanlineOutcome, SomeGames, for_every_game, offer,
    panel_scanlines, picture_choices,
};
pub use padding::{PADDED_FILL_MIN_PERCENT, Padding, SMALL_PICTURE_MIN_PERCENT};
pub use panel::PANEL_FILL_MIN_SCALE_MILLI;
pub use placement::{
    HorizontalScale, Placement, ScaleClaim, UiPlacement, ui_placement, whole_multiple,
};
pub use plan::{
    Considered, HeldWhole, Plan, Proposal, StrategyReplaced, Timing, WholeReason, speed_beyond,
};
pub use reason::Dropped;
pub use refresh::{
    EXACT_SPEED_MARGIN_PPM, FORCE_50_MHZ, FORCE_60_MHZ, REFRESH_MATCH_MHZ, RefreshKind,
    RefreshTarget, SkippedTarget,
};
pub use settings::{
    FIXED_SUPER_WIDTHS, GAME_WIDTHS_MAX, GameWidths, HorizontalStrategy, PanelScale, PictureStyle,
    PolicySettings, RefreshPolicy, TvWidth, VerticalMode, WidthsError,
};
pub use shape::{
    BORDER_LINES, OVERSCAN_CROP_MAX_LINES_PER_EDGE, OVERSCAN_CROP_SCREEN_LINES, OverscanCrop,
};
pub use source::OrientedSource;
pub use tv::{
    TV_525_CONSOLE_WIDTH_SAMPLES, TV_525_PICTURE_LINES, TV_625_CONSOLE_WIDTH_SAMPLES,
    TV_625_PICTURE_LINES, TV_CONSOLE_LINES_MIN, TV_SAFE_WIDTH_SAMPLES, TV_SPEED_SILENT_PPM,
    TV_WHOLE_FACTOR_TOLERANCE_PERCENT,
};

use neptron_types::{GpuCaps, Modeline, MonitorKind, MonitorProfile, SourceGeometry};

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
/// On a CRT profile, each rung of the picture mode's walk at each refresh
/// target, then a last resort; on a flat panel, each listed mode; on a TV,
/// the modes of the standard's table in the order of the TV rungs. The
/// engine walks the proposals with the validator ([`choose`]); the Display
/// menus call the same function for each option ([`picture_choices`]).
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

/// The plan that the engine shows for `settings`, with its winner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown<V, E> {
    /// The plan of the picture mode whose winner shows.
    pub plan: Plan,
    /// The index of the winner in [`Plan::considered`] and what the
    /// validator returned for it, or `None` when no plan has one.
    pub winner: Option<(usize, V)>,
    /// The proposals of `plan` that the validator refused, in walk order.
    pub refused: Vec<Refused<E>>,
    /// The picture mode of `plan` when it is not the mode of the settings:
    /// the mode of the settings has no picture here.
    pub fallback: Option<PictureStyle>,
    /// The picture mode of the settings.
    pub asked: PictureStyle,
}

impl<V, E> Shown<V, E> {
    /// The notice of the winner (step 8). The picture of the fallback to
    /// `fit` says so in place of the soft reason of the Fill mode, which
    /// the player did not choose.
    pub fn notice(&self) -> Option<Notice> {
        let (index, _) = self.winner.as_ref()?;
        let considered = self.plan.considered.get(*index)?;
        let notice = notice(&self.plan, considered);
        if self.fallback != Some(PictureStyle::Fit) {
            return notice;
        }
        let proposal = considered.outcome.as_ref().ok()?;
        let base = notice.unwrap_or(Notice {
            fitted: considered.rung == Rung::Fitted,
            game_mhz: proposal.timing.game_mhz,
            speed_change: false,
            shown_mhz: proposal.timing.shown_mhz(),
            strategy_replaced: None,
            double_refresh: false,
            style_fallback: false,
            cropped_lines: 0,
            padded_fill_permille: None,
            soft: None,
            small_fill_permille: None,
            whole_steps: None,
            fill_fallback: None,
        });
        Some(Notice {
            fill_fallback: Some(self.asked),
            soft: base.soft.filter(|s| *s != SoftReason::FillMode),
            ..base
        })
    }
}

/// Steps 5 and 6 with the one fallback of a picture mode that has no
/// picture (owner, 2026-11-19: when no mode finds a picture, Fill is the
/// default). The plan of the mode of `settings` comes first; where the
/// validator accepts none of its proposals, the plan of `auto`, then the
/// plan of `fit`, with the same refresh choice and keys. A mode that has a
/// picture never changes. Where no plan has a winner, the plan of the mode
/// of `settings`, with no winner: the engine keeps the mode on screen.
///
/// # Errors
///
/// [`PolicyError`] when the source or the profile gives no plan for the
/// mode of `settings`. A fallback plan that has no plan is skipped.
pub fn decide_shown<V, E>(
    source: &SourceGeometry,
    settings: &PolicySettings,
    profile: &MonitorProfile,
    caps: &GpuCaps,
    mut validate: impl FnMut(&Modeline) -> Result<V, E>,
) -> Result<Shown<V, E>, PolicyError> {
    let asked = settings.style;
    let first = decide(source, settings, profile, caps)?;
    let shown = |plan: Plan, validate: &mut dyn FnMut(&Modeline) -> Result<V, E>, fallback| {
        let choice = choose(&plan, validate);
        let winner = choice.winner.map(|w| (w.index, w.token));
        let refused = choice.refused;
        Shown {
            plan,
            winner,
            refused,
            fallback,
            asked,
        }
    };
    let own = shown(first, &mut validate, None);
    if own.winner.is_some() {
        return Ok(own);
    }
    for style in [PictureStyle::Auto, PictureStyle::Fit] {
        if style == asked {
            continue;
        }
        let other = PolicySettings { style, ..*settings };
        let Ok(plan) = decide(source, &other, profile, caps) else {
            continue;
        };
        let tried = shown(plan, &mut validate, Some(style));
        if tried.winner.is_some() {
            return Ok(tried);
        }
    }
    Ok(own)
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
