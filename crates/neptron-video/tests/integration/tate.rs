//! The engine on a screen turned on its side (docs/spec/tate.md).
//!
//! The expected values are worked out in the tests from the rules of the
//! spec, not from the functions of `Orientation`: the turn of the game is
//! `R = (G' - S) mod 360` in degrees, the turn of the canvas is
//! `U = (0 - S) mod 360`, the policy plans the frame as drawn with the
//! turn `R` (the mode and the placement of a normal engine for that
//! source), and a canvas turned across the lines goes where
//! `ui_dest_turned` places it in the mode on screen.

use neptron_modeline::policy::ui_dest_turned;
use neptron_testkit::SceneSummary;
use neptron_types::{
    Filter, Frame, Modeline, MonitorKind, Orientation, PixelFormat, Rect, Rotation, SourceGeometry,
};
use neptron_video::{Content, Directive, Monitor, PolicySettings, TURNED_MENU_MODE};

use crate::guard_rig::{FRAME_NS, GuardRig, crt_15, crt_31};
use crate::harness::{Rig, SNES, frame, generic_15};
use crate::panel::{mode, panel};

/// DoDonPachi: a 320x240 frame on its side at 57.55 Hz, `R90`.
pub const DDONPACH: SourceGeometry = SourceGeometry {
    width: 320,
    height: 240,
    refresh_mhz: 57_550,
    aspect_milli: 1_333,
    interlaced: false,
    rotation: Rotation::R90,
};

/// Street Fighter II: a horizontal board, 384x224 at 59.637 Hz, `R0`.
pub const SF2: SourceGeometry = SourceGeometry {
    width: 384,
    height: 224,
    refresh_mhz: 59_637,
    aspect_milli: 1_333,
    interlaced: false,
    rotation: Rotation::R0,
};

/// A white 320x240 game frame in RGB565.
static WHITE: [u8; 320 * 240 * 2] = [0xFF; 320 * 240 * 2];

/// The 320x240 canvas of the UI.
static CANVAS: [u8; 320 * 240 * 2] = [0; 320 * 240 * 2];

pub fn canvas() -> Frame<'static> {
    Frame {
        data: &CANVAS,
        width: 320,
        height: 240,
        pitch: 640,
        format: PixelFormat::Rgb565,
    }
}

pub fn at(screen: Rotation, game_override: Option<Rotation>) -> Orientation {
    Orientation {
        screen,
        game_override,
    }
}

fn turn(degrees: u16) -> Rotation {
    Rotation::from_degrees(degrees % 360).unwrap()
}

/// `R = (G' - S) mod 360`.
fn game_turn(screen: Rotation, game: Rotation) -> Rotation {
    turn(game.degrees() + 360 - screen.degrees())
}

/// `U = (0 - S) mod 360`.
fn ui_turn(screen: Rotation) -> Rotation {
    turn(360 - screen.degrees())
}

fn monitors() -> Vec<(&'static str, Monitor)> {
    vec![
        ("generic_15", generic_15()),
        ("pc_31_120", crate::guard_rig::pc_31_120()),
        ("panel_1080", panel(vec![mode(1_920, 1_080, 60)])),
    ]
}

/// One core frame of `source` with the canvas over it: begin, present,
/// wait.
pub fn frame_with_ui(rig: &mut Rig, source: SourceGeometry) -> Directive {
    let directive = rig.engine.begin_frame(source);
    rig.engine
        .present(Some(frame(&source)), Some(canvas()))
        .unwrap();
    rig.engine.wait_flip().unwrap();
    directive
}

/// The scene of the last present.
pub fn last_scene(rig: &Rig) -> SceneSummary {
    rig.fake.frames().last().unwrap().scene
}

/// The mode on screen and the scene that `source` shows with the canvas
/// over it, after one frame on a new engine in `orientation`.
fn shown(
    monitor: Monitor,
    orientation: Orientation,
    source: SourceGeometry,
) -> (Modeline, SceneSummary) {
    let mut rig = Rig::new(monitor, PolicySettings::default());
    rig.engine.set_orientation(orientation);
    frame_with_ui(&mut rig, source);
    (rig.engine.current().unwrap().read_back, last_scene(&rig))
}

/// The destination and the filter of the canvas turned by `ui` in `mode`
/// on `kind`, with the upright destination `upright` of the placement.
fn expected_ui(kind: &MonitorKind, mode: &Modeline, ui: Rotation, upright: Rect) -> (Rect, Filter) {
    if ui.across() {
        let placed = ui_dest_turned(
            kind,
            mode,
            (240, 320),
            PolicySettings::default().solver.monitor_aspect,
        );
        (placed.dest, placed.filter)
    } else {
        (upright, Filter::Nearest)
    }
}

/// For each screen orientation, a vertical board, a horizontal board and
/// a console game, at Auto and with a fixed override: the mode and the
/// game picture are those of the frame as drawn with the turn `R`, and the
/// canvas is upright for the player.
#[test]
fn every_screen_orientation_turns_the_game_and_the_canvas_as_the_spec_says() {
    for (name, monitor) in monitors() {
        let kind = monitor.profile().kind.clone();
        for screen in Rotation::ALL {
            for source in [DDONPACH, SF2, SNES] {
                for game_override in [None, Some(Rotation::R270)] {
                    let case =
                        format!("{name} S={} {source:?} {game_override:?}", screen.degrees());
                    let r = game_turn(screen, game_override.unwrap_or(source.rotation));
                    let (mode, scene) = shown(monitor.clone(), at(screen, game_override), source);
                    let drawn = SourceGeometry {
                        rotation: r,
                        ..source
                    };
                    let (want_mode, want) = shown(monitor.clone(), Orientation::NORMAL, drawn);
                    assert_eq!(mode, want_mode, "{case}: the mode of the frame turned by R");
                    assert_eq!(scene.rotation, r, "{case}");
                    assert_eq!(scene.crop, want.crop, "{case}");
                    assert_eq!(scene.dest, want.dest, "{case}");
                    assert_eq!(scene.filter, want.filter, "{case}");
                    let u = ui_turn(screen);
                    assert_eq!(scene.ui_rotation, u, "{case}");
                    let (ui_dest, ui_filter) = expected_ui(&kind, &mode, u, want.ui_dest);
                    assert_eq!(scene.ui_dest, ui_dest, "{case}");
                    assert_eq!(scene.ui_filter, ui_filter, "{case}");
                }
            }
        }
    }
}

/// At a screen orientation of 0 with Auto, the engine is the engine of
/// today: the same modes and the same scenes as an engine that never had
/// an orientation set.
#[test]
fn the_normal_orientation_gives_the_scenes_of_an_engine_with_none() {
    for (name, monitor) in monitors() {
        for source in [DDONPACH, SF2, SNES] {
            let mut plain = Rig::new(monitor.clone(), PolicySettings::default());
            frame_with_ui(&mut plain, source);
            let (mode, scene) = shown(monitor.clone(), Orientation::NORMAL, source);
            assert_eq!(mode, plain.engine.current().unwrap().read_back, "{name}");
            assert_eq!(scene, last_scene(&plain), "{name} {source:?}");
            assert_eq!(scene.rotation, source.rotation);
            assert_eq!(
                (scene.ui_rotation, scene.ui_filter),
                (Rotation::R0, Filter::Nearest)
            );
        }
    }
}

/// DoDonPachi on a 15 kHz tube turned to the left: its own 240 lines,
/// progressive, at its own 57.55 Hz, not turned, over the whole height.
#[test]
fn dodonpachi_on_a_turned_15khz_tube_runs_240p_at_its_own_rate_full_screen() {
    let (mode, scene) = shown(generic_15(), at(Rotation::R90, None), DDONPACH);
    assert_eq!(mode.vactive, 240);
    assert!(!mode.interlace);
    assert!(mode.field_rate_mhz().abs_diff(57_551) <= 1, "{mode:?}");
    assert_eq!(scene.rotation, Rotation::R0);
    assert_eq!(
        scene.crop,
        Rect {
            x: 0,
            y: 0,
            w: 320,
            h: 240
        }
    );
    assert_eq!(
        (scene.dest.y, scene.dest.h),
        (0, 240),
        "every line of the mode"
    );
    assert_eq!(scene.filter, Filter::Nearest);
    // At 0 the same board stands across the lines: interlaced, as today.
    let (normal, _) = shown(generic_15(), Orientation::NORMAL, DDONPACH);
    assert!(normal.interlace, "{normal:?}");
}

/// The quick menu over DoDonPachi at 240 lines on a turned 15 kHz tube:
/// the mode stays the game's, and the canvas is squeezed into the 240
/// lines with sharp bilinear, in the middle of the game picture, which
/// shows at both sides of it.
#[test]
fn the_quick_menu_over_a_game_on_a_turned_screen_keeps_the_game_mode() {
    let mut rig = Rig::new(generic_15(), PolicySettings::default());
    rig.engine.set_orientation(at(Rotation::R90, None));
    rig.frame(DDONPACH);
    let game_mode = rig.current_mode();
    let calls = rig.fake.mode_calls().len();
    for _ in 0..5 {
        assert_eq!(frame_with_ui(&mut rig, DDONPACH), Directive::Steady);
    }
    assert_eq!(rig.fake.mode_calls().len(), calls, "no modeset for the UI");
    assert_eq!(rig.current_mode(), game_mode);
    let scene = last_scene(&rig);
    assert_eq!(scene.rotation, Rotation::R0);
    assert_eq!(scene.ui_rotation, Rotation::R270);
    assert_eq!(scene.ui_filter, Filter::SharpBilinear);
    assert_eq!((scene.ui_dest.y, scene.ui_dest.h), (0, 240));
    let (ui, game) = (scene.ui_dest, scene.dest);
    assert!(ui.w < game.w, "the canvas is narrower than the game");
    let right = ui.x + i32::try_from(ui.w).unwrap();
    assert!(
        ui.x > game.x && right < game.x + i32::try_from(game.w).unwrap(),
        "the game shows at both sides of the canvas: {ui:?} in {game:?}"
    );

    // A white game under the black canvas: the game shows only outside it.
    let white = Frame {
        data: &WHITE,
        ..frame(&DDONPACH)
    };
    rig.engine.begin_frame(DDONPACH);
    rig.engine.present(Some(white), Some(canvas())).unwrap();
    rig.engine.wait_flip().unwrap();
    let screen = rig.fake.screen().unwrap();
    let x = |x: i32| u32::try_from(x).unwrap();
    let black = [0, 0, 0];
    assert_ne!(screen.pixel(x(ui.x - 1), 120), Some(black), "game, left");
    assert_eq!(screen.pixel(x(ui.x), 120), Some(black), "canvas");
    assert_eq!(screen.pixel(x(right - 1), 120), Some(black), "canvas");
    assert_ne!(screen.pixel(x(right), 120), Some(black), "game, right");
}

/// A change of the screen orientation in a running game is a new source:
/// it waits for the debounce, then sets the mode of the new display
/// source. The answers are made again for the new display source, and the
/// old ones are never served.
#[test]
fn a_change_of_orientation_is_a_new_source_with_new_answers() {
    let mut rig = Rig::new(generic_15(), PolicySettings::default());
    rig.frame(DDONPACH);
    let interlaced = rig.current_mode();
    assert!(interlaced.interlace);
    let request = rig.engine.request_for(&DDONPACH);
    let before = rig.engine.answers_now(&request).unwrap();

    rig.engine.set_orientation(at(Rotation::R90, None));
    assert!(
        rig.engine.answers(&request).is_none(),
        "the answers of the old orientation are not served"
    );
    let mut directives = Vec::new();
    for _ in 0..3 {
        directives.push(rig.frame(DDONPACH).directive);
    }
    assert_eq!(
        directives,
        [
            Directive::Waiting { stable_frames: 1 },
            Directive::Waiting { stable_frames: 2 },
            Directive::Modeset,
        ]
    );
    let progressive = rig.current_mode();
    assert_eq!((progressive.vactive, progressive.interlace), (240, false));
    assert_eq!(rig.engine.current().unwrap().source.rotation, Rotation::R0);

    let after = rig.engine.answers_now(&request).unwrap();
    assert_ne!(after, before);
    let drawn = SourceGeometry {
        rotation: Rotation::R0,
        ..DDONPACH
    };
    let normal = Rig::new(generic_15(), PolicySettings::default());
    assert_eq!(after, normal.engine.picture_choices(&drawn).unwrap());
    assert_eq!(
        rig.engine.offer(&DDONPACH).unwrap(),
        normal.engine.offer(&drawn).unwrap()
    );

    // Back to 0: the mode of today again, and the answers of today.
    rig.engine.set_orientation(Orientation::NORMAL);
    for _ in 0..3 {
        rig.frame(DDONPACH);
    }
    assert_eq!(rig.current_mode(), interlaced);
    assert_eq!(rig.engine.answers_now(&request).unwrap(), before);
}

/// A change of the override in a running game is a new source too; an
/// override equal to the turn of the core changes nothing.
#[test]
fn a_change_of_the_override_is_a_new_source() {
    let mut rig = Rig::new(generic_15(), PolicySettings::default());
    rig.engine.set_orientation(at(Rotation::R90, None));
    rig.frame(DDONPACH);
    rig.engine
        .set_orientation(at(Rotation::R90, Some(Rotation::R90)));
    assert_eq!(rig.frame(DDONPACH).directive, Directive::Steady);
    rig.engine
        .set_orientation(at(Rotation::R90, Some(Rotation::R0)));
    assert_eq!(
        rig.frame(DDONPACH).directive,
        Directive::Waiting { stable_frames: 1 }
    );
    rig.frame(DDONPACH);
    rig.frame(DDONPACH);
    assert_eq!(
        rig.engine.current().unwrap().source.rotation,
        Rotation::R270
    );
    assert!(rig.current_mode().interlace, "across the lines of the tube");
}

/// The engine says when a horizontal game stands across a turned screen,
/// for the app's picture notice.
#[test]
fn the_engine_says_when_a_horizontal_game_is_on_a_turned_screen() {
    let mut rig = Rig::new(generic_15(), PolicySettings::default());
    assert!(!rig.engine.turns(&SF2).horizontal_on_turned_screen);
    rig.engine.set_orientation(at(Rotation::R270, None));
    assert!(rig.engine.turns(&SF2).horizontal_on_turned_screen);
    assert!(!rig.engine.turns(&DDONPACH).horizontal_on_turned_screen);
    rig.engine
        .set_orientation(at(Rotation::R270, Some(Rotation::R90)));
    assert!(!rig.engine.turns(&SF2).horizontal_on_turned_screen);
}

/// The menu engine reads no game override in the turns, as in its
/// answers: the turns are those of a game at Auto.
#[test]
fn the_turns_of_the_menu_engine_read_no_override() {
    let mut rig = Rig::new(generic_15(), PolicySettings::default());
    rig.engine.set_content(Content::Menu(TURNED_MENU_MODE));
    rig.engine
        .set_orientation(at(Rotation::R270, Some(Rotation::R90)));
    let turns = rig.engine.turns(&SF2);
    assert!(turns.horizontal_on_turned_screen);
    assert_eq!(turns.game, Rotation::R90, "R0 less 270");
}

/// Safe mode and the blind recovery change the monitor, not the
/// orientation: the safe default's picture is drawn with the same turns,
/// and every mode on the wire passes the validator (the rig checks it on
/// every frame).
#[test]
fn safe_mode_keeps_the_orientation() {
    let mut rig = GuardRig::confirmed_on(crt_31(), crt_15());
    rig.source = DDONPACH;
    rig.engine.set_orientation(at(Rotation::R90, None));
    rig.run_for(10 * FRAME_NS);
    let scene = rig.fake.frames().last().unwrap().scene;
    assert_eq!(
        (scene.rotation, scene.ui_rotation),
        (Rotation::R0, Rotation::R270)
    );
    rig.recover();
    rig.run_for(10 * FRAME_NS);
    assert_eq!(rig.engine.orientation(), at(Rotation::R90, None));
    let wire = rig.wire().unwrap();
    assert_eq!((wire.vactive, wire.interlace), (240, false), "{wire:?}");
    let scene = rig.fake.frames().last().unwrap().scene;
    assert_eq!(
        (scene.rotation, scene.ui_rotation),
        (Rotation::R0, Rotation::R270)
    );
}
