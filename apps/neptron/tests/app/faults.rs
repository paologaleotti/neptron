//! A display that stops flipping in a game: the session gives up with
//! `DisplayStalled`, and the run goes back to the game list, which opens
//! the display again.

use neptron::{NextRun, Outcome};
use neptron_testkit::FlipFault;

use crate::rig::{Rig, in_child, test_name};

#[test]
#[ignore = "tier 2"]
fn a_stalled_display_ends_the_game_and_relaunches_the_list() {
    in_child(
        &test_name(
            module_path!(),
            "a_stalled_display_ends_the_game_and_relaunches_the_list",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            rig.finish();
            let next = NextRun {
                game: Some(neptron::relaunch::GameRef {
                    system: "nes".to_owned(),
                    path: "nes/Test Game.nes".to_owned(),
                }),
                notice: None,
            };
            rig.boot(Some(next));
            rig.frames(30);
            let from = rig.display.next_present_index();
            for index in from..from + 64 {
                rig.display.fault_flip(index, FlipFault::Lost);
            }
            let outcome = rig.until_end(64);
            let Outcome::Relaunch(next) = outcome else {
                panic!("a stall relaunches: {outcome:?}");
            };
            assert_eq!(next.game, None);
            let notice = next.notice.expect("the reason for the list");
            assert!(notice.contains("no flip completed"), "{notice}");
            rig.finish();
        },
    );
}
