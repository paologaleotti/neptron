use neptron_types::{CrtRange, GpuCaps, MonitorKind, MonitorProfile};

use super::ladder::walk_ladder;
use super::refresh::{
    below_ranges_double_target, game_target, native_double_target, refresh_targets,
};
use super::settings::Effective;
use super::shape::{Shape, blank_lines, overscan_crop, progressive_capacity, shape};
use super::solve::solve;
use super::{
    Considered, Dropped, HorizontalStrategy, OrientedSource, PictureStyle, Plan, PolicyError,
    Proposal, REFRESH_MATCH_MHZ, RefreshKind, RefreshTarget, Rung, SMALL_PICTURE_MIN_PERCENT,
    StrategyReplaced, VerticalMode,
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
///
/// For every style, an exact mode at the asked rate (`exact_x2`,
/// `force_50`, `force_60`) ranks before every exact mode at another rate:
/// the asked rate is the user's choice, so no fill comparison and no crop
/// overrides it.
pub(crate) fn crt_plan(crt: &Crt<'_>) -> Result<Plan, PolicyError> {
    let style = crt.settings.style;
    let rungs = walk_ladder(style);
    let (mut targets, skipped) =
        refresh_targets(crt.settings.refresh, crt.source.refresh_mhz, crt.ranges);
    let below =
        below_ranges_double_target(crt.settings.refresh, crt.source.refresh_mhz, crt.ranges);
    if let Some(double) = below {
        targets.insert(targets.len().saturating_sub(1), double);
    }
    let mut walk: Vec<(RefreshTarget, &[Rung])> = targets.iter().map(|&t| (t, rungs)).collect();
    if style == PictureStyle::Native
        && below.is_none()
        && let Some(double) =
            native_double_target(crt.settings.refresh, crt.source.refresh_mhz, crt.ranges)
    {
        walk.push((double, &[Rung::Native]));
    }

    let singles = single_ranges(crt.profile, crt.ranges);
    let mut entries = Vec::with_capacity(walk.len() * rungs.len() + 1);
    for (tier, &(target, rungs)) in walk.iter().enumerate() {
        for (place, &rung) in rungs.iter().enumerate() {
            let mut outcomes = candidate(rung, target, true, crt)?;
            // Each range alone too: the whole profile may give a mode that
            // the validator refuses (a sync pulse too long on a narrow
            // mode) where another range holds the game.
            if !rung.is_fitted() {
                outcomes.extend(per_range(rung, target, crt, &singles)?);
            }
            for (sub, outcome) in outcomes.into_iter().enumerate() {
                let outcome = vertical_fit(crt, rung, outcome);
                entries.push(entry(style, (tier, place, sub), rung, target, outcome));
            }
        }
    }
    // The exact rungs gave no mode at the game's rate (or the nearest
    // covered one): try them at the nearest rate where a range holds the
    // game's lines.
    let base = game_target(crt.source.refresh_mhz, crt.ranges);
    let exact_at_base = entries
        .iter()
        .any(|(_, c)| !c.rung.is_fitted() && c.target == base && c.outcome.is_ok());
    let window = (!exact_at_base)
        .then(|| window_target(base.rate_mhz, crt.source.height, crt.ranges))
        .flatten();
    if let Some(target) = window {
        let tier = walk.len() + 1;
        for (place, &rung) in rungs.iter().enumerate().filter(|(_, r)| !r.is_fitted()) {
            let mut outcomes = candidate(rung, target, true, crt)?;
            outcomes.extend(per_range(rung, target, crt, &singles)?);
            for (sub, outcome) in outcomes.into_iter().enumerate() {
                let outcome = vertical_fit(crt, rung, outcome);
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

    if let Some(target) = window {
        speed_order(&mut entries, target);
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

/// The band after every fitted entry.
const AFTER_FIT_BAND: u8 = 3;

/// The target of the exact rungs when they gave no mode at `base_mhz`
/// (the game's rate, or the nearest covered one): the rate nearest it, at
/// or below it, at which a range holds the game's lines, progressive or
/// interlaced, at least its fewest lines (`arcade_25` holds 384
/// progressive lines up to 60.000 Hz, not at 60.099 Hz). Fewer lines per
/// field at a lower rate is the only way a fixed line rate gives more.
/// `None` when no range holds them below `base_mhz`.
fn window_target(base_mhz: u32, game_lines: u16, ranges: &[CrtRange]) -> Option<RefreshTarget> {
    let base = u64::from(base_mhz);
    let highest = |range: &CrtRange, fewest: u16, most: u16, fields: u64| {
        let lines = u64::from(game_lines.max(fewest));
        if fewest == 0 || lines > u64::from(most) {
            return None;
        }
        // A field holds the lines of the field and the blanking.
        let blank = blank_lines(range, range.h_max_mhz);
        let rate = fields * range.h_max_mhz / (lines + fields * blank).max(1);
        let rate = rate.min(range.v_max_mhz).min(base);
        (rate >= range.v_min_mhz && rate < base).then_some(rate)
    };
    let rate = ranges
        .iter()
        .flat_map(|r| {
            [
                highest(r, r.progressive_lines_min, r.progressive_lines_max, 1),
                highest(r, r.interlaced_lines_min, r.interlaced_lines_max, 2),
            ]
        })
        .flatten()
        .max()?;
    Some(RefreshTarget {
        kind: RefreshKind::Nearest,
        rate_mhz: u32::try_from(rate).ok()?,
        repeat: 1,
    })
}

/// The exact entries at the window target win over the fit only when they
/// change the game's speed no more than the fit does, within
/// [`REFRESH_MATCH_MHZ`] (a whole-kHz clock moves a rate by a few
/// millihertz): a fit at the game's own rate keeps them after it.
fn speed_order(entries: &mut [(Key, Considered)], window: RefreshTarget) {
    let error = |p: &Proposal| {
        let shown = p.timing.field_rate_mhz / u64::from(p.timing.repeat().max(1));
        shown.abs_diff(u64::from(p.timing.game_mhz))
    };
    let fit = entries
        .iter()
        .filter(|(_, c)| c.rung.is_fitted())
        .filter_map(|(_, c)| c.outcome.as_ref().ok())
        .map(error)
        .min();
    for (key, considered) in entries.iter_mut() {
        let Ok(proposal) = &considered.outcome else {
            continue;
        };
        if considered.target == window
            && !considered.rung.is_fitted()
            && fit.is_some_and(|fit| error(proposal) >= fit + REFRESH_MATCH_MHZ)
        {
            key.0 = AFTER_FIT_BAND;
        }
    }
}

/// `display.vertical = fit` for a turned game: an interlaced 1:1 mode and
/// an exact picture below [`SMALL_PICTURE_MIN_PERCENT`] of the picture
/// lines drop, so the fit fills the screen with no flicker. An exact
/// picture that fills more stays: the fit would not be larger.
fn vertical_fit(crt: &Crt<'_>, rung: Rung, outcome: Outcome) -> Outcome {
    let asked = crt.source.turned && crt.settings.vertical == VerticalMode::Fit;
    let Ok(proposal) = outcome else {
        return outcome;
    };
    if !asked || rung.is_fitted() {
        return Ok(proposal);
    }
    let fill_permille = proposal.padding.map_or(1_000, |p| p.fill_permille());
    let small = u64::from(fill_permille) * 100 < SMALL_PICTURE_MIN_PERCENT * 1_000;
    if proposal.mode.interlace || small {
        return Err(Dropped::VerticalFit {
            fill_permille,
            interlaced: proposal.mode.interlace,
        });
    }
    Ok(proposal)
}

/// Each range of a profile with more than one range as a profile of its
/// own, with the index of the range in the whole profile.
fn single_ranges(profile: &MonitorProfile, ranges: &[CrtRange]) -> Vec<(usize, MonitorProfile)> {
    if ranges.len() < 2 {
        return Vec::new();
    }
    ranges
        .iter()
        .enumerate()
        .map(|(index, range)| {
            let single = MonitorProfile {
                id: profile.id.clone(),
                kind: MonitorKind::Crt {
                    ranges: vec![*range],
                },
            };
            (index, single)
        })
        .collect()
}

/// Steps 4 and 5 for one rung in each range that covers the target rate,
/// as if the range were the whole profile.
///
/// The ranges of a profile have gaps: 15 kHz holds 192 to 288 lines,
/// 31 kHz 400 to 512. Taken as one window, a 320-line game needs no
/// border (192 is the least) and fits no interlaced rung (512 is the
/// most), and Switchres then puts it in a range whose window starts above
/// it. Each range alone gives the border, the capacity and the interlaced
/// window of that range. The proposal keeps the index of the range in the
/// whole profile, so the validator and the padding read the right range.
fn per_range(
    rung: Rung,
    target: RefreshTarget,
    crt: &Crt<'_>,
    singles: &[(usize, MonitorProfile)],
) -> Result<Vec<Outcome>, ModelineError> {
    let rate = u64::from(target.rate_mhz);
    let mut outcomes = Vec::new();
    for (index, single) in singles {
        let MonitorKind::Crt { ranges } = &single.kind else {
            continue;
        };
        let covering = crt
            .ranges
            .iter()
            .filter(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz)
            .count();
        let covers = ranges
            .iter()
            .all(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz);
        // With one covering range the whole profile is that range already.
        if covering < 2 || !covers {
            continue;
        }
        let one = Crt {
            profile: single,
            ranges,
            ..*crt
        };
        let found = candidate(rung, target, true, &one)?;
        outcomes.extend(found.into_iter().map(|outcome| {
            outcome.map(|proposal| Proposal {
                source_index: *index,
                ..proposal
            })
        }));
    }
    Ok(outcomes)
}

/// Steps 4 and 5 for one rung at one target. One outcome, or two when the
/// horizontal strategy of the settings is below the GPU's pixel clock
/// floor: the dropped candidate, then the dynamic super width that
/// replaces it, so the screen still gets a mode. When a whole multiple
/// misses the lines the range holds, the dropped candidate is followed by
/// the same multiple with an overscan crop, if one fits.
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
        Ok(shape) => fit_progressive(shape, target, crt),
        Err(dropped) => return Ok(vec![Err(dropped)]),
    };
    let mut outcomes = solve_replacing(&shape, target, enforce_refresh, crt)?;
    let missed = matches!(outcomes.last(), Some(Err(Dropped::RefreshMissed { .. })));
    let lines_missed = missed || matches!(outcomes.last(), Some(Err(Dropped::LinesChanged { .. })));
    if lines_missed
        && let Some(cropped) =
            overscan_crop(&shape, &crt.source, crt.ranges, aspect, target.rate_mhz)
    {
        outcomes.extend(solve_replacing(&cropped, target, enforce_refresh, crt)?);
        return Ok(outcomes);
    }
    if rung.is_fitted()
        && enforce_refresh
        && missed
        && let Some(retry) = fitted_at_capacity(&shape, target, crt)?
    {
        return Ok(retry);
    }
    Ok(outcomes)
}

/// The fitted rung of a turned game with `display.vertical = fit`: the
/// most progressive lines a range holds at the rate, so the fit has no
/// flicker. Every other shape stays.
fn fit_progressive(shape: Shape, target: RefreshTarget, crt: &Crt<'_>) -> Shape {
    let asked = crt.source.turned && crt.settings.vertical == VerticalMode::Fit;
    let capacity = progressive_capacity(crt.ranges, target.rate_mhz);
    if !asked || !shape.rung.is_fitted() || capacity == 0 || capacity >= shape.lines {
        return shape;
    }
    Shape {
        lines: capacity,
        interlace: false,
        ..shape
    }
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

/// Band (exact at the asked rate, other exact, fitted), group, then four
/// places: the entries sort by it.
type Key = (u8, u8, usize, usize, usize, usize);

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
    // NativeDouble stays out: native takes it on its own, the user did not
    // ask for it.
    let asked_rate = matches!(
        target.kind,
        RefreshKind::Forced50 | RefreshKind::Forced60 | RefreshKind::Double
    );
    let band = u8::from(!asked_rate);
    let key = match (rung.is_fitted(), style) {
        (true, _) => (2, 0, tier, 0, place, sub),
        (false, PictureStyle::Native) => native_key(band, rung, rank, (tier, place, sub)),
        (false, _) => (band, 0, tier, order_rank(rung, &outcome), place, sub),
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

/// The key of an exact entry of the `native` style, within its band: the
/// native rung with no crop and no padding at every target, then rung 3,
/// then the cropped native rung, then every padded mode by its fill, then
/// by target.
fn native_key(band: u8, rung: Rung, rank: usize, (tier, place, sub): (usize, usize, usize)) -> Key {
    match (rank, rung) {
        (0, Rung::Native) => (band, 0, tier, 0, place, sub),
        (0, _) => (band, 1, tier, 0, place, sub),
        (CROPPED_RANK, _) => (band, 2, tier, 0, place, sub),
        _ => (band, 3, rank, tier, place, sub),
    }
}

/// The rank of a cropped mode that is not padded.
const CROPPED_RANK: usize = 1;

/// The first rank of a small picture: a mode below
/// [`SMALL_PICTURE_MIN_PERCENT`] of the picture lines, and every
/// interlaced 1:1 mode. Every padded rank of a larger progressive picture
/// is below it.
const SMALL_RANK: usize = 1_002;

/// The rank of an exact entry of every style but `native`, within its
/// target: [`padded_rank`], except that a progressive picture of at least
/// [`SMALL_PICTURE_MIN_PERCENT`] of the picture lines ranks before every
/// interlaced 1:1 mode, and a smaller picture ranks with the interlaced
/// modes by its fill. So an interlaced 1:1 mode wins only where every
/// progressive exact picture is small, and then only when it fills more.
fn order_rank(rung: Rung, outcome: &Outcome) -> usize {
    let Ok(proposal) = outcome else {
        return 0;
    };
    let fill = proposal.padding.map_or(PERMILLE, |p| {
        usize::try_from(p.fill_permille()).unwrap_or(0)
    });
    let small = fill * 100 < usize::try_from(SMALL_PICTURE_MIN_PERCENT).unwrap_or(0) * PERMILLE;
    if rung == Rung::Interlaced || small {
        return SMALL_RANK + PERMILLE.saturating_sub(fill);
    }
    padded_rank(outcome)
}

const PERMILLE: usize = 1_000;

/// The rank of an exact entry within its target: 0 for a mode with no
/// crop and no padding (and for a dropped entry); [`CROPPED_RANK`] for a
/// cropped mode; above it for a padded mode, lower when it fills more of
/// its picture lines.
fn padded_rank(outcome: &Outcome) -> usize {
    match outcome {
        Ok(Proposal {
            padding: Some(padding),
            ..
        }) => {
            let fill = usize::try_from(padding.fill_permille()).unwrap_or(usize::MAX);
            1_002_usize.saturating_sub(fill).max(CROPPED_RANK + 1)
        }
        Ok(Proposal {
            overscan_crop: Some(_),
            ..
        }) => CROPPED_RANK,
        _ => 0,
    }
}
