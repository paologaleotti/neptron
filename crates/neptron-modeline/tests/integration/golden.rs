//! Timing check 1: the golden list of systems in tests/golden/systems.toml.
//! The file header says where the expected values come from.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use neptron_modeline::{Settings, modelines_with};
use neptron_types::{GpuCaps, Rotation, SourceGeometry};
use serde::Deserialize;

use crate::presets::{all_presets, modeline_text, open_caps, preset};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Golden {
    pub system: Vec<System>,
    pub case: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct System {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub refresh_mhz: u32,
    pub interlaced: bool,
    pub rotation: Rotation,
    pub origin: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub system: String,
    pub preset: String,
    pub doublescan: Option<bool>,
    pub interlace: Option<bool>,
    pub pclock_min_hz: Option<u32>,
    pub expect: String,
}

pub fn golden() -> Golden {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/systems.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The first candidate as the standalone program prints it, or "none".
fn actual(case: &Case, system: &System) -> String {
    let source = SourceGeometry {
        width: system.width,
        height: system.height,
        refresh_mhz: system.refresh_mhz,
        aspect_milli: 1_333,
        interlaced: system.interlaced,
        rotation: system.rotation,
    };
    let caps = GpuCaps {
        pclock_min_hz: case.pclock_min_hz.unwrap_or(0),
        interlace: case.interlace.unwrap_or(true),
        ..open_caps()
    };
    let settings = Settings {
        doublescan: case.doublescan.unwrap_or(false),
        ..Settings::default()
    };
    let list = modelines_with(&source, &preset(&case.preset), &caps, &settings)
        .unwrap_or_else(|e| panic!("{} on {}: {e}", case.system, case.preset));
    list.first()
        .map_or_else(|| "none".to_owned(), |c| modeline_text(&c.modeline))
}

#[test]
fn every_system_gives_its_golden_modeline() {
    let golden = golden();
    let systems: BTreeMap<&str, &System> =
        golden.system.iter().map(|s| (s.name.as_str(), s)).collect();
    let mut diffs = Vec::new();
    for case in &golden.case {
        let system = systems
            .get(case.system.as_str())
            .unwrap_or_else(|| panic!("case names an unknown system {}", case.system));
        let got = actual(case, system);
        if got != case.expect {
            diffs.push(format!(
                "{} on {}:\n  expect {}\n  actual {}",
                case.system, case.preset, case.expect, got
            ));
        }
    }
    assert!(
        diffs.is_empty(),
        "{} of {} golden modelines differ:\n{}",
        diffs.len(),
        golden.case.len(),
        diffs.join("\n")
    );
}

#[test]
fn the_golden_list_covers_every_preset_and_system() {
    let golden = golden();
    assert!(golden.system.len() >= 30, "{} systems", golden.system.len());
    assert!(golden.case.len() >= 40, "{} cases", golden.case.len());

    let names: BTreeSet<&str> = golden.system.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names.len(), golden.system.len(), "duplicate system names");
    let used: BTreeSet<&str> = golden.case.iter().map(|c| c.system.as_str()).collect();
    assert_eq!(used, names, "every system has a case, every case a system");

    let presets: BTreeSet<String> = all_presets().into_iter().map(|p| p.id).collect();
    let covered: BTreeSet<String> = golden.case.iter().map(|c| c.preset.clone()).collect();
    assert_eq!(covered, presets);

    for system in &golden.system {
        assert!(!system.origin.is_empty(), "{} has no origin", system.name);
    }
}
