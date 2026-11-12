//! The end-to-end run of tier 2: the game list, a launch, the mode
//! switches of the game, the quick menu with a display change, and the
//! quit back to the list.

use neptron::{NextRun, Outcome};
use neptron_types::Control;
use neptron_video::Phase;

use crate::rig::{Rig, in_child, test_name};

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
            assert_eq!(rig.screen(), "quick-menu");
            rig.focus("Picture style");
            rig.tap(&[Control::A]);
            rig.focus("native");
            rig.tap(&[Control::A]);
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(rig.shell().config().text("display.style"), "native");

            // Quit: the quick menu asks where to keep the change.
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
