//! The end-to-end run of tier 2: the game list, a launch, the mode
//! switches of the game, the quick menu with a display change, and the
//! quit back to the list.

use neptron::{NextRun, Outcome};
use neptron_types::Control;
use neptron_ui::snapshot::SettingValue;
use neptron_video::Phase;

use crate::rig::{Rig, in_child, in_children, test_name};

/// Frames of 10 s at 60 Hz, the confirm window, and a margin.
const CONFIRM_WINDOW_FRAMES: u32 = 660;

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

            // Run 1, a new boot: nothing was ever confirmed, so the
            // settings wait for a confirmation over the game list.
            rig.boot(None);
            rig.frames(2);
            assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
            assert_eq!(rig.screen(), "confirm");
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
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
            let record = rig.state_file("video/guard.toml");
            assert!(record.contains("id = \"generic_15\""), "{record}");

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
            rig.tap(&[Control::Select, Control::Start]);
            rig.frames(1);
            assert_eq!(rig.screen(), "quick-menu");
            assert!(rig.shell().paused());
            rig.tap(&[Control::Select, Control::Start]);
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

            // A display change with no answer goes back after 10 s.
            rig.focus("Picture style");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "options");
            rig.focus("native");
            rig.tap(&[Control::A]);
            assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
            let confirm = rig.shell().snapshot().confirm.clone();
            assert_eq!(rig.screen(), "confirm", "{confirm:?} {:?}", rig.tree());
            assert_eq!(rig.shell().config().text("display.style"), "native");
            rig.frames(CONFIRM_WINDOW_FRAMES);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(rig.shell().config().text("display.style"), "auto");

            // The same change kept reaches the game's settings file.
            assert_eq!(rig.screen(), "notice");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "quick-menu/video");
            rig.focus("Picture style");
            rig.tap(&[Control::A]);
            rig.focus("native");
            rig.tap(&[Control::A]);
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(rig.shell().config().text("display.style"), "native");

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
                game: None,
                notice: None,
            }));
            rig.frames(2);
            assert_eq!(rig.screen(), "home");
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.finish();
            let record = rig.state_file("video/guard.toml");
            assert!(
                record.contains("failed_boots = 1"),
                "one boot counted: {record}"
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
                game: Some(neptron::relaunch::GameRef {
                    system: "nes".to_owned(),
                    path: "nes/Test Game.nes".to_owned(),
                }),
                notice: None,
            };
            let list = NextRun {
                game: None,
                notice: None,
            };
            match stage {
                0 => {
                    rig.boot(None);
                    rig.frames(2);
                    rig.keep();
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
            // The confirmation of stage 0 clears the count; every later run
            // continues that boot, which counts once.
            let count = if stage == 0 { 0 } else { 1 };
            let record = rig.state_file("video/guard.toml");
            assert!(
                record.contains(&format!("failed_boots = {count}")),
                "stage {stage}: {record}"
            );
        },
    );
}
