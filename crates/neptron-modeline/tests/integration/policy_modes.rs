//! The picture modes (`display.style`) on every display of the arcade
//! table: the rule of each mode as docs/spec/video-policy-cases.md,
//! section 8, gives it, checked on every source of the table.

use neptron_modeline::policy::{
    DeclineReason, NoticeReason, PictureStyle, Plan, PolicySettings, Proposal, REFRESH_MATCH_MHZ,
    RefreshKind, Rung, SMALL_PICTURE_MIN_PERCENT, SoftReason, VerticalMode, Winner, choose, decide,
    notice, picture_choices,
};
use neptron_safety::validate;
use neptron_types::{MonitorKind, Rotation, SourceGeometry};

use crate::arcade_table::{self, Board, Display};
use crate::presets::{open_caps, panel_1080, preset, source};

/// Every board and source of the arcade table.
fn sources() -> impl Iterator<Item = &'static Board> {
    arcade_table::BOARDS
        .iter()
        .chain(arcade_table::SOURCES.iter())
}

/// The plan and the winner of `style` for `board` on `display`, with the
/// other settings of `base`.
fn decide_with(
    board: &SourceGeometry,
    style: PictureStyle,
    base: PolicySettings,
    display: &Display,
) -> Plan {
    let settings = PolicySettings { style, ..base };
    decide(board, &settings, &display.profile, &display.caps)
        .unwrap_or_else(|e| panic!("{}: {e}", display.profile.id))
}

fn won<'p>(plan: &'p Plan, display: &Display) -> Option<Winner<'p, ()>> {
    choose(plan, |m| {
        validate(m, &display.profile, &display.caps).map(|_| ())
    })
    .winner
}

/// How far the game's speed moves, in ppm of its rate.
fn speed_ppm(p: &Proposal) -> u64 {
    let shown = p.timing.field_rate_mhz / u64::from(p.timing.repeat().max(1));
    let game = u64::from(p.timing.game_mhz).max(1);
    shown.abs_diff(game) * 1_000_000 / game
}

fn is_panel(display: &Display) -> bool {
    matches!(display.profile.kind, MonitorKind::FlatPanel { .. })
}

/// Every cell of the table with `check`, and the list of the cells that
/// fail it.
fn every_cell(mut check: impl FnMut(&Board, &Display) -> Result<(), String>) {
    let displays = arcade_table::displays();
    let mut failures = Vec::new();
    for board in sources() {
        for display in &displays {
            if let Err(e) = check(board, display) {
                failures.push(format!("{} on {}: {e}", board.name, display.profile.id));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn sharp_is_exact_wherever_an_exact_mode_validates() {
    every_cell(|board, display| {
        let base = arcade_table::settings_of(board);
        let plan = decide_with(&board.source(), PictureStyle::Sharp, base, display);
        let Some(w) = won(&plan, display) else {
            return Ok(());
        };
        let exact_validates = plan.proposals().any(|(_, c, p)| {
            !c.rung.is_fitted() && validate(&p.mode, &display.profile, &display.caps).is_ok()
        });
        if w.considered.rung.is_fitted() && exact_validates {
            return Err(format!("the winner {} is a fit", w.considered));
        }
        let auto = decide_with(&board.source(), PictureStyle::Auto, base, display);
        let auto_exact = won(&auto, display).is_some_and(|a| !a.considered.rung.is_fitted());
        if auto_exact && w.considered.rung.is_fitted() {
            return Err("auto is exact and sharp is not".to_owned());
        }
        Ok(())
    });
}

/// 500 progressive lines at 60 Hz on a 15 kHz tube: no exact mode at
/// 60 Hz; the interlaced 1:1 mode holds them only at a slower rate, which
/// changes the speed more than the fit by more than the margin of V6.
/// `auto` takes the fit, `sharp` the exact lines, `speed` the fit.
#[test]
fn sharp_keeps_the_exact_lines_where_auto_takes_the_fit_for_the_speed() {
    let display = Display {
        profile: preset("generic_15"),
        caps: open_caps(),
    };
    let tall = source(640, 500, 60_000);
    let base = PolicySettings::default();
    let rung = |style| {
        let plan = decide_with(&tall, style, base, &display);
        let w = won(&plan, &display).expect("a winner");
        (
            w.considered.rung,
            w.considered.target.kind,
            speed_ppm(w.proposal),
        )
    };
    let (auto, _, auto_ppm) = rung(PictureStyle::Auto);
    assert_eq!(auto, Rung::Fitted);
    let (sharp, kind, sharp_ppm) = rung(PictureStyle::Sharp);
    assert_eq!((sharp, kind), (Rung::Interlaced, RefreshKind::Nearest));
    assert!(sharp_ppm > auto_ppm + 20_000, "{sharp_ppm} {auto_ppm}");
    assert_eq!(rung(PictureStyle::Speed).0, Rung::Fitted);
}

#[test]
fn speed_never_changes_the_speed_more_than_auto() {
    every_cell(|board, display| {
        let base = arcade_table::settings_of(board);
        let speed = decide_with(&board.source(), PictureStyle::Speed, base, display);
        let auto = decide_with(&board.source(), PictureStyle::Auto, base, display);
        let (Some(s), Some(a)) = (won(&speed, display), won(&auto, display)) else {
            return Ok(());
        };
        let game = u64::from(board.refresh_mhz);
        // A rate a whole-kHz clock gives is a few millihertz off; a panel
        // groups its rates in steps of 0.1 %.
        let slack = if is_panel(display) {
            1_000
        } else {
            REFRESH_MATCH_MHZ * 1_000_000 / game + 1
        };
        if speed_ppm(s.proposal) > speed_ppm(a.proposal) + slack {
            return Err(format!(
                "speed {} ppm ({}), auto {} ppm ({})",
                speed_ppm(s.proposal),
                s.considered,
                speed_ppm(a.proposal),
                a.considered
            ));
        }
        Ok(())
    });
}

/// DOS 640x400 at 70 Hz on a 15 kHz tube: `auto` shows the lines 1:1 at
/// 64.29 Hz (-8.28 %), `speed` the fit at 65 Hz (-7.26 %).
#[test]
fn speed_takes_the_fit_at_the_nearest_rate_over_the_slower_exact_rung() {
    let display = Display {
        profile: preset("generic_15"),
        caps: open_caps(),
    };
    let dos = source(640, 400, 70_086);
    let base = PolicySettings::default();
    let auto = decide_with(&dos, PictureStyle::Auto, base, &display);
    let a = won(&auto, &display).unwrap();
    assert_eq!(a.considered.rung, Rung::Interlaced);
    let speed = decide_with(&dos, PictureStyle::Speed, base, &display);
    let s = won(&speed, &display).unwrap();
    assert_eq!(s.considered.rung, Rung::Fitted);
    assert!(s.proposal.timing.field_rate_mhz.abs_diff(65_000) < 10);
    assert!(speed_ppm(s.proposal) < speed_ppm(a.proposal));
}

/// `speed` takes no frame shown two times that `auto` does not take: a
/// doubled rate comes only from the refresh choice or from a source below
/// every range, as in `auto`.
#[test]
fn speed_takes_no_doubled_frames_that_auto_does_not_take() {
    every_cell(|board, display| {
        let base = arcade_table::settings_of(board);
        let speed = decide_with(&board.source(), PictureStyle::Speed, base, display);
        let auto = decide_with(&board.source(), PictureStyle::Auto, base, display);
        let (Some(s), Some(a)) = (won(&speed, display), won(&auto, display)) else {
            return Ok(());
        };
        if s.proposal.timing.repeat() == 2 && a.proposal.timing.repeat() != 2 {
            return Err(format!("speed doubles the frames: {}", s.considered));
        }
        Ok(())
    });
}

/// The most progressive lines a range of `profile` holds at `rate_mhz`, as
/// the policy counts them; 0 when no range holds progressive lines there.
fn progressive_capacity(profile: &neptron_types::MonitorProfile, rate_mhz: u64) -> u32 {
    let MonitorKind::Crt { ranges } = &profile.kind else {
        return 0;
    };
    ranges
        .iter()
        .filter(|r| {
            r.progressive_lines_min > 0 && r.v_min_mhz <= rate_mhz && rate_mhz <= r.v_max_mhz
        })
        .map(|r| {
            let most =
                arcade_table::field_lines(r, rate_mhz).min(u32::from(r.progressive_lines_max));
            (u32::from(r.progressive_lines_min), most)
        })
        .filter(|&(fewest, most)| fewest <= most)
        .map(|(_, most)| most)
        .max()
        .unwrap_or(0)
}

/// Under `fit` a progressive game gets no interlaced mode where the
/// monitor holds progressive lines at the game's rate or at the rate of
/// the winner, and no small exact picture. The only interlaced fit is on
/// a CRT that holds no progressive lines at either rate.
#[test]
fn fit_gives_a_progressive_game_no_interlaced_and_no_small_exact_picture() {
    every_cell(|board, display| {
        if board.interlaced || is_panel(display) {
            return Ok(());
        }
        let base = arcade_table::settings_of(board);
        let plan = decide_with(&board.source(), PictureStyle::Fit, base, display);
        let Some(w) = won(&plan, display) else {
            return Ok(());
        };
        let capacity = progressive_capacity(&display.profile, u64::from(board.refresh_mhz)).max(
            progressive_capacity(&display.profile, w.proposal.timing.field_rate_mhz),
        );
        let tv = matches!(display.profile.kind, MonitorKind::Tv { .. });
        if w.proposal.mode.interlace && (capacity > 0 || tv) {
            return Err(format!(
                "{} is interlaced, {capacity} progressive lines exist",
                w.considered
            ));
        }
        if w.considered.rung.is_fitted() {
            return Ok(());
        }
        let fill = w.proposal.padding.map_or(1_000, |p| p.fill_permille());
        if u64::from(fill) * 100 < SMALL_PICTURE_MIN_PERCENT * 1_000 {
            return Err(format!("{} fills {fill} per mille", w.considered));
        }
        Ok(())
    });
}

/// On a flat panel a fill below the scale of 4 is soft: the notice names
/// the picture mode `fit`, or the expert key `display.vertical = fit` for
/// a turned game under `auto`. A fill from 4 is the panel rule, with no
/// notice of its own.
#[test]
fn a_panel_fill_below_the_scale_of_4_names_its_reason_in_the_notice() {
    let display = Display {
        profile: panel_1080(),
        caps: open_caps(),
    };
    let soft_of = |source: &SourceGeometry, settings: PolicySettings| {
        let plan = decide(source, &settings, &display.profile, &display.caps).unwrap();
        let w = won(&plan, &display).unwrap();
        let soft = notice(&plan, w.considered).and_then(|n| n.soft);
        (w.considered.rung, w.proposal.placement.dest.h, soft)
    };
    let fit = PolicySettings {
        style: PictureStyle::Fit,
        ..PolicySettings::default()
    };
    // DoDonPachi upright: 1080 / 320 = 3.375.
    let ddp = SourceGeometry {
        rotation: Rotation::R90,
        ..source(320, 240, 57_550)
    };
    assert_eq!(
        soft_of(&ddp, fit),
        (Rung::Panel, 1_080, Some(SoftReason::FillMode))
    );
    let vertical_fit = PolicySettings {
        vertical: VerticalMode::Fit,
        ..PolicySettings::default()
    };
    assert_eq!(
        soft_of(&ddp, vertical_fit),
        (Rung::Panel, 1_080, Some(SoftReason::Chosen))
    );
    // auto with the default keys: 3x, exact, no soft reason.
    assert_eq!(
        soft_of(&ddp, PolicySettings::default()),
        (Rung::Panel, 960, None)
    );
    // The NES: 1080 / 240 = 4.5, the panel rule fills with no notice.
    let nes = source(256, 240, 60_099);
    assert_eq!(soft_of(&nes, fit).2, None);
    assert_eq!(soft_of(&nes, PolicySettings::default()).2, None);
}

#[test]
fn fit_keeps_the_interlaced_mode_of_an_interlaced_game() {
    every_cell(|board, display| {
        if !board.interlaced || is_panel(display) {
            return Ok(());
        }
        let base = arcade_table::settings_of(board);
        let auto = decide_with(&board.source(), PictureStyle::Auto, base, display);
        let fit = decide_with(&board.source(), PictureStyle::Fit, base, display);
        let pick = |plan: &Plan| won(plan, display).map(|w| (w.considered.clone(), *w.proposal));
        if pick(&auto) != pick(&fit) {
            return Err("fit changes the decision of an interlaced game".to_owned());
        }
        Ok(())
    });
}

#[test]
fn on_a_panel_fit_fills_and_sharp_takes_the_largest_whole_multiple() {
    for display in arcade_table::panels() {
        for board in sources() {
            let base = arcade_table::settings_of(board);
            let source = board.source();
            let fit = decide_with(&source, PictureStyle::Fit, base, &display);
            let w = won(&fit, &display).unwrap();
            let (mode, dest) = (&w.proposal.mode, &w.proposal.placement.dest);
            assert!(
                dest.h == u32::from(mode.vactive) || dest.w == u32::from(mode.hactive),
                "{} on {}: fit {dest:?} in {}x{}",
                board.name,
                display.profile.id,
                mode.hactive,
                mode.vactive
            );
            let sharp = decide_with(&source, PictureStyle::Sharp, base, &display);
            let w = won(&sharp, &display).unwrap();
            let lines = if matches!(source.rotation, Rotation::R90 | Rotation::R270) {
                source.width
            } else {
                source.height
            };
            if lines <= w.proposal.mode.vactive {
                assert!(
                    w.proposal.placement.scale.y.is_some(),
                    "{} on {}: sharp {}",
                    board.name,
                    display.profile.id,
                    w.considered
                );
            }
        }
    }
}

#[test]
fn on_a_panel_native_and_a_multiple_give_their_factor_or_the_largest_smaller_one() {
    let display = Display {
        profile: panel_1080(),
        caps: open_caps(),
    };
    let base = PolicySettings::default();
    // 256x224 at 4:3: 1080 lines hold four times 224, and 4.8 times.
    let snes = source(256, 224, 60_099);
    for (style, factor, rung) in [
        (PictureStyle::Native, 1, Rung::Native),
        (PictureStyle::X2, 2, Rung::Multiple(2)),
        (PictureStyle::X3, 3, Rung::Multiple(3)),
        (PictureStyle::X4, 4, Rung::Multiple(4)),
    ] {
        let plan = decide_with(&snes, style, base, &display);
        let w = won(&plan, &display).unwrap();
        assert_eq!(w.considered.rung, rung, "{style}");
        assert_eq!(w.proposal.placement.scale.y, Some(factor), "{style}");
        assert_eq!(w.proposal.placement.dest.h, 224 * u32::from(factor));
        let n = notice(&plan, w.considered);
        assert!(
            !n.is_some_and(|n| n.style_fallback),
            "{style}: its own rung"
        );
    }
    // 320x400 holds only twice in 1080 lines: 4x falls to 2x, with the
    // notice of a fall.
    let tall = source(320, 400, 60_000);
    let plan = decide_with(&tall, PictureStyle::X4, base, &display);
    let w = won(&plan, &display).unwrap();
    assert_eq!(w.considered.rung, Rung::Multiple(2));
    assert!(notice(&plan, w.considered).is_some_and(|n| n.style_fallback));
    // A game larger than the panel is fitted.
    let huge = source(640, 1_200, 60_000);
    let plan = decide_with(&huge, PictureStyle::Native, base, &display);
    let w = won(&plan, &display).unwrap();
    assert_eq!(w.considered.rung, Rung::Fitted);
}

/// The `speed` style on a panel takes the mode nearest the game's rate:
/// a 50 Hz game on a panel whose preferred size has only 60 Hz and whose
/// other size has 50 Hz.
#[test]
fn on_a_panel_speed_takes_the_mode_nearest_the_game_rate() {
    let mut profile = panel_1080();
    let MonitorKind::FlatPanel { modes } = &mut profile.kind else {
        unreachable!("panel_1080 is a panel");
    };
    // The preferred 1080p 60 Hz, and 720p at 50 Hz in place of 1080p 50 Hz.
    let p720_50 = neptron_types::Modeline {
        htotal: 1_980,
        hsync_start: 1_720,
        hsync_end: 1_760,
        ..modes[2]
    };
    modes.truncate(1);
    modes.push(p720_50);
    let display = Display {
        profile,
        caps: open_caps(),
    };
    let pal = source(320, 240, 50_000);
    let base = PolicySettings::default();
    let rate = |style| {
        let plan = decide_with(&pal, style, base, &display);
        won(&plan, &display).unwrap().proposal.timing.field_rate_mhz
    };
    assert_eq!(
        rate(PictureStyle::Auto),
        60_000,
        "auto keeps the preferred size"
    );
    assert_eq!(rate(PictureStyle::Speed), 50_000);
}

/// A turned game: `sharp` reads the expert key `display.vertical = fit`
/// as exact. The `fit` mode fills the screen height at least as much as
/// `auto` with the key (it also fills the progressive lines above the
/// game's own and tries the window rate; `auto` with the key decides as
/// before the picture modes,
/// `auto_with_vertical_fit_decides_as_before_the_picture_modes`). A fitted
/// winner of the mode names why it is soft, never the key, and a fit of
/// the key never names the mode.
#[test]
fn fit_and_sharp_override_the_expert_key_of_vertical_games() {
    every_cell(|board, display| {
        let source = board.source();
        let turned = matches!(source.rotation, Rotation::R90 | Rotation::R270);
        if !turned || board.interlaced {
            return Ok(());
        }
        let base = arcade_table::settings_of(board);
        let vertical_fit = PolicySettings {
            vertical: VerticalMode::Fit,
            ..base
        };
        let pick = |plan: &Plan| won(plan, display).map(|w| (w.considered.clone(), *w.proposal));
        let old = decide_with(&source, PictureStyle::Auto, vertical_fit, display);
        let fit = decide_with(&source, PictureStyle::Fit, base, display);
        let sharp_fit = decide_with(&source, PictureStyle::Sharp, vertical_fit, display);
        let sharp = decide_with(&source, PictureStyle::Sharp, base, display);
        if pick(&sharp_fit) != pick(&sharp) {
            return Err("sharp reads display.vertical".to_owned());
        }
        let soft = |plan: &Plan| {
            won(plan, display)
                .and_then(|w| notice(plan, w.considered))
                .and_then(|n| n.soft)
        };
        // The share of the screen height that the game fills, in per mille.
        let fill = |plan: &Plan| {
            won(plan, display).map(|w| {
                u64::from(w.proposal.placement.dest.h) * 1_000
                    / u64::from(w.proposal.mode.vactive).max(1)
            })
        };
        if fill(&fit) < fill(&old) {
            return Err(format!(
                "fit fills less than the key: {:?} < {:?}",
                fill(&fit),
                fill(&old)
            ));
        }
        let fit_fitted = won(&fit, display).is_some_and(|w| w.considered.rung.is_fitted());
        // A fit of the mode says why it is soft: the mode, or lines that
        // no exact mode holds; never the key.
        if fit_fitted && matches!(soft(&fit), None | Some(SoftReason::Chosen)) {
            return Err(format!("a fit of the mode is soft for {:?}", soft(&fit)));
        }
        if soft(&old) == Some(SoftReason::FillMode) {
            return Err("a fit of the key names the mode".to_owned());
        }
        Ok(())
    });
}

/// The rows of the picture mode list: every mode, in the order of the
/// schema. On a TV `speed` is greyed; `sharp` is greyed where only a fit
/// validates.
#[test]
fn the_mode_rows_grey_speed_on_a_tv_and_sharp_where_only_a_fit_exists() {
    let tv = arcade_table::displays()
        .into_iter()
        .find(|d| d.profile.id == "tv_ntsc")
        .unwrap();
    let snes = source(256, 224, 60_099);
    let rows = picture_choices(
        &snes,
        &PolicySettings::default(),
        neptron_modeline::policy::ScanlineChoice {
            mode: neptron_types::ScanlineMode::Auto,
            strength_percent: 75,
        },
        &tv.profile,
        &tv.caps,
        |m| validate(m, &tv.profile, &tv.caps).map(|_| ()),
    )
    .unwrap()
    .modes;
    let styles: Vec<_> = rows.iter().map(|(style, _)| *style).collect();
    assert_eq!(styles, PictureStyle::ALL);
    let speed = rows
        .iter()
        .find(|(style, _)| *style == PictureStyle::Speed)
        .unwrap();
    assert!(matches!(speed.1.answer.offer(), Err(d) if d.reason == DeclineReason::OneRateOnTv));
    // 480 upright lines on a 31 kHz tube with no interlace: only the fit.
    let arcade_31 = Display {
        profile: preset("arcade_31"),
        caps: open_caps(),
    };
    let tall = SourceGeometry {
        rotation: Rotation::R90,
        ..source(640, 480, 60_000)
    };
    let rows = picture_choices(
        &tall,
        &PolicySettings::default(),
        neptron_modeline::policy::ScanlineChoice {
            mode: neptron_types::ScanlineMode::Auto,
            strength_percent: 75,
        },
        &arcade_31.profile,
        &arcade_31.caps,
        |m| validate(m, &arcade_31.profile, &arcade_31.caps).map(|_| ()),
    )
    .unwrap()
    .modes;
    let available = |style| {
        rows.iter()
            .find(|(s, _)| *s == style)
            .unwrap()
            .1
            .answer
            .offer()
            .is_ok()
    };
    assert!(available(PictureStyle::Auto));
    assert!(!available(PictureStyle::Sharp));
    assert!(available(PictureStyle::Speed));
    assert!(available(PictureStyle::Fit));
}

/// The speed sentence of the notice follows the one alert threshold
/// (`SPEED_ALERT_MIN_PPM`, 0.5 % of the shown rate) on a CRT too: the NES
/// at 59.94 Hz (0.26 %) has none, the WonderSwan at 65 Hz (-13.9 %) has
/// it, and `sharp` has it where its exact rung is slower than the fit.
#[test]
fn the_speed_notice_starts_above_the_alert_threshold() {
    use neptron_modeline::policy::{SPEED_ALERT_MIN_PPM, speed_alert};
    assert_eq!(SPEED_ALERT_MIN_PPM, 5_000);
    assert!(!speed_alert(60_099, 59_940));
    assert!(!speed_alert(60_000 - 300, 60_000), "at the threshold");
    assert!(speed_alert(60_000 - 302, 60_000));
    let speed_in = |source: SourceGeometry, style, profile: &str| {
        let display = Display {
            profile: preset(profile),
            caps: open_caps(),
        };
        let plan = decide_with(&source, style, PolicySettings::default(), &display);
        let w = won(&plan, &display).expect("a winner");
        notice(&plan, w.considered).is_some_and(|n| {
            n.reasons()
                .any(|r| matches!(r, NoticeReason::SpeedChange { .. }))
        })
    };
    assert!(!speed_in(
        source(256, 240, 60_099),
        PictureStyle::Auto,
        "ntsc"
    ));
    let swan = SourceGeometry {
        aspect_milli: 1_556,
        ..source(224, 144, 75_472)
    };
    assert!(speed_in(swan, PictureStyle::Auto, "generic_15"));
    assert!(speed_in(swan, PictureStyle::Speed, "generic_15"));
    assert!(speed_in(
        source(640, 500, 60_000),
        PictureStyle::Sharp,
        "generic_15"
    ));
}
