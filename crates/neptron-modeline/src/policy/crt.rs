use neptron_types::{CrtRange, GpuCaps, MonitorProfile};

use super::ladder::walk_ladder;
use super::refresh::{game_target, refresh_targets};
use super::settings::Effective;
use super::shape::{Shape, progressive_capacity, shape};
use super::solve::{Solved, solve};
use super::{
    Considered, Dropped, OrientedSource, Plan, PolicyError, Proposal, RefreshKind, RefreshTarget,
    Rung,
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
/// padded exact rungs. A padded mode loses to every exact mode that is not
/// padded, so line doubling or double refresh wins on 31 kHz; it still wins
/// over a fitted scale, so a handheld on 15 kHz is centred with borders.
/// The last resort is the fitted rung at whatever rate Switchres reaches,
/// so a profile that has any mode for the source gives a proposal.
pub(crate) fn crt_plan(crt: &Crt<'_>) -> Result<Plan, PolicyError> {
    let rungs = walk_ladder(crt.settings.style);
    let (targets, skipped) =
        refresh_targets(crt.settings.refresh, crt.source.refresh_mhz, crt.ranges);

    let mut entries = Vec::with_capacity(targets.len() * rungs.len() + 1);
    for (tier, &target) in targets.iter().enumerate() {
        for (place, &rung) in rungs.iter().enumerate() {
            let outcome = candidate(rung, target, true, crt)?;
            entries.push(entry(tier, place, rung, target, outcome));
        }
    }
    let last_resort = RefreshTarget {
        kind: RefreshKind::Unmatched,
        ..game_target(crt.source.refresh_mhz, crt.ranges)
    };
    let outcome = candidate(Rung::Fitted, last_resort, false, crt)?;
    entries.push(entry(targets.len(), 0, Rung::Fitted, last_resort, outcome));

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

/// Steps 4 and 5 for one rung at one target.
fn candidate(rung: Rung, target: RefreshTarget, enforce_refresh: bool, crt: &Crt<'_>) -> Solved {
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
        Err(dropped) => return Ok(Err(dropped)),
    };
    let solved = solve(&shape, target, enforce_refresh, crt)?;
    match solved {
        Err(Dropped::RefreshMissed { .. }) if rung.is_fitted() => {
            Ok(fitted_at_capacity(&shape, target, crt)?.unwrap_or(solved))
        }
        _ => Ok(solved),
    }
}

/// The fitted rung at the target rate: when Switchres scales the source to
/// more lines than the ranges hold at the rate, ask again for the most
/// progressive lines a range holds at that rate, so the game keeps its
/// rate when the profile allows it ("refresh matches the core to two
/// decimals when the profile allows it").
/// `None` when there is no other line count to try, or when the retry
/// misses as well: the first reason then stays.
fn fitted_at_capacity(
    shape: &Shape,
    target: RefreshTarget,
    crt: &Crt<'_>,
) -> Result<Option<Result<Proposal, Dropped>>, ModelineError> {
    let capacity = progressive_capacity(crt.ranges, target.rate_mhz);
    if capacity == 0 || capacity == shape.lines {
        return Ok(None);
    }
    let retry = Shape {
        lines: capacity,
        interlace: false,
        ..*shape
    };
    Ok(solve(&retry, target, true, crt)?.ok().map(Ok))
}

type Key = (bool, usize, u8, usize);

fn entry(
    tier: usize,
    place: usize,
    rung: Rung,
    target: RefreshTarget,
    outcome: Result<Proposal, Dropped>,
) -> (Key, Considered) {
    let class = match &outcome {
        Ok(p) if p.padding.is_some() => 1,
        _ => 0,
    };
    (
        (rung.is_fitted(), tier, class, place),
        Considered {
            rung,
            target,
            outcome,
        },
    )
}
