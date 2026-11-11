//! Property test of the policy, with a fixed seed: over random sources,
//! settings and every shipped preset, the policy never panics, every winner
//! passes the validator, every game rectangle lies inside its mode, every
//! whole-scale claim holds exactly, and the decision is deterministic.

use neptron_modeline::policy::{
    HorizontalStrategy, OrientedSource, PictureStyle, Plan, PolicySettings, Proposal,
    RefreshPolicy, choose, decide,
};
use neptron_safety::validate;
use neptron_types::{GpuCaps, MonitorKind, MonitorProfile, Rotation, SourceGeometry, ui_dest_fill};

use crate::presets::{
    all_presets, edid_caps, gpu_table, open_caps, panel_1080, pc_crt_30_71, unknown_gpu,
};

/// Sources per profile and GPU. With 27 profiles and 4 GPUs this is 4,320
/// decisions.
const SOURCES_PER_PROFILE: usize = 40;

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

fn random_source(rng: &mut Lcg) -> SourceGeometry {
    SourceGeometry {
        width: rng.range(64, 1_024) as u16,
        height: rng.range(64, 600) as u16,
        refresh_mhz: rng.range(20_000, 130_000) as u32,
        aspect_milli: rng.pick(&[0, 1_333, 1_111, 750, 1_777]),
        interlaced: rng.range(0, 9) == 0,
        rotation: rng.pick(&[Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270]),
    }
}

fn random_settings(rng: &mut Lcg) -> PolicySettings {
    PolicySettings {
        style: rng.pick(&PictureStyle::ALL),
        refresh: rng.pick(&[
            RefreshPolicy::Exact,
            RefreshPolicy::ExactX2,
            RefreshPolicy::Force50,
            RefreshPolicy::Force60,
        ]),
        horizontal: rng.pick(&[
            None,
            Some(HorizontalStrategy::Native),
            Some(HorizontalStrategy::DynamicSuperWidth),
            Some(HorizontalStrategy::FixedSuperWidth { width: 2_560 }),
        ]),
        ..PolicySettings::default()
    }
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

/// The rectangle lies inside the active area, the crop is the whole
/// frame, every whole-scale claim holds exactly, and on a CRT the UI fills
/// the active area.
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
    if (crop.x, crop.y, crop.w, crop.h)
        != (
            0,
            0,
            u32::from(source.frame_width),
            u32::from(source.frame_height),
        )
    {
        return Err(format!("crop {crop:?} is not the whole frame"));
    }
    if let Some(k) = place.scale.x
        && d.w != u32::from(source.width) * u32::from(k)
    {
        return Err(format!("x claim {k} but {} from {}", d.w, source.width));
    }
    if let Some(k) = place.scale.y
        && d.h != u32::from(source.height) * u32::from(k)
    {
        return Err(format!("y claim {k} but {} from {}", d.h, source.height));
    }
    if crt && place.ui_dest != ui_dest_fill(m.hactive, m.vactive) {
        return Err(format!("ui {:?} does not fill the mode", place.ui_dest));
    }
    Ok(())
}

fn check_plan(plan: &Plan, profile: &MonitorProfile, caps: &GpuCaps) -> Result<(), String> {
    let crt = matches!(profile.kind, MonitorKind::Crt { .. });
    for (index, considered, proposal) in plan.proposals() {
        check_placement(proposal, &plan.source, crt)
            .map_err(|e| format!("#{index} {considered}: {e}"))?;
    }
    let choice = choose(plan, |m| validate(m, profile, caps));
    if let Some(w) = choice.winner {
        validate(&w.proposal.mode, profile, caps)
            .map_err(|e| format!("winner {} refused: {e}", w.considered))?;
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
    let mut winners = 0;
    for (profile, edid) in &profiles {
        for (gpu, caps) in gpus() {
            let caps = if *edid { edid_caps(caps) } else { caps };
            for _ in 0..SOURCES_PER_PROFILE {
                let source = random_source(&mut rng);
                let settings = random_settings(&mut rng);
                let case = format!("{} {gpu} {source:?} {settings:?}", profile.id);
                let plan = match decide(&source, &settings, profile, &caps) {
                    Ok(plan) => plan,
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
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(decisions, profiles.len() * 4 * SOURCES_PER_PROFILE);
    // Most random sources fit some mode; a policy that drops everything
    // would pass every check above.
    assert!(winners * 2 > decisions, "{winners} winners in {decisions}");
}
