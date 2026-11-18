//! Blind recovery from end to end: the 5 s hold of Select and Start in
//! `neptron-input` to `Guard::recover`, with the record written.

use neptron::NextRun;
use neptron_input::{BLIND_RECOVERY_HOLD_NS, QUICK_MENU_HOLD_NS};
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
fn the_hold_in_a_game_restores_the_safe_default_with_a_tone_under_the_quick_menu() {
    in_child(
        &test_name(
            module_path!(),
            "the_hold_in_a_game_restores_the_safe_default_with_a_tone_under_the_quick_menu",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
            rig.finish();
            let next = NextRun {
                pads: Vec::new(),
                game: Some(neptron::relaunch::GameRef {
                    system: "nes".to_owned(),
                    path: "nes/Test Game.nes".to_owned(),
                }),
                notice: None,
            };
            rig.boot(Some(next));
            rig.frames(30);
            rig.hold(&[Control::Select, Control::Start]);
            rig.hold_for(QUICK_MENU_HOLD_NS);
            assert!(rig.shell().paused(), "the hold opens the quick menu first");
            assert_ne!(
                rig.shell().guard().phase(),
                Phase::Safe(SafeReason::Recovery),
                "no recovery yet"
            );
            rig.until(HOLD_FRAMES, "the recovery", |r| {
                r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
            });
            rig.release(&[Control::Select, Control::Start]);
            rig.frames(30);
            let stats = rig.shell().audio_stats().expect("the audio");
            // The tone is 200 ms at the 48 kHz of the fake sink.
            assert_eq!(stats.cue_frames, 9_600, "the whole tone");
            assert_eq!(stats.overrun_frames, 0);
            assert!(rig.shell().paused(), "the game stays paused");
            // The quick menu is open, so the notice shows at once.
            assert_eq!(rig.screen(), "notice", "the notice of the safe mode");
            rig.finish();
            assert!(
                rig.state_file("video/guard.toml")
                    .contains("safe_mode = true")
            );
        },
    );
}

/// In safe mode a picture style still waits for a Keep: the monitor on
/// screen is the safe default, not a kept configuration.
#[test]
#[ignore = "tier 2"]
fn a_picture_style_in_safe_mode_opens_the_dialog() {
    in_child(
        &test_name(
            module_path!(),
            "a_picture_style_in_safe_mode_opens_the_dialog",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.scanned();
            rig.hold(&[Control::Select, Control::Start]);
            rig.until(HOLD_FRAMES, "the recovery", |r| {
                r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
            });
            rig.release(&[Control::Select, Control::Start]);
            rig.frames(2);
            while rig.screen() == "notice" {
                rig.tap(&[Control::A]);
            }
            rig.focus("Settings");
            rig.tap(&[Control::A]);
            rig.focus("Display");
            rig.tap(&[Control::A]);
            rig.focus("Picture style");
            rig.tap(&[Control::A]);
            rig.focus("native");
            rig.tap(&[Control::A]);
            assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
            assert_eq!(rig.screen(), "confirm");
            rig.finish();
        },
    );
}

/// The picture on screen in safe mode can be kept from the Display panel,
/// and keeping it ends safe mode for the next boots.
#[test]
#[ignore = "tier 2"]
fn safe_mode_ends_when_the_picture_on_screen_is_kept() {
    in_child(
        &test_name(
            module_path!(),
            "safe_mode_ends_when_the_picture_on_screen_is_kept",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.scanned();
            rig.hold(&[Control::Select, Control::Start]);
            rig.until(HOLD_FRAMES, "the recovery", |r| {
                r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
            });
            rig.release(&[Control::Select, Control::Start]);
            rig.frames(2);
            while rig.screen() == "notice" {
                rig.tap(&[Control::A]);
            }
            rig.focus("Settings");
            rig.tap(&[Control::A]);
            rig.focus("Display");
            rig.tap(&[Control::A]);
            rig.focus(neptron::pages::KEEP_PICTURE_LABEL);
            rig.tap(&[Control::A]);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            let labels: Vec<String> = rig
                .shell()
                .snapshot()
                .settings
                .rows
                .iter()
                .map(|r| r.label.clone())
                .collect();
            assert!(
                !labels
                    .iter()
                    .any(|l| l == neptron::pages::KEEP_PICTURE_LABEL),
                "the row goes with safe mode: {labels:?}"
            );
            rig.finish();
            let record = rig.state_file("video/guard.toml");
            assert!(record.contains("safe_mode = false"), "{record}");

            rig.boot(None);
            rig.frames(2);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.finish();
        },
    );
}

/// The texts of the safe-mode notices of the run.
fn safe_notices(rig: &mut Rig) -> usize {
    rig.shell()
        .snapshot()
        .notices
        .iter()
        .filter(|n| n.text.starts_with("Safe mode"))
        .count()
}

/// A start in safe mode shows its notice once per power cycle, not at
/// every relaunch or restart.
#[test]
#[ignore = "tier 2"]
fn the_safe_mode_notice_of_a_start_shows_once_per_power_cycle() {
    in_child(
        &test_name(
            module_path!(),
            "the_safe_mode_notice_of_a_start_shows_once_per_power_cycle",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.scanned();
            rig.hold(&[Control::Select, Control::Start]);
            rig.until(HOLD_FRAMES, "the recovery", |r| {
                r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
            });
            rig.release(&[Control::Select, Control::Start]);
            rig.frames(2);
            assert_eq!(safe_notices(&mut rig), 1, "the hold always says it");
            rig.finish();

            // A new power cycle: the run folder is empty.
            std::fs::remove_dir_all(&rig.paths.run).expect("the run folder");
            rig.boot(None);
            assert_eq!(
                rig.shell().guard().phase(),
                Phase::Safe(SafeReason::Recovery)
            );
            assert_eq!(safe_notices(&mut rig), 1, "the first start says it");
            rig.finish();
            for next in [
                None,
                Some(NextRun {
                    pads: Vec::new(),
                    game: None,
                    notice: None,
                }),
            ] {
                rig.boot(next);
                assert_eq!(
                    rig.shell().guard().phase(),
                    Phase::Safe(SafeReason::Recovery)
                );
                assert_eq!(safe_notices(&mut rig), 0, "said once");
                rig.finish();
            }
        },
    );
}
