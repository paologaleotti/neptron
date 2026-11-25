//! The policy on a screen turned on its side (docs/spec/tate.md): the
//! reference boards as display sources at a screen orientation of 90 and
//! 270, and the menu as the raster of its turned canvas
//! (docs/spec/ui-layout.md, section 6.3), in tests/golden/policy_tate.toml.
//! The policy does not change; the display source carries the turn `R`.

use std::collections::BTreeMap;

use neptron_modeline::policy::{PolicySettings, choose, decide, ui_placement};
use neptron_safety::validate;
use neptron_types::{Filter, Form, Orientation, Rotation, SourceGeometry};

use crate::arcade_table::{self, BOARDS, Display};
use crate::policy_golden::{compare, expected, outcome_with};

/// One display of each class, as the spec names them.
const TATE_DISPLAYS: [&str; 7] = [
    "generic_15",
    "arcade_25",
    "arcade_31",
    "d9800",
    "tv_ntsc",
    "tv_pal",
    "panel_1080",
];

/// The two turned screens.
const TURNED_SCREENS: [Rotation; 2] = [Rotation::R90, Rotation::R270];

/// The source of the menu on a screen at 0: the raster of the 320x240
/// canvas at 60 Hz, 4:3, not turned.
const MENU_SOURCE: SourceGeometry = SourceGeometry {
    width: 320,
    height: 240,
    refresh_mhz: 60_000,
    aspect_milli: 1_333,
    interlaced: false,
    rotation: Rotation::R0,
};

fn displays() -> Vec<Display> {
    let displays: Vec<_> = arcade_table::displays()
        .into_iter()
        .filter(|d| TATE_DISPLAYS.contains(&d.profile.id.as_str()))
        .collect();
    assert_eq!(displays.len(), TATE_DISPLAYS.len());
    displays
}

fn screen(screen: Rotation) -> Orientation {
    Orientation {
        screen,
        game_override: None,
    }
}

/// The canvas of `T`'s size `canvas` in the winner's mode:
/// " ui WxH+X+Y filter", or " ui none" when no proposal validates.
fn placed_ui(
    source: &SourceGeometry,
    canvas: (u16, u16),
    settings: &PolicySettings,
    display: &Display,
) -> String {
    let (profile, caps) = (&display.profile, &display.caps);
    let plan = decide(source, settings, profile, caps).unwrap();
    let choice = choose(&plan, |m| validate(m, profile, caps));
    let Some(winner) = choice.winner else {
        return " ui none".into();
    };
    let ui = ui_placement(&profile.kind, &winner.proposal.mode, canvas);
    let d = ui.dest;
    format!(
        " ui {}x{}+{}+{} {}",
        d.w,
        d.h,
        d.x,
        d.y,
        match ui.filter {
            Filter::Nearest => "nearest",
            Filter::SharpBilinear => "sharp",
        }
    )
}

/// The menu on a turned screen, worked out from the rules of the spec and
/// not from the engine: the portrait canvas turned by `U = (0 - S) mod
/// 360` across the lines is `T`, and the policy plans the raster of `T`,
/// progressive and not turned, at the rate of the menu, with the default
/// settings. The value is the outcome, then the place of `T`.
fn turned_menu(display: &Display) -> String {
    let (w, h) = Form::Portrait.canvas();
    let t = (h, w);
    let source = SourceGeometry {
        width: t.0,
        height: t.1,
        ..MENU_SOURCE
    };
    let settings = PolicySettings::default();
    let (profile, caps) = (&display.profile, &display.caps);
    outcome_with(&source, &settings, profile, caps) + &placed_ui(&source, t, &settings, display)
}

/// The menu on a screen at 0: the landscape canvas, upright.
fn upright_menu(display: &Display) -> String {
    let settings = PolicySettings::default();
    let source = Orientation::NORMAL.display_source(&MENU_SOURCE);
    let (profile, caps) = (&display.profile, &display.caps);
    outcome_with(&source, &settings, profile, caps)
        + &placed_ui(&source, Form::Landscape.canvas(), &settings, display)
}

/// Each reference board as the display source on each turned screen,
/// "<board> <display> s<S>", and the menu with its turned canvas,
/// "menu <display> s<S>".
#[test]
fn the_display_sources_of_a_turned_screen_give_their_golden_decisions() {
    let displays = displays();
    let mut actual = BTreeMap::new();
    for s in TURNED_SCREENS {
        let o = screen(s);
        for display in &displays {
            let (profile, caps) = (&display.profile, &display.caps);
            for board in &BOARDS {
                let key = format!("{} {} s{}", board.name, profile.id, s.degrees());
                let source = o.display_source(&board.source());
                actual.insert(
                    key,
                    outcome_with(&source, &arcade_table::settings(), profile, caps),
                );
            }
            actual.insert(
                format!("menu {} s{}", profile.id, s.degrees()),
                turned_menu(display),
            );
        }
    }
    compare("policy_tate.toml", actual);
}

/// The claim of docs/spec/ui-layout.md, section 6.3: on a CRT, a TV and a
/// landscape panel the menu of a turned screen has the mode of the menu at
/// 0, and the canvas the place of the menu at 0. Each recorded menu case
/// equals the menu on a screen at 0.
#[test]
fn the_menu_of_a_turned_screen_is_the_menu_of_a_screen_at_0() {
    let recorded = expected("policy_tate.toml");
    for display in &displays() {
        let at_0 = upright_menu(display);
        for s in TURNED_SCREENS {
            let key = format!("menu {} s{}", display.profile.id, s.degrees());
            assert_eq!(recorded.get(&key), Some(&at_0), "{key}");
        }
    }
}

/// A vertical board on a turned screen is a source of its own lines: it
/// gives the decision of the same raster at `R0` (at 90) or `R180` (at
/// 270), which the policy places as it places a horizontal board.
#[test]
fn a_vertical_board_on_a_turned_screen_decides_as_its_raster_not_turned() {
    let settings = arcade_table::settings();
    for display in &displays() {
        let (profile, caps) = (&display.profile, &display.caps);
        for board in BOARDS.iter().filter(|b| b.rotation.across()) {
            for s in TURNED_SCREENS {
                let turned = screen(s).display_source(&board.source());
                assert!(!turned.rotation.across(), "{} at {s:?}", board.name);
                let upright = SourceGeometry {
                    rotation: Rotation::R0,
                    ..board.source()
                };
                assert_eq!(
                    outcome_with(&turned, &settings, profile, caps),
                    outcome_with(&upright, &settings, profile, caps),
                    "{} on {} at {s:?}",
                    board.name,
                    profile.id
                );
            }
        }
    }
}

/// DoDonPachi on a screen at 90 is 240 lines at its own 57.55 Hz on a
/// 15 kHz tube, the decision for Power Instinct 2, a horizontal Cave board
/// of the same size and rate. On a screen at 0 the display source is the
/// source, and the decision is the arcade golden of today.
#[test]
fn dodonpachi_on_a_screen_at_90_runs_at_240_lines_like_a_horizontal_board() {
    let board = |name: &str| BOARDS.iter().find(|b| b.name == name).unwrap().source();
    let (ddonpach, pwrinst2) = (board("ddonpach"), board("pwrinst2"));
    let settings = arcade_table::settings();
    let recorded = expected("policy_arcade.toml");
    for display in &displays() {
        let (profile, caps) = (&display.profile, &display.caps);
        let turned = screen(Rotation::R90).display_source(&ddonpach);
        let got = outcome_with(&turned, &settings, profile, caps);
        assert_eq!(
            got,
            outcome_with(&pwrinst2, &settings, profile, caps),
            "{}",
            profile.id
        );
        if profile.id == "generic_15" {
            assert!(got.contains("x240p ") && got.contains(" 57.55"), "{got}");
        }
        let normal = Orientation::NORMAL.display_source(&ddonpach);
        assert_eq!(normal, ddonpach);
        if let Some(want) = recorded.get(&format!("ddonpach {}", profile.id)) {
            assert_eq!(
                &outcome_with(&normal, &settings, profile, caps),
                want,
                "{}",
                profile.id
            );
        }
    }
}
