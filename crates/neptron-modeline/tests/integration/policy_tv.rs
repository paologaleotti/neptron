//! The policy on a TV profile: the rungs over the fixed mode table of each
//! standard, the width rule of `display.tv_width`, the speed bound, and
//! the validator's second opinion on every proposal over a sweep of
//! sources. The console rates are the ones of tests/golden/systems.toml
//! and docs/spec/raspberry-pi.md.

use neptron_modeline::policy::{
    Dropped, PictureStyle, Plan, PolicySettings, RefreshKind, RefreshPolicy, Rung, SkippedTarget,
    TV_SAFE_WIDTH_SAMPLES, TV_SPEED_SILENT_PPM, TvWidth, choose, decide, notice, style_rows,
};
use neptron_safety::validate;
use neptron_types::{
    Filter, GpuCaps, MonitorProfile, Rect, Rotation, SourceGeometry, TV_525_240P, TV_525_480I,
    TV_625_288P, TV_625_576I, TvStandard,
};

use crate::presets::{gpu_table, open_caps};

fn pi_composite() -> GpuCaps {
    gpu_table()
        .into_iter()
        .find(|(id, _)| id == "pi_vc4_composite")
        .map(|(_, caps)| caps)
        .unwrap()
}

fn game(width: u16, height: u16, refresh_mhz: u32, interlaced: bool) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz,
        aspect_milli: 1_333,
        interlaced,
        rotation: Rotation::R0,
    }
}

fn plan(source: &SourceGeometry, settings: &PolicySettings, standard: TvStandard) -> Plan {
    decide(source, settings, &standard.profile(), &pi_composite()).unwrap()
}

/// The winner after the validator, as `(rung, mode, dest, filter,
/// changes speed)`.
struct Won {
    rung: Rung,
    mode: neptron_types::Modeline,
    dest: Rect,
    filter: Filter,
    speed: bool,
    crop_per_edge: u16,
}

fn won(source: &SourceGeometry, settings: &PolicySettings, standard: TvStandard) -> Won {
    let profile = standard.profile();
    let caps = pi_composite();
    let plan = decide(source, settings, &profile, &caps).unwrap();
    let choice = choose(&plan, |m| validate(m, &profile, &caps));
    let w = choice.winner.expect("a TV always gives a mode");
    assert!(choice.refused.is_empty(), "{:?}", choice.refused);
    Won {
        rung: w.considered.rung,
        mode: w.proposal.mode,
        dest: w.proposal.placement.dest,
        filter: w.proposal.placement.filter,
        speed: w.proposal.timing.changes_speed(),
        crop_per_edge: w.proposal.overscan_crop.map_or(0, |c| c.lines_per_edge),
    }
}

fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

/// The off rate of the winner, in parts per million of the field rate,
/// rounded: positive when the game runs fast.
fn ppm(source: &SourceGeometry, mode: &neptron_types::Modeline) -> i64 {
    let field = i64::try_from(mode.field_rate_mhz()).unwrap();
    let off = field - i64::from(source.refresh_mhz);
    (off * 1_000_000 + off.signum() * field / 2) / field
}

#[test]
fn the_four_field_rates_are_the_exact_millihertz_of_the_table() {
    assert_eq!(TV_525_240P.field_rate_mhz(), 60_054);
    assert_eq!(TV_625_288P.field_rate_mhz(), 50_080);
    assert_eq!(TV_525_480I.field_rate_mhz(), 59_940);
    assert_eq!(TV_625_576I.field_rate_mhz(), 50_000);
}

/// The table "Choosing the mode for a game" of the spec: per console, the
/// mode, the lines and the speed alert on NTSC and on PAL. The ppm column
/// is the difference over the field rate.
#[test]
fn every_console_of_the_spec_gets_its_mode_and_its_speed_result() {
    // (name, source, NTSC mode, ppm, alert, PAL mode, ppm, alert)
    let rows = [
        (
            "NES",
            game(256, 240, 60_099, false),
            TV_525_240P,
            -749,
            false,
            TV_625_288P,
            -200_060,
            true,
        ),
        (
            "NES PAL",
            game(256, 240, 50_007, false),
            TV_525_240P,
            167_299,
            true,
            TV_625_288P,
            1_458,
            false,
        ),
        (
            "SNES",
            game(256, 224, 60_099, false),
            TV_525_240P,
            -749,
            false,
            TV_625_288P,
            -200_060,
            true,
        ),
        (
            "SNES PAL",
            game(256, 239, 50_007, false),
            TV_525_240P,
            167_299,
            true,
            TV_625_288P,
            1_458,
            false,
        ),
        (
            "Mega Drive",
            game(320, 224, 59_923, false),
            TV_525_240P,
            2_181,
            false,
            TV_625_288P,
            -196_546,
            true,
        ),
        (
            "Mega Drive PAL",
            game(320, 240, 49_701, false),
            TV_525_240P,
            172_395,
            true,
            TV_625_288P,
            7_568,
            true,
        ),
        (
            "PC Engine",
            game(256, 240, 59_826, false),
            TV_525_240P,
            3_797,
            false,
            TV_625_288P,
            -194_609,
            true,
        ),
        (
            "CPS1",
            game(384, 224, 59_637, false),
            TV_525_240P,
            6_944,
            true,
            TV_625_288P,
            -190_835,
            true,
        ),
        (
            "Neo Geo",
            game(320, 224, 59_186, false),
            TV_525_240P,
            14_454,
            true,
            TV_625_288P,
            -181_829,
            true,
        ),
        (
            "256-line board",
            game(384, 256, 55_018, false),
            TV_525_240P,
            83_858,
            true,
            TV_625_288P,
            -98_602,
            true,
        ),
        (
            "SNES hires",
            game(512, 448, 60_099, true),
            TV_525_480I,
            -2_653,
            false,
            TV_625_576I,
            -201_980,
            true,
        ),
        (
            "480i",
            game(640, 480, 59_940, true),
            TV_525_480I,
            0,
            false,
            TV_625_576I,
            -198_800,
            true,
        ),
    ];
    let settings = PolicySettings::default();
    let mut wrong = Vec::new();
    for (name, source, ntsc, ntsc_ppm, ntsc_alert, pal, pal_ppm, pal_alert) in rows {
        for (standard, mode, want_ppm, alert) in [
            (TvStandard::Ntsc, ntsc, ntsc_ppm, ntsc_alert),
            (TvStandard::Pal, pal, pal_ppm, pal_alert),
        ] {
            let w = won(&source, &settings, standard);
            let got = (w.mode, ppm(&source, &w.mode), w.speed);
            if got != (mode, want_ppm, alert) {
                wrong.push(format!(
                    "{name} on {}: {}x{} {} ppm alert {} (want {}x{} {want_ppm} ppm alert {alert})",
                    standard.key(),
                    got.0.hactive,
                    got.0.vactive,
                    got.1,
                    got.2,
                    mode.hactive,
                    mode.vactive
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_game_that_fits_the_progressive_mode_is_centred_with_one_line_per_scan_line() {
    let settings = PolicySettings::default();
    // SNES: 224 lines, 8 black lines above and below, as on the console.
    let w = won(&game(256, 224, 60_099, false), &settings, TvStandard::Ntsc);
    assert_eq!((w.rung, w.dest.y, w.dest.h), (Rung::Native, 8, 224));
    // NES: 240 of 240.
    let w = won(&game(256, 240, 60_099, false), &settings, TvStandard::Ntsc);
    assert_eq!((w.rung, w.dest.y, w.dest.h), (Rung::Native, 0, 240));
    // SNES PAL: 239 lines in 288, centred.
    let w = won(&game(256, 239, 50_007, false), &settings, TvStandard::Pal);
    assert_eq!((w.rung, w.dest.y, w.dest.h), (Rung::Native, 24, 239));
    // 288 lines fill 288p.
    let w = won(&game(320, 288, 50_000, false), &settings, TvStandard::Pal);
    assert_eq!((w.rung, w.mode, w.dest.h), (Rung::Native, TV_625_288P, 288));
    // A 240-line game is never doubled into 576i.
    let w = won(&game(256, 240, 60_099, false), &settings, TvStandard::Pal);
    assert_eq!((w.rung, w.mode, w.dest.y), (Rung::Native, TV_625_288P, 24));
}

#[test]
fn up_to_8_lines_per_edge_are_cropped_to_stay_progressive() {
    let settings = PolicySettings::default();
    // 256 lines miss 240 by 16: 8 per edge, the bound.
    let w = won(&game(384, 256, 55_018, false), &settings, TvStandard::Ntsc);
    assert_eq!(
        (w.rung, w.mode, w.crop_per_edge),
        (Rung::Native, TV_525_240P, 8)
    );
    assert_eq!((w.dest.y, w.dest.h), (0, 240));
    // 241 lines: 1 per edge, 239 lines shown, centred.
    let w = won(&game(256, 241, 60_000, false), &settings, TvStandard::Ntsc);
    assert_eq!((w.crop_per_edge, w.dest.y, w.dest.h), (1, 0, 239));
    // 304 lines miss 288 by 16: 8 per edge on PAL.
    let w = won(&game(320, 304, 50_000, false), &settings, TvStandard::Pal);
    assert_eq!((w.mode, w.crop_per_edge, w.dest.h), (TV_625_288P, 8, 288));
    // 257 lines would need 9 per edge: 480i, 1:1.
    let w = won(&game(384, 257, 60_000, false), &settings, TvStandard::Ntsc);
    assert_eq!(
        (w.rung, w.mode, w.crop_per_edge),
        (Rung::Interlaced, TV_525_480I, 0)
    );
    assert_eq!((w.dest.y, w.dest.h), (111, 257));
}

#[test]
fn a_tall_or_interlaced_game_takes_the_interlaced_mode_1_to_1() {
    let settings = PolicySettings::default();
    // A progressive 640x480 game: 480i, every line.
    let w = won(&game(640, 480, 59_940, false), &settings, TvStandard::Ntsc);
    assert_eq!(
        (w.rung, w.mode, w.dest.y, w.dest.h),
        (Rung::Interlaced, TV_525_480I, 0, 480)
    );
    // SNES interlaced, 448 lines: 16 black lines above and below.
    let w = won(&game(512, 448, 60_099, true), &settings, TvStandard::Ntsc);
    assert_eq!(
        (w.rung, w.mode, w.dest.y, w.dest.h),
        (Rung::Native, TV_525_480I, 16, 448)
    );
    // 576 progressive lines on PAL.
    let w = won(&game(720, 576, 50_000, false), &settings, TvStandard::Pal);
    assert_eq!(
        (w.rung, w.mode, w.dest.h),
        (Rung::Interlaced, TV_625_576I, 576)
    );
    // An interlaced 480-line source on PAL: 576i, centred.
    let w = won(&game(640, 480, 59_940, true), &settings, TvStandard::Pal);
    assert_eq!((w.mode, w.dest.y, w.dest.h), (TV_625_576I, 48, 480));
}

#[test]
fn a_game_taller_than_every_mode_is_fitted_into_the_progressive_mode() {
    let settings = PolicySettings::default();
    let source = game(640, 600, 60_000, false);
    let plan = plan(&source, &settings, TvStandard::Ntsc);
    let interlaced = plan
        .considered
        .iter()
        .find(|c| c.rung == Rung::Interlaced)
        .unwrap();
    assert_eq!(
        interlaced.outcome,
        Err(Dropped::TvLines {
            lines: 600,
            mode_lines: 480
        })
    );
    let w = won(&source, &settings, TvStandard::Ntsc);
    assert_eq!(
        (w.rung, w.mode, w.dest.y, w.dest.h),
        (Rung::Fitted, TV_525_240P, 0, 240)
    );
    assert_eq!(w.filter, Filter::SharpBilinear);
    let n = notice(&plan, &plan.considered[plan.considered.len() - 1]).unwrap();
    assert!(n.fitted);
}

#[test]
fn without_interlace_a_tall_game_is_fitted() {
    let caps = GpuCaps {
        interlace: false,
        ..pi_composite()
    };
    let profile = TvStandard::Ntsc.profile();
    let source = game(640, 480, 59_940, false);
    let plan = decide(&source, &PolicySettings::default(), &profile, &caps).unwrap();
    let interlaced = plan
        .considered
        .iter()
        .find(|c| c.rung == Rung::Interlaced)
        .unwrap();
    assert_eq!(interlaced.outcome, Err(Dropped::NoInterlace));
    let choice = choose(&plan, |m| validate(m, &profile, &caps));
    assert_eq!(choice.winner.unwrap().considered.rung, Rung::Fitted);
}

/// The width rule, in samples of the 720-sample line, for the widths of
/// the spec on each line system: `(width, aspect, integer, fill)`, each
/// `(samples, filter)`. A half factor (2.5x for 256 wide) takes sharp
/// bilinear, a whole factor nearest.
#[test]
fn the_width_rule_gives_the_table_of_the_readme() {
    use Filter::{Nearest, SharpBilinear};
    let ntsc = [
        (
            256,
            (640, SharpBilinear),
            (512, Nearest),
            (704, SharpBilinear),
        ),
        (320, (640, Nearest), (640, Nearest), (704, SharpBilinear)),
        (
            512,
            (644, SharpBilinear),
            (512, Nearest),
            (704, SharpBilinear),
        ),
        (640, (640, Nearest), (640, Nearest), (704, SharpBilinear)),
    ];
    let pal = [
        (
            256,
            (640, SharpBilinear),
            (512, Nearest),
            (704, SharpBilinear),
        ),
        (320, (640, Nearest), (640, Nearest), (704, SharpBilinear)),
        (
            512,
            (650, SharpBilinear),
            (512, Nearest),
            (704, SharpBilinear),
        ),
        (640, (640, Nearest), (640, Nearest), (704, SharpBilinear)),
    ];
    for (standard, rows) in [(TvStandard::Ntsc, ntsc), (TvStandard::Pal, pal)] {
        for (width, aspect, integer, fill) in rows {
            for (rule, (samples, filter)) in [
                (TvWidth::Aspect, aspect),
                (TvWidth::Integer, integer),
                (TvWidth::Fill, fill),
            ] {
                let settings = PolicySettings {
                    tv_width: rule,
                    ..PolicySettings::default()
                };
                let w = won(&game(width, 224, 60_000, false), &settings, standard);
                let x = (720 - samples) / 2;
                assert_eq!(
                    (w.dest.x, w.dest.w, w.filter),
                    (i32::try_from(x).unwrap(), samples, filter),
                    "{width} wide, {rule}, {}",
                    standard.key()
                );
            }
        }
    }
}

/// A source that is not a 4:3 console picture keeps its display aspect
/// with the sample shape of the line system: 2.2 samples per line of
/// aspect 1 on 525 lines, 1.8333 on 625. `(source, aspect, NTSC, PAL)`.
#[test]
fn a_handheld_or_a_turned_board_keeps_its_shape_on_both_line_systems() {
    let rows = [
        // Game Boy, 10:9.
        (game(160, 144, 59_728, false), 1_111, 352, 293),
        // Game Boy Advance, 3:2.
        (game(240, 160, 59_727, false), 1_500, 528, 440),
        // A 224x256 board turned upright, 3:4: 8 lines cropped per edge on
        // NTSC, and the crop keeps the width of the whole frame.
        (game(224, 256, 60_000, false), 750, 422, 352),
        // A 224x288 board turned upright, 3:4: 480i on NTSC, 144 lines
        // per field; 288p on PAL.
        (game(224, 288, 60_000, false), 750, 238, 396),
    ];
    for (source, aspect_milli, want_ntsc, want_pal) in rows {
        let source = SourceGeometry {
            aspect_milli,
            ..source
        };
        for (standard, want) in [(TvStandard::Ntsc, want_ntsc), (TvStandard::Pal, want_pal)] {
            let w = won(&source, &PolicySettings::default(), standard);
            assert_eq!(
                (w.dest.w, w.filter),
                (want, Filter::SharpBilinear),
                "{}x{} on {}",
                source.width,
                source.height,
                standard.key()
            );
        }
    }
}

#[test]
fn the_interlaced_width_follows_the_lines_of_one_field() {
    // 448 interlaced lines are 224 per field: the console width, which
    // no whole or half factor of 512 meets.
    let w = won(
        &game(512, 448, 60_099, true),
        &PolicySettings::default(),
        TvStandard::Ntsc,
    );
    assert_eq!((w.dest.w, w.filter), (644, Filter::SharpBilinear));
    let w = won(
        &game(512, 448, 50_007, true),
        &PolicySettings::default(),
        TvStandard::Pal,
    );
    assert_eq!((w.dest.w, w.filter), (650, Filter::SharpBilinear));
    let w = won(
        &game(640, 480, 59_940, true),
        &PolicySettings::default(),
        TvStandard::Ntsc,
    );
    assert_eq!((w.dest.w, w.filter), (640, Filter::Nearest));
}

#[test]
fn the_ui_fills_the_safe_width_and_every_active_line() {
    for standard in TvStandard::ALL {
        for source in [game(256, 224, 60_099, false), game(640, 480, 59_940, true)] {
            let plan = plan(&source, &PolicySettings::default(), standard);
            for (_, _, p) in plan.proposals() {
                assert_eq!(
                    p.placement.ui_dest,
                    rect(
                        8,
                        0,
                        u32::from(TV_SAFE_WIDTH_SAMPLES),
                        u32::from(p.mode.vactive)
                    )
                );
            }
        }
    }
}

#[test]
fn the_speed_bound_is_5000_ppm_of_the_field_rate() {
    assert_eq!(TV_SPEED_SILENT_PPM, 5_000);
    // 5000 ppm of 60.054 Hz is 300.27 mHz.
    let settings = PolicySettings::default();
    let silent = won(
        &game(256, 224, 60_054 - 300, false),
        &settings,
        TvStandard::Ntsc,
    );
    assert!(!silent.speed);
    let alert = won(
        &game(256, 224, 60_054 - 301, false),
        &settings,
        TvStandard::Ntsc,
    );
    assert!(alert.speed);
    let silent = won(
        &game(256, 224, 60_054 + 300, false),
        &settings,
        TvStandard::Ntsc,
    );
    assert!(!silent.speed);
    let alert = won(
        &game(256, 224, 60_054 + 301, false),
        &settings,
        TvStandard::Ntsc,
    );
    assert!(alert.speed);
}

#[test]
fn the_notice_shows_the_rates_only_beyond_the_bound() {
    let settings = PolicySettings::default();
    let nes = plan(&game(256, 240, 60_099, false), &settings, TvStandard::Ntsc);
    let first = nes.proposals().next().unwrap().1;
    assert_eq!(notice(&nes, first), None);
    // A 50 Hz game on a 60 Hz standard: a valid mode and the speed alert.
    let pal = plan(&game(256, 239, 50_007, false), &settings, TvStandard::Ntsc);
    let first = pal.proposals().next().unwrap().1;
    let n = notice(&pal, first).unwrap();
    assert_eq!((n.game_mhz, n.shown_mhz), (50_007, 60_054));
    // And the reverse.
    let ntsc = plan(&game(256, 224, 60_099, false), &settings, TvStandard::Pal);
    let first = ntsc.proposals().next().unwrap().1;
    let n = notice(&ntsc, first).unwrap();
    assert_eq!((n.game_mhz, n.shown_mhz), (60_099, 50_080));
}

#[test]
fn a_game_near_half_the_field_rate_shows_each_frame_twice() {
    let p = plan(
        &game(320, 240, 30_000, false),
        &PolicySettings::default(),
        TvStandard::Ntsc,
    );
    let (_, c, proposal) = p.proposals().next().unwrap();
    assert_eq!((c.target.kind, c.target.repeat), (RefreshKind::Tv, 2));
    assert!(!proposal.timing.changes_speed());
}

#[test]
fn a_doubled_or_forced_rate_is_skipped_because_the_standard_fixes_the_rate() {
    let source = game(256, 224, 60_099, false);
    for (refresh, kind, rate_mhz) in [
        (RefreshPolicy::ExactX2, RefreshKind::Double, 120_198),
        (RefreshPolicy::Force50, RefreshKind::Forced50, 50_000),
        (RefreshPolicy::Force60, RefreshKind::Forced60, 60_000),
    ] {
        let settings = PolicySettings {
            refresh,
            ..PolicySettings::default()
        };
        let plan = plan(&source, &settings, TvStandard::Ntsc);
        assert_eq!(plan.skipped, [SkippedTarget { kind, rate_mhz }]);
        let w = won(&source, &settings, TvStandard::Ntsc);
        assert_eq!((w.rung, w.mode), (Rung::Native, TV_525_240P));
    }
    let plan = plan(&source, &PolicySettings::default(), TvStandard::Ntsc);
    assert!(plan.skipped.is_empty());
}

#[test]
fn a_multiple_style_has_no_mode_and_its_row_is_greyed_out() {
    let source = game(256, 224, 60_099, false);
    let settings = PolicySettings {
        style: PictureStyle::X4,
        ..PolicySettings::default()
    };
    let plan = plan(&source, &settings, TvStandard::Ntsc);
    let drops: Vec<_> = plan.considered.iter().take(3).map(|c| &c.outcome).collect();
    assert_eq!(
        drops,
        [
            &Err(Dropped::TvNoMultiple { factor: 4 }),
            &Err(Dropped::TvNoMultiple { factor: 3 }),
            &Err(Dropped::TvNoMultiple { factor: 2 }),
        ]
    );
    let w = won(&source, &settings, TvStandard::Ntsc);
    assert_eq!(w.rung, Rung::Native);
    let profile = TvStandard::Ntsc.profile();
    let caps = pi_composite();
    let rows = style_rows(&source, &settings, &profile, &caps, |m| {
        validate(m, &profile, &caps).map(|_| ())
    })
    .unwrap();
    let available: Vec<_> = rows.iter().map(|r| (r.style, r.available)).collect();
    assert_eq!(
        available,
        [
            (PictureStyle::Auto, true),
            (PictureStyle::Native, true),
            (PictureStyle::X2, false),
            (PictureStyle::X3, false),
            (PictureStyle::X4, false),
        ]
    );
}

fn every_settings() -> Vec<PolicySettings> {
    let refreshes = [
        RefreshPolicy::Exact,
        RefreshPolicy::ExactX2,
        RefreshPolicy::Force50,
        RefreshPolicy::Force60,
    ];
    let mut all = Vec::new();
    for style in PictureStyle::ALL {
        for refresh in refreshes {
            for tv_width in TvWidth::ALL {
                all.push(PolicySettings {
                    style,
                    refresh,
                    tv_width,
                    ..PolicySettings::default()
                });
            }
        }
    }
    all
}

fn sweep_sources() -> Vec<SourceGeometry> {
    let widths = [1, 160, 256, 320, 384, 512, 640, 720, 1_024, u16::MAX];
    let heights = [
        1,
        144,
        192,
        224,
        239,
        240,
        241,
        256,
        257,
        288,
        289,
        304,
        305,
        448,
        480,
        481,
        576,
        577,
        1_000,
        u16::MAX,
    ];
    let rates = [1, 25_000, 49_701, 50_000, 59_940, 60_099, 120_000, u32::MAX];
    let aspects = [0, 1_111, 1_333, 1_778, u32::MAX];
    let mut all = Vec::new();
    let mut i = 0usize;
    for &width in &widths {
        for &height in &heights {
            for &refresh_mhz in &rates {
                for interlaced in [false, true] {
                    i += 1;
                    all.push(SourceGeometry {
                        width,
                        height,
                        refresh_mhz,
                        aspect_milli: aspects[i % aspects.len()],
                        interlaced,
                        rotation: if i.is_multiple_of(7) {
                            Rotation::R90
                        } else {
                            Rotation::R0
                        },
                    });
                }
            }
        }
    }
    all
}

/// Every proposal the policy can give, for every standard, a sweep of
/// sources and every setting, is a table mode of the standard, never of
/// the other line system, and passes the validator. The placement lies
/// inside the mode, and a game that fits the progressive mode wins it
/// with one line per scan line.
#[test]
fn every_tv_proposal_is_a_table_mode_that_the_validator_accepts() {
    let settings = every_settings();
    let mut decisions = 0usize;
    for caps in [pi_composite(), open_caps()] {
        for standard in TvStandard::ALL {
            let profile: MonitorProfile = standard.profile();
            for (i, source) in sweep_sources().iter().enumerate() {
                let s = &settings[i % settings.len()];
                let plan = decide(source, s, &profile, &caps).unwrap();
                decisions += 1;
                let mut proposals = 0;
                for (_, c, p) in plan.proposals() {
                    proposals += 1;
                    assert!(
                        standard.modes().contains(&p.mode),
                        "{source:?} on {}: {c}",
                        standard.key()
                    );
                    assert!(
                        validate(&p.mode, &profile, &caps).is_ok(),
                        "{source:?} on {}: {c}",
                        standard.key()
                    );
                    assert_eq!(c.target.kind, RefreshKind::Tv);
                    let d = p.placement.dest;
                    assert!(d.x >= 0 && d.y >= 0, "{c}");
                    assert!(
                        d.x as u32 + d.w <= u32::from(p.mode.hactive)
                            && d.y as u32 + d.h <= u32::from(p.mode.vactive),
                        "{source:?}: {c}"
                    );
                    assert!(d.w >= 1 && d.w <= u32::from(TV_SAFE_WIDTH_SAMPLES), "{c}");
                    if c.rung != Rung::Fitted {
                        assert_eq!(p.placement.scale.y, Some(1), "{c}");
                    }
                    if p.timing.changes_speed() {
                        assert!(notice(&plan, c).is_some(), "{c}");
                    }
                }
                assert!(proposals >= 1, "{source:?}: no proposal");
                let last = plan.considered.last().unwrap();
                assert_eq!(last.rung, Rung::Fitted);
                let first = plan.proposals().next().unwrap().1;
                let lines = plan.source.height;
                let progressive = standard.modes()[0];
                if !plan.source.interlaced && lines <= progressive.vactive + 16 {
                    assert_eq!(first.rung, Rung::Native, "{source:?}");
                    assert_eq!(
                        first.outcome.as_ref().unwrap().mode,
                        progressive,
                        "{source:?}"
                    );
                }
                assert_eq!(
                    decide(source, s, &profile, &caps).unwrap(),
                    plan,
                    "two decisions differ"
                );
            }
        }
    }
    assert_eq!(decisions, 2 * 10 * sweep_sources().len());
}

/// Every setting on a few sources, so each combination of style, refresh
/// and width meets each rung.
#[test]
fn every_setting_gives_a_mode_on_every_standard() {
    let sources = [
        game(256, 224, 60_099, false),
        game(384, 256, 55_018, false),
        game(640, 480, 59_940, false),
        game(512, 448, 60_099, true),
        game(640, 600, 50_000, false),
    ];
    let caps = pi_composite();
    for standard in TvStandard::ALL {
        let profile = standard.profile();
        for settings in every_settings() {
            for source in &sources {
                let plan = decide(source, &settings, &profile, &caps).unwrap();
                let choice = choose(&plan, |m| validate(m, &profile, &caps));
                assert!(choice.winner.is_some(), "{source:?} {settings:?}");
                assert!(choice.refused.is_empty(), "{source:?} {settings:?}");
            }
        }
    }
}
