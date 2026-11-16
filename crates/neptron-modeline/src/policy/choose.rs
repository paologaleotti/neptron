use neptron_types::{GpuCaps, Modeline, MonitorProfile, SourceGeometry};

use super::{
    Considered, Dropped, PictureStyle, Plan, PolicyError, PolicySettings, Proposal, RefreshKind,
    Rung, SMALL_PICTURE_MIN_PERCENT, StrategyReplaced, Timing, decide,
};

/// The result of step 6: the first proposal that the validator accepts,
/// and the validator's reason for each proposal before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice<'p, V, E> {
    /// The winner, or `None` when the validator refuses every proposal.
    pub winner: Option<Winner<'p, V>>,
    /// The proposals the validator refused, in walk order.
    pub refused: Vec<Refused<E>>,
}

/// The proposal that won, with what the validator returned for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Winner<'p, V> {
    /// Index of the winner in [`Plan::considered`].
    pub index: usize,
    /// The candidate: its rung and its target.
    pub considered: &'p Considered,
    /// The mode and its placement.
    pub proposal: &'p Proposal,
    /// What the validator returned: a `ValidatedMode` for the engine.
    pub token: V,
}

/// A proposal the validator refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused<E> {
    /// Index of the proposal in [`Plan::considered`].
    pub index: usize,
    /// The validator's reason.
    pub reason: E,
}

/// What step 7 does with a winner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Switch {
    /// The winner equals the current mode: update only scale and crop.
    ScaleOnly,
    /// A new mode: commit it once the request has been stable for the
    /// debounce frames.
    Modeset,
}

/// What step 8 tells the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Notice {
    /// The winner is the fitted rung: the picture is not pixel exact.
    pub fitted: bool,
    /// The game's rate, in millihertz.
    pub game_mhz: u32,
    /// The game runs at another rate than its own (`Timing::changes_speed`),
    /// on a CRT or a TV. On a panel the status alone shows the speed.
    pub speed_change: bool,
    /// The rate the game runs at: the field rate per game frame, in
    /// millihertz.
    pub shown_mhz: u64,
    /// The horizontal strategy of the settings gave no mode above the
    /// GPU's pixel clock floor, and the dynamic super width replaced it.
    pub strategy_replaced: Option<StrategyReplaced>,
    /// The `native` style shows each frame twice at twice the game's rate,
    /// although the refresh policy did not ask for it, because the monitor
    /// cannot show the game's lines at the game's rate. The game speed does
    /// not change.
    pub double_refresh: bool,
    /// The winner is not on the first rung of the picture style: the
    /// picture is not the one the style names (a smaller multiple, rung 3
    /// or the fit). Auto has no rung of its own.
    pub style_fallback: bool,
    /// Source lines that the overscan crop hides, both edges together; 0
    /// when none.
    pub cropped_lines: u16,
    /// The winner is padded: the game fills this share of the picture
    /// lines, in per mille (`Padding::fill_permille`), so the picture is
    /// small. `None` when the winner is not padded.
    pub padded_fill_permille: Option<u16>,
    /// Why the picture is soft: the winner is a fit, and no exact mode
    /// exists, or the user asked for the fit. `None` for an exact winner
    /// and for the fill of the panel rule.
    pub soft: Option<SoftReason>,
    /// The game fills less than `SMALL_PICTURE_MIN_PERCENT` of the screen
    /// height: this share, in per mille. A CRT counts the picture lines of
    /// the range; a panel and a TV count the active lines of the mode.
    pub small_fill_permille: Option<u16>,
}

/// Why a picture is soft (the winner is a fit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoftReason {
    /// The game's lines fit no exact mode of the display at its rate.
    TooTall {
        /// Lines of the game on the screen.
        lines: u16,
    },
    /// The game needs an interlaced mode for its lines, and neither the
    /// monitor profile nor the GPU offers one.
    NoInterlace {
        /// Lines of the game on the screen.
        lines: u16,
    },
    /// No exact mode at any rate the display covers: the fit runs at the
    /// rate the display reaches.
    NoRate,
    /// `display.vertical = fit` asks a turned game to fill the screen.
    Chosen,
}

/// One thing that the quick menu tells the player about the picture, from
/// [`Notice::reasons`]. The app holds the texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoticeReason {
    /// The picture is soft, for this reason.
    Soft(SoftReason),
    /// The picture is small: the game fills this share of the screen
    /// height, in per mille.
    SmallPicture {
        /// The share, in per mille.
        fill_permille: u16,
    },
    /// The game runs at another rate than its own.
    SpeedChange {
        /// The game's rate, in millihertz.
        game_mhz: u32,
        /// The rate it runs at, in millihertz.
        shown_mhz: u64,
    },
    /// Each frame shows twice; the game speed does not change.
    DoubleRefresh {
        /// The field rate, in millihertz.
        field_mhz: u64,
    },
}

impl Notice {
    /// The reasons that the quick menu shows, in this order: soft, small,
    /// speed, double refresh. No allocation: the notice is read on the
    /// frame thread.
    pub fn reasons(&self) -> impl Iterator<Item = NoticeReason> {
        let speed = self.speed_change.then_some(NoticeReason::SpeedChange {
            game_mhz: self.game_mhz,
            shown_mhz: self.shown_mhz,
        });
        [
            self.soft.map(NoticeReason::Soft),
            self.small_fill_permille
                .map(|fill_permille| NoticeReason::SmallPicture { fill_permille }),
            speed,
            self.double_refresh.then_some(NoticeReason::DoubleRefresh {
                field_mhz: self.shown_mhz * 2,
            }),
        ]
        .into_iter()
        .flatten()
    }
}

/// One row of the picture style list of the Display panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleRow<E> {
    /// The style.
    pub style: PictureStyle,
    /// The plan for the style, from the same [`decide`] the engine calls.
    pub plan: Plan,
    /// Index of the winner in the plan, or `None` when nothing validates.
    pub winner: Option<usize>,
    /// The proposals the validator refused.
    pub refused: Vec<Refused<E>>,
    /// The winner is on the style's own rung: the row is not greyed out.
    /// Auto is available whenever something wins.
    pub available: bool,
}

/// Step 6: walk the proposals in order and ask the validator about each.
/// The first one it accepts wins; its reasons for the earlier ones are kept
/// for the log and the notice.
///
/// The engine passes `|mode| neptron_safety::validate(mode, profile, caps)`
/// and gets the `ValidatedMode` to set as the winner's token.
pub fn choose<V, E>(
    plan: &Plan,
    mut validate: impl FnMut(&Modeline) -> Result<V, E>,
) -> Choice<'_, V, E> {
    let mut refused = Vec::new();
    for (index, considered, proposal) in plan.proposals() {
        match validate(&proposal.mode) {
            Ok(token) => {
                return Choice {
                    winner: Some(Winner {
                        index,
                        considered,
                        proposal,
                        token,
                    }),
                    refused,
                };
            }
            Err(reason) => refused.push(Refused { index, reason }),
        }
    }
    Choice {
        winner: None,
        refused,
    }
}

/// Step 7: a winner equal to the current mode changes only scale and crop;
/// any other winner is a modeset, which the engine debounces.
pub fn switch_for(current: Option<&Modeline>, winner: &Modeline) -> Switch {
    if current == Some(winner) {
        Switch::ScaleOnly
    } else {
        Switch::Modeset
    }
}

/// Step 8 for a candidate of `plan`, the winner: a notice when the winner
/// is the fitted rung, when the game runs at a rate other than its own,
/// when the dynamic super width replaced the horizontal strategy of the
/// settings, when the `native` style took double refresh on its own, when
/// the winner is not on the style's own rung, when an overscan crop
/// hides source lines, or when the winner is padded. Double refresh that the
/// refresh policy asks for shows each frame twice at the game's own rate,
/// so it needs no notice. A flat panel needs none for its rate: the game
/// and its audio run at the panel's flip rate over `repeat`. A TV shows
/// one only beyond
/// `TV_SPEED_SILENT_PPM`, the threshold of the alert
/// ([`super::Timing::changes_speed`]).
pub fn notice(plan: &Plan, considered: &Considered) -> Option<Notice> {
    let proposal = considered.outcome.as_ref().ok()?;
    let timing = &proposal.timing;
    let fitted = considered.rung == Rung::Fitted;
    // A panel runs every game at its own rate and the status shows the
    // speed; the notice speaks only for a large change.
    let refresh_differs = timing.changes_speed()
        && (timing.target.kind != RefreshKind::Panel
            || speed_ppm(timing) > PANEL_SPEED_NOTICE_MIN_PPM);
    let strategy_replaced = proposal.strategy_replaced;
    let double_refresh = timing.target.kind == RefreshKind::NativeDouble;
    let style_fallback = !on_own_rung(plan, plan.style, considered.rung);
    let cropped_lines = proposal.overscan_crop.map_or(0, |crop| crop.lines());
    let padded_fill_permille = proposal
        .padding
        .map(|pad| u16::try_from(pad.fill_permille()).unwrap_or(u16::MAX));
    let soft = fitted.then(|| soft_reason(plan, considered));
    let small_fill_permille = small_fill(proposal);
    let any = fitted
        || refresh_differs
        || strategy_replaced.is_some()
        || double_refresh
        || style_fallback
        || cropped_lines > 0
        || padded_fill_permille.is_some()
        || small_fill_permille.is_some();
    any.then(|| Notice {
        fitted,
        game_mhz: timing.game_mhz,
        speed_change: refresh_differs,
        shown_mhz: timing.field_rate_mhz / u64::from(timing.repeat().max(1)),
        strategy_replaced,
        double_refresh,
        style_fallback,
        cropped_lines,
        padded_fill_permille,
        soft,
        small_fill_permille,
    })
}

/// The speed change of a flat panel, in ppm of the game's rate, from
/// which the notice shows it: 5 %. A smaller change shows in the status
/// only.
pub const PANEL_SPEED_NOTICE_MIN_PPM: u64 = 50_000;

/// How far the game's speed moves, in ppm of its rate.
fn speed_ppm(timing: &Timing) -> u64 {
    let shown = timing.field_rate_mhz / u64::from(timing.repeat().max(1));
    let game = u64::from(timing.game_mhz).max(1);
    shown.abs_diff(game) * 1_000_000 / game
}

/// Why the fitted winner of `plan` is soft, from the reasons of the exact
/// candidates before it.
fn soft_reason(plan: &Plan, winner: &Considered) -> SoftReason {
    let lines = plan.source.height;
    let dropped = || {
        plan.considered
            .iter()
            .filter_map(|c| c.outcome.as_ref().err())
    };
    if dropped().any(|d| matches!(d, Dropped::VerticalFit { .. })) {
        return SoftReason::Chosen;
    }
    if winner.target.kind == RefreshKind::Unmatched {
        return SoftReason::NoRate;
    }
    let interlace_refused = plan.considered.iter().any(|c| {
        c.rung == Rung::Interlaced
            && matches!(
                c.outcome,
                Err(Dropped::NoInterlace
                    | Dropped::ScanChanged {
                        asked_interlace: true
                    })
            )
    });
    if interlace_refused {
        SoftReason::NoInterlace { lines }
    } else {
        SoftReason::TooTall { lines }
    }
}

/// The share of the screen height of a small picture, in per mille, or
/// `None` when it fills at least [`SMALL_PICTURE_MIN_PERCENT`]. A fit
/// fills the screen.
fn small_fill(proposal: &Proposal) -> Option<u16> {
    let fill = match proposal.padding {
        Some(pad) => u64::from(pad.fill_permille()),
        None => {
            let shown = u64::from(proposal.placement.dest.h) * 1_000;
            shown / u64::from(proposal.mode.vactive.max(1))
        }
    };
    (fill * 100 < SMALL_PICTURE_MIN_PERCENT * 1_000).then(|| u16::try_from(fill).unwrap_or(0))
}

/// The picture style rows of the Display panel: for each style, the plan
/// and the winner of the same [`decide`] and [`choose`] that the engine
/// runs. A style whose winner is not on its own rung is greyed out; the
/// first candidate on its own rung holds the reason (a dropped reason, a
/// padding, or a validator refusal in `refused`).
pub fn style_rows<E>(
    source: &SourceGeometry,
    settings: &PolicySettings,
    profile: &MonitorProfile,
    caps: &GpuCaps,
    mut validate: impl FnMut(&Modeline) -> Result<(), E>,
) -> Result<Vec<StyleRow<E>>, PolicyError> {
    PictureStyle::ALL
        .into_iter()
        .map(|style| {
            let settings = PolicySettings { style, ..*settings };
            let plan = decide(source, &settings, profile, caps)?;
            let choice = choose(&plan, &mut validate);
            let winner = choice.winner.as_ref().map(|w| (w.index, w.considered.rung));
            let available = winner.is_some_and(|(_, rung)| on_own_rung(&plan, style, rung));
            Ok(StyleRow {
                style,
                winner: winner.map(|(index, _)| index),
                refused: choice.refused,
                available,
                plan,
            })
        })
        .collect()
}

/// `rung` is the style's own rung: the first rung of its walk, or any rung
/// for Auto. A flat panel has one rung.
fn on_own_rung(plan: &Plan, style: PictureStyle, rung: Rung) -> bool {
    if style == PictureStyle::Auto {
        return true;
    }
    let panel = plan
        .considered
        .iter()
        .any(|c| c.target.kind == RefreshKind::Panel);
    if panel {
        return rung == Rung::Panel;
    }
    super::walk_ladder(style).first() == Some(&rung)
}
