//! The end-to-end run of tier 2: the game list, a launch, the mode
//! switches of the game, the quick menu with a display change, and the
//! quit back to the list.

use neptron::{NextRun, Outcome};
use neptron_types::Control;
use neptron_ui::snapshot::SettingValue;
use neptron_video::Phase;

use crate::rig::{Rig, in_child, in_children, test_name};

#[test]
#[ignore = "tier 2"]
fn list_launch_mode_switches_quick_menu_and_quit() {
    in_child(
        &test_name(
            module_path!(),
            "list_launch_mode_switches_quick_menu_and_quit",
        ),
        |dir| {
            let mut rig = Rig::new(dir);

            // Run 1, a new boot: the settings give the safe default, so
            // nothing waits for a confirmation.
            rig.boot(None);
            rig.frames(2);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_ne!(rig.screen(), "confirm");
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
            assert_eq!(rig.screen(), "home", "{:?}", rig.shell().snapshot().notices);
            assert_eq!(
                rig.focused().as_deref(),
                Some("Nintendo Entertainment System")
            );
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "games");
            rig.until(10, "the game rows", |r| {
                r.focused().as_deref() == Some("Test Game")
            });
            rig.hold(&[Control::A]);
            let outcome = rig.until_end(2);
            let Outcome::Relaunch(next) = outcome else {
                panic!("a launch relaunches: {outcome:?}");
            };
            let game = next.game.clone().expect("a game");
            assert_eq!(
                (game.system.as_str(), game.path.as_str()),
                ("nes", "nes/Test Game.nes")
            );
            rig.release(&[Control::A]);
            rig.finish();

            // Run 2, the game: the confirmed monitor needs no prompt.
            rig.boot(Some(next));
            assert!(rig.shell().in_game());
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.frames(60);
            assert_eq!(rig.screen(), "game");
            let first = rig
                .shell()
                .engine()
                .expect("engine")
                .current()
                .expect("a mode")
                .read_back;
            assert_eq!(first.vactive, 224, "{first:?}");
            assert!(rig.shell().snapshot().video.is_some(), "the video status");
            rig.frames(300);
            let second = rig
                .shell()
                .engine()
                .expect("engine")
                .current()
                .expect("a mode")
                .read_back;
            assert_eq!(second.vactive, 240, "the mode follows the core: {second:?}");
            assert!(rig.display.mode_switches().len() >= 2);

            // The quick menu, never a toggle.
            rig.open_menu();
            rig.frames(1);
            assert_eq!(rig.screen(), "quick-menu");
            assert!(rig.shell().paused());
            rig.open_menu();
            assert_eq!(
                rig.screen(),
                "quick-menu",
                "a second hotkey does not close it"
            );

            // The picture styles show what they give on this monitor.
            rig.focus("Video");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "quick-menu/video");
            let style = rig
                .shell()
                .snapshot()
                .settings
                .rows
                .iter()
                .find(|r| r.label == "Picture style")
                .map(|r| r.value.clone());
            let Some(SettingValue::Choice { options, .. }) = style else {
                panic!("the style row: {style:?}");
            };
            assert!(options[0].detail.contains("kHz"), "{options:?}");
            assert!(options.iter().any(|o| !o.is_enabled()), "{options:?}");
            // The help line puts "Not available." before the reason.
            assert!(
                options
                    .iter()
                    .filter_map(|o| o.unavailable.as_deref())
                    .all(|r| !r.starts_with("Not available")),
                "{options:?}"
            );

            // A picture style on a confirmed monitor applies with no
            // dialog: the validator alone decides the modes of a style.
            rig.focus("Picture style");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "options");
            rig.focus("native");
            rig.tap(&[Control::A]);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(rig.screen(), "quick-menu/video", "{:?}", rig.tree());
            assert_eq!(rig.shell().config().text("display.style"), "native");
            rig.frames(2);
            assert_eq!(rig.shell().guard().active().settings.style.key(), "native");

            // Quit: the quick menu asks where to keep the change.
            rig.tap(&[Control::B]);
            assert_eq!(rig.screen(), "quick-menu");
            assert_eq!(rig.focused().as_deref(), Some("Video"));
            rig.focus("Quit game");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "quick-menu/keep");
            rig.hold(&[Control::A]);
            let outcome = rig.until_end(4);
            assert_eq!(
                outcome,
                Outcome::Relaunch(NextRun {
                    pads: Vec::new(),
                    game: None,
                    notice: None,
                })
            );
            rig.release(&[Control::A]);
            rig.finish();
            let game_file = rig.state_file("config/games/nes/Test Game.toml");
            assert!(game_file.contains("style = \"native\""), "{game_file}");
            let save =
                std::fs::read(rig.paths.state.join("saves/nes/Test Game.srm")).expect("save");
            assert_eq!(
                save.first(),
                Some(&7),
                "the save RAM is flushed at the quit"
            );

            // Run 3, the game list again.
            rig.boot(Some(NextRun {
                pads: Vec::new(),
                game: None,
                notice: None,
            }));
            rig.frames(2);
            assert_eq!(rig.screen(), "home");
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.finish();
            let record = rig.state_file("video/guard.toml");
            assert!(
                record.contains("failed_boots = 0") && record.contains("run_open = false"),
                "the runs ended cleanly: {record}"
            );
        },
    );
}

/// A relaunch is not a new boot: a chain of runs through two games, each
/// run in its own process as the launcher starts it, counts one boot and
/// never reaches safe mode.
#[test]
#[ignore = "tier 2"]
fn a_chain_of_two_games_counts_one_boot() {
    in_children(
        &test_name(module_path!(), "a_chain_of_two_games_counts_one_boot"),
        5,
        |dir, stage| {
            let mut rig = Rig::new(dir);
            let game = NextRun {
                pads: Vec::new(),
                game: Some(neptron::relaunch::GameRef {
                    system: "nes".to_owned(),
                    path: "nes/Test Game.nes".to_owned(),
                }),
                notice: None,
            };
            let list = NextRun {
                pads: Vec::new(),
                game: None,
                notice: None,
            };
            match stage {
                0 => {
                    rig.boot(None);
                    rig.frames(2);
                }
                1 | 3 => rig.boot(Some(game)),
                _ => rig.boot(Some(list)),
            }
            rig.frames(5);
            assert_eq!(rig.shell().in_game(), stage % 2 == 1, "stage {stage}");
            assert_eq!(
                rig.shell().guard().phase(),
                Phase::Confirmed,
                "stage {stage}"
            );
            rig.finish();
            // Every run ends cleanly, and every later run continues the
            // boot of stage 0: no failed boot.
            let count = 0;
            let record = rig.state_file("video/guard.toml");
            assert!(
                record.contains(&format!("failed_boots = {count}")),
                "stage {stage}: {record}"
            );
        },
    );
}
