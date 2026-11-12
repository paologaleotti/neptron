//! Timing check 3: the policy follows the decision procedure. One row per
//! rule: given a source, a picture style, a refresh policy, a profile and
//! a GPU, the rung and the mode family that win after the validator.

use neptron_modeline::policy::{
    Considered, Dropped, HorizontalScale, HorizontalStrategy, PictureStyle, Plan, PolicySettings,
    Proposal, REFRESH_MATCH_MHZ, RefreshKind, RefreshPolicy, Rung, SkippedTarget, Switch, choose,
    decide, notice, style_rows, switch_for,
};
use neptron_safety::validate;
use neptron_types::{
    Filter, GpuCaps, MonitorProfile, Rect, Rotation, SourceGeometry, ui_dest_fill,
};

use crate::presets::{edid_caps, intel_gen9, open_caps, panel_1080, pc_crt_30_71, preset, source};

const SNES: (u16, u16, u32) = (256, 224, 60_099);

fn snes() -> SourceGeometry {
    source(SNES.0, SNES.1, SNES.2)
}

fn with_interlace(caps: GpuCaps) -> GpuCaps {
    GpuCaps {
        interlace: true,
        flip_per_field: true,
        ..caps
    }
}

struct Row {
    rule: &'static str,
    source: SourceGeometry,
    style: PictureStyle,
    refresh: RefreshPolicy,
    horizontal: Option<HorizontalStrategy>,
    profile: MonitorProfile,
    caps: GpuCaps,
    rung: Rung,
    /// Active size and scan of the winner: "1280x224p".
    mode: &'static str,
    kind: RefreshKind,
    /// The field rate the winner aims for, in millihertz.
    rate_mhz: u32,
}

fn row(rule: &'static str, profile: MonitorProfile, caps: GpuCaps) -> Row {
    Row {
        rule,
        source: snes(),
        style: PictureStyle::Auto,
        refresh: RefreshPolicy::Exact,
        horizontal: None,
        profile,
        caps,
        rung: Rung::Native,
        mode: "",
        kind: RefreshKind::Exact,
        rate_mhz: SNES.2,
    }
}

fn settings(style: PictureStyle, refresh: RefreshPolicy) -> PolicySettings {
    PolicySettings {
        style,
        refresh,
        ..PolicySettings::default()
    }
}

fn plan_of(r: &Row) -> Plan {
    let settings = PolicySettings {
        horizontal: r.horizontal,
        ..settings(r.style, r.refresh)
    };
    decide(&r.source, &settings, &r.profile, &r.caps).unwrap_or_else(|e| panic!("{}: {e}", r.rule))
}

fn family(p: &Proposal) -> String {
    let m = &p.mode;
    format!(
        "{}x{}{}",
        m.hactive,
        m.vactive,
        if m.interlace { "i" } else { "p" }
    )
}

/// The winner of a plan after the real validator.
fn winner<'p>(
    plan: &'p Plan,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> (&'p Considered, &'p Proposal) {
    let choice = choose(plan, |m| validate(m, profile, caps));
    let w = choice.winner.expect("a proposal validates");
    (w.considered, w.proposal)
}

fn rows() -> Vec<Row> {
    let intel = intel_gen9();
    let edid = edid_caps(intel);
    let gb = SourceGeometry {
        aspect_milli: 1_111,
        ..source(160, 144, 59_728)
    };
    let ntsc_480i = SourceGeometry {
        interlaced: true,
        ..source(640, 480, 59_940)
    };
    vec![
        // Step 1: a turned game gives its long side as lines. 288 lines do
        // not fit progressive at 60 Hz on 15 kHz: interlaced 1:1, centred
        // in the interlaced minimum.
        Row {
            source: SourceGeometry {
                rotation: Rotation::R90,
                aspect_milli: 1_333,
                ..source(288, 224, 60_000)
            },
            rung: Rung::Interlaced,
            mode: "619x448i",
            rate_mhz: 60_000,
            ..row(
                "step1_turned_source",
                preset("generic_15"),
                with_interlace(open_caps()),
            )
        },
        // Step 2: a GPU with a 25 MHz floor gets the dynamic super width:
        // 5 x 256.
        Row {
            mode: "1280x224p",
            ..row(
                "step2_floor_gives_dynamic_super_width",
                preset("generic_15"),
                intel,
            )
        },
        // Step 2: a GPU with no floor sends the game width.
        Row {
            mode: "256x224p",
            ..row(
                "step2_no_floor_gives_native_width",
                preset("generic_15"),
                open_caps(),
            )
        },
        // Step 2: the native width on a GPU with a floor is below it; the
        // walk goes on with the dynamic super width (5 x 256), and the
        // notice says so.
        Row {
            horizontal: Some(HorizontalStrategy::Native),
            mode: "1280x224p",
            ..row(
                "step2_native_width_below_floor_replaced",
                preset("generic_15"),
                intel,
            )
        },
        // Step 2: a fixed super width wider than the GPU becomes its own.
        Row {
            horizontal: Some(HorizontalStrategy::FixedSuperWidth { width: 2_560 }),
            mode: "1920x224p",
            ..row(
                "step2_fixed_width_clamped_to_gpu",
                preset("generic_15"),
                intel,
            )
        },
        // Step 3: Auto on 15 kHz is native.
        Row {
            mode: "256x224p",
            ..row(
                "step3_auto_on_15khz_is_native",
                preset("arcade_15"),
                open_caps(),
            )
        },
        // Step 3: Auto on 31 kHz is 2x at the game's rate.
        Row {
            rung: Rung::Multiple(2),
            mode: "256x448p",
            ..row(
                "step3_auto_on_31khz_is_2x",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // Step 3: native with exact_x2 on a 120 Hz capable monitor.
        Row {
            style: PictureStyle::Native,
            refresh: RefreshPolicy::ExactX2,
            mode: "256x224p",
            kind: RefreshKind::Double,
            rate_mhz: 2 * SNES.2,
            ..row(
                "step3_native_double_refresh",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // Native is the game's own lines 1:1, never a multiple. On 15 kHz
        // that is the native mode at the game's rate.
        Row {
            style: PictureStyle::Native,
            mode: "256x224p",
            ..row("style_native_on_15khz", preset("generic_15"), open_caps())
        },
        // pc_31_120 at 60.099 Hz forces 523 lines less 18 of blanking: 224
        // lines in 505 are padded. At twice the rate, range 1 holds 200 to
        // 256 lines and forces ceil(31,400 / 120.198) = 262 less 18 = 244:
        // 224 lines fill 92 %. Native takes double refresh, also with the
        // refresh policy exact.
        Row {
            style: PictureStyle::Native,
            mode: "256x224p",
            kind: RefreshKind::NativeDouble,
            rate_mhz: 2 * SNES.2,
            ..row(
                "style_native_on_31khz_is_double_refresh",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // The EDID bench profile, 30 to 71 kHz and 50 to 160 Hz: 224 of 480
        // picture lines at 60.099 Hz, 224 of 250 - 20 = 230 at 120.198 Hz.
        Row {
            style: PictureStyle::Native,
            mode: "256x224p",
            kind: RefreshKind::NativeDouble,
            rate_mhz: 2 * SNES.2,
            ..row(
                "style_native_on_edid_is_double_refresh",
                pc_crt_30_71(),
                edid_caps(open_caps()),
            )
        },
        // arcade_31 (31.4 to 31.5 kHz, 49.5 to 65 Hz, 400 to 512 lines)
        // has no 120 Hz. At 60.099 Hz it forces ceil(31,400 / 60.099) = 523
        // lines less ceil(1.43 ms x 31.4 kHz) = 45, and holds floor(31,500
        // / 60.099) = 524 less 46: 478 either way. 224 lines are centred in
        // 478, padded. Width: 256 x 478 x 4 / (3 x 224 x 1.333) = 546.4.
        Row {
            style: PictureStyle::Native,
            mode: "546x478p",
            ..row(
                "style_native_on_31khz_60_only_is_padded",
                preset("arcade_31"),
                open_caps(),
            )
        },
        // force_50 is the user's choice of speed: it ranks before any fill
        // comparison. NES native on arcade_31 at 50 Hz: the range holds
        // floor(31,500 / 50) = 630 less 46 lines, above its 512 limit, and
        // forces 628 - 45 = 583, so 240 lines sit in 512 (41 %). At 60.099
        // Hz they would fill more (240 of 478, 50 %), but the forced rate
        // wins. Width: 256 x 512 x 4 / (3 x 240 x 1.333) = 546.3.
        Row {
            source: source(256, 240, 60_099),
            style: PictureStyle::Native,
            refresh: RefreshPolicy::Force50,
            mode: "546x512p",
            kind: RefreshKind::Forced50,
            rate_mhz: 50_000,
            ..row(
                "style_native_force_50_before_fill",
                preset("arcade_31"),
                open_caps(),
            )
        },
        // exact_x2 is the user's choice of rate, as a forced rate is: it
        // ranks before every mode at another rate, also for native. 560
        // lines at 59.923 Hz on the EDID bench profile: at 119.846 Hz the
        // range holds floor(71,000 / 119.846) = 592 lines less
        // ceil(641.081 us x 71 kHz) = 46 of blanking, 546. 14 lines too
        // many: 7 per edge, inside the bound of 8, leave 546. At 59.923 Hz,
        // 560 lines fit with no crop, but the asked double rate wins.
        Row {
            source: source(256, 560, 59_923),
            style: PictureStyle::Native,
            refresh: RefreshPolicy::ExactX2,
            mode: "256x546p",
            kind: RefreshKind::Double,
            rate_mhz: 119_846,
            ..row(
                "style_native_exact_x2_keeps_the_asked_rate",
                pc_crt_30_71(),
                edid_caps(open_caps()),
            )
        },
        // The double rate that native takes on its own is not an asked
        // rate: 400 lines at 60 Hz on the EDID bench profile fill 400 of
        // ceil(30,000 / 60) - 20 = 480 picture lines (83 %), so native stays
        // at the game's rate, although 400 lines fit at 120 Hz too.
        Row {
            source: source(256, 400, 60_000),
            style: PictureStyle::Native,
            mode: "256x400p",
            rate_mhz: 60_000,
            ..row(
                "style_native_double_only_when_needed",
                pc_crt_30_71(),
                edid_caps(open_caps()),
            )
        },
        // Auto on the same monitor: 2x, 448 lines in 478.
        Row {
            rung: Rung::Multiple(2),
            mode: "256x448p",
            ..row(
                "style_auto_on_31khz_60_only_is_2x",
                preset("arcade_31"),
                open_caps(),
            )
        },
        // Auto with exact_x2 gives the "native, double refresh" row.
        Row {
            refresh: RefreshPolicy::ExactX2,
            mode: "256x224p",
            kind: RefreshKind::Double,
            rate_mhz: 2 * SNES.2,
            ..row(
                "style_auto_exact_x2_is_double_refresh",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // A Game Boy with native on pc_31_120: every native mode is padded.
        // At 59.728 Hz, 144 lines in 508 (28 %); at 119.456 Hz, 144 lines
        // in the 224-line standard border, of 263 - 18 = 245 picture lines
        // (58 %). The fuller one wins. Width: 160 x 224 x 4 / (3 x 144 x
        // 1.111) = 298.7.
        Row {
            source: gb,
            style: PictureStyle::Native,
            mode: "299x224p",
            kind: RefreshKind::NativeDouble,
            rate_mhz: 119_456,
            ..row(
                "style_native_handheld_on_31khz",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // Step 3: with exact_x2, 480 lines fit no range at 119.88 Hz
        // (range 1 holds 200 to 256 lines). The fitted rung at the doubled
        // rate must not beat the exact native rung at the game's rate:
        // only the fitted rung is not pixel exact.
        Row {
            source: source(640, 480, 59_940),
            refresh: RefreshPolicy::ExactX2,
            mode: "640x480p",
            rate_mhz: 59_940,
            ..row(
                "step3_fitted_at_double_rate_after_exact_rungs",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // The fitted rung keeps the game's rate when the profile allows it.
        // 500 lines at 60.606 Hz on arcade_31: the range holds floor(31,500
        // / 60.606) = 519 lines less ceil(1.43 ms x 31.5 kHz) = 46 of
        // blanking, 473. No exact rung holds 500 lines, and a crop needs 14
        // lines per edge, above the bound of 8. Switchres alone keeps 500
        // lines at 57.798 Hz; asked for 473 lines, it reaches 60.606 Hz.
        Row {
            source: source(256, 500, 60_606),
            rung: Rung::Fitted,
            mode: "256x473p",
            rate_mhz: 60_606,
            ..row(
                "step5_fitted_keeps_the_game_rate",
                preset("arcade_31"),
                open_caps(),
            )
        },
        // The last resort: 70 Hz on arcade_25 (24.96 kHz fixed, 384 to 400
        // lines, no interlace on this GPU). The nearest covered rate is 65
        // Hz; 384 lines need 384 + ceil(1.28 ms x 24.96 kHz) = 416 lines per
        // field, and 24,960 / 416 = 60.000 Hz. Nothing reaches 65 Hz, so the
        // fitted rung at the rate Switchres reaches wins: 384 lines, 4:3
        // wide (512).
        Row {
            source: source(320, 240, 70_000),
            rung: Rung::Fitted,
            mode: "512x384p",
            kind: RefreshKind::Unmatched,
            rate_mhz: 65_000,
            ..row(
                "step5_unmatched_last_resort",
                preset("arcade_25"),
                GpuCaps {
                    interlace: false,
                    ..open_caps()
                },
            )
        },
        // Step 3: an explicit multiple starts at its own rung.
        Row {
            style: PictureStyle::X3,
            rung: Rung::Multiple(3),
            mode: "512x672p",
            ..row("step3_explicit_3x", pc_crt_30_71(), edid)
        },
        // Step 3: 4x and 3x do not fit 31 kHz: the walk falls to 2x.
        Row {
            style: PictureStyle::X4,
            rung: Rung::Multiple(2),
            mode: "256x448p",
            ..row(
                "step3_explicit_4x_falls_to_2x",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // Step 3: 2x does not fit 15 kHz: the walk ends at the fitted rung.
        Row {
            style: PictureStyle::X2,
            rung: Rung::Fitted,
            mode: "256x224p",
            ..row(
                "step3_explicit_2x_on_15khz_is_fitted",
                preset("generic_15"),
                open_caps(),
            )
        },
        // Step 4: exact_x2 that no range covers falls back to the game's rate.
        Row {
            refresh: RefreshPolicy::ExactX2,
            mode: "256x224p",
            ..row(
                "step4_double_not_covered",
                preset("generic_15"),
                open_caps(),
            )
        },
        // Step 4: force_50 where the range covers 50 Hz.
        Row {
            source: source(256, 240, 60_099),
            refresh: RefreshPolicy::Force50,
            mode: "256x240p",
            kind: RefreshKind::Forced50,
            rate_mhz: 50_000,
            ..row("step4_force_50", preset("generic_15"), open_caps())
        },
        // Step 4: 288 lines do not fit progressive at a forced 60 Hz on
        // 15 kHz: rung 3, interlaced 1:1, centred in the interlaced minimum.
        Row {
            source: source(320, 288, 50_000),
            refresh: RefreshPolicy::Force60,
            rung: Rung::Interlaced,
            mode: "498x448i",
            kind: RefreshKind::Forced60,
            rate_mhz: 60_000,
            ..row(
                "step4_force_60_needs_rung_3",
                preset("generic_15"),
                with_interlace(open_caps()),
            )
        },
        // Step 4: with no interlace, nothing reaches the forced rate, so the
        // plan falls back to the game's own 50 Hz.
        Row {
            source: source(320, 288, 50_000),
            refresh: RefreshPolicy::Force60,
            mode: "320x288p",
            rate_mhz: 50_000,
            ..row(
                "step4_force_60_missed_falls_back",
                preset("generic_15"),
                GpuCaps {
                    interlace: false,
                    ..open_caps()
                },
            )
        },
        // Step 4: force_50 on a 59.94 Hz only profile is not offered.
        Row {
            refresh: RefreshPolicy::Force50,
            mode: "256x224p",
            kind: RefreshKind::Nearest,
            rate_mhz: 59_940,
            ..row("step4_force_50_not_covered", preset("ntsc"), open_caps())
        },
        // Sources that do not fit, refresh outside the range: 70 Hz DOS on
        // a TV gets the nearest rate the range covers.
        Row {
            source: source(320, 200, 70_086),
            mode: "320x200p",
            kind: RefreshKind::Nearest,
            rate_mhz: 65_000,
            ..row(
                "misfit_refresh_outside_range",
                preset("generic_15"),
                open_caps(),
            )
        },
        // Sources that do not fit, fewer lines than the minimum: a Game Boy
        // centred in a 224-line mode.
        Row {
            source: gb,
            mode: "299x224p",
            rate_mhz: 59_728,
            ..row(
                "misfit_fewer_lines_than_minimum",
                preset("generic_15"),
                open_caps(),
            )
        },
        // Pixel exact before a fit: a 192-line game on pc_31_120 at
        // 59.923 Hz. Only the 400 to 512 line range covers the rate. 2x is
        // 384 lines, below 400, so it is centred in the first standard
        // height for 2x that the range holds: 2 x 224 = 448. The range
        // forces ceil(31,400 / 59.923) = 525 lines less ceil(571 us x 31.4
        // kHz) = 18 of blanking, 507 picture lines: 384 fill 75.7 %, not
        // padded. The width keeps 4:3: 256 x 448 x 4 / (3 x 384 x 1.333) =
        // 298.7.
        Row {
            source: source(256, 192, 59_923),
            rung: Rung::Multiple(2),
            mode: "299x448p",
            rate_mhz: 59_923,
            ..row(
                "border_192_lines_2x_on_31khz",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // ZX Spectrum, 192 lines at 50.08 Hz on pc_31_120: the range forces
        // ceil(31,400 / 50.08) = 627 less 18 = 609 picture lines, above its
        // 512-line limit. Native is 192 lines centred in 512 (31 %), 2x is
        // 384 lines centred in 448 (63 %), 3x (576) is above 512. Both are
        // padded; the one that fills more of the picture lines wins.
        Row {
            source: source(256, 192, 50_080),
            rung: Rung::Multiple(2),
            mode: "299x448p",
            rate_mhz: 50_080,
            ..row(
                "border_padded_fuller_wins",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // WonderSwan, 224x144 at 75.472 Hz on generic_15: the nearest
        // covered rate is 65 Hz. At 65 Hz the range holds floor(15,750 /
        // 65) = 242 less ceil(1.28 ms x 15.75 kHz) = 21: 221 lines, so
        // neither 224 nor 240 fits. The smallest height in range that
        // Switchres does not pad is ceil(15,625 / 65) = 241 less 20 = 221.
        // Width: 224 x 221 x 4 / (3 x 144 x 1.333) = 343.9.
        Row {
            source: source(224, 144, 75_472),
            mode: "344x221p",
            kind: RefreshKind::Nearest,
            rate_mhz: 65_000,
            ..row(
                "border_any_height_wonderswan",
                preset("generic_15"),
                open_caps(),
            )
        },
        // Overscan crop: NES on arcade_31 at 60.099 Hz. The range holds
        // floor(31,500 / 60.099) = 524 less ceil(1.43 ms x 31.5 kHz) = 46:
        // 478 lines, and 2x needs 480. One source line per edge leaves 238
        // lines, 476 at 2x: pixel exact, 1 line of 240 hidden at the top
        // and 1 at the bottom, as a CRT's overscan hides them.
        Row {
            source: source(256, 240, 60_099),
            rung: Rung::Multiple(2),
            mode: "256x476p",
            ..row("crop_nes_2x_on_arcade_31", preset("arcade_31"), open_caps())
        },
        // Donkey Kong, turned (256 lines) on pc_31_120 at 60.606 Hz: 2x
        // needs 512 lines, the range holds 502. 256 - 2c lines at 2x fit
        // 502 from c = 2.5, so 3 lines per edge: 250 lines, 500 at 2x.
        Row {
            source: SourceGeometry {
                rotation: Rotation::R90,
                ..source(256, 224, 60_606)
            },
            rung: Rung::Multiple(2),
            mode: "224x500p",
            rate_mhz: 60_606,
            ..row(
                "crop_donkey_kong_2x_on_31khz",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // The crop applies to the native rung too: 518 lines at 60.606 Hz
        // on pc_31_120 miss the 502 the range holds by 16, so 8 lines per
        // edge, the bound, leave 502.
        Row {
            source: source(256, 518, 60_606),
            mode: "256x502p",
            rate_mhz: 60_606,
            ..row("crop_native_at_the_bound", preset("pc_31_120"), open_caps())
        },
        // 2x at the bound: 266 lines at 60.606 Hz on pc_31_120. The range
        // holds 502 lines, 251 source lines at 2x: 15 too many, 8 per edge.
        // 250 lines at 2x are 500.
        Row {
            source: source(256, 266, 60_606),
            rung: Rung::Multiple(2),
            mode: "256x500p",
            rate_mhz: 60_606,
            ..row("crop_2x_at_the_bound", preset("pc_31_120"), open_caps())
        },
        // Just outside the bound: 268 lines need 17 fewer, 9 per edge. Native
        // is 268 lines centred in the 501 lines that the range forces, 53.5
        // %: padded, but exact. Width: 256 x 501 x 4 / (3 x 268 x 1.333) =
        // 478.7.
        Row {
            source: source(256, 268, 60_606),
            mode: "479x501p",
            rate_mhz: 60_606,
            ..row(
                "crop_just_outside_the_bound",
                preset("pc_31_120"),
                open_caps(),
            )
        },
        // A 152-line game at 55 Hz on generic_15 stays native progressive.
        // 152 lines are below the 192 minimum: centred in 224, which the
        // range holds at 55 Hz (floor(15,750 / 55) = 286 less 21 = 265).
        // 2x needs 304: 132 source lines fit, 10 per edge, above the bound.
        // The fill is 152 of 265 picture lines (57 %): padded, but exact,
        // so no fit. Width: 256 x 224 x 4 / (3 x 152 x 1.333) = 377.4.
        Row {
            source: source(256, 152, 55_000),
            mode: "377x224p",
            rate_mhz: 55_000,
            ..row(
                "short_game_at_55_hz_stays_native",
                preset("generic_15"),
                open_caps(),
            )
        },
        // Sources that do not fit, more lines than progressive allows:
        // 480 progressive lines on 15 kHz are rung 3.
        Row {
            source: source(640, 480, 60_000),
            rung: Rung::Interlaced,
            mode: "640x480i",
            rate_mhz: 60_000,
            ..row(
                "misfit_taller_than_progressive",
                preset("generic_15"),
                with_interlace(open_caps()),
            )
        },
        // Medium resolution: native on a multisync profile.
        Row {
            source: source(496, 384, 55_000),
            mode: "496x384p",
            rate_mhz: 55_000,
            ..row(
                "misfit_medium_res_on_multisync",
                preset("arcade_15_25_31"),
                open_caps(),
            )
        },
        // Medium resolution on 15 kHz: interlaced 1:1, centred in 448 lines.
        // Rung 3 is above the fitted rung of the ladder.
        Row {
            source: source(496, 384, 55_000),
            rung: Rung::Interlaced,
            mode: "579x448i",
            rate_mhz: 55_000,
            ..row(
                "misfit_medium_res_on_15khz",
                preset("generic_15"),
                with_interlace(open_caps()),
            )
        },
        // Medium resolution on 31 kHz: pixel exact has priority over the
        // "480p fit" of the sources table. arcade_31 forces ceil(31,400 /
        // 55) = 571 lines less ceil(1.43 ms x 31.4 kHz) = 45 of blanking at
        // 55 Hz, above its 512-line limit: 384 lines centred in 512. The
        // fill is 384 of 526 picture lines, 73.0 %: padded, but the only
        // exact placement, so it wins over the fit.
        // Width: 496 x 512 x 4 / (3 x 384 x 1.333) = 661.5, rounded to 661.
        Row {
            source: source(496, 384, 55_000),
            mode: "661x512p",
            rate_mhz: 55_000,
            ..row(
                "misfit_medium_res_on_31khz",
                preset("arcade_31"),
                open_caps(),
            )
        },
        // Wider than the GPU sends: a fixed width at the GPU's widest.
        Row {
            source: source(2_048, 240, 60_000),
            mode: "1920x240p",
            rate_mhz: 60_000,
            ..row("misfit_wider_than_gpu", preset("generic_15"), intel)
        },
        // An interlaced source on 15 kHz is native interlaced.
        Row {
            source: ntsc_480i,
            mode: "1280x480i",
            rate_mhz: 59_940,
            ..row(
                "class_15khz_480i_native",
                preset("generic_15"),
                with_interlace(intel),
            )
        },
        // An interlaced source on 31 kHz is shown as 480p.
        Row {
            source: ntsc_480i,
            mode: "1280x480p",
            rate_mhz: 59_940,
            ..row("class_31khz_480i_as_480p", pc_crt_30_71(), edid)
        },
        // A PAL 288-line game on 15 kHz.
        Row {
            source: source(320, 288, 50_007),
            mode: "1280x288p",
            rate_mhz: 50_007,
            ..row("class_15khz_288p", preset("generic_15"), intel)
        },
        // Step 5, OP-026: the padded native mode is not chosen on a 30 kHz
        // EDID profile; line doubling wins.
        Row {
            rung: Rung::Multiple(2),
            mode: "768x448p",
            ..row("step5_padded_mode_not_chosen", pc_crt_30_71(), edid)
        },
        // The bench modes on the EDID profile.
        Row {
            source: source(640, 480, 60_000),
            mode: "1280x480p",
            rate_mhz: 60_000,
            ..row("bench_640x480_60", pc_crt_30_71(), edid)
        },
        Row {
            source: source(320, 240, 120_000),
            mode: "640x240p",
            rate_mhz: 120_000,
            ..row("bench_320x240_120", pc_crt_30_71(), edid)
        },
        // A flat panel: the preferred mode at the nearest rate.
        Row {
            rung: Rung::Panel,
            mode: "1920x1080p",
            kind: RefreshKind::Panel,
            rate_mhz: 60_000,
            ..row("panel_ntsc_preferred_mode", panel_1080(), open_caps())
        },
        Row {
            source: source(256, 239, 50_007),
            rung: Rung::Panel,
            mode: "1920x1080p",
            kind: RefreshKind::Panel,
            rate_mhz: 50_000,
            ..row("panel_pal_50hz_mode", panel_1080(), open_caps())
        },
    ]
}

#[test]
fn every_rule_gives_its_rung_and_mode_family() {
    let mut wrong = Vec::new();
    for r in rows() {
        let plan = plan_of(&r);
        let (considered, proposal) = winner(&plan, &r.profile, &r.caps);
        let got = (
            considered.rung,
            family(proposal),
            considered.target.kind,
            considered.target.rate_mhz,
        );
        let want = (r.rung, r.mode.to_owned(), r.kind, r.rate_mhz);
        if got != want {
            wrong.push(format!(
                "{}:\n  want {want:?}\n  got  {got:?}\n  {considered}",
                r.rule
            ));
        }
        if r.kind != RefreshKind::Panel && r.kind != RefreshKind::Unmatched {
            let off = proposal
                .timing
                .field_rate_mhz
                .abs_diff(u64::from(r.rate_mhz));
            if off >= REFRESH_MATCH_MHZ {
                wrong.push(format!(
                    "{}: {off} mHz from the target\n  {considered}",
                    r.rule
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_padded_mode_keeps_its_reason_and_numbers() {
    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &pc_crt_30_71(),
        &edid_caps(intel_gen9()),
    )
    .unwrap();
    let native = plan
        .considered
        .iter()
        .find(|c| c.rung == Rung::Native && c.target.kind == RefreshKind::Exact)
        .unwrap();
    let padding = native.outcome.as_ref().unwrap().padding.unwrap();
    // 224 active lines; ceil(30 kHz / 60.099 Hz) = 500 lines per field less
    // 20 of blanking leaves 480.
    assert_eq!((padding.game_lines, padding.picture_lines), (224, 480));
    assert_eq!(padding.line_rate_min_mhz, 30_000_000);
    // The padded mode is the one Switchres picks on its own (OP-026).
    assert_eq!(native.outcome.as_ref().unwrap().mode.vtotal, 500);
    let doubled = plan.proposals().next().unwrap().1;
    assert_eq!(doubled.rung, Rung::Multiple(2));
}

#[test]
fn a_dynamic_super_width_is_the_smallest_multiple_above_the_floor() {
    let caps = intel_gen9();
    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &preset("generic_15"),
        &caps,
    )
    .unwrap();
    let (_, _, p) = plan.proposals().next().unwrap();
    assert_eq!(p.horizontal, Some(HorizontalScale::Multiple(5)));
    assert!(p.mode.pclock_hz >= caps.pclock_min_hz);
    // Four times the width at the same line rate is below the floor.
    assert!(u64::from(p.mode.pclock_hz) * 4 / 5 < u64::from(caps.pclock_min_hz));
    assert_eq!(
        p.placement.dest,
        Rect {
            x: 0,
            y: 0,
            w: 1_280,
            h: 224
        }
    );
    assert_eq!(p.placement.filter, Filter::Nearest);
}

#[test]
fn a_floor_that_no_width_reaches_drops_the_candidate() {
    let caps = GpuCaps {
        pclock_min_hz: 200_000_000,
        max_width: 1_024,
        ..intel_gen9()
    };
    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &preset("generic_15"),
        &caps,
    )
    .unwrap();
    assert!(matches!(
        plan.considered[0].outcome,
        Err(Dropped::PixelClockFloor {
            floor_hz: 200_000_000,
            max_width: 1_024,
            ..
        })
    ));
}

#[test]
fn a_skipped_refresh_target_is_the_reason_for_the_fallback() {
    let plan = decide(
        &snes(),
        &settings(PictureStyle::Auto, RefreshPolicy::ExactX2),
        &preset("generic_15"),
        &open_caps(),
    )
    .unwrap();
    assert_eq!(
        plan.skipped,
        [SkippedTarget {
            kind: RefreshKind::Double,
            rate_mhz: 120_198
        }]
    );
}

#[test]
fn a_mode_taller_than_the_gpu_is_dropped_with_its_numbers() {
    let plan = decide(
        &snes(),
        &settings(PictureStyle::X4, RefreshPolicy::Exact),
        &pc_crt_30_71(),
        &GpuCaps {
            max_height: 800,
            ..open_caps()
        },
    )
    .unwrap();
    assert_eq!(
        plan.considered[0].outcome,
        Err(Dropped::TallerThanGpu {
            lines: 896,
            max_lines: 800
        })
    );
}

#[test]
fn step6_the_first_proposal_that_validates_wins_and_the_refusals_are_kept() {
    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &pc_crt_30_71(),
        &open_caps(),
    )
    .unwrap();
    let first = plan.proposals().next().unwrap().2.mode;
    let choice = choose(&plan, |m| if *m == first { Err("refused") } else { Ok(()) });
    let w = choice.winner.unwrap();
    assert_eq!(choice.refused.len(), 1);
    assert_eq!(choice.refused[0].reason, "refused");
    assert!(choice.refused[0].index < w.index);
    assert_ne!(w.proposal.mode, first);
}

#[test]
fn step6_no_winner_when_the_validator_refuses_everything() {
    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &preset("generic_15"),
        &open_caps(),
    )
    .unwrap();
    let choice = choose(&plan, |_| Err::<(), _>(()));
    assert!(choice.winner.is_none());
    assert_eq!(choice.refused.len(), plan.proposals().count());
}

/// The first two entries of a plan whose strategy cannot reach the GPU
/// floor: the dropped candidate, then its replacement.
fn floor_pair(source: SourceGeometry, horizontal: HorizontalStrategy, caps: GpuCaps) -> Plan {
    let s = PolicySettings {
        horizontal: Some(horizontal),
        ..PolicySettings::default()
    };
    decide(&source, &s, &preset("generic_15"), &caps).unwrap()
}

#[test]
fn a_native_width_below_the_gpu_floor_is_replaced_by_the_dynamic_super_width() {
    let caps = intel_gen9();
    let plan = floor_pair(snes(), HorizontalStrategy::Native, caps);
    let first = &plan.considered[0];
    assert_eq!(first.rung, Rung::Native);
    let Err(Dropped::PixelClockFloor {
        width,
        pclock_hz,
        floor_hz,
        max_width,
    }) = first.outcome
    else {
        panic!("{first}");
    };
    assert_eq!((width, floor_hz, max_width), (256, 25_000_000, 1_920));
    assert!(pclock_hz < floor_hz);

    let (considered, proposal) = winner(&plan, &preset("generic_15"), &caps);
    assert_eq!(considered, &plan.considered[1]);
    assert_eq!(proposal.horizontal, Some(HorizontalScale::Multiple(5)));
    let replaced = proposal.strategy_replaced.unwrap();
    assert_eq!(replaced.from, HorizontalStrategy::Native);
    assert_eq!(replaced.reason, first.outcome.unwrap_err());
    let n = notice(&plan, considered).unwrap();
    assert!(!n.fitted);
    assert_eq!(n.strategy_replaced, Some(replaced));
}

#[test]
fn a_fixed_width_below_the_gpu_floor_is_replaced_by_the_dynamic_super_width() {
    let caps = GpuCaps {
        pclock_min_hz: 60_000_000,
        max_width: 3_840,
        ..intel_gen9()
    };
    let plan = floor_pair(
        snes(),
        HorizontalStrategy::FixedSuperWidth { width: 1_920 },
        caps,
    );
    assert!(matches!(
        plan.considered[0].outcome,
        Err(Dropped::PixelClockFloor {
            width: 1_920,
            floor_hz: 60_000_000,
            ..
        })
    ));
    let (_, proposal) = winner(&plan, &preset("generic_15"), &caps);
    let Some(HorizontalScale::Multiple(k)) = proposal.horizontal else {
        panic!("{proposal}");
    };
    assert!(proposal.mode.pclock_hz >= caps.pclock_min_hz);
    assert!(u64::from(proposal.mode.pclock_hz) * u64::from(k - 1) / u64::from(k) < 60_000_000);
}

#[test]
fn a_source_wider_than_the_gpu_below_the_floor_is_dropped_with_the_numbers() {
    // At the GPU's widest mode the pixel clock is below the floor, and no
    // whole multiple of 2048 pixels fits 1920: the candidate is dropped
    // once, with no replacement.
    let caps = GpuCaps {
        pclock_min_hz: 60_000_000,
        ..intel_gen9()
    };
    let plan = floor_pair(source(2_048, 224, 60_099), HorizontalStrategy::Native, caps);
    let first = &plan.considered[0];
    assert_eq!(first.rung, Rung::Native);
    assert!(
        matches!(
            first.outcome,
            Err(Dropped::PixelClockFloor {
                width: 1_920,
                floor_hz: 60_000_000,
                max_width: 1_920,
                ..
            })
        ),
        "{first}"
    );
    assert_ne!(plan.considered[1].rung, Rung::Native);
}

#[test]
fn step7_an_equal_mode_changes_only_scale_and_crop() {
    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &preset("generic_15"),
        &open_caps(),
    )
    .unwrap();
    let mode = plan.proposals().next().unwrap().2.mode;
    assert_eq!(switch_for(Some(&mode), &mode), Switch::ScaleOnly);
    assert_eq!(switch_for(None, &mode), Switch::Modeset);
    let other = neptron_types::Modeline {
        vtotal: mode.vtotal + 1,
        ..mode
    };
    assert_eq!(switch_for(Some(&other), &mode), Switch::Modeset);
}

#[test]
fn step8_a_fitted_winner_shows_a_notice() {
    let r = rows()
        .into_iter()
        .find(|r| r.rule == "step3_explicit_2x_on_15khz_is_fitted")
        .unwrap();
    let plan = plan_of(&r);
    let (considered, _) = winner(&plan, &r.profile, &r.caps);
    let n = notice(&plan, considered).unwrap();
    assert!(n.fitted);
}

#[test]
fn step8_a_forced_rate_shows_a_notice_and_changes_the_speed() {
    let r = rows()
        .into_iter()
        .find(|r| r.rule == "step4_force_50")
        .unwrap();
    let plan = plan_of(&r);
    let (considered, proposal) = winner(&plan, &r.profile, &r.caps);
    assert!(proposal.timing.changes_speed());
    let n = notice(&plan, considered).unwrap();
    assert!(!n.fitted);
    assert_eq!(n.game_mhz, 60_099);
    assert!(n.shown_mhz.abs_diff(50_000) < REFRESH_MATCH_MHZ);
}

#[test]
fn step8_exact_double_refresh_and_panel_rates_show_no_notice() {
    for rule in [
        "step2_floor_gives_dynamic_super_width",
        "step3_native_double_refresh",
        "panel_ntsc_preferred_mode",
    ] {
        let r = rows().into_iter().find(|r| r.rule == rule).unwrap();
        let plan = plan_of(&r);
        let (considered, proposal) = winner(&plan, &r.profile, &r.caps);
        assert_eq!(notice(&plan, considered), None, "{rule}");
        if rule == "step3_native_double_refresh" {
            assert_eq!(proposal.timing.repeat(), 2);
            assert!(!proposal.timing.changes_speed());
        }
    }
}

#[test]
fn step8_double_refresh_that_the_native_style_takes_shows_a_notice() {
    let r = rows()
        .into_iter()
        .find(|r| r.rule == "style_native_on_31khz_is_double_refresh")
        .unwrap();
    let plan = plan_of(&r);
    let (considered, proposal) = winner(&plan, &r.profile, &r.caps);
    assert_eq!(proposal.timing.repeat(), 2);
    assert!(!proposal.timing.changes_speed());
    let n = notice(&plan, considered).unwrap();
    assert!(n.double_refresh);
    assert!(!n.fitted && !n.style_fallback);
    assert_eq!(n.game_mhz, SNES.2);
    assert!(n.shown_mhz.abs_diff(u64::from(SNES.2)) < REFRESH_MATCH_MHZ);
}

#[test]
fn step8_a_style_that_falls_to_another_rung_shows_a_notice() {
    let r = rows()
        .into_iter()
        .find(|r| r.rule == "step3_explicit_4x_falls_to_2x")
        .unwrap();
    let plan = plan_of(&r);
    let (considered, _) = winner(&plan, &r.profile, &r.caps);
    assert_eq!(considered.rung, Rung::Multiple(2));
    let n = notice(&plan, considered).unwrap();
    assert!(n.style_fallback);
    assert!(!n.fitted && !n.double_refresh);
}

#[test]
fn the_padded_native_mode_keeps_its_padding() {
    let r = rows()
        .into_iter()
        .find(|r| r.rule == "style_native_on_31khz_60_only_is_padded")
        .unwrap();
    let plan = plan_of(&r);
    let (considered, proposal) = winner(&plan, &r.profile, &r.caps);
    let padding = proposal.padding.expect("the native mode is padded");
    assert_eq!((padding.game_lines, padding.picture_lines), (224, 478));
    assert_eq!(padding.line_rate_min_mhz, 31_400_000);
    assert_eq!(padding.fill_permille(), 468);
    assert!(proposal.bordered);
    // The game's own lines, centred: (478 - 224) / 2 = 127.
    assert_eq!(
        (proposal.placement.dest.y, proposal.placement.scale.y),
        (127, Some(1))
    );
    // Not a fall to another rung and not a changed rate, but a small
    // picture: the notice gives its fill, 224 of 478 lines.
    let n = notice(&plan, considered).expect("a padded winner gives a notice");
    assert_eq!(n.padded_fill_permille, Some(468));
    assert!(!n.fitted && !n.style_fallback && !n.double_refresh);
    assert_eq!(n.cropped_lines, 0);
    // arcade_31 has no range at 120.198 Hz, so native has no double
    // refresh target; the plan does not list it as skipped, because the
    // refresh policy did not ask for it.
    assert!(
        plan.considered
            .iter()
            .all(|c| c.target.kind != RefreshKind::NativeDouble)
    );
    assert!(plan.skipped.is_empty());
}

#[test]
fn native_never_walks_a_multiple() {
    for profile in [preset("generic_15"), preset("pc_31_120"), pc_crt_30_71()] {
        for refresh in [RefreshPolicy::Exact, RefreshPolicy::ExactX2] {
            let plan = decide(
                &snes(),
                &settings(PictureStyle::Native, refresh),
                &profile,
                &open_caps(),
            )
            .unwrap();
            for c in &plan.considered {
                assert!(
                    matches!(c.rung, Rung::Native | Rung::Interlaced | Rung::Fitted),
                    "{} {c}",
                    profile.id
                );
            }
        }
    }
}

#[test]
fn native_with_exact_x2_has_one_double_refresh_target() {
    let plan = decide(
        &snes(),
        &settings(PictureStyle::Native, RefreshPolicy::ExactX2),
        &preset("pc_31_120"),
        &open_caps(),
    )
    .unwrap();
    let kinds: Vec<RefreshKind> = plan
        .considered
        .iter()
        .filter(|c| c.rung == Rung::Native)
        .map(|c| c.target.kind)
        .collect();
    assert_eq!(kinds, [RefreshKind::Double, RefreshKind::Exact]);
}

#[test]
fn an_overscan_crop_keeps_the_picture_exact_and_says_so() {
    let r = rows()
        .into_iter()
        .find(|r| r.rule == "crop_nes_2x_on_arcade_31")
        .unwrap();
    let plan = plan_of(&r);
    let (considered, proposal) = winner(&plan, &r.profile, &r.caps);
    let crop = proposal.overscan_crop.expect("the 2x mode is cropped");
    assert_eq!(
        (crop.lines_per_edge, crop.asked_lines, crop.capacity_lines),
        (1, 480, 478)
    );
    let place = &proposal.placement;
    assert_eq!(
        place.crop,
        Rect {
            x: 0,
            y: 1,
            w: 256,
            h: 238
        }
    );
    assert_eq!(
        place.dest,
        Rect {
            x: 0,
            y: 0,
            w: 256,
            h: 476
        }
    );
    assert_eq!(place.filter, Filter::Nearest);
    assert_eq!((place.scale.x, place.scale.y), (Some(1), Some(2)));
    // 476 of the 478 lines that the range forces: not padded.
    assert_eq!(proposal.padding, None);
    let n = notice(&plan, considered).unwrap();
    assert_eq!(n.cropped_lines, 2);
    assert!(!n.fitted && !n.style_fallback);
    // The uncropped 2x is kept before it, with the reason.
    let index = plan
        .considered
        .iter()
        .position(|c| std::ptr::eq(c, considered))
        .unwrap();
    assert!(
        plan.considered[..index]
            .iter()
            .any(|c| c.rung == Rung::Multiple(2) && c.outcome.is_err()),
        "no dropped 2x before the cropped one"
    );
}

#[test]
fn a_turned_game_is_cropped_on_the_columns_of_its_frame() {
    let r = rows()
        .into_iter()
        .find(|r| r.rule == "crop_donkey_kong_2x_on_31khz")
        .unwrap();
    let plan = plan_of(&r);
    let (_, proposal) = winner(&plan, &r.profile, &r.caps);
    assert_eq!(proposal.overscan_crop.unwrap().lines_per_edge, 3);
    assert_eq!(
        proposal.placement.crop,
        Rect {
            x: 3,
            y: 0,
            w: 250,
            h: 224
        }
    );
}

#[test]
fn a_multiple_that_fits_is_not_cropped() {
    for rule in ["step3_auto_on_31khz_is_2x", "crop_just_outside_the_bound"] {
        let r = rows().into_iter().find(|r| r.rule == rule).unwrap();
        let plan = plan_of(&r);
        let (considered, proposal) = winner(&plan, &r.profile, &r.caps);
        assert_eq!(proposal.overscan_crop, None, "{rule}");
        let (fw, fh) = (u32::from(r.source.width), u32::from(r.source.height));
        assert_eq!(
            proposal.placement.crop,
            Rect {
                x: 0,
                y: 0,
                w: fw,
                h: fh
            },
            "{rule}"
        );
        assert_eq!(
            notice(&plan, considered).map_or(0, |n| n.cropped_lines),
            0,
            "{rule}"
        );
    }
}

#[test]
fn the_ui_fills_a_crt_mode_and_scales_uniformly_on_a_panel() {
    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &preset("generic_15"),
        &intel_gen9(),
    )
    .unwrap();
    let p = plan.proposals().next().unwrap().2;
    assert_eq!(p.placement.ui_dest, ui_dest_fill(1_280, 224));

    let plan = decide(
        &snes(),
        &PolicySettings::default(),
        &panel_1080(),
        &open_caps(),
    )
    .unwrap();
    let p = plan.proposals().next().unwrap().2;
    assert_eq!(
        p.placement.ui_dest,
        Rect {
            x: 320,
            y: 60,
            w: 1_280,
            h: 960
        }
    );
    // 224 x 4 = 896 lines, at an aspect of 1.333: 1194 pixels, centred.
    assert_eq!(
        p.placement.dest,
        Rect {
            x: 363,
            y: 92,
            w: 1_194,
            h: 896
        }
    );
    assert_eq!(p.placement.scale.y, Some(4));
}

#[test]
fn the_display_panel_rows_come_from_the_same_plan_and_validator() {
    let profile = pc_crt_30_71();
    let caps = edid_caps(intel_gen9());
    let rows = style_rows(&snes(), &PolicySettings::default(), &profile, &caps, |m| {
        validate(m, &profile, &caps).map(|_| ())
    })
    .unwrap();
    let summary: Vec<(PictureStyle, bool, Option<Rung>)> = rows
        .iter()
        .map(|r| {
            (
                r.style,
                r.available,
                r.winner.map(|i| r.plan.considered[i].rung),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (PictureStyle::Auto, true, Some(Rung::Multiple(2))),
            // Native shows the game's own 224 lines, at double refresh.
            (PictureStyle::Native, true, Some(Rung::Native)),
            (PictureStyle::X2, true, Some(Rung::Multiple(2))),
            (PictureStyle::X3, true, Some(Rung::Multiple(3))),
            (PictureStyle::X4, true, Some(Rung::Multiple(4))),
        ]
    );
    let native = &rows[1];
    let target = native.plan.considered[native.winner.unwrap()].target;
    assert_eq!(
        (target.kind, target.rate_mhz),
        (RefreshKind::NativeDouble, 2 * SNES.2)
    );
    // Each row is the engine's own decision for that style.
    for r in &rows {
        let plan = decide(
            &snes(),
            &settings(r.style, RefreshPolicy::Exact),
            &profile,
            &caps,
        )
        .unwrap();
        assert_eq!(plan, r.plan);
    }
}

#[test]
fn an_empty_source_or_profile_is_an_error() {
    use neptron_modeline::policy::PolicyError;
    let empty = source(0, 224, 60_000);
    assert!(matches!(
        decide(
            &empty,
            &PolicySettings::default(),
            &preset("generic_15"),
            &open_caps()
        ),
        Err(PolicyError::EmptySource { width: 0, .. })
    ));
    let no_ranges = MonitorProfile {
        id: "none".into(),
        kind: neptron_types::MonitorKind::Crt { ranges: vec![] },
    };
    assert!(matches!(
        decide(
            &snes(),
            &PolicySettings::default(),
            &no_ranges,
            &open_caps()
        ),
        Err(PolicyError::EmptyProfile { .. })
    ));
}

/// One row of the drop table: a rung at the game's rate, and the reason
/// the policy drops it, with numbers from the Definition or a hand
/// calculation.
struct DropRow {
    rule: &'static str,
    source: SourceGeometry,
    style: PictureStyle,
    solver: neptron_modeline::Settings,
    profile: MonitorProfile,
    caps: GpuCaps,
    rung: Rung,
    want: Dropped,
}

fn drop_row(rule: &'static str, rung: Rung, want: Dropped, profile: MonitorProfile) -> DropRow {
    DropRow {
        rule,
        source: snes(),
        style: PictureStyle::Auto,
        solver: neptron_modeline::Settings::default(),
        profile,
        caps: open_caps(),
        rung,
        want,
    }
}

fn drop_rows() -> Vec<DropRow> {
    let no_interlace = GpuCaps {
        interlace: false,
        ..open_caps()
    };
    vec![
        // 480 progressive lines are above the 288 of generic_15 and inside
        // its 448 to 576 interlaced lines: Switchres interlaces them.
        DropRow {
            source: source(640, 480, 60_000),
            ..drop_row(
                "scan_changed",
                Rung::Native,
                Dropped::ScanChanged {
                    asked_interlace: false,
                },
                preset("generic_15"),
            )
        },
        // A 16:9 monitor aspect makes Switchres widen the mode by
        // (16/9) / (4/3): 256 x 4/3 = 341.3, truncated to 341.
        DropRow {
            solver: neptron_modeline::Settings {
                monitor_aspect: neptron_modeline::Aspect {
                    width: 16,
                    height: 9,
                },
                ..neptron_modeline::Settings::default()
            },
            ..drop_row(
                "width_changed",
                Rung::Native,
                Dropped::WidthChanged {
                    asked: 256,
                    got: 341,
                },
                preset("generic_15"),
            )
        },
        // 448 lines fit no progressive range of generic_15 and the GPU has
        // no interlace: Switchres stretches to the most lines at 60.099 Hz,
        // 15,625 / 60.099 = 260 per field less 20 of blanking = 240.
        DropRow {
            caps: no_interlace,
            ..drop_row(
                "lines_changed",
                Rung::Multiple(2),
                Dropped::LinesChanged {
                    asked: 448,
                    got: 240,
                },
                preset("generic_15"),
            )
        },
        // A Game Boy at 2x: 288 lines plus 20 of blanking at the highest
        // 15.75 kHz is 15,750 / 308 = 51.136 Hz, not 59.728 Hz.
        DropRow {
            source: SourceGeometry {
                aspect_milli: 1_111,
                ..source(160, 144, 59_728)
            },
            ..drop_row(
                "refresh_missed",
                Rung::Multiple(2),
                Dropped::RefreshMissed {
                    target_mhz: 59_728,
                    got_mhz: 51_136,
                },
                preset("generic_15"),
            )
        },
        // A 192-line game at 65 Hz on arcade_25 (24.96 kHz fixed, 384 to
        // 400 lines): at 65 Hz the range holds floor(24,960 / 65) = 384
        // lines less ceil(1.28 ms x 24.96 kHz) = 32 of blanking, 352, below
        // its 384-line minimum. No mode height holds the game.
        DropRow {
            source: source(256, 192, 65_000),
            ..drop_row(
                "no_border_mode",
                Rung::Native,
                Dropped::NoBorderMode {
                    lines: 192,
                    line_min: 384,
                },
                preset("arcade_25"),
            )
        },
        // 224 lines fit progressive at 60.099 Hz on generic_15:
        // floor(15,750 / 60.099) = 262 less ceil(1.28 ms x 15.75 kHz) = 21.
        drop_row(
            "fits_progressive",
            Rung::Interlaced,
            Dropped::FitsProgressive {
                lines: 224,
                progressive_max: 241,
            },
            preset("generic_15"),
        ),
        // Rung 3 of an interlaced source asks for the mode of rung 1.
        DropRow {
            source: SourceGeometry {
                interlaced: true,
                ..source(640, 480, 59_940)
            },
            ..drop_row(
                "same_as_native",
                Rung::Interlaced,
                Dropped::SameAsNative,
                preset("generic_15"),
            )
        },
        // 480 lines need rung 3 on generic_15, and the GPU has no interlace.
        DropRow {
            source: source(640, 480, 60_000),
            caps: no_interlace,
            ..drop_row(
                "no_interlace",
                Rung::Interlaced,
                Dropped::NoInterlace,
                preset("generic_15"),
            )
        },
    ]
}

#[test]
fn every_drop_rule_gives_its_reason_and_numbers() {
    let mut wrong = Vec::new();
    for r in drop_rows() {
        let settings = PolicySettings {
            solver: r.solver,
            ..settings(r.style, RefreshPolicy::Exact)
        };
        let plan = decide(&r.source, &settings, &r.profile, &r.caps)
            .unwrap_or_else(|e| panic!("{}: {e}", r.rule));
        let entry = plan
            .considered
            .iter()
            .find(|c| c.rung == r.rung && c.target.kind != RefreshKind::Unmatched)
            .unwrap_or_else(|| panic!("{}: no {} entry", r.rule, r.rung));
        if entry.outcome != Err(r.want) {
            wrong.push(format!("{}:\n  want {:?}\n  got  {entry}", r.rule, r.want));
        }
        if r.want.rule() != r.rule {
            wrong.push(format!("{}: the reason names {}", r.rule, r.want.rule()));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The one exception to the scan check: an interlaced source on a monitor
/// with no interlaced range is shown progressive, 480i as 480p.
#[test]
fn an_interlaced_source_on_31_khz_is_shown_progressive() {
    let source = SourceGeometry {
        interlaced: true,
        ..source(640, 480, 59_940)
    };
    let plan = decide(
        &source,
        &PolicySettings::default(),
        &pc_crt_30_71(),
        &open_caps(),
    )
    .unwrap();
    let native = &plan.considered[0];
    assert_eq!(native.rung, Rung::Native);
    let mode = native.outcome.as_ref().unwrap().mode;
    assert_eq!(
        (mode.hactive, mode.vactive, mode.interlace),
        (640, 480, false)
    );
}

/// A width of 65535 pixels at the native width: the porches push the
/// horizontal timings past the 16 bits of the contract, so the candidate
/// is dropped and the plan goes on.
#[test]
fn a_timing_outside_its_contract_field_drops_only_that_candidate() {
    let s = PolicySettings {
        horizontal: Some(HorizontalStrategy::Native),
        ..PolicySettings::default()
    };
    let plan = decide(
        &source(u16::MAX, 224, 60_000),
        &s,
        &preset("generic_15"),
        &open_caps(),
    )
    .unwrap();
    let native = &plan.considered[0];
    assert_eq!(native.rung, Rung::Native);
    assert!(
        matches!(
            native.outcome,
            Err(Dropped::TimingOutOfRange {
                value,
                max: 65_535,
                ..
            }) if value > 65_535
        ),
        "{native}"
    );
}
