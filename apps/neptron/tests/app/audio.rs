//! The audio settings from the menus: the audio panel at home writes the
//! global file, and the volume row of the quick menu reaches the running
//! game at once and keeps its value for the next game.

use neptron::NextRun;
use neptron_settings::{AUDIO_OUTPUT, AUDIO_VOLUME, Level, Value};
use neptron_types::Control;

use crate::rig::{Rig, in_child, in_children, test_name};

fn game() -> NextRun {
    NextRun {
        game: Some(neptron::relaunch::GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    }
}

#[test]
#[ignore = "tier 2"]
fn the_audio_panel_writes_the_volume_and_the_output_to_the_global_file() {
    in_child(
        &test_name(
            module_path!(),
            "the_audio_panel_writes_the_volume_and_the_output_to_the_global_file",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
            rig.focus("Audio");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "audio");
            rig.focus("Volume");
            rig.tap(&[Control::Left]);
            rig.tap(&[Control::Left]);
            rig.focus("Audio output");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "options");
            rig.focus("USB Audio analog");
            rig.tap(&[Control::A]);
            rig.frames(2);
            let config = rig.shell().config().clone();
            assert_eq!(
                config.get(Level::Global, AUDIO_VOLUME),
                Some(&Value::Int(90))
            );
            assert_eq!(
                config.get(Level::Global, AUDIO_OUTPUT),
                Some(&Value::Text("hw:CARD=Device,DEV=0".to_owned()))
            );
            rig.finish();
            let file = std::fs::read_to_string(rig.paths.config().join("global.toml"))
                .expect("the global file");
            assert!(file.contains("volume = 90"), "{file}");
            assert!(file.contains("output = \"hw:CARD=Device,DEV=0\""), "{file}");
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn the_quick_menu_volume_reaches_the_game_at_once() {
    // One process hosts one core, so each run is a stage of its own, as
    // the launcher starts a process for each run.
    in_children(
        &test_name(
            module_path!(),
            "the_quick_menu_volume_reaches_the_game_at_once",
        ),
        3,
        |dir, stage| {
            let mut rig = Rig::new(dir);
            match stage {
                // A first boot with nothing confirmed: keep the display,
                // so no prompt or notice stands in the way of the game.
                0 => {
                    rig.boot(None);
                    rig.frames(2);
                    rig.keep();
                }
                1 => {
                    rig.boot(Some(game()));
                    rig.frames(30);
                    assert_eq!(rig.shell().game_volume_percent(), Some(100));
                    rig.tap(&[Control::Select, Control::Start]);
                    rig.frames(1);
                    assert_eq!(rig.screen(), "quick-menu");
                    rig.focus("Volume");
                    rig.tap(&[Control::Left]);
                    assert_eq!(rig.shell().game_volume_percent(), Some(95));
                    rig.tap(&[Control::Left]);
                    assert_eq!(rig.shell().game_volume_percent(), Some(90));
                    assert_eq!(
                        rig.shell().config().get(Level::Game, AUDIO_VOLUME),
                        Some(&Value::Int(90))
                    );
                }
                _ => {
                    rig.boot(Some(game()));
                    rig.frames(30);
                    assert_eq!(
                        rig.shell().game_volume_percent(),
                        Some(90),
                        "the next run of the game resolves its volume"
                    );
                }
            }
            rig.finish();
        },
    );
}
