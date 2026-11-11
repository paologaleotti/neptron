//! The golden list of systems run through the policy: the profile goldens
//! in tests/golden/policy.toml and timing check 4, the per-GPU goldens in
//! tests/golden/policy_gpus.toml. The file headers say where the expected
//! values come from.

use std::collections::BTreeMap;
use std::path::Path;

use neptron_modeline::policy::{PolicySettings, choose, decide};
use neptron_safety::validate;
use neptron_types::{Filter, GpuCaps, MonitorProfile, SourceGeometry};
use serde::Deserialize;

use crate::golden::golden;
use crate::presets::{edid_caps, gpu_table, open_caps, panel_1080, pc_crt_30_71, preset};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    case: BTreeMap<String, String>,
}

fn expected(file: &str) -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed: Expected =
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    parsed.case
}

fn systems() -> Vec<(String, SourceGeometry)> {
    golden()
        .system
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                SourceGeometry {
                    width: s.width,
                    height: s.height,
                    refresh_mhz: s.refresh_mhz,
                    aspect_milli: 1_333,
                    interlaced: s.interlaced,
                    rotation: s.rotation,
                },
            )
        })
        .collect()
}

/// The winner for a source with the default settings (`auto`, `exact`, the
/// GPU's horizontal strategy), in one line: rung, refresh kind, mode, line
/// rate, field rate, the game's rectangle and the filter. "none" when no
/// proposal validates.
fn outcome(source: &SourceGeometry, profile: &MonitorProfile, caps: &GpuCaps) -> String {
    let plan = decide(source, &PolicySettings::default(), profile, caps)
        .unwrap_or_else(|e| panic!("{}: {e}", profile.id));
    let choice = choose(&plan, |m| validate(m, profile, caps));
    let Some(w) = choice.winner else {
        return "none".into();
    };
    let p = w.proposal;
    let m = &p.mode;
    let d = &p.placement.dest;
    format!(
        "{} {} {}x{}{} {}.{:03}kHz {}.{:03}Hz game {}x{}+{}+{} {}",
        w.considered.rung,
        w.considered.target.kind,
        m.hactive,
        m.vactive,
        if m.interlace { "i" } else { "p" },
        p.timing.line_rate_mhz / 1_000_000,
        p.timing.line_rate_mhz % 1_000_000 / 1_000,
        p.timing.field_rate_mhz / 1_000,
        p.timing.field_rate_mhz % 1_000,
        d.w,
        d.h,
        d.x,
        d.y,
        match p.placement.filter {
            Filter::Nearest => "nearest",
            Filter::SharpBilinear => "sharp",
        }
    )
}

/// Compares every case with the file and lists each difference, missing
/// case and extra case as a line of the file.
fn compare(file: &str, actual: BTreeMap<String, String>) {
    let expected = expected(file);
    let mut diffs = Vec::new();
    for (key, got) in &actual {
        match expected.get(key) {
            Some(want) if want == got => {}
            Some(want) => diffs.push(format!("# was: {want}\n\"{key}\" = \"{got}\"")),
            None => diffs.push(format!("# missing\n\"{key}\" = \"{got}\"")),
        }
    }
    for key in expected.keys().filter(|k| !actual.contains_key(*k)) {
        diffs.push(format!("# extra case in the file: {key}"));
    }
    assert!(
        diffs.is_empty(),
        "{} of {} cases differ from {file}:\n{}",
        diffs.len(),
        actual.len(),
        diffs.join("\n")
    );
}

#[test]
fn every_system_on_each_profile_gives_its_golden_decision() {
    let profiles: Vec<(MonitorProfile, GpuCaps)> = vec![
        (preset("generic_15"), open_caps()),
        (preset("arcade_15"), open_caps()),
        (preset("pc_31_120"), open_caps()),
        (pc_crt_30_71(), edid_caps(open_caps())),
        (panel_1080(), open_caps()),
    ];
    let mut actual = BTreeMap::new();
    for (name, source) in systems() {
        for (profile, caps) in &profiles {
            let key = format!("{name} {}", profile.id);
            actual.insert(key, outcome(&source, profile, caps));
        }
    }
    compare("policy.toml", actual);
}

/// Timing check 4: the suite reruns for each entry of the GPU capability
/// table, on a 15 kHz TV and on the EDID profile.
#[test]
fn every_gpu_of_the_capability_table_gives_its_golden_decisions() {
    let names = [
        "nes_ntsc",
        "snes_ntsc",
        "md_ntsc_h40",
        "md_pal_h40",
        "game_boy",
        "neogeo",
        "vga_640x480",
        "ntsc_480i",
    ];
    let systems: Vec<(String, SourceGeometry)> = systems()
        .into_iter()
        .filter(|(n, _)| names.contains(&n.as_str()))
        .collect();
    assert_eq!(systems.len(), names.len());
    let mut actual = BTreeMap::new();
    for (gpu, caps) in gpu_table() {
        for (profile, caps) in [
            (preset("generic_15"), caps),
            (pc_crt_30_71(), edid_caps(caps)),
        ] {
            for (name, source) in &systems {
                let key = format!("{gpu} {} {name}", profile.id);
                actual.insert(key, outcome(source, &profile, &caps));
            }
        }
    }
    compare("policy_gpus.toml", actual);
}
