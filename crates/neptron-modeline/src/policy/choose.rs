use neptron_types::Modeline;

use super::panel::PANEL_FILL_MIN_SCALE_MILLI;
use super::placement::{fitted_height, scale_milli};
use super::{
    Considered, DeclineReason, Dropped, PictureStyle, Plan, Proposal, RefreshKind, Rung,
    SMALL_PICTURE_MIN_PERCENT, StrategyReplaced, WholeReason, speed_beyond,
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
    /// although the refresh choice did not ask for it, because the monitor
    /// cannot show the game's lines at the game's rate. The game speed does
    /// not change.
    pub double_refresh: bool,
    /// The winner is not on the first rung of the picture mode: the
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
    /// A flat panel shows a whole multiple, with borders, where the picture
    /// mode would fill the height, for this reason.
    pub whole_steps: Option<WholeReason>,
    /// No other picture mode has a picture for this game on this display,
    /// so the picture is the one of `fit` ([`super::decide_shown`]).
    pub fill_fallback: bool,
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
    /// The picture mode `fit` asks the game to fill the screen.
    FillMode,
}

/// One thing that the quick menu tells the player about the picture, from
/// [`Notice::reasons`]. The app holds the texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoticeReason {
    /// No other picture mode has a picture here: the picture is the one of
    /// Fill.
    FillFallback,
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
    /// A flat panel shows a whole multiple, with borders, where the
    /// picture mode would fill the height.
    WholeSteps(WholeReason),
}

impl Notice {
    /// The reasons that the quick menu shows, in this order: the Fill
    /// picture of the fallback, soft, small,
    /// speed, double refresh, whole steps. No allocation: the notice is read on the
    /// frame thread.
    pub fn reasons(&self) -> impl Iterator<Item = NoticeReason> {
        let speed = self.speed_change.then_some(NoticeReason::SpeedChange {
            game_mhz: self.game_mhz,
            shown_mhz: self.shown_mhz,
        });
        [
            self.fill_fallback.then_some(NoticeReason::FillFallback),
            self.soft.map(NoticeReason::Soft),
            self.small_fill_permille
                .map(|fill_permille| NoticeReason::SmallPicture { fill_permille }),
            speed,
            self.double_refresh.then_some(NoticeReason::DoubleRefresh {
                field_mhz: self.shown_mhz * 2,
            }),
            self.whole_steps.map(NoticeReason::WholeSteps),
        ]
        .into_iter()
        .flatten()
    }
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
/// is the fitted rung, when the game's speed changes by more than
/// [`SPEED_ALERT_MIN_PPM`] (on a flat panel by more than
/// [`PANEL_SPEED_NOTICE_MIN_PPM`] too), when the dynamic super width
/// replaced the horizontal strategy of the settings, when the `native`
/// style took double refresh on its own, when the winner is not what the
/// picture mode asks for ([`mode_refusal`]), when an overscan crop hides
/// source lines, when the winner is padded or small, when a flat panel
/// fills below the scale of 4 because the fit is asked, or when a flat
/// panel keeps whole steps where the mode would fill. Double refresh that
/// the refresh choice asks for shows each frame twice at the game's own
/// rate, so it needs no notice.
pub fn notice(plan: &Plan, considered: &Considered) -> Option<Notice> {
    let proposal = considered.outcome.as_ref().ok()?;
    let timing = &proposal.timing;
    let fitted = considered.rung == Rung::Fitted;
    // A panel runs every game at its own rate and the status shows the
    // speed; the notice speaks only for a large change.
    let game = u64::from(timing.game_mhz);
    let shown = timing.shown_mhz();
    let refresh_differs = timing.changes_speed()
        && speed_alert(game, shown)
        && (timing.target.kind != RefreshKind::Panel
            || speed_beyond(game, shown, PANEL_SPEED_NOTICE_MIN_PPM));
    let strategy_replaced = proposal.strategy_replaced;
    let double_refresh = timing.target.kind == RefreshKind::NativeDouble;
    let style_fallback = mode_refusal(plan, plan.style, considered).is_some();
    let cropped_lines = proposal.overscan_crop.map_or(0, |crop| crop.lines());
    let padded_fill_permille = proposal
        .padding
        .map(|pad| u16::try_from(pad.fill_permille()).unwrap_or(u16::MAX));
    let soft = if fitted {
        Some(soft_reason(plan, considered))
    } else {
        panel_fill_reason(plan, considered, proposal)
    };
    let small_fill_permille = small_fill(proposal);
    let whole_steps = held_whole(plan, considered, proposal);
    let any = fitted
        || refresh_differs
        || strategy_replaced.is_some()
        || double_refresh
        || style_fallback
        || cropped_lines > 0
        || padded_fill_permille.is_some()
        || small_fill_permille.is_some()
        || soft.is_some()
        || whole_steps.is_some();
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
        whole_steps,
        fill_fallback: false,
    })
}

/// The soft reason of a flat-panel fill below `PANEL_FILL_MIN_SCALE_MILLI`:
/// only the picture mode `fit` or `display.vertical = fit` asks for it, and
/// sharp bilinear blends edges wider than a quarter of a game pixel there.
/// `None` for every other placement.
fn panel_fill_reason(
    plan: &Plan,
    considered: &Considered,
    proposal: &Proposal,
) -> Option<SoftReason> {
    let soft_fill = considered.rung == Rung::Panel
        && proposal.placement.scale.y.is_none()
        && scale_milli(&plan.source, &proposal.mode) < PANEL_FILL_MIN_SCALE_MILLI;
    soft_fill.then(|| {
        if plan.style == PictureStyle::Fit {
            SoftReason::FillMode
        } else {
            SoftReason::Chosen
        }
    })
}

/// The reason of a whole multiple that a flat panel keeps where the
/// picture mode would fill: the winner is that multiple, and the fill of
/// the mode would differ from it.
fn held_whole(plan: &Plan, considered: &Considered, proposal: &Proposal) -> Option<WholeReason> {
    let held = plan.held_whole?;
    let scale = scale_milli(&plan.source, &proposal.mode);
    let fills = scale >= held.fill_from_milli.max(SCALE_ONE_MILLI);
    let differs = proposal.placement.dest.h != fitted_height(&plan.source, &proposal.mode);
    (considered.rung == Rung::Panel && proposal.placement.scale.y.is_some() && fills && differs)
        .then_some(held.by)
}

/// One, in thousandths.
const SCALE_ONE_MILLI: u32 = 1_000;

/// The smallest speed change that sets the alert, in ppm of the shown
/// rate: 0.5 %, on every display class. It is the bound of the audio rate
/// control (`neptron_audio::MAX_RATE_DEVIATION_PPM`): below it the change
/// of pitch and speed cannot be heard or seen in play.
pub const SPEED_ALERT_MIN_PPM: u32 = 5_000;

/// The speed of a game at `game_mhz` shown at `shown_mhz` (the field rate
/// per game frame) changes by more than [`SPEED_ALERT_MIN_PPM`] of the
/// shown rate: the status line sets its alert, and the notice names the
/// speed. The one rule of the alert for every display class.
pub fn speed_alert(game_mhz: u64, shown_mhz: u64) -> bool {
    speed_beyond(game_mhz, shown_mhz, SPEED_ALERT_MIN_PPM)
}

/// The speed change of a flat panel, in ppm of the shown rate, from which
/// the notice names it: 5 % (V2, owner, 2026-11-16). A panel runs every
/// game at its own rate, so a change of a few percent is its normal way:
/// the status line shows it with the alert above [`SPEED_ALERT_MIN_PPM`],
/// and the notice sentence waits for 5 %.
pub const PANEL_SPEED_NOTICE_MIN_PPM: u32 = 50_000;

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
        return if plan.style == PictureStyle::Fit {
            SoftReason::FillMode
        } else {
            SoftReason::Chosen
        };
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

/// Why the winner `considered` is not what the picture mode `style` asks
/// for, or `None` when it is. Auto and `fit` take every winner. `sharp`
/// takes every exact rung. `speed` takes a winner that keeps the game's
/// speed within [`SPEED_ALERT_MIN_PPM`], off a TV; a TV standard fixes the
/// rate. `native` and a multiple take the first rung of their walk, on a
/// flat panel too. The menus ([`super::offer`]) and the notice
/// (`style_fallback`) read this one rule.
pub(crate) fn mode_refusal(
    plan: &Plan,
    style: PictureStyle,
    considered: &Considered,
) -> Option<DeclineReason> {
    let rung = considered.rung;
    let on = |kind| plan.considered.iter().any(|c| c.target.kind == kind);
    match style {
        PictureStyle::Auto | PictureStyle::Fit => None,
        PictureStyle::Sharp => rung.is_fitted().then_some(DeclineReason::NoExactMode),
        PictureStyle::Speed if on(RefreshKind::Tv) => Some(DeclineReason::OneRateOnTv),
        PictureStyle::Speed => {
            let timing = considered.outcome.as_ref().ok()?.timing;
            speed_alert(u64::from(timing.game_mhz), timing.shown_mhz())
                .then_some(DeclineReason::SpeedNotHeld)
        }
        PictureStyle::Native | PictureStyle::X2 | PictureStyle::X3 | PictureStyle::X4 => {
            if super::walk_ladder(style).first() == Some(&rung) {
                None
            } else if on(RefreshKind::Panel) {
                Some(DeclineReason::TooLargeForScreen {
                    asked: style,
                    shown: rung,
                })
            } else {
                Some(DeclineReason::NotShownHere {
                    asked: style,
                    shown: rung,
                })
            }
        }
    }
}
