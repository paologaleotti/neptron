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
        // Donkey Kong, turned: 256 lines at 60.606 Hz. 2x (512) misses the
        // rate on pc_31_120, and Switchres alone doubles the fitted request
        // to 512 lines at 59.623 Hz. The range holds floor(31.6 kHz /
        // 60.606 Hz) = 521 lines less ceil(571 us x 31.6 kHz) = 19 of
        // blanking: 502 lines at 60.606 Hz.
        Row {
            source: SourceGeometry {
                rotation: Rotation::R90,
                ..source(256, 224, 60_606)
            },
            rung: Rung::Fitted,
            mode: "224x502p",
            rate_mhz: 60_606,
            ..row(
                "step5_fitted_keeps_the_game_rate",
                preset("pc_31_120"),
                open_caps(),
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
        // Medium resolution: a progressive fit on 31 kHz.
        Row {
            source: source(496, 384, 55_000),
            rung: Rung::Fitted,
            mode: "682x512p",
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
        if r.kind != RefreshKind::Panel {
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

#[test]
fn step6_the_validator_refuses_a_mode_below_the_gpu_floor() {
    // Native width on a GPU with a floor: Switchres gives 256 pixels, the
    // validator refuses every one, and no mode is set.
    let caps = intel_gen9();
    let s = PolicySettings {
        horizontal: Some(HorizontalStrategy::Native),
        ..PolicySettings::default()
    };
    let plan = decide(&snes(), &s, &preset("generic_15"), &caps).unwrap();
    let choice = choose(&plan, |m| validate(m, &preset("generic_15"), &caps));
    assert!(choice.winner.is_none());
    assert!(!choice.refused.is_empty());
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
    let n = notice(considered).unwrap();
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
    let n = notice(considered).unwrap();
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
        assert_eq!(notice(considered), None, "{rule}");
        if rule == "step3_native_double_refresh" {
            assert_eq!(proposal.timing.repeat(), 2);
            assert!(!proposal.timing.changes_speed());
        }
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
            // Native is greyed out: its own rung is padded, 2x wins.
            (PictureStyle::Native, false, Some(Rung::Multiple(2))),
            (PictureStyle::X2, true, Some(Rung::Multiple(2))),
            (PictureStyle::X3, true, Some(Rung::Multiple(3))),
            (PictureStyle::X4, true, Some(Rung::Multiple(4))),
        ]
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
