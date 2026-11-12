//! Blind recovery from end to end: the 5 s hold of Select and Start in
//! `neptron-input` to `Guard::recover`, with the record written.

use neptron::NextRun;
use neptron_input::BLIND_RECOVERY_HOLD_NS;
use neptron_types::Control;
use neptron_video::{Phase, SafeReason};

use crate::rig::{Rig, in_child, test_name};

/// The frames of the hold at 60 Hz, and a margin.
const HOLD_FRAMES: u32 = (BLIND_RECOVERY_HOLD_NS / 16_000_000) as u32 + 30;

#[test]
#[ignore = "tier 2"]
fn the_hold_in_the_game_list_restores_the_safe_default() {
    in_child(
        &test_name(
            module_path!(),
            "the_hold_in_the_game_list_restores_the_safe_default",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.hold(&[Control::Select, Control::Start]);
            rig.until(HOLD_FRAMES, "the recovery", |r| {
                r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
            });
            rig.release(&[Control::Select, Control::Start]);
            rig.frames(2);
            assert!(rig.audio.written_frames() > 0, "the audio cue");
            rig.finish();
            let record = rig.state_file("video/guard.toml");
            assert!(record.contains("safe_mode = true"), "{record}");
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn the_hold_in_a_game_restores_the_safe_default_under_the_quick_menu() {
    in_child(
        &test_name(
            module_path!(),
            "the_hold_in_a_game_restores_the_safe_default_under_the_quick_menu",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
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
            rig.hold(&[Control::Select, Control::Start]);
            rig.until(HOLD_FRAMES, "the recovery", |r| {
                r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
            });
            rig.frames(1);
            assert_eq!(rig.screen(), "notice", "the notice of the safe mode");
            rig.release(&[Control::Select, Control::Start]);
            rig.finish();
            assert!(
                rig.state_file("video/guard.toml")
                    .contains("safe_mode = true")
            );
        },
    );
}
