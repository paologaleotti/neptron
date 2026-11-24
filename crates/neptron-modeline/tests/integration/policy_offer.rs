//! The one decision of what the display offers (`offer`) and the menu
//! rows built from it (`picture_choices`): every picture mode, refresh
//! choice and scanline setting, on every preset of data/monitors with
//! every GPU entry of data/hardware/matrix.toml, the EDID tubes, the panels
//! and every TV standard, for every source of the arcade table, and on
//! generated ranges and sources with their edge values.
//!
//! An offered candidate keeps the promise of its option, recomputed here
//! from the plan, the profile, the caps and the validator, and shows a
//! validated mode inside the profile. A declined candidate has a reason
//! that the test finds true by itself, and the mode it shows in its place
//! validates and lies inside the profile. On each menu row that is enabled,
//! Auto is selectable for every stored pair. The spec table of section 8.7
//! is the output of the same function.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use neptron_modeline::policy::{
    DeclineReason, Declined, Offer, Offered, PictureChoices, PictureStyle, Plan, PolicyError,
    PolicySettings, RefreshKind, RefreshPolicy, RowAnswer, Rung, ScanlineChoice, choose, decide,
    offer, panel_scanlines, picture_choices, walk_ladder,
};
use neptron_safety::validate;
use neptron_types::{
    CrtRange, GpuCaps, Modeline, MonitorKind, MonitorProfile, ScanlineMode, SourceGeometry,
    TvStandard,
};

use crate::arcade_table::{self, Board};
use crate::presets::{all_presets, gpu_table, open_caps, panel_1080, preset, source};

/// A display of the run: its profile and its GPU.
struct Screen {
    profile: MonitorProfile,
    caps: GpuCaps,
}

const AUTO_75: ScanlineChoice = ScanlineChoice {
    mode: ScanlineMode::Auto,
    strength_percent: 75,
};

/// The scanline settings of a flat panel, whose plan they change: each
/// mode, and `on` with no strength. A CRT and a TV draw the same plan for
/// every setting, so they run `auto` and `on`, which cover the read-back
/// of the scanline rule.
const PANEL_SCANLINES: [ScanlineChoice; 4] = [
    AUTO_75,
    ScanlineChoice {
        mode: ScanlineMode::On,
        strength_percent: 75,
    },
    ScanlineChoice {
        mode: ScanlineMode::Off,
        strength_percent: 75,
    },
    ScanlineChoice {
        mode: ScanlineMode::On,
        strength_percent: 0,
    },
];

const CRT_SCANLINES: [ScanlineChoice; 2] = [
    AUTO_75,
    ScanlineChoice {
        mode: ScanlineMode::On,
        strength_percent: 75,
    },
];

fn sources() -> Vec<(&'static str, SourceGeometry)> {
    arcade_table::BOARDS
        .iter()
        .chain(arcade_table::SOURCES.iter())
        .map(|b: &Board| (b.name, b.source()))
        .collect()
}

fn caps_of(id: &str) -> GpuCaps {
    gpu_table()
        .into_iter()
        .find(|(gpu, _)| gpu == id)
        .map(|(_, caps)| caps)
        .unwrap_or_else(|| panic!("{id} is in data/hardware/matrix.toml"))
}

fn composite() -> GpuCaps {
    caps_of("pi_vc4_composite")
}

fn presets_on(caps: GpuCaps) -> Vec<Screen> {
    all_presets()
        .into_iter()
        .map(|profile| Screen { profile, caps })
        .collect()
}

/// The shown mode lies inside the profile: a range of a CRT holds its
/// field rate, and its line rate up to one field rate below the least line
/// rate (the tolerance of the validator, which a whole-kHz clock needs); a
/// panel lists it; a TV standard has it.
fn inside(profile: &MonitorProfile, mode: &Modeline) -> bool {
    match &profile.kind {
        MonitorKind::Crt { ranges } => {
            let (line, field) = (mode.line_rate_mhz(), mode.field_rate_mhz());
            ranges.iter().any(|r| {
                (r.h_min_mhz.saturating_sub(field)..=r.h_max_mhz).contains(&line)
                    && (r.v_min_mhz..=r.v_max_mhz).contains(&field)
            })
        }
        MonitorKind::FlatPanel { modes } => modes.contains(mode),
        MonitorKind::Tv { standard } => standard.modes().contains(mode),
    }
}

fn mode_of(plan: &Plan, index: usize) -> Modeline {
    plan.considered[index]
        .outcome
        .as_ref()
        .map(|p| p.mode)
        .expect("a winner has a proposal")
}

/// The asked rate of a refresh choice, and the target kind of a winner at
/// it.
fn asked_rate(refresh: RefreshPolicy, game_mhz: u32) -> Option<(u64, RefreshKind)> {
    match refresh {
        RefreshPolicy::Exact => None,
        RefreshPolicy::ExactX2 => Some((2 * u64::from(game_mhz), RefreshKind::Double)),
        RefreshPolicy::Force50 => Some((50_000, RefreshKind::Forced50)),
        RefreshPolicy::Force60 => Some((60_000, RefreshKind::Forced60)),
    }
}

fn covers(ranges: &[CrtRange], rate_mhz: u64) -> bool {
    ranges
        .iter()
        .any(|r| r.v_min_mhz <= rate_mhz && rate_mhz <= r.v_max_mhz)
}

/// The most progressive lines a range of `profile` holds at `rate_mhz`.
fn progressive_capacity(profile: &MonitorProfile, rate_mhz: u64) -> u32 {
    let MonitorKind::Crt { ranges } = &profile.kind else {
        return 0;
    };
    ranges
        .iter()
        .filter(|r| {
            r.progressive_lines_min > 0 && r.v_min_mhz <= rate_mhz && rate_mhz <= r.v_max_mhz
        })
        .map(|r| {
            let most =
                arcade_table::field_lines(r, rate_mhz).min(u32::from(r.progressive_lines_max));
            (u32::from(r.progressive_lines_min), most)
        })
        .filter(|&(fewest, most)| fewest <= most)
        .map(|(_, most)| most)
        .max()
        .unwrap_or(0)
}

/// The lines of the game on the screen: the crop of the frame, turned.
fn lines_of(plan: &Plan, index: usize) -> u32 {
    let p = plan.considered[index].outcome.as_ref().expect("a proposal");
    if plan.source.turned {
        p.placement.crop.w
    } else {
        p.placement.crop.h
    }
}

/// The speed of the winner changes by more than 0.5 % of the shown rate.
fn speed_changes(plan: &Plan, index: usize) -> bool {
    let p = plan.considered[index].outcome.as_ref().expect("a proposal");
    let shown = p.timing.field_rate_mhz / u64::from(p.timing.repeat().max(1));
    let game = u64::from(p.timing.game_mhz);
    u128::from(shown.abs_diff(game)) * 1_000 > u128::from(shown) * 5
}

/// The promise of an offered option, recomputed from the plan.
fn promise_holds(
    offered: &Offered,
    settings: &PolicySettings,
    screen: &Screen,
) -> Result<(), String> {
    let plan = &offered.plan;
    let w = offered.winner;
    let considered = &plan.considered[w];
    let mode = mode_of(plan, w);
    let lines = lines_of(plan, w);
    let dest_h = considered
        .outcome
        .as_ref()
        .map(|p| p.placement.dest.h)
        .unwrap_or(0);
    let tv = matches!(screen.profile.kind, MonitorKind::Tv { .. });
    let fault = match settings.style {
        PictureStyle::Auto => None,
        PictureStyle::Sharp => considered.rung.is_fitted().then_some("sharp is a fit"),
        PictureStyle::Native => {
            (considered.rung != Rung::Native || dest_h != lines).then_some("native is not 1:1")
        }
        PictureStyle::X2 | PictureStyle::X3 | PictureStyle::X4 => {
            let k = u32::from(settings.style.factor().unwrap_or(0));
            (dest_h != k * lines).then_some("the multiple is not that multiple")
        }
        PictureStyle::Speed => (tv || speed_changes(plan, w)).then_some("speed is not held"),
        PictureStyle::Fit => {
            let capacity =
                progressive_capacity(&screen.profile, u64::from(plan.source.refresh_mhz))
                    .max(progressive_capacity(&screen.profile, mode.field_rate_mhz()));
            // The last resort lets the solver pick the rate and the scan.
            let last_resort = considered.target.kind == RefreshKind::Unmatched;
            (!plan.source.interlaced && mode.interlace && !last_resort && (capacity > 0 || tv))
                .then_some("fill is interlaced where progressive lines exist")
        }
    };
    if let Some(fault) = fault {
        return Err(format!("offered, {fault}: {considered}"));
    }
    if let (Some((rate, kind)), MonitorKind::Crt { .. }) = (
        asked_rate(settings.refresh, plan.source.refresh_mhz),
        &screen.profile.kind,
    ) {
        let p = considered.outcome.as_ref().expect("a proposal");
        let shown = p.timing.field_rate_mhz;
        if considered.target.kind != kind || shown.abs_diff(rate) > 50 {
            return Err(format!(
                "offered {} away from {rate} mHz: {considered}",
                settings.refresh
            ));
        }
    }
    Ok(())
}

/// The test's own reading of why a declined candidate is declined, from
/// the profile, the caps, the plan, the winner the answer names and the
/// validator.
fn reason_holds(
    declined: &Declined,
    settings: &PolicySettings,
    screen: &Screen,
    source: &SourceGeometry,
    scan: ScanlineChoice,
) -> Result<(), String> {
    let plan = &declined.plan;
    let valid = |m: &Modeline| validate(m, &screen.profile, &screen.caps).is_ok();
    let won = declined.winner.map(|w| &plan.considered[w]);
    let rung = won.map(|w| w.rung);
    let factor = settings.style.factor();
    let own_rung = walk_ladder(settings.style).first().copied();
    // A proposal of the asked rung, whole, at the rate of the winner, that
    // the validator accepts.
    let asked_at_winner = || {
        won.and_then(|w| {
            plan.proposals()
                .find(|(_, c, p)| {
                    Some(c.rung) == own_rung
                        && c.target.kind == w.target.kind
                        && c.target.rate_mhz == w.target.rate_mhz
                        && p.overscan_crop.is_none()
                        && valid(&p.mode)
                })
                .map(|(_, _, p)| *p)
        })
    };
    let whole_at_winner = || asked_at_winner().is_some();
    // The share of the screen height that the game fills, in per mille:
    // the height of the game's place on the screen over the lines that the
    // screen scans in one frame (the active lines, or the lines that a
    // padded range forces in each field).
    let fill = |p: &neptron_modeline::policy::Proposal| {
        let scanned = p.padding.map_or(0, |pad| {
            u64::from(pad.picture_lines) * if p.mode.interlace { 2 } else { 1 }
        });
        let frame = u64::from(p.mode.vactive).max(scanned);
        u64::from(p.placement.dest.h) * 1_000 / frame.max(1)
    };
    // The winner fills more of the screen than the asked rung at the
    // winner's rate: (the winner's rung, true) when it does.
    let fills_more = || {
        let (Some(w), Some(a)) = (won, asked_at_winner()) else {
            return None;
        };
        let wp = w.outcome.as_ref().ok()?;
        (fill(wp) > fill(&a)).then_some(w.rung)
    };
    let smaller_fills_more = || match (fills_more(), own_rung) {
        (Some(Rung::Multiple(k)), Some(Rung::Multiple(n))) => k < n,
        _ => false,
    };
    let interlaced_fills_more = || fills_more() == Some(Rung::Interlaced);
    let holds = match declined.reason {
        // The plan of the asked candidate has no proposal that validates;
        // the winner, if any, is the one of the fallback.
        DeclineReason::NoPicture => {
            let asked = PolicySettings {
                panel_scanlines: panel_scanlines(&screen.profile.kind, scan),
                ..*settings
            };
            let own = decide(source, &asked, &screen.profile, &screen.caps)
                .map_err(|e| format!("no plan: {e}"))?;
            !own.proposals().any(|(_, _, p)| valid(&p.mode))
                && (declined.winner.is_none() || plan.style != settings.style)
        }
        DeclineReason::RateFixedByTv { .. } | DeclineReason::RateOfPanel { .. } => false,
        DeclineReason::RateNotCovered { asked, rate_mhz } => match &screen.profile.kind {
            MonitorKind::Crt { ranges } => {
                asked == settings.refresh
                    && asked_rate(asked, plan.source.refresh_mhz).map(|(r, _)| r) == Some(rate_mhz)
                    && !covers(ranges, rate_mhz)
            }
            MonitorKind::FlatPanel { .. } | MonitorKind::Tv { .. } => false,
        },
        DeclineReason::RateNotHeld { asked } => match &screen.profile.kind {
            MonitorKind::Crt { ranges } => {
                asked_rate(asked, plan.source.refresh_mhz).is_some_and(|(rate, kind)| {
                    covers(ranges, rate)
                        && !plan.proposals().any(|(_, c, p)| {
                            c.target.kind == kind && !c.rung.is_fitted() && valid(&p.mode)
                        })
                        && won.is_some_and(|w| w.target.kind != kind)
                })
            }
            MonitorKind::FlatPanel { .. } | MonitorKind::Tv { .. } => false,
        },
        // The asked rung is not reachable whole at the rate the winner
        // runs: no proposal of it at that target validates with no crop.
        DeclineReason::NotShownHere { asked, shown } => {
            !matches!(screen.profile.kind, MonitorKind::FlatPanel { .. })
                && asked == settings.style
                && factor.is_some()
                && rung == Some(shown)
                && own_rung != Some(shown)
                && !whole_at_winner()
        }
        // The asked rung is reachable whole at the rate the winner runs,
        // and the winner, a smaller multiple, fills more of the screen.
        DeclineReason::SmallerFillsMore { asked, shown } => {
            !matches!(screen.profile.kind, MonitorKind::FlatPanel { .. })
                && asked == settings.style
                && rung == Some(shown)
                && smaller_fills_more()
        }
        // The asked rung is reachable whole at the rate the winner runs,
        // and the winner, the interlaced picture, fills more of the screen.
        DeclineReason::InterlacedFillsMore { asked } => {
            !matches!(screen.profile.kind, MonitorKind::FlatPanel { .. })
                && asked == settings.style
                && rung == Some(Rung::Interlaced)
                && interlaced_fills_more()
        }
        DeclineReason::TooLargeForScreen { asked, shown } => match &screen.profile.kind {
            MonitorKind::FlatPanel { modes } => {
                let k = u32::from(factor.unwrap_or(0));
                let lines = u32::from(plan.source.height);
                let width = |h: u32| {
                    if plan.source.aspect_milli == 0 {
                        h * u32::from(plan.source.width) / lines.max(1)
                    } else {
                        h * plan.source.aspect_milli / 1_000
                    }
                };
                asked == settings.style
                    && rung == Some(shown)
                    && modes.iter().all(|m| {
                        k * lines > u32::from(m.vactive) || width(k * lines) > u32::from(m.hactive)
                    })
            }
            MonitorKind::Crt { .. } | MonitorKind::Tv { .. } => false,
        },
        DeclineReason::NoExactMode => {
            settings.style == PictureStyle::Sharp
                && !plan
                    .proposals()
                    .any(|(_, c, p)| !c.rung.is_fitted() && valid(&p.mode))
        }
        DeclineReason::OneRateOnTv => {
            matches!(screen.profile.kind, MonitorKind::Tv { .. })
                && settings.style == PictureStyle::Speed
        }
        // The speed of the winner is beyond the alert threshold.
        DeclineReason::SpeedNotHeld => {
            settings.style == PictureStyle::Speed
                && !matches!(screen.profile.kind, MonitorKind::Tv { .. })
                && declined.winner.is_some_and(|w| speed_changes(plan, w))
        }
    };
    holds
        .then_some(())
        .ok_or_else(|| format!("the reason {:?} does not hold", declined.reason))
}

fn reason_name(reason: DeclineReason) -> &'static str {
    match reason {
        DeclineReason::NoPicture => "no picture",
        DeclineReason::RateFixedByTv { .. } => "rate fixed by TV",
        DeclineReason::RateOfPanel { .. } => "rate of panel",
        DeclineReason::RateNotCovered { .. } => "rate not covered",
        DeclineReason::RateNotHeld { .. } => "rate not held",
        DeclineReason::NotShownHere { .. } => "not shown here",
        DeclineReason::SmallerFillsMore { .. } => "smaller fills more",
        DeclineReason::InterlacedFillsMore { .. } => "interlaced fills more",
        DeclineReason::TooLargeForScreen { .. } => "too large for screen",
        DeclineReason::NoExactMode => "no exact mode",
        DeclineReason::OneRateOnTv => "one rate on TV",
        DeclineReason::SpeedNotHeld => "speed not held",
    }
}

/// The checks of one stored candidate, and the name of its result. With
/// `deep`, the winner of an offered candidate is also the first proposal
/// of its plan that validates.
fn check(
    answer: &Offer,
    settings: &PolicySettings,
    screen: &Screen,
    source: &SourceGeometry,
    scan: ScanlineChoice,
    deep: bool,
) -> Result<&'static str, String> {
    match answer {
        Ok(offered) => {
            let mode = mode_of(&offered.plan, offered.winner);
            validate(&mode, &screen.profile, &screen.caps)
                .map_err(|e| format!("offered, the validator refuses: {e:?}"))?;
            if !inside(&screen.profile, &mode) {
                return Err(format!("offered {mode:?} lies outside the profile"));
            }
            if deep {
                let w = choose(&offered.plan, |m| {
                    validate(m, &screen.profile, &screen.caps)
                })
                .winner
                .ok_or("offered with no winner")?;
                if w.index != offered.winner {
                    return Err("the offered winner is not the engine's winner".to_owned());
                }
            }
            promise_holds(offered, settings, screen)?;
            let fallback = offered.notice.is_some_and(|n| n.fill_fallback.is_some());
            if fallback != (offered.plan.style != settings.style) {
                return Err("the fill fallback and its notice disagree".to_owned());
            }
            Ok(if fallback {
                "offered, the fill picture"
            } else {
                "offered"
            })
        }
        Err(declined) => {
            reason_holds(declined, settings, screen, source, scan)?;
            if let Some(index) = declined.winner {
                let mode = mode_of(&declined.plan, index);
                validate(&mode, &screen.profile, &screen.caps)
                    .map_err(|e| format!("the shown mode of a refusal fails: {e:?}"))?;
                if !inside(&screen.profile, &mode) {
                    return Err("the shown mode of a refusal lies outside".to_owned());
                }
            }
            Ok(match (declined.reason, declined.winner) {
                (DeclineReason::NoPicture, None) => "no picture",
                (DeclineReason::NoPicture, Some(_)) => "no picture, the fallback picture",
                (reason, _) => reason_name(reason),
            })
        }
    }
}

/// The answer of one pair, as the run keeps it: the index of the winner
/// of an offered pair, or the reason of a declined one.
type Pair = Result<usize, DeclineReason>;

/// What an option of a row gives by the pair rule: plain, with the reset
/// of the other row (and the reason of the stored pair), or declined (with
/// the reason of the pair with the other row at Auto).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Plain,
    Reset(DeclineReason),
    Declined(DeclineReason),
}

fn row_of(answer: &RowAnswer) -> Row {
    match answer {
        RowAnswer::Plain(_) => Row::Plain,
        RowAnswer::ResetsOther { stored_pair, .. } => Row::Reset(*stored_pair),
        RowAnswer::Declined(declined) => Row::Declined(declined.reason),
    }
}

/// The pair rule, from the answers of the pairs: the pair with the stored
/// value of the other row, else the pair with the other row at Auto.
fn rule(stored: Pair, at_auto: Option<Pair>) -> Row {
    match (stored, at_auto) {
        (Ok(_), _) => Row::Plain,
        (Err(reason), None) => Row::Declined(reason),
        (Err(reason), Some(Ok(_))) => Row::Reset(reason),
        (Err(_), Some(Err(reason))) => Row::Declined(reason),
    }
}

fn is_rate_reason(reason: DeclineReason) -> bool {
    match reason {
        DeclineReason::RateFixedByTv { .. }
        | DeclineReason::RateOfPanel { .. }
        | DeclineReason::RateNotCovered { .. }
        | DeclineReason::RateNotHeld { .. } => true,
        DeclineReason::NoPicture
        | DeclineReason::NotShownHere { .. }
        | DeclineReason::SmallerFillsMore { .. }
        | DeclineReason::InterlacedFillsMore { .. }
        | DeclineReason::TooLargeForScreen { .. }
        | DeclineReason::NoExactMode
        | DeclineReason::OneRateOnTv
        | DeclineReason::SpeedNotHeld => false,
    }
}

/// The rows by the pair rule for the stored pair `(style, refresh)`, from
/// `pairs`, the answers of every pair of the source. A TV and a flat panel
/// give no refresh choice but `exact`.
fn rows_by_rule(
    pairs: &HashMap<(PictureStyle, RefreshPolicy), Pair>,
    style: PictureStyle,
    refresh: RefreshPolicy,
    kind: &MonitorKind,
) -> Option<(Vec<Row>, Vec<Row>)> {
    let pair = |s, r| pairs.get(&(s, r)).copied();
    let refresh_pair = |s, r: RefreshPolicy| match (kind, r) {
        (MonitorKind::Crt { .. }, _) | (_, RefreshPolicy::Exact) => pair(s, r),
        (MonitorKind::Tv { .. }, asked) => Some(Err(DeclineReason::RateFixedByTv { asked })),
        (MonitorKind::FlatPanel { .. }, asked) => Some(Err(DeclineReason::RateOfPanel { asked })),
    };
    let modes = PictureStyle::ALL
        .into_iter()
        .map(|x| {
            let stored = pair(x, refresh)?;
            let at_auto = match refresh {
                RefreshPolicy::Exact => None,
                _ => Some(pair(x, RefreshPolicy::Exact)?),
            };
            Some(rule(stored, at_auto))
        })
        .collect::<Option<Vec<_>>>()?;
    let rates = RefreshPolicy::ALL
        .into_iter()
        .map(|x| {
            let stored = refresh_pair(style, x)?;
            let at_auto = match style {
                PictureStyle::Auto => None,
                _ => Some(refresh_pair(PictureStyle::Auto, x)?),
            };
            Some(rule(stored, at_auto))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((modes, rates))
}

/// The invariants of the rows of one stored pair: Auto of each row is
/// selectable where (Auto, Auto) has a picture, and no option is greyed
/// with a reason of the other row.
fn rows_hold(modes: &[Row], rates: &[Row], auto_has_picture: bool) -> Result<(), String> {
    let auto_mode = modes.first().copied();
    let auto_rate = rates.first().copied();
    if auto_has_picture
        && (matches!(auto_mode, Some(Row::Declined(_)))
            || matches!(auto_rate, Some(Row::Declined(_))))
    {
        return Err(format!("Auto is greyed: {auto_mode:?} {auto_rate:?}"));
    }
    for row in modes {
        if let Row::Declined(reason) = row
            && is_rate_reason(*reason)
        {
            return Err(format!("a rate reason on the picture mode row: {reason:?}"));
        }
    }
    for row in rates {
        if let Row::Declined(reason) = row
            && !is_rate_reason(*reason)
            && *reason != DeclineReason::NoPicture
        {
            return Err(format!("a mode reason on the refresh row: {reason:?}"));
        }
    }
    Ok(())
}

/// The rows of `picture_choices` for one stored pair are the rows of the
/// rule, and each selectable option runs the very pair that `offer` gives
/// for it (the same winner).
fn choices_hold(
    choices: &PictureChoices,
    modes: &[Row],
    rates: &[Row],
    pairs: &HashMap<(PictureStyle, RefreshPolicy), Pair>,
    stored: (PictureStyle, RefreshPolicy),
) -> Result<(), String> {
    let got_modes: Vec<Row> = choices
        .modes
        .iter()
        .map(|(_, c)| row_of(&c.answer))
        .collect();
    let got_rates: Vec<Row> = choices
        .refresh
        .iter()
        .map(|(_, c)| row_of(&c.answer))
        .collect();
    if got_modes != modes || got_rates != rates {
        return Err(format!(
            "the rows are not the rule: {got_modes:?} {got_rates:?}, rule {modes:?} {rates:?}"
        ));
    }
    // Each pick with the pair it stores, and the pair it stores when it
    // also sets the other row to Auto.
    let picks = choices
        .modes
        .iter()
        .map(|(s, c)| (c, (*s, stored.1), (*s, RefreshPolicy::Exact)))
        .chain(
            choices
                .refresh
                .iter()
                .map(|(r, c)| (c, (stored.0, *r), (PictureStyle::Auto, *r))),
        );
    for (choice, (style, refresh), with_reset) in picks {
        let run_pair = match &choice.answer {
            RowAnswer::Plain(offered) => Some((offered, (style, refresh))),
            RowAnswer::ResetsOther { offered, .. } => Some((offered, with_reset)),
            RowAnswer::Declined(_) => None,
        };
        if let Some((offered, pair)) = run_pair
            && pairs.get(&pair).copied() != Some(Ok(offered.winner))
        {
            return Err(format!(
                "a pick of {style} {refresh} runs {pair:?}, which offer gives as {:?}",
                pairs.get(&pair)
            ));
        }
    }
    Ok(())
}

/// A cell of a run: the profile, the source with its rate in millihertz,
/// and the picture mode.
type Cell = (String, &'static str, u32, &'static str);

/// The result of a run: the counts by result, the cells with no picture at
/// all, and the cells where `auto` takes the Fit picture.
struct Run {
    counts: BTreeMap<&'static str, usize>,
    no_picture: BTreeSet<Cell>,
    fill_fallback: BTreeSet<Cell>,
}

/// Which screens of a run a test takes: the screens whose index is
/// `part` modulo `parts`. The parts of a run cover every screen once.
#[derive(Clone, Copy)]
struct Part {
    part: usize,
    parts: usize,
}

/// How much a run checks beyond every candidate and the rows of every
/// stored pair.
#[derive(Clone, Copy)]
struct Depth {
    /// Every scanline setting that a plan reads, on a CRT and a TV too.
    every_scanline: bool,
    /// The winner of each offered candidate is also the first proposal of
    /// its plan that validates, and `picture_choices` is compared with the
    /// rows of the rule. The runs of the GPU groups leave it to the run
    /// with an open GPU, which covers the same code with other limits.
    deep: bool,
    /// A profile that the solver refuses counts, and is no failure.
    tolerate_solver: bool,
}

const FULL: Depth = Depth {
    every_scanline: true,
    deep: true,
    tolerate_solver: false,
};

/// Every candidate on `screens` for `sources`, and the rows of every stored
/// pair by the pair rule, for each scanline setting of a panel (where the
/// scanlines change the plan) and for `auto` elsewhere. With `deep`,
/// `picture_choices` is compared with the rule for one stored pair of each
/// screen, source and scanline setting, a different one from one source to
/// the next, so each stored pair is compared on each screen.
fn run(
    sources: &[(&'static str, SourceGeometry)],
    screens: &[Screen],
    depth: Depth,
    seed: Option<u64>,
) -> Run {
    let mut counts = BTreeMap::new();
    let mut no_picture = BTreeSet::new();
    let mut fill_fallback = BTreeSet::new();
    let mut failures = Vec::new();
    for (screen_index, screen) in screens.iter().enumerate() {
        let panel = matches!(screen.profile.kind, MonitorKind::FlatPanel { .. });
        let scanlines: &[ScanlineChoice] = match (panel, depth.every_scanline) {
            (true, _) => &PANEL_SCANLINES,
            (false, true) => &CRT_SCANLINES,
            (false, false) => &CRT_SCANLINES[..1],
        };
        // The scanline settings whose rows the run checks: every one that
        // changes the plan.
        let row_scans = if panel { scanlines.len() } else { 1 };
        let validator = |m: &Modeline| validate(m, &screen.profile, &screen.caps).map(drop);
        for (source_index, (name, source)) in sources.iter().enumerate() {
            let mut pairs: Vec<HashMap<(PictureStyle, RefreshPolicy), Pair>> =
                vec![HashMap::new(); row_scans];
            let cell = |style: PictureStyle| {
                (
                    screen.profile.id.clone(),
                    *name,
                    source.refresh_mhz,
                    style.key(),
                )
            };
            for style in PictureStyle::ALL {
                for refresh in RefreshPolicy::ALL {
                    let settings = PolicySettings {
                        style,
                        refresh,
                        ..PolicySettings::default()
                    };
                    for (i, &scan) in scanlines.iter().enumerate() {
                        let answer = offer(
                            source,
                            &settings,
                            scan,
                            &screen.profile,
                            &screen.caps,
                            validator,
                        );
                        let result = match answer {
                            Ok(answer) => {
                                if let Some(map) = pairs.get_mut(i) {
                                    let pair = match &answer {
                                        Ok(offered) => Ok(offered.winner),
                                        Err(declined) => Err(declined.reason),
                                    };
                                    map.insert((style, refresh), pair);
                                }
                                check(&answer, &settings, screen, source, scan, depth.deep)
                            }
                            Err(PolicyError::Solver(_)) if depth.tolerate_solver => {
                                Ok("solver refuses")
                            }
                            Err(e) => Err(format!("no plan: {e}")),
                        };
                        match result {
                            Ok(result) => {
                                if result == "no picture" {
                                    no_picture.insert(cell(style));
                                }
                                if result == "offered, the fill picture" {
                                    fill_fallback.insert(cell(style));
                                }
                                *counts.entry(result).or_insert(0) += 1;
                            }
                            Err(e) => failures.push(format!(
                                "seed {seed:?} {} {name} {source:?} {style} {refresh} {scan:?}: {e}",
                                screen.profile.id
                            )),
                        }
                    }
                }
            }
            for (i, pairs) in pairs.iter().enumerate() {
                let scan = scanlines[i];
                let auto_has_picture = pairs
                    .get(&(PictureStyle::Auto, RefreshPolicy::Exact))
                    .is_some_and(Result::is_ok);
                let sample = (screen_index * 7 + source_index + i * 3) % 32;
                for (k, (style, refresh)) in PictureStyle::ALL
                    .into_iter()
                    .flat_map(|s| RefreshPolicy::ALL.into_iter().map(move |r| (s, r)))
                    .enumerate()
                {
                    let Some((modes, rates)) =
                        rows_by_rule(pairs, style, refresh, &screen.profile.kind)
                    else {
                        continue;
                    };
                    *counts.entry("stored pairs with rows").or_insert(0) += 1;
                    let case = format!(
                        "seed {seed:?} {} {name} rows of {style} {refresh} {scan:?}",
                        screen.profile.id
                    );
                    if let Err(e) = rows_hold(&modes, &rates, auto_has_picture) {
                        failures.push(format!("{case}: {e}"));
                    }
                    if k != sample || !depth.deep {
                        continue;
                    }
                    let settings = PolicySettings {
                        style,
                        refresh,
                        ..PolicySettings::default()
                    };
                    match picture_choices(
                        source,
                        &settings,
                        scan,
                        &screen.profile,
                        &screen.caps,
                        validator,
                    ) {
                        Ok(choices) => {
                            *counts.entry("rows compared").or_insert(0) += 1;
                            if let Err(e) =
                                choices_hold(&choices, &modes, &rates, pairs, (style, refresh))
                            {
                                failures.push(format!("{case}: {e}"));
                            }
                        }
                        Err(e) => failures.push(format!("{case}: no plan: {e}")),
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures[..failures.len().min(40)].join("\n")
    );
    Run {
        counts,
        no_picture,
        fill_fallback,
    }
}

fn report(test: &str, run: &Run) {
    let total: usize = run.counts.values().sum();
    println!("{test}: {total} counts: {:?}", run.counts);
}

/// The screens of `part`.
fn part_of(screens: Vec<Screen>, part: Part) -> Vec<Screen> {
    screens
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % part.parts == part.part)
        .map(|(_, s)| s)
        .collect()
}

/// The presets of `part` with `gpu`, every candidate.
fn presets_run(gpu: &str, part: Part, every_scanline: bool) {
    let caps = if gpu == "open" {
        open_caps()
    } else {
        caps_of(gpu)
    };
    let screens = part_of(presets_on(caps), part);
    let depth = Depth {
        every_scanline,
        deep: gpu == "open",
        tolerate_solver: false,
    };
    let run = run(&sources(), &screens, depth, None);
    report(gpu, &run);
    // Every preset shows a picture for every candidate with every GPU of
    // the matrix, and `auto` never needs the Fill picture there.
    assert!(run.no_picture.is_empty(), "{gpu}: {:?}", run.no_picture);
    assert!(
        run.fill_fallback.is_empty(),
        "{gpu}: {:?}",
        run.fill_fallback
    );
}

/// The CRT presets with an open GPU, every scanline setting that a plan
/// reads, in twelve parts that run side by side: each part stays under
/// 20 s on a loaded machine.
macro_rules! open_presets {
    ($($test:ident: $part:literal,)*) => {
        $(
            #[test]
            fn $test() {
                presets_run("open", Part { part: $part, parts: 12 }, true);
            }
        )*
    };
}

open_presets! {
    offer_on_the_presets_with_an_open_gpu_part_1: 0,
    offer_on_the_presets_with_an_open_gpu_part_2: 1,
    offer_on_the_presets_with_an_open_gpu_part_3: 2,
    offer_on_the_presets_with_an_open_gpu_part_4: 3,
    offer_on_the_presets_with_an_open_gpu_part_5: 4,
    offer_on_the_presets_with_an_open_gpu_part_6: 5,
    offer_on_the_presets_with_an_open_gpu_part_7: 6,
    offer_on_the_presets_with_an_open_gpu_part_8: 7,
    offer_on_the_presets_with_an_open_gpu_part_9: 8,
    offer_on_the_presets_with_an_open_gpu_part_10: 9,
    offer_on_the_presets_with_an_open_gpu_part_11: 10,
    offer_on_the_presets_with_an_open_gpu_part_12: 11,
}

#[test]
fn offer_on_the_edid_tubes() {
    let screens: Vec<Screen> = arcade_table::edid_tubes()
        .into_iter()
        .map(|d| Screen {
            profile: d.profile,
            caps: d.caps,
        })
        .collect();
    let run = run(&sources(), &screens, FULL, None);
    report("EDID tubes", &run);
    println!(
        "no picture {:?}, fill {:?}",
        run.no_picture, run.fill_fallback
    );
    assert_eq!(run.no_picture, BTreeSet::new());
    assert_eq!(run.fill_fallback, BTreeSet::new());
}

#[test]
fn offer_on_the_panels() {
    let screens: Vec<Screen> = arcade_table::panels()
        .into_iter()
        .map(|d| Screen {
            profile: d.profile,
            caps: d.caps,
        })
        .collect();
    let run = run(&sources(), &screens, FULL, None);
    report("panels", &run);
    assert!(run.no_picture.is_empty(), "{:?}", run.no_picture);
    assert!(run.fill_fallback.is_empty(), "{:?}", run.fill_fallback);
}

#[test]
fn offer_on_the_tv_standards_of_the_composite_output() {
    let screens: Vec<Screen> = TvStandard::ALL
        .into_iter()
        .map(|standard| Screen {
            profile: standard.profile(),
            caps: composite(),
        })
        .collect();
    let run = run(&sources(), &screens, FULL, None);
    report("TV standards", &run);
    assert!(run.no_picture.is_empty(), "{:?}", run.no_picture);
    assert!(run.fill_fallback.is_empty(), "{:?}", run.fill_fallback);
}

/// The GPU entries of the matrix that drive a CRT, by their limits: the
/// entries with the same limits give the same decisions, so one run covers
/// them all (`every_gpu_of_the_matrix_has_its_run` holds the groups). Each
/// run has three parts, which run side by side. `pi_vc4_composite` drives
/// only a TV: the TV test takes it.
const GPU_GROUPS: [(&str, &[&str]); 3] = [
    (
        "intel_gen9",
        &[
            "intel_gen9",
            "intel_i915",
            "intel_xe",
            "amd_amdgpu",
            "amd_radeon",
            "nvidia_nouveau",
            "unknown",
        ],
    ),
    ("pi_vc4_dpi", &["pi_vc4_dpi"]),
    ("pi5_rp1_dpi", &["pi5_rp1_dpi"]),
];

macro_rules! every_gpu_group {
    ($($test:ident: $gpu:literal, $part:literal;)*) => {
        $(
            #[test]
            fn $test() {
                presets_run($gpu, Part { part: $part, parts: 3 }, false);
            }
        )*
    };
}

every_gpu_group! {
    gpus_with_a_25_mhz_floor_part_1: "intel_gen9", 0;
    gpus_with_a_25_mhz_floor_part_2: "intel_gen9", 1;
    gpus_with_a_25_mhz_floor_part_3: "intel_gen9", 2;
    pi_vc4_dpi_part_1: "pi_vc4_dpi", 0;
    pi_vc4_dpi_part_2: "pi_vc4_dpi", 1;
    pi_vc4_dpi_part_3: "pi_vc4_dpi", 2;
    pi5_rp1_dpi_part_1: "pi5_rp1_dpi", 0;
    pi5_rp1_dpi_part_2: "pi5_rp1_dpi", 1;
    pi5_rp1_dpi_part_3: "pi5_rp1_dpi", 2;
}

/// Every GPU entry of the matrix is in one group, and has the limits of
/// the entry that the group runs; `pi_vc4_composite` is the TV run.
#[test]
fn every_gpu_of_the_matrix_has_its_run() {
    let mut covered: Vec<&str> = GPU_GROUPS
        .iter()
        .flat_map(|(_, ids)| ids.iter().copied())
        .chain(["pi_vc4_composite"])
        .collect();
    covered.sort_unstable();
    let mut all: Vec<String> = gpu_table().into_iter().map(|(id, _)| id).collect();
    all.sort_unstable();
    assert_eq!(covered, all);
    for (run, ids) in GPU_GROUPS {
        for id in ids {
            assert_eq!(caps_of(id), caps_of(run), "{id} has the limits of {run}");
        }
    }
}

/// A small generator with a fixed seed: the same values on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn within(&mut self, low: u64, high: u64) -> u64 {
        low + self.next() % (high - low + 1)
    }
}

const SEED: u64 = 0x005E_ED0F_0FFE;

/// The cells of the generated run with no picture at all.
fn generated_no_picture() -> Vec<Cell> {
    Vec::new()
}

/// The cells of the generated run where `auto` takes the Fill picture: a
/// source at 130.001 Hz, one millihertz above the top of a range of
/// `pc_70_120`, on its ranges moved by the seed. Before the fallback it had
/// no picture under `auto`, `native`, `sharp` and `speed`; those three now
/// show the picture of `auto`, which is the Fill picture.
fn generated_fill_fallback() -> Vec<Cell> {
    vec![("pc_70_120~moved".to_owned(), "edge", 130_001, "auto")]
}

/// The CRT presets with their line rate limits moved by up to 1 % and
/// their field rate limits by up to 3 %, generated sources, and sources at
/// each field rate edge of each range, one millihertz inside and outside:
/// the same invariants hold. A profile that the solver refuses gives an
/// error, never a panic; such errors stay below a quarter of the cases. A
/// failure prints the seed and the case. The run has twelve parts.
/// The parts of the generated run, which run side by side: each stays
/// under 20 s on a loaded machine.
macro_rules! generated_parts {
    ($($test:ident: $part:literal,)*) => {
        $(
            #[test]
            fn $test() {
                generated_run(Part { part: $part, parts: 12 });
            }
        )*
    };
}

generated_parts! {
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_1: 0,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_2: 1,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_3: 2,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_4: 3,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_5: 4,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_6: 5,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_7: 6,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_8: 7,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_9: 8,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_10: 9,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_11: 10,
    generated_ranges_and_sources_with_their_edges_keep_every_invariant_part_12: 11,
}

fn generated_run(part: Part) {
    let mut rng = Lcg(SEED);
    let mut screens = Vec::new();
    let mut sources: Vec<(&'static str, SourceGeometry)> = Vec::new();
    for profile in all_presets() {
        let MonitorKind::Crt { ranges } = &profile.kind else {
            continue;
        };
        for range in ranges {
            for rate in [
                range.v_min_mhz.saturating_sub(1),
                range.v_min_mhz,
                range.v_min_mhz + 1,
                range.v_max_mhz.saturating_sub(1),
                range.v_max_mhz,
                range.v_max_mhz + 1,
            ] {
                sources.push((
                    "edge",
                    source(320, 240, u32::try_from(rate).unwrap_or(u32::MAX)),
                ));
            }
        }
        let moved: Vec<CrtRange> = ranges
            .iter()
            .map(|r| {
                let h_min = r.h_min_mhz * rng.within(990, 1_010) / 1_000;
                let h_max = (r.h_max_mhz * rng.within(990, 1_010) / 1_000).max(h_min);
                let v_min = r.v_min_mhz * rng.within(97, 103) / 100;
                let v_max = (r.v_max_mhz * rng.within(97, 103) / 100).max(v_min);
                CrtRange {
                    h_min_mhz: h_min,
                    h_max_mhz: h_max,
                    v_min_mhz: v_min,
                    v_max_mhz: v_max,
                    ..*r
                }
            })
            .collect();
        screens.push(Screen {
            profile: MonitorProfile {
                id: format!("{}~moved", profile.id),
                kind: MonitorKind::Crt { ranges: moved },
            },
            caps: open_caps(),
        });
        screens.push(Screen {
            profile,
            caps: open_caps(),
        });
    }
    sources.sort_by_key(|(_, s)| (s.refresh_mhz, s.height));
    sources.dedup_by_key(|(_, s)| s.refresh_mhz);
    for _ in 0..24 {
        sources.push((
            "generated",
            SourceGeometry {
                interlaced: rng.within(0, 5) == 0,
                ..source(
                    u16::try_from(rng.within(160, 720)).unwrap_or(320),
                    u16::try_from(rng.within(144, 600)).unwrap_or(240),
                    u32::try_from(rng.within(25_000, 125_000)).unwrap_or(60_000),
                )
            },
        ));
    }
    // The parts split the sources, not the screens: a profile that the
    // solver refuses then spreads over every part, as over the whole run.
    let sources: Vec<(&'static str, SourceGeometry)> = sources
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % part.parts == part.part)
        .map(|(_, s)| s)
        .collect();
    let depth = Depth {
        every_scanline: false,
        deep: true,
        tolerate_solver: true,
    };
    let run = run(&sources, &screens, depth, Some(SEED));
    println!(
        "no picture {:?}, fill {:?}",
        run.no_picture, run.fill_fallback
    );
    let named = |list: &[Cell]| -> BTreeSet<Cell> {
        list.iter()
            .filter(|(_, name, rate, _)| {
                sources
                    .iter()
                    .any(|(n, s)| n == name && s.refresh_mhz == *rate)
            })
            .cloned()
            .collect()
    };
    assert_eq!(
        run.no_picture,
        named(&generated_no_picture()),
        "seed {SEED}"
    );
    assert_eq!(
        run.fill_fallback,
        named(&generated_fill_fallback()),
        "seed {SEED}"
    );
    report("generated", &run);
    let total: usize = run.counts.values().sum();
    assert!(
        run.counts.get("solver refuses").copied().unwrap_or(0) * 4 < total,
        "seed {SEED}: {:?}",
        run.counts
    );
}

/// 640x500 at 60.000 Hz on `generic_15` with `force_60`: the winner is a
/// soft fit at 60 Hz. The option is offered: it gives the rate it names,
/// and the notice names the soft picture.
#[test]
fn a_forced_rate_with_a_soft_fit_is_offered_with_its_notice() {
    let profile = preset("generic_15");
    let caps = open_caps();
    let settings = PolicySettings {
        refresh: RefreshPolicy::Force60,
        ..PolicySettings::default()
    };
    let answer = offer(
        &source(640, 500, 60_000),
        &settings,
        AUTO_75,
        &profile,
        &caps,
        |m| validate(m, &profile, &caps).map(drop),
    )
    .unwrap();
    let offered = answer.expect("offered");
    let considered = &offered.plan.considered[offered.winner];
    assert!(considered.rung.is_fitted());
    assert_eq!(considered.target.kind, RefreshKind::Forced60);
    assert!(offered.notice.is_some_and(|n| n.soft.is_some()));
}

/// On a TV and a flat panel a stored refresh choice other than Auto greys
/// nothing: the display does not read it.
#[test]
fn a_stored_refresh_choice_greys_nothing_where_the_display_does_not_read_it() {
    for (profile, caps) in [
        (TvStandard::Ntsc.profile(), composite()),
        (panel_1080(), open_caps()),
    ] {
        for refresh in RefreshPolicy::ALL {
            let settings = PolicySettings {
                refresh,
                ..PolicySettings::default()
            };
            let choices = picture_choices(
                &source(256, 224, 60_099),
                &settings,
                AUTO_75,
                &profile,
                &caps,
                |m| validate(m, &profile, &caps).map(drop),
            )
            .unwrap();
            for style in [PictureStyle::Auto, PictureStyle::Sharp, PictureStyle::Fit] {
                let choice = &choices.modes.iter().find(|(s, _)| *s == style).unwrap().1;
                assert!(
                    matches!(choice.answer, RowAnswer::Plain(_)),
                    "{} {refresh} {style}",
                    profile.id
                );
            }
            let stored = offer(
                &source(256, 224, 60_099),
                &settings,
                AUTO_75,
                &profile,
                &caps,
                |m| validate(m, &profile, &caps).map(drop),
            )
            .unwrap();
            assert!(stored.is_ok(), "{} {refresh}: no stored notice", profile.id);
        }
    }
}

/// The options of the picture mode row and of the refresh row, by the pair
/// rule.
type Rows = (Vec<(PictureStyle, Row)>, Vec<(RefreshPolicy, Row)>);

/// The rows of one stored pair on `profile` with an open GPU.
fn rows_for(
    source: SourceGeometry,
    style: PictureStyle,
    refresh: RefreshPolicy,
    profile: &MonitorProfile,
) -> Rows {
    let settings = PolicySettings {
        style,
        refresh,
        ..PolicySettings::default()
    };
    let caps = open_caps();
    let choices = picture_choices(&source, &settings, AUTO_75, profile, &caps, |m| {
        validate(m, profile, &caps).map(drop)
    })
    .unwrap();
    (
        choices
            .modes
            .iter()
            .map(|(s, c)| (*s, row_of(&c.answer)))
            .collect(),
        choices
            .refresh
            .iter()
            .map(|(r, c)| (*r, row_of(&c.answer)))
            .collect(),
    )
}

fn row<K: PartialEq + Copy>(rows: &[(K, Row)], key: K) -> Row {
    rows.iter().find(|(k, _)| *k == key).unwrap().1
}

/// On a CRT, a stored pair that the display does not give, `2x` with
/// `exact_x2` on `generic_15` for the SNES: an option whose pair with the
/// stored value of the other row is declined, and whose pair with the other
/// row at Auto is offered, resets the other row; an option declined with
/// the other row at Auto too is greyed with a reason of its own row.
#[test]
fn a_stored_pair_that_the_display_declines_resets_or_greys_by_the_pair_rule() {
    let generic = preset("generic_15");
    let (modes, rates) = rows_for(
        source(256, 224, 60_099),
        PictureStyle::X2,
        RefreshPolicy::ExactX2,
        &generic,
    );
    assert!(matches!(row(&modes, PictureStyle::Auto), Row::Reset(_)));
    assert!(matches!(
        row(&modes, PictureStyle::X2),
        Row::Declined(DeclineReason::NotShownHere { .. })
    ));
    assert!(matches!(row(&rates, RefreshPolicy::Exact), Row::Reset(_)));
    assert!(matches!(
        row(&rates, RefreshPolicy::ExactX2),
        Row::Declined(DeclineReason::RateNotCovered { .. })
    ));
}

/// The SNES on `generic_15` with a stored `force_50`: Game speed does not
/// hold the speed at 50 Hz, but it does at the game's rate, so it is not
/// greyed: a pick also sets the refresh rate to Auto.
#[test]
fn game_speed_with_a_stored_forced_rate_resets_the_rate() {
    let (modes, _) = rows_for(
        source(256, 224, 60_099),
        PictureStyle::Auto,
        RefreshPolicy::Force50,
        &preset("generic_15"),
    );
    assert_eq!(
        row(&modes, PictureStyle::Speed),
        Row::Reset(DeclineReason::SpeedNotHeld)
    );
}

/// A 160x144 source at 50 Hz on `generic_15` with a stored (`2x`,
/// `force_60`): Game speed is plain only where the pair (`speed`,
/// `force_60`), which a pick stores, is offered.
#[test]
fn an_option_is_plain_only_where_the_pair_a_pick_stores_is_offered() {
    let profile = preset("generic_15");
    let caps = open_caps();
    let game = SourceGeometry {
        aspect_milli: 1_111,
        ..source(160, 144, 50_000)
    };
    let (modes, _) = rows_for(game, PictureStyle::X2, RefreshPolicy::Force60, &profile);
    let stored = offer(
        &game,
        &PolicySettings {
            style: PictureStyle::Speed,
            refresh: RefreshPolicy::Force60,
            ..PolicySettings::default()
        },
        AUTO_75,
        &profile,
        &caps,
        |m| validate(m, &profile, &caps).map(drop),
    )
    .unwrap();
    assert!(stored.is_err(), "the pair (speed, force_60) is declined");
    assert_ne!(row(&modes, PictureStyle::Speed), Row::Plain);
}

/// Game speed is declined where its winner still changes the speed by
/// more than 0.5 %: DOS 640x400 at 70 Hz on `generic_15`, a 50 Hz game on a
/// panel with only a 60 Hz mode. It is offered where it holds the speed.
#[test]
fn game_speed_is_declined_where_it_cannot_hold_the_speed() {
    let speed = PolicySettings {
        style: PictureStyle::Speed,
        ..PolicySettings::default()
    };
    let generic = preset("generic_15");
    let answer = |source: SourceGeometry, profile: &MonitorProfile| {
        offer(&source, &speed, AUTO_75, profile, &open_caps(), |m| {
            validate(m, profile, &open_caps()).map(drop)
        })
        .unwrap()
        .map(drop)
        .map_err(|declined| declined.reason)
    };
    let dos = answer(source(640, 400, 70_086), &generic);
    assert_eq!(dos, Err(DeclineReason::SpeedNotHeld));
    let panel_60 = arcade_table::panels()
        .into_iter()
        .find(|d| d.profile.id == "panel_720")
        .unwrap()
        .profile;
    let pal = answer(source(320, 240, 50_000), &panel_60);
    assert_eq!(pal, Err(DeclineReason::SpeedNotHeld));
    assert!(answer(source(256, 224, 60_099), &generic).is_ok());
}

/// The table of section 8 of docs/spec/video-policy-cases.md is the
/// output of [`picture_choices`] for the SNES (256x224 at 60.099 Hz) with
/// scanlines `auto`: one table per display class, a row per picture mode,
/// a column per refresh choice. To print it again:
/// `cargo xtask test -p neptron-modeline --no-capture spec_table`.
#[test]
fn the_spec_table_of_offered_combinations_is_the_output_of_the_function() {
    let displays = [
        (
            "15 kHz CRT (`generic_15`)",
            preset("generic_15"),
            open_caps(),
        ),
        ("31 kHz CRT (`arcade_31`)", preset("arcade_31"), open_caps()),
        (
            "31 kHz PC CRT up to 120 Hz (`pc_31_120`)",
            preset("pc_31_120"),
            open_caps(),
        ),
        ("TV (`tv_ntsc`)", TvStandard::Ntsc.profile(), composite()),
        ("Flat panel (`panel_1080`)", panel_1080(), open_caps()),
    ];
    let snes = source(256, 224, 60_099);
    let scanlines = AUTO_75;
    let mut table = String::new();
    for (title, profile, caps) in &displays {
        table.push_str(&format!("\n**{title}**\n\n| Mode |"));
        for refresh in RefreshPolicy::ALL {
            table.push_str(&format!(" `{refresh}` |"));
        }
        table.push_str("\n| --- |");
        for _ in RefreshPolicy::ALL {
            table.push_str(" --- |");
        }
        table.push('\n');
        for style in PictureStyle::ALL {
            table.push_str(&format!("| `{style}` |"));
            for refresh in RefreshPolicy::ALL {
                let settings = PolicySettings {
                    style,
                    refresh,
                    ..PolicySettings::default()
                };
                let answer = offer(&snes, &settings, scanlines, profile, caps, |m| {
                    validate(m, profile, caps).map(drop)
                })
                .unwrap();
                let cell = match &answer {
                    Ok(o) => {
                        let m = mode_of(&o.plan, o.winner);
                        format!(
                            "{}x{}{} {}.{:02} Hz",
                            m.hactive,
                            m.vactive,
                            if m.interlace { "i" } else { "p" },
                            m.field_rate_mhz() / 1_000,
                            m.field_rate_mhz() % 1_000 / 10
                        )
                    }
                    Err(d) => format!("no: {}", reason_name(d.reason)),
                };
                table.push_str(&format!(" {cell} |"));
            }
            table.push('\n');
        }
    }
    // The rows of the menus come from picture_choices, the same function.
    let choices = picture_choices(
        &snes,
        &PolicySettings::default(),
        scanlines,
        &displays[0].1,
        &displays[0].2,
        |m| validate(m, &displays[0].1, &displays[0].2).map(drop),
    )
    .unwrap();
    assert_eq!(choices.modes.len(), PictureStyle::ALL.len());
    assert_eq!(choices.refresh.len(), RefreshPolicy::ALL.len());
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/video-policy-cases.md");
    let text = std::fs::read_to_string(&spec).unwrap();
    let begin = "<!-- offer-table:begin -->\n";
    let end = "<!-- offer-table:end -->";
    let start = text
        .find(begin)
        .map(|i| i + begin.len())
        .expect("the begin marker");
    let stop = text.find(end).expect("the end marker");
    println!("{table}");
    assert_eq!(
        &text[start..stop],
        table.trim_start_matches('\n').to_owned() + "\n",
        "the spec table drifted; print it again and copy it"
    );
}

/// Where the plan of `auto` has no picture, the decision takes the picture
/// of `fit`, with the notice of the fallback in place of the soft reason of
/// the Fill mode; a stored mode with no picture shows the picture of
/// `auto`, which is then the Fill picture, and is declined with
/// `NoPicture`. A validator that accepts only the modes that the plan of
/// `fit` has and the plan of `auto` lacks makes the case.
#[test]
fn auto_with_no_picture_takes_the_fill_picture() {
    let profile = preset("arcade_25");
    let caps = open_caps();
    let ddonpach = SourceGeometry {
        rotation: neptron_types::Rotation::R90,
        ..source(320, 240, 57_550)
    };
    let plan_of = |style| {
        let settings = PolicySettings {
            style,
            ..PolicySettings::default()
        };
        decide(&ddonpach, &settings, &profile, &caps).unwrap()
    };
    let auto_modes: Vec<Modeline> = plan_of(PictureStyle::Auto)
        .proposals()
        .map(|(_, _, p)| p.mode)
        .collect();
    let only_fit: Vec<Modeline> = plan_of(PictureStyle::Fit)
        .proposals()
        .map(|(_, _, p)| p.mode)
        .filter(|m| !auto_modes.contains(m))
        .collect();
    assert!(!only_fit.is_empty(), "the plan of fit has modes of its own");
    let validate = |m: &Modeline| {
        if only_fit.contains(m) {
            Ok(())
        } else {
            Err(())
        }
    };
    let shown = neptron_modeline::policy::decide_shown(
        &ddonpach,
        &PolicySettings::default(),
        &profile,
        &caps,
        validate,
    )
    .unwrap();
    assert_eq!(shown.fallback, Some(PictureStyle::Fit));
    let notice = shown.notice().expect("a notice");
    assert_eq!(notice.fill_fallback, Some(PictureStyle::Auto));
    assert_ne!(
        notice.soft,
        Some(neptron_modeline::policy::SoftReason::FillMode)
    );
    let auto = offer(
        &ddonpach,
        &PolicySettings::default(),
        AUTO_75,
        &profile,
        &caps,
        validate,
    )
    .unwrap();
    assert!(auto.is_ok_and(|o| {
        o.notice
            .is_some_and(|n| n.fill_fallback == Some(PictureStyle::Auto))
    }));
    let sharp = offer(
        &ddonpach,
        &PolicySettings {
            style: PictureStyle::Sharp,
            ..PolicySettings::default()
        },
        AUTO_75,
        &profile,
        &caps,
        validate,
    )
    .unwrap();
    let declined = sharp.expect_err("declined");
    assert_eq!(declined.reason, DeclineReason::NoPicture);
    assert_eq!(declined.plan.style, PictureStyle::Fit);
    assert!(declined.winner.is_some(), "the Fill picture shows");
    // The notice of that picture names the stored mode, which has no
    // picture, and Auto, which has none either.
    let stored = neptron_modeline::policy::decide_shown(
        &ddonpach,
        &PolicySettings {
            style: PictureStyle::Sharp,
            ..PolicySettings::default()
        },
        &profile,
        &caps,
        validate,
    )
    .unwrap();
    assert_eq!(
        stored.notice().and_then(|n| n.fill_fallback),
        Some(PictureStyle::Sharp)
    );
    // Where Fill has no picture either, nothing changes: no winner.
    let none = neptron_modeline::policy::decide_shown(
        &ddonpach,
        &PolicySettings::default(),
        &profile,
        &caps,
        |_: &Modeline| Err::<(), ()>(()),
    )
    .unwrap();
    assert_eq!((none.winner, none.fallback), (None, None));
}

/// A whole multiple that validates but ranks after a smaller step of a
/// lower line rate range: the Lynx in 3x at 50 Hz on `d9200` shows a padded
/// 2x; the reason is `SmallerFillsMore`, not `NotShownHere`. The order of
/// the plan does not change.
#[test]
fn a_whole_multiple_that_ranks_after_a_smaller_step_says_so() {
    let profile = preset("d9200");
    let caps = open_caps();
    let lynx = SourceGeometry {
        aspect_milli: 1_569,
        ..source(160, 102, 75_000)
    };
    let settings = PolicySettings {
        style: PictureStyle::X3,
        refresh: RefreshPolicy::Force50,
        ..PolicySettings::default()
    };
    let answer = offer(&lynx, &settings, AUTO_75, &profile, &caps, |m| {
        validate(m, &profile, &caps).map(drop)
    })
    .unwrap();
    let declined = answer.expect_err("declined");
    assert_eq!(
        declined.reason,
        DeclineReason::SmallerFillsMore {
            asked: PictureStyle::X3,
            shown: Rung::Multiple(2),
        }
    );
}

/// A turned 288x224 board at 60 Hz in Native on `d9200`: Native is
/// reachable at the rate of the winner, but the winner, the interlaced
/// picture, fills more of the screen height. The reason says so, and is
/// never `SmallerFillsMore`, which names a smaller multiple.
#[test]
fn the_interlaced_picture_that_fills_more_says_so() {
    let profile = preset("d9200");
    let caps = open_caps();
    let board = SourceGeometry {
        rotation: neptron_types::Rotation::R270,
        ..source(288, 224, 60_000)
    };
    let settings = PolicySettings {
        style: PictureStyle::Native,
        ..PolicySettings::default()
    };
    let answer = offer(&board, &settings, AUTO_75, &profile, &caps, |m| {
        validate(m, &profile, &caps).map(drop)
    })
    .unwrap();
    assert_eq!(
        answer.expect_err("declined").reason,
        DeclineReason::InterlacedFillsMore {
            asked: PictureStyle::Native
        }
    );
}

/// The answers made a few decisions at a time are the answers made at
/// once, for one source and for every reference source: a frame can spread
/// the work and get the same page.
#[test]
fn the_answers_made_step_by_step_are_the_answers_made_at_once() {
    use neptron_modeline::policy::{ChoicesWork, REFERENCE_SOURCES, for_every_game};
    for id in ["generic_15", "pc_31_120"] {
        let profile = preset(id);
        let caps = open_caps();
        let validator = |m: &Modeline| validate(m, &profile, &caps).map(drop);
        let settings = PolicySettings {
            refresh: RefreshPolicy::Force50,
            ..PolicySettings::default()
        };
        let at_once: Vec<_> = REFERENCE_SOURCES
            .iter()
            .map(|s| picture_choices(s, &settings, AUTO_75, &profile, &caps, validator).unwrap())
            .collect();
        let mut work = ChoicesWork::new(&REFERENCE_SOURCES, settings, AUTO_75);
        let mut steps = 0;
        while !work.step(&profile, &caps, validator, 2) {
            steps += 1;
        }
        assert!(steps > 10, "{id}: the work took {steps} steps");
        assert_eq!(work.finish().ok(), for_every_game(&at_once), "{id}");
        let mut one = ChoicesWork::new(&REFERENCE_SOURCES[..1], settings, AUTO_75);
        while !one.step(&profile, &caps, validator, 1) {}
        assert_eq!(one.finish().ok().as_ref(), at_once.first(), "{id}");
    }
}
