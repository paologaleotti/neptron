//! The session and the UI agree on the pause: a paused game always has the
//! quick menu or a dialog on screen.

use neptron::NextRun;
use neptron_settings::Level;
use neptron_types::Control;

use crate::rig::{Rig, in_child, test_name};

/// The game of the test core.
pub fn game() -> NextRun {
    NextRun {
        game: Some(neptron::relaunch::GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    }
}

/// A rig in a game run, with the display kept in the game list before.
pub fn in_game(dir: &std::path::Path) -> Rig {
    let mut rig = Rig::new(dir);
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    rig.finish();
    rig.boot(Some(game()));
    rig.frames(30);
    rig
}

#[test]
#[ignore = "tier 2"]
fn the_menu_hotkey_under_a_confirm_dialog_opens_the_quick_menu_after_it() {
    in_child(
        &test_name(
            module_path!(),
            "the_menu_hotkey_under_a_confirm_dialog_opens_the_quick_menu_after_it",
        ),
        |dir| {
            let mut rig = in_game(dir);
            rig.shell()
                .set_from_protocol(
                    "display.style",
                    Level::Game,
                    &serde_json::Value::String("native".to_owned()),
                )
                .expect("the change");
            rig.frames(2);
            assert_eq!(rig.screen(), "confirm");
            assert!(!rig.shell().paused());

            rig.tap(&[Control::Select, Control::Start]);
            rig.frames(2);
            assert!(rig.shell().paused());
            assert_eq!(rig.screen(), "confirm", "the dialog stays on top");

            rig.tap(&[Control::B]);
            rig.frames(2);
            assert!(rig.shell().guard().pending().is_none());
            assert!(rig.shell().paused());
            assert_eq!(rig.screen(), "notice", "the revert notice over the menu");
            rig.tap(&[Control::A]);
            assert!(rig.shell().paused());
            assert_eq!(rig.screen(), "quick-menu", "a paused game shows the menu");

            rig.tap(&[Control::B]);
            rig.frames(2);
            assert!(!rig.shell().paused());
            assert_eq!(rig.screen(), "game");
            rig.finish();
        },
    );
}
