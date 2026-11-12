//! Property test of the policy, with a fixed seed: over random sources,
//! settings and every shipped preset, the policy never panics, every winner
//! passes the validator, every game rectangle lies inside its mode, every
//! whole-scale claim holds exactly, no fitted proposal comes before a pixel
//! exact one, the `native` style never scales the game's lines by a whole
//! multiple above 1 on a CRT, and the decision is deterministic. Each
//! source runs with every picture style and every refresh policy. On the
//! winner: a doubled or forced rate that the policy asks for wins whenever
//! an exact mode at it validates; a progressive source never gets an
//! interlaced fit while a progressive exact mode validates; the notice
//! says "fitted" exactly for a fitted winner, gives the fill exactly for a
//! padded winner, and gives the cropped lines exactly when a crop exists.

use neptron_modeline::policy::{
    Considered, HorizontalStrategy, OVERSCAN_CROP_MAX_LINES_PER_EDGE, OrientedSource, PictureStyle,
    Plan, PolicyError, PolicySettings, Proposal, RefreshKind, RefreshPolicy, Rung, Winner, choose,
    decide, notice,
};
use neptron_safety::validate;
use neptron_types::{GpuCaps, MonitorKind, MonitorProfile, Rotation, SourceGeometry, ui_dest_fill};

use crate::presets::{
    all_presets, edid_caps, gpu_table, open_caps, panel_1080, pc_crt_30_71, unknown_gpu,
};

/// Sources per profile and GPU, one in four near the lines a range holds. With 27 profiles, 4 GPUs, 5 styles and 4
/// refresh policies this is 43,200 decisions.
const SOURCES_PER_PROFILE: usize = 20;

const REFRESH_POLICIES: [RefreshPolicy; 4] = [
    RefreshPolicy::Exact,
    RefreshPolicy::ExactX2,
    RefreshPolicy::Force50,
    RefreshPolicy::Force60,
];

const SEED: u64 = 0x4e45_5054_524f_4e15;

/// A 64-bit linear congruential generator (Knuth's MMIX constants): no
/// dependency, and the same sequence on every host.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn range(&mut self, low: u64, high: u64) -> u64 {
        low + self.next() % (high - low + 1)
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.next() as usize % items.len()]
    }
}

/// A source whose lines miss what one range of a CRT profile holds by 1
/// to 16 lines, so that a crop of 1 to 8 lines per edge applies: at the
/// game's rate, at twice it (the rate of `exact_x2`), or at 2x. `None` on
/// a panel or for a range with no progressive window.
fn near_capacity_source(rng: &mut Lcg, profile: &MonitorProfile) -> Option<SourceGeometry> {
    let MonitorKind::Crt { ranges } = &profile.kind else {
        return None;
    };
    let range = ranges[rng.next() as usize % ranges.len()];
    if range.progressive_lines_min == 0 || range.v_max_mhz < range.v_min_mhz {
        return None;
    }
    let rate = rng.range(range.v_min_mhz.max(1), range.v_max_mhz.max(1));
    let blank_ns = u128::from(range.v_front_porch_ns)
        + u128::from(range.v_sync_ns)
        + u128::from(range.v_back_porch_ns);
    let blank = (blank_ns * u128::from(range.h_max_mhz)).div_ceil(1_000_000_000_000);
    let lines = u128::from(range.h_max_mhz / rate).saturating_sub(blank);
    let capacity = u64::try_from(lines)
        .unwrap_or(u64::MAX)
        .min(u64::from(range.progressive_lines_max));
    let (game_rate, height) = match rng.range(0, 2) {
        0 => (rate, capacity + rng.range(1, 16)),
        1 => (rate / 2, capacity + rng.range(1, 16)),
        _ => (rate, capacity / 2 + rng.range(1, 8)),
    };
    Some(SourceGeometry {
        width: rng.range(64, 1_024) as u16,
        height: u16::try_from(height).ok()?,
        refresh_mhz: u32::try_from(game_rate).ok()?.max(1),
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    })
}

/// A source of a usual shape, or one time in eight a value at an extreme:
/// zero, one or the largest the type holds, on any axis.
fn random_source(rng: &mut Lcg) -> SourceGeometry {
    let extreme = rng.range(0, 7) == 0;
    let (width, height, refresh_mhz) = if extreme {
        (
            rng.pick(&[0, 1, u16::MAX, 256]),
            rng.pick(&[0, 1, u16::MAX, 224]),
            rng.pick(&[0, 1, u32::MAX, 60_000]),
        )
    } else {
        (
            rng.range(64, 1_024) as u16,
            rng.range(64, 600) as u16,
            rng.range(20_000, 130_000) as u32,
        )
    };
    SourceGeometry {
        width,
        height,
        refresh_mhz,
        aspect_milli: rng.pick(&[0, 1_333, 1_111, 750, 1_777]),
        interlaced: rng.range(0, 9) == 0,
        rotation: rng.pick(&[Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270]),
    }
}

/// The settings of one source with a random horizontal strategy, for
/// every picture style and every refresh policy.
fn every_setting(rng: &mut Lcg) -> Vec<PolicySettings> {
    let horizontal = random_horizontal(rng);
    PictureStyle::ALL
        .into_iter()
        .flat_map(|style| {
            REFRESH_POLICIES
                .into_iter()
                .map(move |refresh| PolicySettings {
                    style,
                    refresh,
                    horizontal,
                    ..PolicySettings::default()
                })
        })
        .collect()
}

fn random_horizontal(rng: &mut Lcg) -> Option<HorizontalStrategy> {
    rng.pick(&[
        None,
        Some(HorizontalStrategy::Native),
        Some(HorizontalStrategy::DynamicSuperWidth),
        Some(HorizontalStrategy::FixedSuperWidth { width: 2_560 }),
    ])
}

fn gpus() -> Vec<(String, GpuCaps)> {
    let mut gpus: Vec<(String, GpuCaps)> = gpu_table()
        .into_iter()
        .filter(|(id, _)| id == "intel_gen9" || id == "pi5_rp1_dpi")
        .collect();
    gpus.push(("open".into(), open_caps()));
    gpus.push(("unknown".into(), unknown_gpu()));
    gpus
}

/// The rectangle lies inside the active area, the crop is the whole frame
/// less the overscan crop (lines only, the same at both edges, within the
/// bound), every whole-scale claim holds exactly against the crop, and on
/// a CRT the UI fills the active area.
fn check_placement(p: &Proposal, source: &OrientedSource, crt: bool) -> Result<(), String> {
    let (m, place) = (&p.mode, &p.placement);
    let d = &place.dest;
    let inside = d.x >= 0
        && d.y >= 0
        && i64::from(d.x) + i64::from(d.w) <= i64::from(m.hactive)
        && i64::from(d.y) + i64::from(d.h) <= i64::from(m.vactive)
        && d.w > 0
        && d.h > 0;
    if !inside {
        return Err(format!("game {d:?} outside {}x{}", m.hactive, m.vactive));
    }
    let crop = &place.crop;
    let per_edge = p.overscan_crop.map_or(0, |c| c.lines_per_edge);
    if per_edge > OVERSCAN_CROP_MAX_LINES_PER_EDGE {
        return Err(format!("{per_edge} lines cropped per edge"));
    }
    let (fw, fh) = (
        u32::from(source.frame_width),
        u32::from(source.frame_height),
    );
    let cut = u32::from(per_edge);
    let edge = i32::from(per_edge);
    let want = if source.turned {
        (edge, 0, fw.saturating_sub(2 * cut), fh)
    } else {
        (0, edge, fw, fh.saturating_sub(2 * cut))
    };
    if (crop.x, crop.y, crop.w, crop.h) != want {
        return Err(format!(
            "crop {crop:?} is not the frame less {per_edge} lines per edge"
        ));
    }
    let (crop_width, crop_lines) = if source.turned {
        (crop.h, crop.w)
    } else {
        (crop.w, crop.h)
    };
    if let Some(k) = place.scale.x
        && d.w != crop_width * u32::from(k)
    {
        return Err(format!("x claim {k} but {} from {crop_width}", d.w));
    }
    if let Some(k) = place.scale.y
        && d.h != crop_lines * u32::from(k)
    {
        return Err(format!("y claim {k} but {} from {crop_lines}", d.h));
    }
    if crt && place.ui_dest != ui_dest_fill(m.hactive, m.vactive) {
        return Err(format!("ui {:?} does not fill the mode", place.ui_dest));
    }
    Ok(())
}

/// On a CRT, every exact proposal of the `native` style shows the game's
/// lines once: the native rung or rung 3, at a vertical scale of 1.
fn check_native(considered: &Considered, proposal: &Proposal) -> Result<(), String> {
    match considered.rung {
        Rung::Fitted => Ok(()),
        Rung::Native | Rung::Interlaced if proposal.placement.scale.y == Some(1) => Ok(()),
        rung => Err(format!(
            "native gives {rung} at a vertical scale of {:?}",
            proposal.placement.scale.y
        )),
    }
}

fn check_plan(plan: &Plan, profile: &MonitorProfile, caps: &GpuCaps) -> Result<(), String> {
    let crt = matches!(profile.kind, MonitorKind::Crt { .. });
    let mut fitted_seen = None;
    for (index, considered, proposal) in plan.proposals() {
        check_placement(proposal, &plan.source, crt)
            .map_err(|e| format!("#{index} {considered}: {e}"))?;
        if crt && plan.style == PictureStyle::Native {
            check_native(considered, proposal)
                .map_err(|e| format!("#{index} {considered}: {e}"))?;
        }
        match (considered.rung.is_fitted(), fitted_seen) {
            (true, None) => fitted_seen = Some(index),
            (false, Some(fitted)) => {
                return Err(format!(
                    "#{index} {considered} comes after fitted #{fitted}"
                ));
            }
            _ => {}
        }
    }
    let choice = choose(plan, |m| validate(m, profile, caps));
    // choose() returns the token of the mode it validated; this checks that
    // the winner's proposal is that same mode.
    if let Some(w) = choice.winner {
        validate(&w.proposal.mode, profile, caps)
            .map_err(|e| format!("winner {} refused: {e}", w.considered))?;
        let valid = |p: &Proposal| validate(&p.mode, profile, caps).is_ok();
        check_asked_rate(plan, &w, valid)?;
        check_no_interlaced_fit(plan, &w, valid)?;
        check_notice(plan, &w)?;
    }
    Ok(())
}

/// (a) With `exact_x2`, `force_50` or `force_60`, the asked rate wins
/// whenever an exact mode at it validates: the user's choice of rate is
/// never dropped for a mode at another rate.
fn check_asked_rate<V>(
    plan: &Plan,
    w: &Winner<'_, V>,
    valid: impl Fn(&Proposal) -> bool,
) -> Result<(), String> {
    let asked = match plan.refresh {
        RefreshPolicy::Exact => return Ok(()),
        RefreshPolicy::ExactX2 => RefreshKind::Double,
        RefreshPolicy::Force50 => RefreshKind::Forced50,
        RefreshPolicy::Force60 => RefreshKind::Forced60,
    };
    let exists = plan
        .proposals()
        .any(|(_, c, p)| !c.rung.is_fitted() && c.target.kind == asked && valid(p));
    if exists && w.considered.target.kind != asked {
        return Err(format!(
            "winner {} is not at the asked {asked}, an exact mode there validates",
            w.considered
        ));
    }
    Ok(())
}

/// (b) A progressive source never gets an interlaced fit while a
/// progressive exact mode validates.
fn check_no_interlaced_fit<V>(
    plan: &Plan,
    w: &Winner<'_, V>,
    valid: impl Fn(&Proposal) -> bool,
) -> Result<(), String> {
    let interlaced_fit = w.considered.rung.is_fitted() && w.proposal.mode.interlace;
    if plan.source.interlaced || !interlaced_fit {
        return Ok(());
    }
    let progressive_exact = plan
        .proposals()
        .any(|(_, c, p)| !c.rung.is_fitted() && !p.mode.interlace && valid(p));
    if progressive_exact {
        return Err(format!(
            "winner {} is an interlaced fit, a progressive exact mode validates",
            w.considered
        ));
    }
    Ok(())
}

/// (c) The notice says "fitted" exactly for a fitted winner, gives the
/// fill exactly when the winner is padded, and gives the cropped lines
/// exactly when the winner has a crop.
fn check_notice<V>(plan: &Plan, w: &Winner<'_, V>) -> Result<(), String> {
    let n = notice(plan, w.considered);
    let fitted = n.is_some_and(|n| n.fitted);
    if fitted != w.considered.rung.is_fitted() {
        return Err(format!("notice fitted {fitted} for {}", w.considered));
    }
    let padded = n.and_then(|n| n.padded_fill_permille);
    let fill = w
        .proposal
        .padding
        .map(|p| u16::try_from(p.fill_permille()).unwrap_or(u16::MAX));
    if padded != fill {
        return Err(format!(
            "notice fill {padded:?}, the winner {fill:?}: {}",
            w.considered
        ));
    }
    let cropped = n.map_or(0, |n| n.cropped_lines);
    let crop = w.proposal.overscan_crop.map_or(0, |c| c.lines());
    if cropped != crop {
        return Err(format!(
            "notice crops {cropped} lines, the winner {crop}: {}",
            w.considered
        ));
    }
    Ok(())
}

#[test]
fn the_policy_is_safe_and_deterministic_over_random_sources() {
    let mut profiles: Vec<(MonitorProfile, bool)> =
        all_presets().into_iter().map(|p| (p, false)).collect();
    profiles.push((pc_crt_30_71(), true));
    profiles.push((panel_1080(), false));

    let mut rng = Lcg(SEED);
    let mut failures = Vec::new();
    let mut decisions = 0;
    let mut empty_sources = 0;
    let mut winners = 0;
    for (profile, edid) in &profiles {
        for (gpu, caps) in gpus() {
            let caps = if *edid { edid_caps(caps) } else { caps };
            for _ in 0..SOURCES_PER_PROFILE {
                let near = rng.range(0, 3) == 0;
                let source = near
                    .then(|| near_capacity_source(&mut rng, profile))
                    .flatten()
                    .unwrap_or_else(|| random_source(&mut rng));
                for settings in every_setting(&mut rng) {
                    let case = format!("{} {gpu} {source:?} {settings:?}", profile.id);
                    let empty = source.width == 0 || source.height == 0 || source.refresh_mhz == 0;
                    let plan = match decide(&source, &settings, profile, &caps) {
                        Ok(plan) => plan,
                        Err(PolicyError::EmptySource { .. }) if empty => {
                            empty_sources += 1;
                            continue;
                        }
                        Err(e) => {
                            failures.push(format!("{case}: {e}"));
                            continue;
                        }
                    };
                    decisions += 1;
                    let again = decide(&source, &settings, profile, &caps);
                    if again.as_ref() != Ok(&plan) {
                        failures.push(format!("{case}: two decisions differ"));
                    }
                    if let Err(e) = check_plan(&plan, profile, &caps) {
                        failures.push(format!("{case}: {e}"));
                    }
                    if choose(&plan, |m| validate(m, profile, &caps))
                        .winner
                        .is_some()
                    {
                        winners += 1;
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(
        decisions + empty_sources,
        profiles.len() * 4 * SOURCES_PER_PROFILE * PictureStyle::ALL.len() * REFRESH_POLICIES.len()
    );
    assert!(empty_sources > 0, "no source with a zero value was drawn");
    // Most random sources fit some mode; a policy that drops everything
    // would pass every check above.
    assert!(winners * 2 > decisions, "{winners} winners in {decisions}");
}
