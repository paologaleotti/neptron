//! The solver and the validator agree: every golden modeline validates
//! against its preset. The validator shares no code with Switchres, so this
//! is the second opinion on each golden.

use neptron_modeline::{Settings, modelines_with};
use neptron_safety::validate;
use neptron_types::{GpuCaps, SourceGeometry};

use crate::golden::golden;
use crate::presets::{open_caps, preset};

#[test]
fn every_golden_modeline_validates_against_its_preset() {
    let golden = golden();
    let mut refused = Vec::new();
    for case in &golden.case {
        let system = golden
            .system
            .iter()
            .find(|s| s.name == case.system)
            .unwrap_or_else(|| panic!("case names an unknown system {}", case.system));
        let source = SourceGeometry {
            width: system.width,
            height: system.height,
            refresh_mhz: system.refresh_mhz,
            aspect_milli: 1_333,
            interlaced: system.interlaced,
            rotation: system.rotation,
        };
        let solver_caps = GpuCaps {
            pclock_min_hz: case.pclock_min_hz.unwrap_or(0),
            interlace: case.interlace.unwrap_or(true),
            ..open_caps()
        };
        let settings = Settings {
            doublescan: case.doublescan.unwrap_or(false),
            ..Settings::default()
        };
        let profile = preset(&case.preset);
        let list = modelines_with(&source, &profile, &solver_caps, &settings)
            .unwrap_or_else(|e| panic!("{} on {}: {e}", case.system, case.preset));
        let Some(first) = list.first() else {
            panic!("{} on {}: no modeline", case.system, case.preset);
        };
        // A GPU that limits nothing, so that only the monitor judges.
        if let Err(rejection) = validate(&first.modeline, &profile, &open_caps()) {
            refused.push(format!(
                "{} on {}: {rejection} ({:?})",
                case.system, case.preset, first.modeline
            ));
        }
    }
    assert!(
        refused.is_empty(),
        "{} of {} golden modelines do not validate:\n{}",
        refused.len(),
        golden.case.len(),
        refused.join("\n")
    );
}
