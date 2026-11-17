//! A vertical arcade board, as FinalBurn Neo sends DoDonPachi: 320x240
//! frames on their side, the 3:4 aspect of the upright picture, 57.55 Hz,
//! and a turn at load. The test core sends the frames and asks for the
//! turn; the frontend shows the picture upright, with the turn that the
//! core asks for. The turn comes from the core, not from the system, so
//! the test core runs as the game of the NES folder of the rig: it cannot
//! load an arcade zip, which the host passes only to a core that asks for
//! it as it is.

use neptron::NextRun;
use neptron::notices::VERTICAL_INTERLACED_TEXT;
use neptron::relaunch::GameRef;
use neptron_testcore::{PixelFormat, pixel};
use neptron_testkit::RgbImage;
use neptron_types::{Filter, Rect, Rotation};

use crate::panel::{PANEL_1080P, sysfs_with_panel};
use crate::rig::{Rig, in_child_with, in_children_with, test_name};

/// The board with the aspect `aspect_milli` and the `SET_ROTATION` value
/// `rotation` at load.
fn board(aspect_milli: u32, rotation: u32) -> String {
    format!(
        "neptron-testcore 1\ngeometry 320 240 {aspect_milli}\ntiming 57550 48000\nrotation {rotation}\n"
    )
}

/// The relaunch file of the game of the rig.
fn the_game() -> Option<NextRun> {
    Some(NextRun {
        game: Some(GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    })
}

fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

const ALL: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

/// The frame pixel of pixel `(tx, ty)` of the 320x240 frame turned by
/// `rotation`, by the rule of `Scene::rotation`.
fn frame_pixel(rotation: Rotation, tx: u32, ty: u32) -> (u32, u32) {
    let (w, h) = (320, 240);
    match rotation {
        Rotation::R0 => (tx, ty),
        Rotation::R90 => (w - 1 - ty, tx),
        Rotation::R180 => (w - 1 - tx, h - 1 - ty),
        Rotation::R270 => (ty, h - 1 - tx),
    }
}

/// The turned picture's size for `rotation`.
fn turned_size(rotation: Rotation) -> (u32, u32) {
    match rotation {
        Rotation::R0 | Rotation::R180 => (320, 240),
        Rotation::R90 | Rotation::R270 => (240, 320),
    }
}

/// True when the screen shows core frame `n` turned by `rotation` in
/// `dest`: at the centre of the block of each sampled pixel of the turned
/// picture, where neither filter blends, the colour of the frame pixel of
/// the turn rule.
fn shows(screen: &RgbImage, dest: Rect, n: u64, rotation: Rotation) -> bool {
    let (tw, th) = turned_size(rotation);
    let samples = [
        (0, 0),
        (tw - 1, 0),
        (0, th - 1),
        (tw - 1, th - 1),
        (5, 7),
        (tw / 2, th / 3),
    ];
    samples.into_iter().all(|(tx, ty)| {
        let x = dest.x as u32 + (2 * tx + 1) * dest.w / (2 * tw);
        let y = dest.y as u32 + (2 * ty + 1) * dest.h / (2 * th);
        let (fx, fy) = frame_pixel(rotation, tx, ty);
        let v = pixel(PixelFormat::Xrgb8888, n, 0, fx, fy);
        let expected = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        screen.pixel(x, y) == Some(expected)
    })
}

/// Boots the board on the 1080p panel, and checks that the screen shows a
/// frame of the core turned by `rotation` in `dest`, and by no other turn.
fn upright_on_the_panel(dir: &std::path::Path, rotation: Rotation, dest: Rect, filter: Filter) {
    let mut rig = Rig::new(dir);
    sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
    rig.output = "HDMI-A-1".to_owned();
    rig.boot(the_game());
    assert!(rig.shell().in_game());
    rig.frames(30);
    let scene = rig.display.frames().last().expect("a frame").scene;
    assert_eq!(scene.rotation, rotation);
    assert_eq!(scene.game.map(|g| (g.0, g.1)), Some((320, 240)));
    assert_eq!(scene.crop, rect(0, 0, 320, 240), "frame pixels");
    assert_eq!(scene.dest, dest);
    assert_eq!(scene.filter, filter);
    assert_eq!(scene.ui, None, "no notice over the game");
    let screen = rig.display.screen().expect("the screen");
    let found = (0..=200).find(|&n| shows(&screen, dest, n, rotation));
    let Some(n) = found else {
        panic!("the screen shows no frame of the core turned by {rotation:?}");
    };
    for other in ALL.into_iter().filter(|&r| r != rotation) {
        assert!(
            !shows(&screen, dest, n, other),
            "frame {n} also matches {other:?}"
        );
    }
    let mode = rig.display.active_mode().expect("a mode");
    assert_eq!((mode.hactive, mode.vactive), (1920, 1080));
    rig.finish();
}

/// `SET_ROTATION(1)`, as DoDonPachi asks: 90 degrees counter-clockwise.
/// The upright 240x320 at 3x on 1080p (1080 / 320 = 3.4, under the fill
/// scale of 4): 720x960, nearest, centred at (600, 60).
#[test]
#[ignore = "tier 2"]
fn a_board_turned_by_90_degrees_shows_upright_on_a_panel() {
    let test = test_name(
        module_path!(),
        "a_board_turned_by_90_degrees_shows_upright_on_a_panel",
    );
    in_child_with(&test, &board(750, 1), |dir| {
        upright_on_the_panel(dir, Rotation::R90, rect(600, 60, 720, 960), Filter::Nearest);
    });
}

/// `SET_ROTATION(3)`, a vertical board that is also flipped: the same
/// place as 90 degrees, the picture the other way up.
#[test]
#[ignore = "tier 2"]
fn a_board_turned_by_270_degrees_shows_upright_on_a_panel() {
    let test = test_name(
        module_path!(),
        "a_board_turned_by_270_degrees_shows_upright_on_a_panel",
    );
    in_child_with(&test, &board(750, 3), |dir| {
        upright_on_the_panel(
            dir,
            Rotation::R270,
            rect(600, 60, 720, 960),
            Filter::Nearest,
        );
    });
}

/// `SET_ROTATION(2)`, a flipped horizontal board with its 4:3 aspect,
/// which the half turn does not change. 1080 / 240 = 4.5, at least the
/// fill scale of 4, so the picture fills the height: 1080 lines, 1440
/// wide, centred at 240, sharp bilinear.
#[test]
#[ignore = "tier 2"]
fn a_board_turned_by_180_degrees_shows_the_right_way_up_on_a_panel() {
    let test = test_name(
        module_path!(),
        "a_board_turned_by_180_degrees_shows_the_right_way_up_on_a_panel",
    );
    in_child_with(&test, &board(1_333, 2), |dir| {
        upright_on_the_panel(
            dir,
            Rotation::R180,
            rect(240, 0, 1440, 1080),
            Filter::SharpBilinear,
        );
    });
}

/// The notices of the snapshot.
fn notices(rig: &mut Rig) -> Vec<String> {
    rig.shell()
        .snapshot()
        .notices
        .iter()
        .map(|n| n.text.clone())
        .collect()
}

/// Boots DoDonPachi (`R90`) on the safe default of the rig, a 15 kHz CRT.
fn on_15_khz(dir: &std::path::Path, game_settings: Option<&str>) -> Rig {
    let mut rig = Rig::new(dir);
    if let Some(text) = game_settings {
        let games = rig.paths.config().join("games").join("nes");
        std::fs::create_dir_all(&games).expect("the settings folder");
        std::fs::write(games.join("Test Game.toml"), text).expect("the settings");
    }
    rig.boot(the_game());
    assert!(rig.shell().in_game());
    rig.frames(30);
    rig
}

/// `display.vertical = exact`, the default: the 320 upright lines 1:1 in
/// an interlaced mode, turned, with every pixel exact, and the one-time
/// notice of a vertical game, which the notices file then records.
#[test]
#[ignore = "tier 2"]
fn on_15_khz_a_vertical_board_is_interlaced_with_its_lines_1_to_1() {
    let test = test_name(
        module_path!(),
        "on_15_khz_a_vertical_board_is_interlaced_with_its_lines_1_to_1",
    );
    in_children_with(&test, &board(750, 1), 2, |dir, stage| {
        let mut rig = on_15_khz(dir, None);
        if stage == 1 {
            // The next run of the game: the notices file holds the notice,
            // so it does not show again.
            let shown = notices(&mut rig);
            assert!(
                !shown.iter().any(|n| n == VERTICAL_INTERLACED_TEXT),
                "{shown:?}"
            );
            rig.finish();
            return;
        }
        let mode = rig.display.active_mode().expect("a mode");
        assert!(mode.interlace, "{mode:?}");
        assert!(mode.line_rate_mhz() < 17_000_000, "15 kHz: {mode:?}");
        let scene = rig.display.frames().last().expect("a frame").scene;
        assert_eq!(scene.rotation, Rotation::R90);
        assert_eq!(scene.dest.h, 320);
        assert_eq!(scene.filter, Filter::Nearest);
        assert!(scene.dest.w < scene.dest.h, "pillarboxed: {:?}", scene.dest);
        let shown = notices(&mut rig);
        assert!(
            shown.iter().any(|n| n == VERTICAL_INTERLACED_TEXT),
            "{shown:?}"
        );
        rig.finish();
        let file = rig.state_file("config/notices.toml");
        assert!(file.contains("vertical_interlaced"), "{file:?}");
    });
}

/// `display.vertical = fit` at the game level: the picture fills a
/// progressive mode, soft, with the notice that names the setting.
#[test]
#[ignore = "tier 2"]
fn on_15_khz_vertical_fit_gives_a_progressive_soft_picture_with_a_notice() {
    let test = test_name(
        module_path!(),
        "on_15_khz_vertical_fit_gives_a_progressive_soft_picture_with_a_notice",
    );
    in_child_with(&test, &board(750, 1), |dir| {
        let settings = "schema_version = 1\n\n[display]\nvertical = \"fit\"\n";
        let mut rig = on_15_khz(dir, Some(settings));
        let mode = rig.display.active_mode().expect("a mode");
        assert!(!mode.interlace, "{mode:?}");
        let scene = rig.display.frames().last().expect("a frame").scene;
        assert_eq!(scene.rotation, Rotation::R90);
        assert_eq!(scene.filter, Filter::SharpBilinear);
        assert!(scene.dest.h < 320, "fitted: {:?}", scene.dest);
        let shown = notices(&mut rig);
        assert!(
            shown
                .iter()
                .any(|n| n == "Soft picture: Vertical games is set to Fit."),
            "{shown:?}"
        );
        rig.finish();
    });
}
