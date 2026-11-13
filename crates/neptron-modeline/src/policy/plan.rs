use std::fmt;

use neptron_types::Modeline;

use super::units::{Hz, KHz};
use super::{
    Dropped, HorizontalScale, HorizontalStrategy, OrientedSource, OverscanCrop, Padding,
    PictureStyle, Placement, REFRESH_MATCH_MHZ, RefreshKind, RefreshPolicy, RefreshTarget, Rung,
    SkippedTarget, TV_SPEED_SILENT_PPM,
};

/// Everything the policy decided for one source: the candidates in the
/// order the engine walks them, each with its reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The source after step 1.
    pub source: OrientedSource,
    /// The picture style the plan walks.
    pub style: PictureStyle,
    /// The refresh policy the plan follows.
    pub refresh: RefreshPolicy,
    /// The horizontal strategy after step 2. `None` on a flat panel and on
    /// a TV.
    pub horizontal: Option<HorizontalStrategy>,
    /// Refresh targets that no range covers: the reason the plan falls back
    /// to the game's rate. On a TV, every doubled or forced rate: the
    /// standard fixes the field rate.
    pub skipped: Vec<SkippedTarget>,
    /// Every candidate, in preference order. A dropped candidate keeps its
    /// place, and the engine skips it.
    pub considered: Vec<Considered>,
}

/// One candidate of a plan: a rung at a refresh target, and what came of
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Considered {
    /// The rung of the ladder.
    pub rung: Rung,
    /// The field rate it aims for.
    pub target: RefreshTarget,
    /// The mode and its placement, or why the policy dropped it.
    pub outcome: Result<Proposal, Dropped>,
}

/// A mode the engine can validate and set, with the game's place in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Proposal {
    /// The mode to validate and set.
    pub mode: Modeline,
    /// Index of the profile range (CRT), of the listed mode (panel), or of
    /// the mode in `TvStandard::modes` (TV: 0 progressive, 1 interlaced)
    /// that gives the mode.
    pub source_index: usize,
    /// How the width was filled. `None` on a flat panel and on a TV, where
    /// the mode is 720 samples wide and the placement holds the width.
    pub horizontal: Option<HorizontalScale>,
    /// Crop, destination, filter and UI destination.
    pub placement: Placement,
    /// Field rate, line rate and how game frames map to fields.
    pub timing: Timing,
    /// The game sits in a mode with more lines than it has, because the
    /// profile allows no fewer.
    pub bordered: bool,
    /// The horizontal strategy of the settings could not reach the GPU's
    /// pixel clock floor, and the dynamic super width replaced it.
    pub strategy_replaced: Option<StrategyReplaced>,
    /// The range forces so many lines that the game fills too little of
    /// the screen: the plan puts this mode after every exact mode that is
    /// not padded.
    pub padding: Option<Padding>,
    /// Source lines hidden at the top and the bottom so that a whole
    /// multiple fits the lines the range holds. The crop of the placement
    /// leaves them out.
    pub overscan_crop: Option<OverscanCrop>,
}

/// A horizontal strategy that the dynamic super width replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrategyReplaced {
    /// The strategy of the settings.
    pub from: HorizontalStrategy,
    /// Why it gave no mode: a `PixelClockFloor` with the numbers.
    pub reason: Dropped,
}

/// The rates of a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timing {
    /// The game's frame rate, in millihertz.
    pub game_mhz: u32,
    /// The field rate the proposal aims for.
    pub target: RefreshTarget,
    /// Line rate of the mode, in millihertz.
    pub line_rate_mhz: u64,
    /// Field rate of the mode, in millihertz.
    pub field_rate_mhz: u64,
}

impl Plan {
    /// The proposals in the order the engine walks them, with their index
    /// in [`Plan::considered`].
    pub fn proposals(&self) -> impl Iterator<Item = (usize, &Considered, &Proposal)> {
        self.considered
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.outcome.as_ref().ok().map(|p| (i, c, p)))
    }
}

impl Timing {
    /// Fields per game frame: 2 for double refresh, else 1. The engine
    /// runs the core one frame per this many flips.
    pub fn repeat(&self) -> u8 {
        self.target.repeat
    }

    /// The game runs at a rate other than its own: the field rate per game
    /// frame differs from the game's rate by [`REFRESH_MATCH_MHZ`] or more.
    /// The speed changes by `field_rate_mhz / (repeat * game_mhz)`.
    ///
    /// On a TV ([`RefreshKind::Tv`]) the bound is [`TV_SPEED_SILENT_PPM`]
    /// of the field rate instead: the audio rate control absorbs a smaller
    /// difference, and the game counts as exact.
    pub fn changes_speed(&self) -> bool {
        let shown = u64::from(self.game_mhz) * u64::from(self.repeat());
        let off = self.field_rate_mhz.abs_diff(shown);
        if self.target.kind == RefreshKind::Tv {
            u128::from(off) * 1_000_000
                > u128::from(self.field_rate_mhz) * u128::from(TV_SPEED_SILENT_PPM)
        } else {
            off >= REFRESH_MATCH_MHZ
        }
    }
}

impl fmt::Display for Considered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {} ({}): ",
            self.rung,
            Hz(u64::from(self.target.rate_mhz)),
            self.target.kind
        )?;
        match &self.outcome {
            Err(dropped) => write!(f, "dropped, {dropped}"),
            Ok(p) => {
                write!(f, "{p}")?;
                if let Some(replaced) = p.strategy_replaced {
                    write!(f, ", replaces {}: {}", replaced.from, replaced.reason)?;
                }
                if let Some(crop) = p.overscan_crop {
                    write!(
                        f,
                        ", {} lines cropped per edge: {} lines do not fit {}",
                        crop.lines_per_edge, crop.asked_lines, crop.capacity_lines
                    )?;
                }
                if let Some(pad) = p.padding {
                    write!(
                        f,
                        ", padded: {} of {} lines at {} and {}",
                        pad.game_lines,
                        pad.picture_lines,
                        KHz(pad.line_rate_min_mhz),
                        Hz(u64::from(pad.field_rate_mhz))
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for Proposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = &self.mode;
        let d = &self.placement.dest;
        write!(
            f,
            "{}x{}{} at {} and {}, game {}x{} at {},{}",
            m.hactive,
            m.vactive,
            if m.interlace { "i" } else { "p" },
            KHz(self.timing.line_rate_mhz),
            Hz(self.timing.field_rate_mhz),
            d.w,
            d.h,
            d.x,
            d.y
        )
    }
}
