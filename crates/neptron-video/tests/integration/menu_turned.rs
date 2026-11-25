//! The menu engine on a turned screen (docs/spec/ui-layout.md, section
//! 6.3): the app draws the canvas of the engine's form, the policy plans
//! the raster of that canvas after its turn, and the menu keeps the mode
//! and the place of a screen at 0 on every CRT, TV and landscape panel.

use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_testkit::{DisplayOptions, FakeDisplay, SimClock};
use neptron_types::{
    Filter, Form, Frame, Modeline, PixelFormat, Rect, Rotation, SourceGeometry, TvStandard,
};
use neptron_video::{Content, Directive, Monitor, PolicySettings, VideoEngine};

use crate::guard_rig::pc_31_120;
use crate::harness::{Rig, generic_15};
use crate::panel::{mode, panel};
use crate::tate::{DDONPACH, at, canvas, last_scene};

/// The source of the menu canvas, as the app plans it.
pub const MENU_SOURCE: SourceGeometry = SourceGeometry {
    width: 320,
    height: 240,
    refresh_mhz: 60_000,
    aspect_milli: 1_333,
    interlaced: false,
    rotation: Rotation::R0,
};

/// The 240x320 portrait canvas of the UI.
static PORTRAIT: [u8; 240 * 320 * 2] = [0; 240 * 320 * 2];

/// The canvas of `form`, as the app draws it.
pub fn canvas_of(form: Form) -> Frame<'static> {
    match form {
        Form::Landscape => canvas(),
        Form::Portrait => Frame {
            data: &PORTRAIT,
            width: 240,
            height: 320,
            pitch: 480,
            format: PixelFormat::Rgb565,
        },
    }
}

fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

fn menu_rig(monitor: Monitor, screen: Rotation) -> Rig {
    let mut rig = Rig::new(monitor, PolicySettings::default());
    rig.engine.set_content(Content::Menu);
    rig.engine.set_orientation(at(screen, None));
    rig
}

fn tv_rig(standard: TvStandard, screen: Rotation) -> Rig {
    let fake = FakeDisplay::with_options(
        SimClock::new(),
        CAPS_LOW_CLOCK,
        DisplayOptions {
            tv_standards: Some(TvStandard::ALL.to_vec()),
            ..DisplayOptions::default()
        },
    );
    let monitor = Monitor::Profile(standard.profile());
    let engine = VideoEngine::new(fake.clone(), monitor, PolicySettings::default());
    let mut rig = Rig { fake, engine };
    rig.engine.set_content(Content::Menu);
    rig.engine.set_orientation(at(screen, None));
    rig
}

/// One frame of the menu: the canvas of the engine's form alone.
fn menu_frame(rig: &mut Rig) -> Directive {
    let directive = rig.engine.begin_frame(MENU_SOURCE);
    let ui = canvas_of(rig.engine.form());
    rig.engine.present(None, Some(ui)).unwrap();
    rig.engine.wait_flip().unwrap();
    directive
}

fn menu_mode(rig: &Rig) -> Modeline {
    rig.engine.current().unwrap().read_back
}

/// The menu of each display class at `screen`: its mode, the place and
/// the filter of its canvas, and the turn of the canvas.
fn menu_at(rig: impl Fn(Rotation) -> Rig, screen: Rotation) -> (Modeline, Rect, Filter, Rotation) {
    let mut rig = rig(screen);
    menu_frame(&mut rig);
    menu_frame(&mut rig);
    let scene = last_scene(&rig);
    (
        menu_mode(&rig),
        scene.ui_dest,
        scene.ui_filter,
        scene.ui_rotation,
    )
}

type RigOf = Box<dyn Fn(Rotation) -> Rig>;

fn display_classes() -> Vec<(&'static str, RigOf)> {
    vec![
        ("generic_15", Box::new(|s| menu_rig(generic_15(), s))),
        ("pc_31_120", Box::new(|s| menu_rig(pc_31_120(), s))),
        (
            "panel_1080",
            Box::new(|s| menu_rig(panel(vec![mode(1_920, 1_080, 60)]), s)),
        ),
        ("tv_ntsc", Box::new(|s| tv_rig(TvStandard::Ntsc, s))),
        ("tv_pal", Box::new(|s| tv_rig(TvStandard::Pal, s))),
    ]
}

/// The claim of section 6.3: at 90 and 270 the portrait canvas turned by
/// `U` is 320x240 in scan-out, so the menu has the mode, the place and
/// the filter of the menu at 0, and only the turn of the canvas differs.
#[test]
fn the_menu_of_a_turned_screen_has_the_mode_and_the_place_of_the_menu_at_0() {
    for (name, rig) in display_classes() {
        let (mode, dest, filter, turn) = menu_at(&rig, Rotation::R0);
        assert_eq!(turn, Rotation::R0, "{name}");
        assert_eq!(filter, Filter::Nearest, "{name}");
        assert!(!mode.interlace, "{name}: {mode:?}");
        for screen in [Rotation::R90, Rotation::R180, Rotation::R270] {
            let got = menu_at(&rig, screen);
            assert_eq!(
                (got.0, got.1, got.2),
                (mode, dest, filter),
                "{name} at {screen:?}"
            );
            assert_eq!(got.3, at(screen, None).ui_turn(), "{name} at {screen:?}");
        }
    }
}

/// The numbers of the table of section 6.3.
#[test]
fn the_turned_menu_lies_where_the_spec_says() {
    let (m, dest, ..) = menu_at(|s| menu_rig(generic_15(), s), Rotation::R90);
    assert_eq!((m.vactive, m.interlace), (240, false), "240p, no flicker");
    assert_eq!(dest, rect(0, 0, u32::from(m.hactive), 240), "every line");
    let (m, dest, ..) = menu_at(|s| tv_rig(TvStandard::Ntsc, s), Rotation::R90);
    assert_eq!((m.vactive, m.interlace), (240, false));
    assert_eq!(dest, rect(40, 0, 640, 240));
    let (m, dest, ..) = menu_at(|s| tv_rig(TvStandard::Pal, s), Rotation::R270);
    assert_eq!(m.vactive, 288);
    assert_eq!(dest, rect(40, 24, 640, 240));
    let panel_1080 = |s| menu_rig(panel(vec![mode(1_920, 1_080, 60)]), s);
    let (m, dest, ..) = menu_at(panel_1080, Rotation::R90);
    assert_eq!((m.hactive, m.vactive), (1_920, 1_080));
    assert_eq!(dest, rect(320, 60, 1_280, 960), "4x");
}

/// A panel with a portrait native mode at 0: the portrait canvas at its
/// native mode, 4x, centred.
#[test]
fn a_portrait_panel_shows_the_portrait_canvas_upright_at_4x() {
    let mut rig = menu_rig(panel(vec![mode(1_080, 1_920, 60)]), Rotation::R0);
    assert_eq!(rig.engine.form(), Form::Portrait);
    menu_frame(&mut rig);
    let m = menu_mode(&rig);
    assert_eq!((m.hactive, m.vactive), (1_080, 1_920));
    let scene = last_scene(&rig);
    assert_eq!(scene.ui, Some((240, 320)));
    assert_eq!(scene.ui_rotation, Rotation::R0);
    assert_eq!(scene.ui_dest, rect(60, 320, 960, 1_280));
    assert_eq!(scene.ui_filter, Filter::Nearest);
}

/// The engine's form follows the screen orientation on a CRT.
#[test]
fn the_form_of_the_engine_follows_the_screen() {
    let mut rig = menu_rig(generic_15(), Rotation::R0);
    for (screen, form) in [
        (Rotation::R0, Form::Landscape),
        (Rotation::R90, Form::Portrait),
        (Rotation::R180, Form::Landscape),
        (Rotation::R270, Form::Portrait),
    ] {
        rig.engine.set_orientation(at(screen, None));
        assert_eq!(rig.engine.form(), form, "{screen:?}");
    }
}

/// A turn of the screen in the menu changes no mode: the source of the
/// menu stays, and the canvas of the new form turns at the next frame.
#[test]
fn a_turn_of_the_screen_in_the_menu_needs_no_modeset() {
    let mut rig = menu_rig(generic_15(), Rotation::R0);
    menu_frame(&mut rig);
    let calls = rig.fake.mode_calls().len();
    let before = menu_mode(&rig);
    rig.engine.set_orientation(at(Rotation::R90, None));
    assert_eq!(menu_frame(&mut rig), Directive::Steady);
    assert_eq!(rig.fake.mode_calls().len(), calls);
    assert_eq!(menu_mode(&rig), before);
    let scene = last_scene(&rig);
    assert_eq!(scene.ui, Some((240, 320)));
    assert_eq!(
        (scene.ui_rotation, scene.ui_filter),
        (Rotation::R270, Filter::Nearest)
    );
}

/// The answers of Settings > Display in the menu engine are those of the
/// games at Auto on the turned screen: a vertical board is a source of its
/// own lines, and no override reaches them.
#[test]
fn the_menu_answers_read_the_screen_and_no_override() {
    let mut rig = menu_rig(generic_15(), Rotation::R90);
    rig.engine
        .set_orientation(at(Rotation::R90, Some(Rotation::R0)));
    let mut game = Rig::new(generic_15(), PolicySettings::default());
    game.engine.set_orientation(at(Rotation::R90, None));
    assert_eq!(
        rig.engine.picture_choices(&DDONPACH).unwrap(),
        game.engine.picture_choices(&DDONPACH).unwrap()
    );
    let normal = Rig::new(generic_15(), PolicySettings::default());
    let drawn = SourceGeometry {
        rotation: Rotation::R0,
        ..DDONPACH
    };
    assert_eq!(
        rig.engine.picture_choices(&DDONPACH).unwrap(),
        normal.engine.picture_choices(&drawn).unwrap()
    );
}
