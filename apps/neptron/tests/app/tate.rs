//! A screen turned on its side: the screen orientation and the game
//! orientation set from the settings, at each orientation on three CRT
//! presets and a flat panel. Every mode that reaches the display passes
//! the validator for the monitor, the menu has the form of the
//! orientation from the first frame, Down moves the focus to the next row,
//! a change from the menu applies at once with no prompt, and the row is
//! disabled during a game.

use std::path::Path;

use neptron::NextRun;
use neptron::notices::{HORIZONTAL_INTERLACED_TEXT, VERTICAL_INTERLACED_TEXT};
use neptron::pages::ORIENTATION_IN_GAME;
use neptron_testkit::preset::validated_mode;
use neptron_types::{Control, MonitorKind, Orientation, Rotation};

use crate::panel::{PANEL_1080P, sysfs_with_panel};
use crate::rig::{Rig, in_child, in_child_with, in_children, test_name};
use crate::tv::dismiss_notices;

/// The displays of the tests: a preset id, or `None` for the 1080p panel.
const DISPLAYS: [Option<&str>; 4] = [
    Some("generic_15"),
    Some("arcade_15"),
    Some("pc_31_120"),
    None,
];

/// The option label of each screen orientation, as the menu shows it.
fn label(screen: Rotation) -> &'static str {
    match screen {
        Rotation::R0 => "Yoko (0)",
        Rotation::R90 => "Tate left (90)",
        Rotation::R180 => "Yoko flipped (180)",
        Rotation::R270 => "Tate right (270)",
    }
}

/// A rig under `dir/name` for `display`, with the screen orientation
/// `screen` in the global settings.
fn turned_rig(dir: &Path, display: Option<&str>, screen: Rotation) -> Rig {
    let root = dir.join(format!(
        "{}-{}",
        display.unwrap_or("panel"),
        screen.degrees()
    ));
    std::fs::create_dir_all(&root).expect("the folder");
    let mut rig = Rig::new(&root);
    let mut text = format!(
        "schema_version = 1\n\n[display]\norientation = \"{}\"\n",
        screen.degrees()
    );
    match display {
        Some(preset) => text.push_str(&format!("monitor = \"{preset}\"\n")),
        None => {
            sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
            rig.output = "HDMI-A-1".to_owned();
        }
    }
    let config = rig.paths.config();
    std::fs::create_dir_all(&config).expect("the config folder");
    std::fs::write(config.join("global.toml"), text).expect("the settings");
    rig
}

/// Boots the game list and waits for the scan, with any boot prompt kept.
fn boot_list(rig: &mut Rig) {
    rig.boot(None);
    rig.frames(2);
    if rig.shell().guard().pending().is_some() {
        rig.keep();
    }
    rig.until(600, "the scan", |r| {
        !r.shell().snapshot().systems.is_empty()
    });
    dismiss_notices(rig);
}

/// The canvas of the tree, as width and height.
fn canvas(rig: &mut Rig) -> (u32, u32) {
    let canvas = rig.tree().canvas.expect("the tree names its canvas");
    (canvas.width, canvas.height)
}

/// The canvas of the form of a screen at `screen` on a landscape display.
fn canvas_at(screen: Rotation) -> (u32, u32) {
    if screen.across() {
        (240, 320)
    } else {
        (320, 240)
    }
}

/// Every mode that reached the display passes the validator for the
/// preset `display`, or is a mode of the panel.
fn assert_valid(rig: &mut Rig, display: Option<&str>) {
    let calls = rig.display.mode_calls();
    assert!(
        !calls.is_empty(),
        "{display:?}: no mode reached the display"
    );
    let caps = *neptron_backend::Display::caps(&rig.display);
    let panel_modes = match &rig.shell().guard().active().monitor.profile().kind {
        MonitorKind::FlatPanel { modes } => modes.clone(),
        _ => Vec::new(),
    };
    for call in &calls {
        match display {
            Some(id) => {
                if let Err(e) = validated_mode(&call.requested, id, &caps) {
                    panic!("call {} is no mode of {id}: {e}", call.call);
                }
            }
            None => assert!(
                panel_modes.contains(&call.requested),
                "call {} is no mode of the panel: {:?}",
                call.call,
                call.requested
            ),
        }
    }
}

/// Every frame presents a canvas of one form, turned by the UI turn of
/// that form: a portrait canvas turned across, a landscape one not.
fn assert_one_form_per_frame(rig: &Rig) {
    for frame in rig.display.frames() {
        let Some((w, h)) = frame.scene.ui else {
            continue;
        };
        assert_eq!(
            h > w,
            frame.scene.ui_rotation.across(),
            "frame {}: a {w}x{h} canvas turned by {:?}",
            frame.index,
            frame.scene.ui_rotation
        );
    }
}

/// Opens Settings, then Display, from the screen on show.
fn open_display(rig: &mut Rig) {
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    rig.focus("Display");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "display");
}

fn game() -> NextRun {
    NextRun {
        pads: Vec::new(),
        game: Some(neptron::relaunch::GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    }
}

#[test]
#[ignore = "tier 2"]
fn at_each_screen_orientation_the_menu_turns_and_every_mode_is_valid() {
    let name = test_name(
        module_path!(),
        "at_each_screen_orientation_the_menu_turns_and_every_mode_is_valid",
    );
    // One process holds one core, so each display and orientation runs its
    // game in a child of its own.
    let stages = (DISPLAYS.len() * Rotation::ALL.len()) as u32;
    in_children(&name, stages, |dir, stage| {
        let display = DISPLAYS[stage as usize / Rotation::ALL.len()];
        let screen = Rotation::ALL[stage as usize % Rotation::ALL.len()];
        let what = format!("{display:?} at {}", screen.degrees());
        let mut rig = turned_rig(dir, display, screen);
        boot_list(&mut rig);
        let ui_turn = Orientation {
            screen,
            game_override: None,
        }
        .ui_turn();
        let first = rig.display.frames().first().expect("a frame").scene;
        assert_eq!(first.ui_rotation, ui_turn, "{what}: the first frame");
        let (w, h) = canvas_at(screen);
        let size = (u16::try_from(w).unwrap(), u16::try_from(h).unwrap());
        assert_eq!(first.ui, Some(size), "{what}: the first canvas");
        assert_eq!(canvas(&mut rig), (w, h), "{what}");

        // Down moves to the next row as the player reads it.
        open_display(&mut rig);
        rig.focus("Monitor profile");
        rig.tap(&[Control::Down]);
        assert_eq!(
            rig.focused().as_deref(),
            Some("Screen orientation"),
            "{what}"
        );
        rig.tap(&[Control::Up]);
        assert_eq!(rig.focused().as_deref(), Some("Monitor profile"), "{what}");
        rig.finish();

        // The game engine has the orientation from its first frame.
        rig.boot(Some(game()));
        rig.frames(30);
        let game_turn = Rotation::R0.minus(screen);
        let game_frames: Vec<_> = rig
            .display
            .frames()
            .into_iter()
            .filter(|f| f.scene.game.is_some())
            .collect();
        assert!(!game_frames.is_empty(), "{what}: no game frame");
        for frame in game_frames {
            assert_eq!(frame.scene.rotation, game_turn, "{what}: {frame:?}");
        }
        rig.open_menu();
        rig.frames(1);
        dismiss_notices(&mut rig);
        assert_eq!(rig.screen(), "quick-menu", "{what}");
        assert_eq!(canvas(&mut rig), (w, h), "{what}: the quick menu");
        open_display(&mut rig);
        rig.focus("Screen orientation");
        let row = rig
            .tree()
            .rows
            .into_iter()
            .find(|r| r.label == "Screen orientation")
            .expect("the row");
        assert_eq!(row.value.as_deref(), Some(ORIENTATION_IN_GAME), "{what}");
        rig.tap(&[Control::A]);
        assert_eq!(rig.screen(), "display", "{what}: the row opens nothing");
        assert_valid(&mut rig, display);
        assert_one_form_per_frame(&rig);
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn a_change_of_the_screen_orientation_applies_at_once_and_is_stored() {
    let name = test_name(
        module_path!(),
        "a_change_of_the_screen_orientation_applies_at_once_and_is_stored",
    );
    in_child(&name, |dir| {
        for display in DISPLAYS {
            let mut rig = turned_rig(dir, display, Rotation::R0);
            boot_list(&mut rig);
            open_display(&mut rig);
            let modes_before = rig.display.mode_calls().len();
            for screen in [Rotation::R90, Rotation::R180, Rotation::R270, Rotation::R0] {
                let what = format!("{display:?} to {}", screen.degrees());
                rig.focus("Screen orientation");
                rig.tap(&[Control::A]);
                assert_eq!(rig.screen(), "options", "{what}");
                rig.focus(label(screen));
                rig.tap(&[Control::A]);
                rig.frames(1);
                assert!(rig.shell().guard().pending().is_none(), "{what}: no prompt");
                assert_eq!(rig.screen(), "display", "{what}");
                assert_eq!(
                    rig.focused().as_deref(),
                    Some("Screen orientation"),
                    "{what}"
                );
                assert_eq!(canvas(&mut rig), canvas_at(screen), "{what}");
                let row = rig
                    .tree()
                    .rows
                    .into_iter()
                    .find(|r| r.label == "Screen orientation")
                    .expect("the row");
                assert!(!row.clipped, "{what}: {row:?}");
                let last = rig.display.frames().last().expect("a frame").scene;
                let ui_turn = Orientation {
                    screen,
                    game_override: None,
                }
                .ui_turn();
                assert_eq!(last.ui_rotation, ui_turn, "{what}");
                assert_eq!(rig.shell().config().orientation().screen, screen, "{what}");
                // Down still moves to the next row in the new form.
                rig.tap(&[Control::Down]);
                assert_ne!(
                    rig.focused().as_deref(),
                    Some("Screen orientation"),
                    "{what}"
                );
                rig.tap(&[Control::Up]);
                assert_eq!(
                    rig.focused().as_deref(),
                    Some("Screen orientation"),
                    "{what}"
                );
            }
            // The menu mode is the same at every orientation.
            assert_eq!(
                rig.display.mode_calls().len(),
                modes_before,
                "{display:?}: a turn of the screen sets no mode"
            );
            rig.tap(&[Control::A]);
            rig.focus(label(Rotation::R270));
            rig.tap(&[Control::A]);
            assert_valid(&mut rig, display);
            assert_one_form_per_frame(&rig);
            rig.finish();
            assert!(
                rig.state_file("config/global.toml")
                    .contains("orientation = \"270\""),
                "{display:?}: {}",
                rig.state_file("config/global.toml")
            );
        }
    });
}

#[test]
#[ignore = "tier 2"]
fn the_game_orientation_of_the_video_page_turns_the_running_game() {
    let name = test_name(
        module_path!(),
        "the_game_orientation_of_the_video_page_turns_the_running_game",
    );
    in_child(&name, |dir| {
        let mut rig = turned_rig(dir, Some("generic_15"), Rotation::R90);
        rig.boot(Some(game()));
        rig.frames(30);
        let last = rig.display.frames().last().expect("a frame").scene;
        assert_eq!(
            last.rotation,
            Rotation::R270,
            "Auto: the game stands upright"
        );
        rig.open_menu();
        rig.frames(1);
        dismiss_notices(&mut rig);
        rig.focus("Video");
        rig.tap(&[Control::A]);
        assert_eq!(rig.screen(), "quick-menu/video");
        rig.focus("Game orientation");
        rig.tap(&[Control::A]);
        rig.focus("Turn left");
        rig.tap(&[Control::A]);
        rig.frames(1);
        assert!(rig.shell().guard().pending().is_none(), "no prompt");
        assert_eq!(
            rig.shell().config().orientation(),
            Orientation {
                screen: Rotation::R90,
                game_override: Some(Rotation::R90),
            }
        );
        rig.tap(&[Control::B]);
        rig.tap(&[Control::B]);
        // The key is of the game only: the prompt keeps it for this game.
        assert_eq!(rig.screen(), "quick-menu/keep");
        rig.tap(&[Control::Right]);
        rig.tap(&[Control::A]);
        assert_eq!(
            rig.shell().config().orientation().game_override,
            Some(Rotation::R90)
        );
        // A new turn is a new source, through the debounce of the engine.
        rig.frames(30);
        let last = rig.display.frames().last().expect("a frame").scene;
        assert_eq!(
            last.rotation,
            Rotation::R0,
            "the override of 90 on a screen at 90"
        );
        assert_valid(&mut rig, Some("generic_15"));
        rig.finish();
        let game_file = rig.state_file("config/games/nes/Test Game.toml");
        assert!(
            game_file.contains("rotation = \"90\""),
            "the game file: {game_file}"
        );
    });
}

#[test]
#[ignore = "tier 2"]
fn the_boot_file_sets_the_screen_orientation_of_the_first_frame() {
    let name = test_name(
        module_path!(),
        "the_boot_file_sets_the_screen_orientation_of_the_first_frame",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        std::fs::create_dir_all(&rig.paths.boot).expect("the boot folder");
        std::fs::write(rig.paths.boot_file(), "[display]\norientation = \"270\"\n")
            .expect("neptron.toml");
        boot_list(&mut rig);
        let first = rig.display.frames().first().expect("a frame").scene;
        assert_eq!(first.ui_rotation, Rotation::R90);
        assert_eq!(first.ui, Some((240, 320)));
        rig.finish();
        assert!(
            rig.state_file("config/global.toml")
                .contains("orientation = \"270\"")
        );
    });
}

#[test]
#[ignore = "tier 2"]
fn the_blind_recovery_on_a_turned_screen_keeps_the_portrait_form() {
    let name = test_name(
        module_path!(),
        "the_blind_recovery_on_a_turned_screen_keeps_the_portrait_form",
    );
    in_child(&name, |dir| {
        use neptron_video::{Phase, SafeReason};
        let hold_frames = (neptron_input::BLIND_RECOVERY_HOLD_NS / 16_000_000) as u32 + 30;
        let mut rig = turned_rig(dir, Some("generic_15"), Rotation::R90);
        boot_list(&mut rig);
        assert_eq!(canvas(&mut rig), (240, 320));
        rig.hold(&[Control::Select, Control::Start]);
        rig.until(hold_frames, "the recovery", |r| {
            r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
        });
        rig.release(&[Control::Select, Control::Start]);
        rig.frames(2);
        dismiss_notices(&mut rig);
        assert_eq!(canvas(&mut rig), (240, 320), "the portrait form stays");
        let turned = Orientation {
            screen: Rotation::R90,
            game_override: None,
        };
        assert_eq!(rig.shell().config().orientation(), turned);
        let engine = rig.shell().engine().expect("the engine").orientation();
        assert_eq!(engine.screen, Rotation::R90);
        let last = rig.display.frames().last().expect("a frame").scene;
        assert_eq!(last.ui_rotation, turned.ui_turn());
        assert_valid(&mut rig, Some("generic_15"));
        assert_one_form_per_frame(&rig);
        rig.finish();
    });
}

/// The notices of the snapshot after the notice `after`, as id and text.
fn notices_after(rig: &mut Rig, after: u32) -> Vec<(u32, String)> {
    rig.shell()
        .snapshot()
        .notices
        .iter()
        .filter(|n| n.id.0 > after)
        .map(|n| (n.id.0, n.text.clone()))
        .collect()
}

/// A horizontal game across a turned 15 kHz tube shows interlaced with
/// every pixel exact: the one-time notice of that case has its own text,
/// not the text of a vertical game, and the notices file records its id.
#[test]
#[ignore = "tier 2"]
fn an_interlaced_horizontal_game_on_a_turned_tube_has_its_own_one_time_notice() {
    let name = test_name(
        module_path!(),
        "an_interlaced_horizontal_game_on_a_turned_tube_has_its_own_one_time_notice",
    );
    // A 320x240 board: its 320 samples across the turned tube are 320
    // lines, which a 15 kHz tube shows only interlaced.
    let board = "neptron-testcore 1\ngeometry 320 240 1333\ntiming 59940 48000\nrotation 0\n";
    in_child_with(&name, board, |dir| {
        let mut rig = turned_rig(dir, Some("generic_15"), Rotation::R90);
        rig.boot(Some(game()));
        rig.frames(30);
        let mode = rig.display.active_mode().expect("a mode");
        assert!(mode.interlace, "{mode:?}");
        let shown = notices_after(&mut rig, 0);
        let texts: Vec<&str> = shown.iter().map(|(_, t)| t.as_str()).collect();
        assert!(texts.contains(&HORIZONTAL_INTERLACED_TEXT), "{texts:?}");
        assert!(!texts.contains(&VERTICAL_INTERLACED_TEXT), "{texts:?}");
        rig.finish();
        let file = rig.state_file("config/notices.toml");
        assert!(file.contains("\"horizontal_interlaced\""), "{file:?}");
        assert!(!file.contains("vertical_interlaced"), "{file:?}");
    });
}
