use neptron_types::{CrtRange, GpuCaps, MonitorProfile};

use super::ladder::walk_ladder;
use super::refresh::{game_target, native_double_target, refresh_targets};
use super::settings::Effective;
use super::shape::{Shape, progressive_capacity, shape};
use super::solve::solve;
use super::{
    Considered, Dropped, HorizontalStrategy, OrientedSource, PictureStyle, Plan, PolicyError,
    Proposal, RefreshKind, RefreshTarget, Rung, StrategyReplaced,
};
use crate::ModelineError;

/// The inputs of a plan on a CRT profile.
pub(crate) struct Crt<'a> {
    pub(crate) source: OrientedSource,
    pub(crate) settings: Effective,
    pub(crate) profile: &'a MonitorProfile,
    pub(crate) ranges: &'a [CrtRange],
    pub(crate) caps: &'a GpuCaps,
}

/// Steps 3 to 5 on a CRT profile: every rung of the walk at every refresh
/// target, then the last resort, in preference order.
///
/// The order is: every exact rung before every fitted rung, because the
/// fitted rung is the only one that is not pixel exact; a fitted mode at a
/// doubled or forced rate never beats an exact mode at the game's rate.
/// Then refresh target (the asked target, then the game's rate). Within a
/// target, the exact rungs that are not padded, in ladder order, then the
/// padded exact rungs, the one that fills more of its picture lines first.
/// A padded mode loses to every exact mode that is not padded, so line
/// doubling or double refresh wins on 31 kHz; it still wins over a fitted
/// scale, so a handheld on 15 kHz is centred with borders.
/// The last resort is the fitted rung at whatever rate Switchres reaches,
/// so a profile that has any mode for the source gives a proposal.
///
/// The `native` style shows the game's own lines and orders by rung first
/// (`native_key`): the native rung at each target, then at twice the game's
/// rate when the refresh policy did not ask for it; rung 3; every padded
/// exact mode, the fuller first; the fitted rung.
pub(crate) fn crt_plan(crt: &Crt<'_>) -> Result<Plan, PolicyError> {
    let style = crt.settings.style;
    let rungs = walk_ladder(style);
    let (targets, skipped) =
        refresh_targets(crt.settings.refresh, crt.source.refresh_mhz, crt.ranges);
    let mut walk: Vec<(RefreshTarget, &[Rung])> = targets.iter().map(|&t| (t, rungs)).collect();
    if style == PictureStyle::Native
        && let Some(double) =
            native_double_target(crt.settings.refresh, crt.source.refresh_mhz, crt.ranges)
    {
        walk.push((double, &[Rung::Native]));
    }

    let mut entries = Vec::with_capacity(walk.len() * rungs.len() + 1);
    for (tier, &(target, rungs)) in walk.iter().enumerate() {
        for (place, &rung) in rungs.iter().enumerate() {
            for (sub, outcome) in candidate(rung, target, true, crt)?.into_iter().enumerate() {
                entries.push(entry(style, (tier, place, sub), rung, target, outcome));
            }
        }
    }
    let last_resort = RefreshTarget {
        kind: RefreshKind::Unmatched,
        ..game_target(crt.source.refresh_mhz, crt.ranges)
    };
    for (sub, outcome) in candidate(Rung::Fitted, last_resort, false, crt)?
        .into_iter()
        .enumerate()
    {
        entries.push(entry(
            style,
            (walk.len(), 0, sub),
            Rung::Fitted,
            last_resort,
            outcome,
        ));
    }

    entries.sort_by_key(|(key, _)| *key);
    Ok(Plan {
        source: crt.source,
        style: crt.settings.style,
        refresh: crt.settings.refresh,
        horizontal: Some(crt.settings.horizontal),
        skipped,
        considered: entries.into_iter().map(|(_, c)| c).collect(),
    })
}

type Outcome = Result<Proposal, Dropped>;

/// Steps 4 and 5 for one rung at one target. One outcome, or two when the
/// horizontal strategy of the settings is below the GPU's pixel clock
/// floor: the dropped candidate, then the dynamic super width that
/// replaces it, so the screen still gets a mode.
fn candidate(
    rung: Rung,
    target: RefreshTarget,
    enforce_refresh: bool,
    crt: &Crt<'_>,
) -> Result<Vec<Outcome>, ModelineError> {
    let aspect = crt.settings.solver.monitor_aspect;
    let shape = match shape(
        rung,
        &crt.source,
        crt.ranges,
        crt.caps,
        aspect,
        target.rate_mhz,
    ) {
        Ok(shape) => shape,
        Err(dropped) => return Ok(vec![Err(dropped)]),
    };
    let outcomes = solve_replacing(&shape, target, enforce_refresh, crt)?;
    let missed = matches!(outcomes.last(), Some(Err(Dropped::RefreshMissed { .. })));
    if rung.is_fitted()
        && enforce_refresh
        && missed
        && let Some(retry) = fitted_at_capacity(&shape, target, crt)?
    {
        return Ok(retry);
    }
    Ok(outcomes)
}

/// Step 5 with the strategy of the settings, and the dynamic super width
/// in its place when that strategy is below the GPU's pixel clock floor.
fn solve_replacing(
    shape: &Shape,
    target: RefreshTarget,
    enforce_refresh: bool,
    crt: &Crt<'_>,
) -> Result<Vec<Outcome>, ModelineError> {
    let strategy = crt.settings.horizontal;
    let first = solve(shape, target, enforce_refresh, strategy, crt)?;
    // No whole multiple of a width above the GPU's widest mode fits it, so
    // the dynamic super width cannot replace that case.
    let replaceable =
        strategy != HorizontalStrategy::DynamicSuperWidth && shape.base_width <= crt.caps.max_width;
    let reason = match first {
        Err(reason @ Dropped::PixelClockFloor { .. }) if replaceable => reason,
        other => return Ok(vec![other]),
    };
    let dynamic = solve(
        shape,
        target,
        enforce_refresh,
        HorizontalStrategy::DynamicSuperWidth,
        crt,
    )?
    .map(|proposal| Proposal {
        strategy_replaced: Some(StrategyReplaced {
            from: strategy,
            reason,
        }),
        ..proposal
    });
    Ok(vec![Err(reason), dynamic])
}

/// The fitted rung at the target rate: when Switchres scales the source to
/// more lines than the ranges hold at the rate, ask again for the most
/// progressive lines a range holds at that rate, so the game keeps its
/// rate when the profile allows it ("refresh matches the core to two
/// decimals when the profile allows it").
///
/// `None` when there is no other line count to try, or when the retry
/// misses as well: the first reasons then stay.
fn fitted_at_capacity(
    shape: &Shape,
    target: RefreshTarget,
    crt: &Crt<'_>,
) -> Result<Option<Vec<Outcome>>, ModelineError> {
    let capacity = progressive_capacity(crt.ranges, target.rate_mhz);
    if capacity == 0 || capacity == shape.lines {
        return Ok(None);
    }
    let retry = Shape {
        lines: capacity,
        interlace: false,
        ..*shape
    };
    let outcomes = solve_replacing(&retry, target, true, crt)?;
    Ok(matches!(outcomes.last(), Some(Ok(_))).then_some(outcomes))
}

/// Group, then four places: the entries sort by it. Group 3 is the fitted
/// rung for every style.
type Key = (u8, usize, usize, usize, usize);

/// The order key of an entry; `(tier, place, sub)` is the refresh target,
/// the place on the ladder and the order within one candidate.
fn entry(
    style: PictureStyle,
    (tier, place, sub): (usize, usize, usize),
    rung: Rung,
    target: RefreshTarget,
    outcome: Outcome,
) -> (Key, Considered) {
    let rank = padded_rank(&outcome);
    let key = match (rung.is_fitted(), style) {
        (true, _) => (3, tier, 0, place, sub),
        (false, PictureStyle::Native) => native_key(rung, rank, (tier, place, sub)),
        (false, _) => (0, tier, rank, place, sub),
    };
    (
        key,
        Considered {
            rung,
            target,
            outcome,
        },
    )
}

/// The key of an exact entry of the `native` style: the native rung that
/// is not padded at every target, then rung 3, then every padded mode by
/// its fill, then by target.
fn native_key(rung: Rung, rank: usize, (tier, place, sub): (usize, usize, usize)) -> Key {
    match (rank, rung) {
        (0, Rung::Native) => (0, tier, 0, place, sub),
        (0, _) => (1, tier, 0, place, sub),
        _ => (2, rank, tier, place, sub),
    }
}

/// 0 for an entry that is not padded; else above 0, and lower for a
/// padded mode that fills more of its picture lines.
fn padded_rank(outcome: &Outcome) -> usize {
    match outcome {
        Ok(Proposal {
            padding: Some(padding),
            ..
        }) => {
            let fill = usize::try_from(padding.fill_permille()).unwrap_or(usize::MAX);
            1_001_usize.saturating_sub(fill).max(1)
        }
        _ => 0,
    }
}
