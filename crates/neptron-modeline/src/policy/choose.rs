use neptron_types::{GpuCaps, Modeline, MonitorProfile, SourceGeometry};

use super::{
    Considered, PictureStyle, Plan, PolicyError, PolicySettings, Proposal, RefreshKind, Rung,
    StrategyReplaced, decide,
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
    let refresh_differs = timing.changes_speed() && timing.target.kind != RefreshKind::Panel;
    let strategy_replaced = proposal.strategy_replaced;
    let double_refresh = timing.target.kind == RefreshKind::NativeDouble;
    let style_fallback = !on_own_rung(plan, plan.style, considered.rung);
    let cropped_lines = proposal.overscan_crop.map_or(0, |crop| crop.lines());
    let padded_fill_permille = proposal
        .padding
        .map(|pad| u16::try_from(pad.fill_permille()).unwrap_or(u16::MAX));
    let any = fitted
        || refresh_differs
        || strategy_replaced.is_some()
        || double_refresh
        || style_fallback
        || cropped_lines > 0
        || padded_fill_permille.is_some();
    any.then(|| Notice {
        fitted,
        game_mhz: timing.game_mhz,
        shown_mhz: timing.field_rate_mhz / u64::from(timing.repeat().max(1)),
        strategy_replaced,
        double_refresh,
        style_fallback,
        cropped_lines,
        padded_fill_permille,
    })
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
