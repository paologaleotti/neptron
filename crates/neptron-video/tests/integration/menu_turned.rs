//! The menu engine on a turned screen: the one choice of the turned menu
//! mode (question T3 of docs/spec/tate.md) and the canvas in that mode.

use neptron_modeline::policy::ui_dest_turned;
use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_testkit::{DisplayOptions, FakeDisplay, SimClock};
use neptron_types::{Filter, Modeline, Rect, Rotation, SourceGeometry, TvStandard};
use neptron_video::{
    Content, Directive, Monitor, PolicySettings, TURNED_MENU_MODE, TurnedMenuMode, VideoEngine,
};

use crate::harness::{Rig, generic_15};
use crate::panel::{mode, panel};
use crate::tate::{DDONPACH, at, canvas, last_scene};

/// The source of the menu canvas, as the app plans it.
const MENU_SOURCE: SourceGeometry = SourceGeometry {
    width: 320,
    height: 240,
    refresh_mhz: 60_000,
    aspect_milli: 1_333,
    interlaced: false,
    rotation: Rotation::R0,
};

fn menu_rig(monitor: Monitor, choice: TurnedMenuMode, screen: Rotation) -> Rig {
    let mut rig = Rig::new(monitor, PolicySettings::default());
    rig.engine.set_content(Content::Menu(choice));
    rig.engine.set_orientation(at(screen, None));
    rig
}

/// One frame of the menu: the canvas alone.
fn menu_frame(rig: &mut Rig) -> Directive {
    let directive = rig.engine.begin_frame(MENU_SOURCE);
    rig.engine.present(None, Some(canvas())).unwrap();
    rig.engine.wait_flip().unwrap();
    directive
}

fn menu_mode(rig: &Rig) -> Modeline {
    rig.engine.current().unwrap().read_back
}

/// The canvas turned across `mode` on `monitor`, by the rule of the spec.
fn turned_ui(monitor: &Monitor, mode: &Modeline) -> (Rect, Filter) {
    let placed = ui_dest_turned(
        &monitor.profile().kind,
        mode,
        (240, 320),
        PolicySettings::default().solver.monitor_aspect,
    );
    (placed.dest, placed.filter)
}

/// The proposed default: on a turned 15 kHz tube the menu takes the
/// policy's interlaced answer for its turned source, and the canvas lies
/// 1:1 on 320 of its 448 lines.
#[test]
fn the_turned_menu_on_a_15khz_tube_is_interlaced_with_the_canvas_one_to_one() {
    assert_eq!(TURNED_MENU_MODE, TurnedMenuMode::Interlaced);
    for screen in [Rotation::R90, Rotation::R270] {
        let mut rig = menu_rig(generic_15(), TURNED_MENU_MODE, screen);
        menu_frame(&mut rig);
        let mode = menu_mode(&rig);
        assert_eq!((mode.vactive, mode.interlace), (448, true), "{mode:?}");
        let scene = last_scene(&rig);
        assert_eq!(scene.ui_rotation, at(screen, None).ui_turn());
        assert_eq!((scene.ui_dest.y, scene.ui_dest.h), (64, 320));
        assert_eq!(
            (scene.ui_dest, scene.ui_filter),
            turned_ui(&generic_15(), &mode)
        );
        assert_eq!(scene.ui_filter, Filter::Nearest);
    }
}

/// The other answer: the menu keeps the mode of a screen at 0, and the
/// canvas is squeezed into its 240 lines with sharp bilinear.
#[test]
fn a_progressive_turned_menu_keeps_the_240p_mode_and_squeezes_the_canvas() {
    let mut normal = menu_rig(generic_15(), TurnedMenuMode::Progressive, Rotation::R0);
    menu_frame(&mut normal);
    let upright = menu_mode(&normal);
    assert_eq!((upright.vactive, upright.interlace), (240, false));
    for screen in [Rotation::R90, Rotation::R270] {
        let mut rig = menu_rig(generic_15(), TurnedMenuMode::Progressive, screen);
        menu_frame(&mut rig);
        assert_eq!(menu_mode(&rig), upright);
        let scene = last_scene(&rig);
        assert_eq!((scene.ui_dest.y, scene.ui_dest.h), (0, 240));
        assert_eq!(
            (scene.ui_dest, scene.ui_filter),
            turned_ui(&generic_15(), &upright)
        );
        assert_eq!(scene.ui_filter, Filter::SharpBilinear);
    }
}

/// At 0 and at 180 the two answers are the same: the menu of today, the
/// canvas upright or upside down over the whole tube.
#[test]
fn both_answers_agree_on_a_screen_that_is_not_turned_across() {
    for screen in [Rotation::R0, Rotation::R180] {
        let scenes = [TurnedMenuMode::Interlaced, TurnedMenuMode::Progressive].map(|choice| {
            let mut rig = menu_rig(generic_15(), choice, screen);
            menu_frame(&mut rig);
            (
                menu_mode(&rig),
                last_scene(&rig).ui_dest,
                last_scene(&rig).ui_filter,
            )
        });
        assert_eq!(scenes[0], scenes[1], "{screen:?}");
        let mut plain = Rig::new(generic_15(), PolicySettings::default());
        menu_frame(&mut plain);
        assert_eq!(scenes[0].0, menu_mode(&plain));
        assert_eq!(scenes[0].1, last_scene(&plain).ui_dest);
        assert_eq!(scenes[0].2, Filter::Nearest);
    }
}

/// A turned flat panel keeps its native mode, and the canvas takes the
/// largest whole scale with its sides swapped: 3x in 1080p.
#[test]
fn the_turned_menu_on_a_flat_panel_is_three_times_in_1080p() {
    let monitor = panel(vec![mode(1_920, 1_080, 60)]);
    for choice in [TurnedMenuMode::Interlaced, TurnedMenuMode::Progressive] {
        let mut rig = menu_rig(monitor.clone(), choice, Rotation::R90);
        menu_frame(&mut rig);
        let m = menu_mode(&rig);
        assert_eq!((m.hactive, m.vactive), (1_920, 1_080));
        let scene = last_scene(&rig);
        assert_eq!(
            scene.ui_dest,
            Rect {
                x: 600,
                y: 60,
                w: 720,
                h: 960
            }
        );
        assert_eq!(scene.ui_filter, Filter::Nearest);
    }
}

/// A turned NTSC TV: 480i, the canvas 1:1 on 320 lines and 240 samples
/// wide, centred.
#[test]
fn the_turned_menu_on_an_ntsc_tv_is_480i_with_240_samples() {
    let fake = FakeDisplay::with_options(
        SimClock::new(),
        CAPS_LOW_CLOCK,
        DisplayOptions {
            tv_standards: Some(TvStandard::ALL.to_vec()),
            ..DisplayOptions::default()
        },
    );
    let monitor = Monitor::Profile(TvStandard::Ntsc.profile());
    let engine = VideoEngine::new(fake.clone(), monitor, PolicySettings::default());
    let mut rig = Rig { fake, engine };
    rig.engine.set_content(Content::Menu(TURNED_MENU_MODE));
    rig.engine.set_orientation(at(Rotation::R90, None));
    menu_frame(&mut rig);
    let m = menu_mode(&rig);
    assert_eq!((m.hactive, m.vactive, m.interlace), (720, 480, true));
    let scene = last_scene(&rig);
    assert_eq!(
        scene.ui_dest,
        Rect {
            x: 240,
            y: 80,
            w: 240,
            h: 320
        }
    );
    assert_eq!(scene.ui_filter, Filter::Nearest);
}

/// A turn of the screen in the menu: with the interlaced answer it is a
/// new source, so a modeset after the debounce; with the progressive one
/// the mode stays, and the canvas turns at the next frame.
#[test]
fn a_turn_of_the_screen_in_the_menu_follows_the_choice() {
    let mut rig = menu_rig(generic_15(), TurnedMenuMode::Interlaced, Rotation::R0);
    menu_frame(&mut rig);
    rig.engine.set_orientation(at(Rotation::R90, None));
    let directives: Vec<_> = (0..3).map(|_| menu_frame(&mut rig)).collect();
    assert_eq!(directives[2], Directive::Modeset);
    assert!(menu_mode(&rig).interlace);
    assert_eq!(last_scene(&rig).ui_rotation, Rotation::R270);

    let mut rig = menu_rig(generic_15(), TurnedMenuMode::Progressive, Rotation::R0);
    menu_frame(&mut rig);
    let calls = rig.fake.mode_calls().len();
    rig.engine.set_orientation(at(Rotation::R90, None));
    assert_eq!(menu_frame(&mut rig), Directive::Steady);
    assert_eq!(rig.fake.mode_calls().len(), calls);
    let scene = last_scene(&rig);
    assert_eq!(
        (scene.ui_rotation, scene.ui_filter),
        (Rotation::R270, Filter::SharpBilinear)
    );
}

/// The answers of Settings > Display in the menu engine are those of the
/// games at Auto on the turned screen: a vertical board is a source of its
/// own lines, and no override reaches them.
#[test]
fn the_menu_answers_read_the_screen_and_no_override() {
    let mut rig = menu_rig(generic_15(), TURNED_MENU_MODE, Rotation::R90);
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
