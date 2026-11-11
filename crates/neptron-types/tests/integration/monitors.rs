//! The presets in data/monitors: each file parses, holds sane ranges, and a
//! few hold the values computed by hand from the Switchres source.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use neptron_types::{CrtRange, MonitorKind, MonitorProfile};

const PRESET_COUNT: usize = 25;

fn monitors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors")
}

fn load_all() -> Vec<(String, MonitorProfile)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(monitors_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "toml"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            let profile: MonitorProfile =
                toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
            (stem, profile)
        })
        .collect()
}

fn ranges(id: &str) -> Vec<CrtRange> {
    let (_, profile) = load_all()
        .into_iter()
        .find(|(_, p)| p.id == id)
        .unwrap_or_else(|| panic!("no preset {id}"));
    match profile.kind {
        MonitorKind::Crt { ranges } => ranges,
        MonitorKind::FlatPanel { .. } => panic!("{id} is not a CRT"),
    }
}

/// The 15 kHz range that generic_15 and arcade_15 share, apart from the
/// highest line rate.
fn standard_15(h_max_mhz: u64) -> CrtRange {
    CrtRange {
        h_min_mhz: 15_625_000,
        h_max_mhz,
        v_min_mhz: 49_500,
        v_max_mhz: 65_000,
        h_front_porch_ns: 2_000,
        h_sync_ns: 4_700,
        h_back_porch_ns: 8_000,
        v_front_porch_ns: 64_000,
        v_sync_ns: 192_000,
        v_back_porch_ns: 1_024_000,
        hsync_positive: false,
        vsync_positive: false,
        progressive_lines_min: 192,
        progressive_lines_max: 288,
        interlaced_lines_min: 448,
        interlaced_lines_max: 576,
    }
}

#[test]
fn every_preset_parses_and_is_sane() {
    let all = load_all();
    assert_eq!(all.len(), PRESET_COUNT);
    let mut ids = BTreeSet::new();
    for (stem, profile) in &all {
        let id = &profile.id;
        assert_eq!(stem, id, "the file name is the id");
        assert!(ids.insert(id.clone()), "duplicate id {id}");
        let MonitorKind::Crt { ranges } = &profile.kind else {
            panic!("{id} is not a CRT");
        };
        assert!(!ranges.is_empty(), "{id} has no range");
        for (i, r) in ranges.iter().enumerate() {
            let at = format!("{id} range {i}");
            assert!(0 < r.h_min_mhz && r.h_min_mhz <= r.h_max_mhz, "{at}: {r:?}");
            assert!(0 < r.v_min_mhz && r.v_min_mhz <= r.v_max_mhz, "{at}: {r:?}");
            for ns in [
                r.h_front_porch_ns,
                r.h_sync_ns,
                r.h_back_porch_ns,
                r.v_front_porch_ns,
                r.v_sync_ns,
                r.v_back_porch_ns,
            ] {
                assert!(ns > 0, "{at}: {r:?}");
            }
            assert!(
                0 < r.progressive_lines_min && r.progressive_lines_min <= r.progressive_lines_max,
                "{at}: {r:?}"
            );
            let no_interlace = r.interlaced_lines_min == 0 && r.interlaced_lines_max == 0;
            let interlace =
                0 < r.interlaced_lines_min && r.interlaced_lines_min <= r.interlaced_lines_max;
            assert!(no_interlace || interlace, "{at}: {r:?}");
        }
    }
}

#[test]
fn generic_15_matches_the_source() {
    // 15625-15750, 49.50-65.00, 2.000, 4.700, 8.000, 0.064, 0.192, 1.024,
    // 0, 0, 192, 288, 448, 576
    assert_eq!(ranges("generic_15"), [standard_15(15_750_000)]);
}

#[test]
fn arcade_15_matches_the_source() {
    // 15625-16200, 49.50-65.00, 2.000, 4.700, 8.000, 0.064, 0.192, 1.024,
    // 0, 0, 192, 288, 448, 576
    assert_eq!(ranges("arcade_15"), [standard_15(16_200_000)]);
}

#[test]
fn ntsc_matches_the_source() {
    // 15734.26-15734.26, 59.94-59.94, 1.500, 4.700, 4.700, 0.191, 0.191,
    // 0.953, 0, 0, 192, 240, 448, 480
    let expected = CrtRange {
        h_min_mhz: 15_734_260,
        h_max_mhz: 15_734_260,
        v_min_mhz: 59_940,
        v_max_mhz: 59_940,
        h_front_porch_ns: 1_500,
        h_sync_ns: 4_700,
        h_back_porch_ns: 4_700,
        v_front_porch_ns: 191_000,
        v_sync_ns: 191_000,
        v_back_porch_ns: 953_000,
        progressive_lines_max: 240,
        interlaced_lines_max: 480,
        ..standard_15(0)
    };
    assert_eq!(ranges("ntsc"), [expected]);
}

#[test]
fn pc_31_120_matches_the_source() {
    // 31400-31600, 100-130, 0.671, 2.683, 3.353, 0.034, 0.101, 0.436,
    // 0, 0, 200, 256, 0, 0
    let high = CrtRange {
        h_min_mhz: 31_400_000,
        h_max_mhz: 31_600_000,
        v_min_mhz: 100_000,
        v_max_mhz: 130_000,
        h_front_porch_ns: 671,
        h_sync_ns: 2_683,
        h_back_porch_ns: 3_353,
        v_front_porch_ns: 34_000,
        v_sync_ns: 101_000,
        v_back_porch_ns: 436_000,
        hsync_positive: false,
        vsync_positive: false,
        progressive_lines_min: 200,
        progressive_lines_max: 256,
        interlaced_lines_min: 0,
        interlaced_lines_max: 0,
    };
    // 31400-31600, 50-65, the same times, 0, 0, 400, 512, 0, 0
    let normal = CrtRange {
        v_min_mhz: 50_000,
        v_max_mhz: 65_000,
        progressive_lines_min: 400,
        progressive_lines_max: 512,
        ..high
    };
    assert_eq!(ranges("pc_31_120"), [high, normal]);
}

#[test]
fn m3129_has_positive_sync() {
    // 15250-16500, 40-80, ..., 1, 1, 192, 288, 448, 576 (and two more ranges)
    let r = ranges("m3129");
    assert_eq!(r.len(), 3);
    assert!(r.iter().all(|r| r.hsync_positive && r.vsync_positive));
    assert_eq!(
        (r[0].h_min_mhz, r[0].v_min_mhz, r[0].v_max_mhz),
        (15_250_000, 40_000, 80_000)
    );
}

#[test]
fn aliases_share_their_ranges() {
    assert_eq!(ranges("d9800"), ranges("d9400"));
    assert_eq!(ranges("h9110"), ranges("polo"));
}
