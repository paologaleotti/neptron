//! The engine on a screen turned on its side (docs/spec/tate.md).
//!
//! The expected values are worked out in the tests from the rules of the
//! spec, not from the functions of `Orientation`: the turn of the game is
//! `R = (G' - S) mod 360` in degrees, the turn of the canvas is
//! `U = (0 - S) mod 360`, the policy plans the frame as drawn with the
//! turn `R` (the mode and the placement of a normal engine for that
//! source), and the canvas of the form (portrait at 90 and 270) goes where
//! `ui_placement` places it after its turn in the mode on screen: 320x240
//! in scan-out at every turn on these landscape displays.

use neptron_modeline::policy::ui_placement;
use neptron_testkit::SceneSummary;
use neptron_types::{
    Filter, Frame, Modeline, MonitorKind, Orientation, PixelFormat, Rect, Rotation, SourceGeometry,
};
use neptron_video::{Content, Directive, Monitor, Outcome, PolicySettings};

use crate::faults::refuse_all;
use crate::guard_rig::{FRAME_NS, GuardRig, crt_15, crt_31};
use crate::harness::{Rig, SNES, frame, generic_15};
use crate::menu_turned::{MENU_SOURCE, canvas_of};
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
    let ui = canvas_of(rig.engine.form());
    rig.engine.present(Some(frame(&source)), Some(ui)).unwrap();
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

/// The destination and the filter of the canvas in `mode` on `kind`: the
/// canvas of the form after its turn is 320x240 in scan-out on a
/// landscape display, at every turn, and its filter follows its scale at
/// every turn.
fn expected_ui(kind: &MonitorKind, mode: &Modeline) -> (Rect, Filter) {
    let placed = ui_placement(kind, mode, (320, 240));
    (placed.dest, placed.filter)
}

/// The side of the red mark, in frame pixels: large enough that a
/// picture scaled down (a fit or a squeezed canvas) keeps it red at the
/// corner pixel of its destination.
const MARK: usize = 8;

/// A black frame with a red square of [`MARK`] pixels at its top left.
fn marked(width: u16, height: u16) -> Vec<u8> {
    let pitch = usize::from(width) * 2;
    let mut data = vec![0; pitch * usize::from(height)];
    for y in 0..MARK {
        for x in 0..MARK {
            // RGB565 red, 0xF800, little endian.
            data[y * pitch + x * 2 + 1] = 0xF8;
        }
    }
    data
}

fn frame_of(data: &[u8], width: u16, height: u16) -> Frame<'_> {
    Frame {
        data,
        width,
        height,
        pitch: usize::from(width) * 2,
        format: PixelFormat::Rgb565,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// The corner of `rect` in which the screen shows red; it fails when no
/// corner or more than one does.
fn red_corner(screen: &neptron_testkit::RgbImage, rect: Rect) -> Corner {
    let x = |v: i32| u32::try_from(v).unwrap();
    let (left, top) = (x(rect.x), x(rect.y));
    let (right, bottom) = (left + rect.w - 1, top + rect.h - 1);
    let red = |px, py| {
        let [r, g, b] = screen.pixel(px, py).unwrap();
        r > 200 && g < 60 && b < 60
    };
    let corners = [
        (Corner::TopLeft, red(left, top)),
        (Corner::TopRight, red(right, top)),
        (Corner::BottomLeft, red(left, bottom)),
        (Corner::BottomRight, red(right, bottom)),
    ];
    let found: Vec<Corner> = corners.iter().filter(|c| c.1).map(|c| c.0).collect();
    assert_eq!(found.len(), 1, "red corners of {rect:?}: {found:?}");
    found[0]
}

/// One frame of `source` with a mark at the top left of the game frame,
/// then one with a mark at the top left of the canvas alone: the corners
/// of `dest` and of `ui_dest` that show them.
fn marked_corners(screen: Rotation, source: SourceGeometry) -> (Corner, Corner) {
    let mut rig = Rig::new(generic_15(), PolicySettings::default());
    rig.engine.set_orientation(at(screen, None));
    let game = marked(source.width, source.height);
    let (ui_w, ui_h) = rig.engine.form().canvas();
    let ui = marked(ui_w, ui_h);
    for _ in 0..2 {
        rig.engine.begin_frame(source);
        rig.engine
            .present(Some(frame_of(&game, source.width, source.height)), None)
            .unwrap();
        rig.engine.wait_flip().unwrap();
    }
    let dest = last_scene(&rig).dest;
    let game_corner = red_corner(&rig.fake.screen().unwrap(), dest);
    rig.engine.begin_frame(source);
    rig.engine
        .present(None, Some(frame_of(&ui, ui_w, ui_h)))
        .unwrap();
    rig.engine.wait_flip().unwrap();
    let ui_dest = last_scene(&rig).ui_dest;
    (
        game_corner,
        red_corner(&rig.fake.screen().unwrap(), ui_dest),
    )
}

/// The corners are worked out by hand from what the player must see, not
/// from `Orientation`: the top left of an upright picture is at the
/// player's top left.
#[test]
fn a_corner_mark_lands_in_the_named_corner_of_the_scan_out() {
    // S=90: the display is turned a quarter turn to the left. Its top edge
    // is at the player's left and its right edge at the player's top, so
    // the player's top left is the display's top right. SF2 is drawn
    // upright, and so is the portrait canvas: both marks are at the top
    // right.
    assert_eq!(
        marked_corners(Rotation::R90, SF2),
        (Corner::TopRight, Corner::TopRight)
    );
    // DoDonPachi is drawn on its side and stands upright after a quarter
    // turn to the left. That turn takes the frame's top left to the
    // picture's bottom left, as the player sees it.
    // S=0: the display stands as usual, so the mark is at the bottom left.
    assert_eq!(marked_corners(Rotation::R0, DDONPACH).0, Corner::BottomLeft);
    // S=270: the display is turned a quarter turn to the right. Its right
    // edge is at the player's bottom and its bottom edge at the player's
    // left, so the player's bottom left is the display's bottom right.
    assert_eq!(
        marked_corners(Rotation::R270, DDONPACH).0,
        Corner::BottomRight
    );
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
                    let (ui_dest, ui_filter) = expected_ui(&kind, &mode);
                    assert_eq!(scene.ui_dest, ui_dest, "{case}");
                    assert_eq!(
                        scene.ui_dest, want.ui_dest,
                        "{case}: the place of the canvas at 0"
                    );
                    assert_eq!(scene.ui_filter, ui_filter, "{case}");
                }
            }
        }
    }
}

/// One frame of `source` as the engine of `content` shows it: a game
/// with the canvas over it, or the canvas alone for the menu.
fn content_frame(rig: &mut Rig, content: Content, source: SourceGeometry) {
    rig.engine.begin_frame(source);
    let game = (content == Content::Game).then(|| frame(&source));
    let ui = canvas_of(rig.engine.form());
    rig.engine.present(game, Some(ui)).unwrap();
    rig.engine.wait_flip().unwrap();
}

/// An engine turned to 90 and back to 0 with Auto is the engine of today:
/// the same mode, the same scene and the same answers as an engine that
/// never had an orientation set, for games and for the menu.
#[test]
fn the_normal_orientation_gives_the_scenes_of_an_engine_with_none() {
    let menu = (Content::Menu, MENU_SOURCE);
    for (name, monitor) in monitors() {
        let cases = [DDONPACH, SF2, SNES]
            .map(|s| (Content::Game, s))
            .into_iter()
            .chain([menu]);
        for (content, source) in cases {
            let case = format!("{name} {content:?} {source:?}");
            let mut plain = Rig::new(monitor.clone(), PolicySettings::default());
            content_frame(&mut plain, content, source);
            let mut rig = Rig::new(monitor.clone(), PolicySettings::default());
            rig.engine.set_content(content);
            rig.engine.set_orientation(at(Rotation::R90, None));
            for _ in 0..4 {
                content_frame(&mut rig, content, source);
            }
            let turned = last_scene(&rig);
            assert_eq!(turned.ui_rotation, Rotation::R270, "{case}");
            rig.engine.set_orientation(Orientation::NORMAL);
            for _ in 0..4 {
                content_frame(&mut rig, content, source);
            }
            assert!(rig.engine.settled(), "{case}");
            assert_eq!(
                rig.engine.current().unwrap().read_back,
                plain.engine.current().unwrap().read_back,
                "{case}"
            );
            let scene = last_scene(&rig);
            assert_eq!(scene, last_scene(&plain), "{case}");
            assert_eq!(scene.rotation, source.rotation, "{case}");
            assert_eq!(scene.ui_rotation, Rotation::R0, "{case}");
            let mode = rig.engine.current().unwrap().read_back;
            let kind = monitor.profile().kind.clone();
            assert_eq!(
                (scene.ui_dest, scene.ui_filter),
                expected_ui(&kind, &mode),
                "{case}"
            );
            assert_eq!(
                rig.engine.picture_choices(&source).unwrap(),
                plain.engine.picture_choices(&source).unwrap(),
                "{case}"
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
/// the mode stays the game's, and the portrait canvas turned across the
/// lines fills the active area 1:1 with nearest (docs/spec/ui-layout.md,
/// section 6.4). The canvas is opaque and covers the game.
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
    assert_eq!(scene.ui, Some((240, 320)), "the portrait canvas");
    assert_eq!(scene.ui_rotation, Rotation::R270);
    assert_eq!(scene.ui_filter, Filter::Nearest);
    let full = Rect {
        x: 0,
        y: 0,
        w: u32::from(game_mode.hactive),
        h: 240,
    };
    assert_eq!(scene.ui_dest, full, "the full active area");
    assert!(u32::from(game_mode.hactive) >= 320, "{game_mode:?}");

    // A white game under the black canvas: no game pixel shows.
    let white = Frame {
        data: &WHITE,
        ..frame(&DDONPACH)
    };
    rig.engine.begin_frame(DDONPACH);
    let ui = canvas_of(rig.engine.form());
    rig.engine.present(Some(white), Some(ui)).unwrap();
    rig.engine.wait_flip().unwrap();
    let screen = rig.fake.screen().unwrap();
    let black = [0, 0, 0];
    let right = u32::from(game_mode.hactive) - 1;
    for (x, y) in [(0, 0), (right, 120), (right / 2, 239)] {
        assert_eq!(screen.pixel(x, y), Some(black), "({x}, {y})");
    }
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

/// A turn whose new display source has no winner keeps the mode and the
/// game's old turn, while the canvas takes the new turn at once. The
/// trace holds the kept display source with its rotation.
#[test]
fn a_kept_source_after_a_turn_keeps_the_old_game_turn_and_the_trace_names_it() {
    let mut rig = Rig::new(generic_15(), PolicySettings::default());
    rig.frame(DDONPACH);
    let mode = rig.current_mode();
    rig.engine.set_veto(Box::new(
        move |m: &Modeline| {
            if *m == mode { Ok(()) } else { refuse_all(m) }
        },
    ));
    rig.frame(DDONPACH);
    rig.engine.set_orientation(at(Rotation::R90, None));
    for _ in 0..4 {
        assert_eq!(frame_with_ui(&mut rig, DDONPACH), Directive::Kept);
    }
    assert_eq!(rig.current_mode(), mode);
    let scene = last_scene(&rig);
    assert_eq!(scene.rotation, Rotation::R90, "the old turn of the game");
    assert_eq!(
        scene.ui_rotation,
        Rotation::R270,
        "the new turn of the canvas"
    );
    let record = rig.engine.trace().last().unwrap();
    assert!(matches!(record.outcome, Outcome::Kept(_)), "{record:?}");
    assert_eq!(
        record.source,
        SourceGeometry {
            rotation: Rotation::R0,
            ..DDONPACH
        }
    );
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
    rig.engine.set_content(Content::Menu);
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
