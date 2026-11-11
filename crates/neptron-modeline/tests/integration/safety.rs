//! The solver and the validator agree: every golden modeline validates
//! against its preset. The validator shares no code with Switchres, so this
//! is the second opinion on each golden and on the modes of a profile built
//! from an EDID.

use neptron_modeline::{Settings, modelines, modelines_with};
use neptron_safety::{Rejection, validate};
use neptron_types::{GpuCaps, Modeline, SourceGeometry};

use crate::golden::golden;
use crate::presets::{modeline_text, open_caps, pc_crt_30_71, preset, source};

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

/// The best candidate for a source on the EDID profile, and what the
/// validator says about it.
fn on_pc_crt(width: u16, height: u16, refresh_mhz: u32) -> (Modeline, Result<(), Rejection>) {
    let profile = pc_crt_30_71();
    let list = modelines(&source(width, height, refresh_mhz), &profile, &open_caps())
        .unwrap_or_else(|e| panic!("{width}x{height}: {e}"));
    let best = list
        .first()
        .unwrap_or_else(|| panic!("{width}x{height}: no modeline"))
        .modeline;
    // limit_caps of neptron-platform lowers the highest pixel clock to the
    // EDID limit.
    let caps = GpuCaps {
        pclock_max_hz: 100_000_000,
        ..open_caps()
    };
    (best, validate(&best, &profile, &caps).map(|_| ()))
}

/// The bench path: a PC CRT with a profile from its EDID. The expected
/// modelines are also the output of the upstream standalone program with
/// the same range (`--monitor custom --crt_range0
/// 30000-71000,50-160,0.817,2.667,3.334,0.033334,0.1,0.507747,0,1,169,1374,0,0
/// --doublescan 0`).
#[test]
fn the_bench_modes_on_an_edid_profile_validate() {
    for (width, height, refresh_mhz, expect) in [
        // 480p at 60 Hz.
        (
            640,
            480,
            60_000,
            "24.120000 640 660 724 804 480 482 485 500 -hsync +vsync",
        ),
        // 240p at twice 60 Hz: the double-refresh picture style.
        (
            320,
            240,
            120_000,
            "12.667200 320 330 364 406 240 241 244 260 -hsync +vsync",
        ),
        // The NES at its own rate. Switchres pads the vertical front porch
        // to reach the 30 kHz floor of the range.
        (
            256,
            224,
            60_099,
            "9.675938 256 264 290 322 224 354 357 500 -hsync +vsync",
        ),
    ] {
        let (mode, verdict) = on_pc_crt(width, height, refresh_mhz);
        assert_eq!(modeline_text(&mode), expect, "{width}x{height}");
        assert_eq!(verdict, Ok(()), "{width}x{height}: {mode:?}");
    }
}
