//! The solver on real presets, its errors, and a grid of sources.

use std::collections::BTreeSet;

use neptron_modeline::{Aspect, BlankingFault, ModelineError, Settings, modelines, modelines_with};
use neptron_types::{CrtRange, GpuCaps, MonitorKind, MonitorProfile, SourceGeometry};

use crate::presets::{all_presets, modeline_text, open_caps, preset, source};

fn ranges(profile: &MonitorProfile) -> Vec<CrtRange> {
    match &profile.kind {
        MonitorKind::Crt { ranges } => ranges.clone(),
        MonitorKind::FlatPanel { .. } => panic!("{} is not a CRT", profile.id),
    }
}

fn with_ranges(id: &str, ranges: Vec<CrtRange>) -> MonitorProfile {
    MonitorProfile {
        id: id.into(),
        kind: MonitorKind::Crt { ranges },
    }
}

// The expected modelines in this file come from the upstream standalone
// program, `switchres <w> <h> <refresh> --calc -m <preset> --doublescan 0`,
// built from the vendored commit.

#[test]
fn generic_15_gives_native_240p() {
    let list = modelines(
        &source(320, 240, 60_000),
        &preset("generic_15"),
        &open_caps(),
    )
    .unwrap();
    assert_eq!(list.len(), 1);
    let first = &list[0];
    assert_eq!(
        modeline_text(&first.modeline),
        "6.514560 320 333 364 416 240 242 245 261 -hsync -vsync"
    );
    assert_eq!(first.range_index, 0);
    assert!(!first.stretched && !first.refresh_off);
    assert_eq!((first.x_scale_milli, first.y_scale_milli), (1_000, 1_000));
    assert_eq!(first.refresh_diff_mhz, 0);
    // 6 514 560 Hz / (416 * 261) = 59.9999 Hz.
    assert_eq!(first.modeline.field_rate_mhz(), 60_000);
}

#[test]
fn a_multi_range_profile_gives_one_candidate_per_range_best_first() {
    let profile = preset("arcade_15_25_31");
    let list = modelines(&source(320, 224, 59_923), &profile, &open_caps()).unwrap();
    assert_eq!(
        modeline_text(&list[0].modeline),
        "6.506199 320 333 364 416 224 234 237 261 -hsync -vsync"
    );
    assert_eq!(list[0].range_index, 0);
    let indexes: BTreeSet<usize> = list.iter().map(|c| c.range_index).collect();
    assert_eq!(indexes.len(), list.len(), "{list:?}");
    assert!(list.len() <= ranges(&profile).len());
}

#[test]
fn doublescan_is_off_by_default() {
    let profile = preset("arcade_31");
    let low = source(320, 240, 60_000);
    let list = modelines(&low, &profile, &open_caps()).unwrap();
    assert_eq!(
        modeline_text(&list[0].modeline),
        "25.452000 640 664 760 808 480 491 493 525 -hsync -vsync"
    );
    let settings = Settings {
        doublescan: true,
        ..Settings::default()
    };
    let list = modelines_with(&low, &profile, &open_caps(), &settings).unwrap();
    assert_eq!(
        modeline_text(&list[0].modeline),
        "12.726000 320 332 380 404 240 245 246 262 doublescan -hsync -vsync"
    );
}

#[test]
fn a_gpu_without_interlace_gets_a_progressive_mode() {
    let interlaced = SourceGeometry {
        interlaced: true,
        ..source(640, 480, 59_940)
    };
    let profile = preset("generic_15");
    let list = modelines(&interlaced, &profile, &open_caps()).unwrap();
    assert!(list[0].modeline.interlace);
    let caps = GpuCaps {
        interlace: false,
        ..open_caps()
    };
    let list = modelines(&interlaced, &profile, &caps).unwrap();
    assert!(list.iter().all(|c| !c.modeline.interlace), "{list:?}");
    assert!(list[0].stretched);
}

#[test]
fn a_pixel_clock_floor_multiplies_the_width() {
    let caps = GpuCaps {
        pclock_min_hz: 25_000_000,
        ..open_caps()
    };
    let list = modelines(&source(256, 240, 60_099), &preset("generic_15"), &caps).unwrap();
    let mode = list[0].modeline;
    assert_eq!(
        modeline_text(&mode),
        "41.533216 2048 2131 2326 2658 240 241 244 260 -hsync -vsync"
    );
    assert!(mode.pclock_hz > caps.pclock_min_hz);
    assert_eq!(mode.hactive % 256, 0);
}

#[test]
fn a_wide_monitor_aspect_widens_the_mode() {
    let settings = Settings {
        monitor_aspect: Aspect {
            width: 16,
            height: 9,
        },
        ..Settings::default()
    };
    let narrow = modelines(
        &source(320, 240, 60_000),
        &preset("generic_15"),
        &open_caps(),
    )
    .unwrap();
    let wide = modelines_with(
        &source(320, 240, 60_000),
        &preset("generic_15"),
        &open_caps(),
        &settings,
    )
    .unwrap();
    assert!(wide[0].modeline.hactive > narrow[0].modeline.hactive);
}

#[test]
fn the_same_call_gives_the_same_list() {
    let profile = preset("arcade_15_25_31");
    let a = modelines(&source(384, 256, 55_018), &profile, &open_caps()).unwrap();
    let b = modelines(&source(384, 256, 55_018), &profile, &open_caps()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_flat_panel_is_an_error() {
    let panel = MonitorProfile {
        id: "panel".into(),
        kind: MonitorKind::FlatPanel { modes: vec![] },
    };
    assert_eq!(
        modelines(&source(320, 240, 60_000), &panel, &open_caps()),
        Err(ModelineError::FlatPanel {
            profile: "panel".into()
        })
    );
}

#[test]
fn a_profile_without_ranges_is_an_error() {
    assert_eq!(
        modelines(
            &source(320, 240, 60_000),
            &with_ranges("none", vec![]),
            &open_caps()
        ),
        Err(ModelineError::NoRanges {
            profile: "none".into()
        })
    );
}

#[test]
fn a_profile_with_more_than_ten_ranges_is_an_error() {
    let range = ranges(&preset("generic_15"))[0];
    let profile = with_ranges("many", vec![range; 11]);
    assert_eq!(
        modelines(&source(320, 240, 60_000), &profile, &open_caps()),
        Err(ModelineError::TooManyRanges {
            profile: "many".into(),
            count: 11,
            max: 10
        })
    );
}

#[test]
fn switchres_refuses_a_bad_range() {
    let good = ranges(&preset("generic_15"))[0];
    let bad = CrtRange {
        h_min_mhz: 0,
        ..good
    };
    let profile = with_ranges("bad", vec![good, bad]);
    assert_eq!(
        modelines(&source(320, 240, 60_000), &profile, &open_caps()),
        Err(ModelineError::InvalidRange {
            profile: "bad".into(),
            index: 1
        })
    );
    let no_porch = CrtRange {
        v_sync_ns: 0,
        ..good
    };
    assert!(matches!(
        modelines(
            &source(320, 240, 60_000),
            &with_ranges("bad", vec![no_porch]),
            &open_caps()
        ),
        Err(ModelineError::InvalidRange { index: 0, .. })
    ));
}

#[test]
fn an_empty_source_is_an_error() {
    let profile = preset("generic_15");
    for empty in [
        source(0, 240, 60_000),
        source(320, 0, 60_000),
        source(320, 240, 0),
    ] {
        assert_eq!(
            modelines(&empty, &profile, &open_caps()),
            Err(ModelineError::EmptySource {
                width: empty.width,
                height: empty.height,
                refresh_mhz: empty.refresh_mhz
            })
        );
    }
}

#[test]
fn an_aspect_out_of_range_is_an_error() {
    let with_aspect = |width, height| Settings {
        monitor_aspect: Aspect { width, height },
        ..Settings::default()
    };
    let run = |settings: &Settings| {
        modelines_with(
            &source(320, 240, 60_000),
            &preset("generic_15"),
            &open_caps(),
            settings,
        )
    };
    for (width, height) in [(4, 0), (0, 3), (1, 5), (17, 4), (u16::MAX, 1)] {
        assert_eq!(
            run(&with_aspect(width, height)),
            Err(ModelineError::AspectOutOfRange { width, height })
        );
    }
    for (width, height) in [(1, 4), (4, 1), (16, 9), (3, 4)] {
        assert!(run(&with_aspect(width, height)).is_ok(), "{width}:{height}");
    }
}

#[test]
fn errors_name_the_profile_and_the_value() {
    let error = ModelineError::InvalidRange {
        profile: "bad".into(),
        index: 1,
    };
    assert_eq!(
        error.to_string(),
        "Switchres refuses range 1 of profile bad"
    );
    let error = ModelineError::EmptySource {
        width: 0,
        height: 240,
        refresh_mhz: 60_000,
    };
    assert_eq!(
        error.to_string(),
        "source 0x240 at 60000 mHz: each value must be above 0"
    );
}

/// Every preset and a grid of sizes and rates, from absurd to large: each
/// call returns a list, and each candidate has its timings in order and a
/// nonzero total on each axis.
#[test]
fn a_grid_of_sources_gives_ordered_timings_on_every_preset() {
    const WIDTHS: [u16; 7] = [1, 160, 256, 320, 640, 1_024, 4_000];
    const HEIGHTS: [u16; 9] = [1, 144, 224, 240, 288, 384, 480, 576, 1_200];
    const RATES_MHZ: [u32; 6] = [1, 24_000, 50_000, 59_940, 120_000, 1_000_000];
    let caps = open_caps();
    let mut calls = 0;
    for profile in all_presets() {
        let range_count = ranges(&profile).len();
        for width in WIDTHS {
            for height in HEIGHTS {
                for refresh_mhz in RATES_MHZ {
                    for interlaced in [false, true] {
                        let src = SourceGeometry {
                            interlaced,
                            ..source(width, height, refresh_mhz)
                        };
                        let list = modelines(&src, &profile, &caps)
                            .unwrap_or_else(|e| panic!("{} {src:?}: {e}", profile.id));
                        assert!(list.len() <= range_count);
                        for c in &list {
                            let m = c.modeline;
                            let ordered = m.hactive <= m.hsync_start
                                && m.hsync_start <= m.hsync_end
                                && m.hsync_end <= m.htotal
                                && m.vactive <= m.vsync_start
                                && m.vsync_start <= m.vsync_end
                                && m.vsync_end <= m.vtotal
                                && m.htotal > 0
                                && m.vtotal > 0;
                            assert!(ordered, "{} {src:?}: {m:?}", profile.id);
                            assert!(c.range_index < range_count);
                        }
                        calls += 1;
                    }
                }
            }
        }
    }
    assert_eq!(calls, 25 * 7 * 9 * 6 * 2);
}

#[test]
fn a_convert_error_is_the_source() {
    use std::error::Error;
    let error = ModelineError::Convert {
        profile: "p".into(),
        error: neptron_modeline::ConvertError::Timing {
            field: "htotal",
            value: 70_000,
            max: 65_535,
        },
    };
    assert_eq!(
        error.to_string(),
        "profile p: Switchres gave htotal 70000, outside 0 to 65535"
    );
    assert!(error.source().is_some());
    assert!(
        ModelineError::NoRanges {
            profile: "p".into()
        }
        .source()
        .is_none()
    );
}

#[test]
fn a_range_whose_blanking_fills_the_line_returns_an_error_at_once() {
    // generic_15 porches at 70 kHz: 14.7 us of blanking in a 14.29 us line.
    // Switchres accepts the range and then never returns.
    let fast = CrtRange {
        h_min_mhz: 70_000_000,
        h_max_mhz: 70_000_000,
        v_min_mhz: 50_000,
        v_max_mhz: 60_000,
        ..ranges(&preset("generic_15"))[0]
    };
    let profile = with_ranges("fast", vec![fast]);
    assert_eq!(
        modelines(&source(320, 240, 60_000), &profile, &open_caps()),
        Err(ModelineError::Blanking {
            profile: "fast".into(),
            index: 0,
            fault: BlankingFault::HorizontalTooLong,
            blanking_ns: 14_700,
            // 0.75 of 1 / 70 kHz = 10.714 us.
            limit_ns: 10_714,
        })
    );
    // The lock is free again: the next call works.
    assert!(
        modelines(
            &source(320, 240, 60_000),
            &preset("generic_15"),
            &open_caps()
        )
        .is_ok()
    );
}

#[test]
fn every_shipped_preset_passes_the_blanking_guard() {
    for profile in all_presets() {
        let result = modelines(&source(320, 240, 60_000), &profile, &open_caps());
        assert!(
            !matches!(result, Err(ModelineError::Blanking { .. })),
            "{}: {result:?}",
            profile.id
        );
    }
}

#[test]
fn a_blanking_error_names_the_profile_and_the_values() {
    let error = ModelineError::Blanking {
        profile: "fast".into(),
        index: 0,
        fault: BlankingFault::HorizontalTooLong,
        blanking_ns: 14_700,
        limit_ns: 10_714,
    };
    assert_eq!(
        error.to_string(),
        "range 0 of profile fast: horizontal blanking 14700 ns, must be below 10714 ns"
    );
}
