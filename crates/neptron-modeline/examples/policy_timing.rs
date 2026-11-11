//! Prints the time of one ODREN policy decision, for the task report and
//! for the engine's choice of where to run the policy. It is a program,
//! not a test: a wall clock gives no stable pass or fail.
//!
//! The policy runs when a core changes geometry, on the frame thread, and
//! each Switchres call takes a lock. Build it in the profile to measure,
//! for example `cargo run --release -p neptron-modeline --example
//! policy_timing`.

#![forbid(unsafe_code)]

use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use neptron_modeline::policy::{PolicySettings, choose, decide, style_rows};
use neptron_safety::validate;
use neptron_types::{CrtRange, GpuCaps, MonitorKind, MonitorProfile, Rotation, SourceGeometry};

const RUNS: u32 = 200;

fn preset(id: &str) -> MonitorProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../data/monitors/{id}.toml"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The profile that neptron-platform builds from the EDID of a 30 to
/// 71 kHz, 50 to 160 Hz PC CRT.
fn pc_crt_30_71() -> MonitorProfile {
    MonitorProfile {
        id: "edid_npt_0071".into(),
        kind: MonitorKind::Crt {
            ranges: vec![CrtRange {
                h_min_mhz: 30_000_000,
                h_max_mhz: 71_000_000,
                v_min_mhz: 50_000,
                v_max_mhz: 160_000,
                h_front_porch_ns: 817,
                h_sync_ns: 2_667,
                h_back_porch_ns: 3_334,
                v_front_porch_ns: 33_334,
                v_sync_ns: 100_000,
                v_back_porch_ns: 507_747,
                hsync_positive: false,
                vsync_positive: true,
                progressive_lines_min: 169,
                progressive_lines_max: 1_374,
                interlaced_lines_min: 0,
                interlaced_lines_max: 0,
            }],
        },
    }
}

/// The intel_gen9 entry of data/hardware/matrix.toml.
const INTEL_GEN9: GpuCaps = GpuCaps {
    pclock_min_hz: 25_000_000,
    pclock_max_hz: 165_000_000,
    interlace: false,
    flip_per_field: false,
    max_width: 1_920,
    max_height: 1_200,
};

fn time_decision(name: &str, source: &SourceGeometry, profile: &MonitorProfile, caps: &GpuCaps) {
    let settings = PolicySettings::default();
    let start = Instant::now();
    for _ in 0..RUNS {
        let plan = decide(black_box(source), &settings, profile, caps).expect("a plan");
        black_box(
            choose(&plan, |m| validate(m, profile, caps))
                .winner
                .is_some(),
        );
    }
    println!("{name}: {:?} per decision", start.elapsed() / RUNS);
}

fn main() {
    let snes = SourceGeometry {
        width: 256,
        height: 224,
        refresh_mhz: 60_099,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    };
    let edid = pc_crt_30_71();
    let edid_caps = GpuCaps {
        pclock_max_hz: 100_000_000,
        ..INTEL_GEN9
    };
    time_decision(
        "SNES on generic_15, intel_gen9",
        &snes,
        &preset("generic_15"),
        &INTEL_GEN9,
    );
    time_decision(
        "SNES on the EDID profile, intel_gen9",
        &snes,
        &edid,
        &edid_caps,
    );
    time_decision(
        "SNES on arcade_15_25_31, intel_gen9",
        &snes,
        &preset("arcade_15_25_31"),
        &INTEL_GEN9,
    );

    let start = Instant::now();
    for _ in 0..RUNS {
        let rows = style_rows(&snes, &PolicySettings::default(), &edid, &edid_caps, |m| {
            validate(m, &edid, &edid_caps).map(|_| ())
        });
        black_box(rows.expect("rows").len());
    }
    println!("the five Display panel rows: {:?}", start.elapsed() / RUNS);
}
