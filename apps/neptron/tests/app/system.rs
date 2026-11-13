//! The System panel: reboot and shut down end the run cleanly with the
//! outcome that the launcher turns into a reboot or a power-off.

use neptron::pages::{POWER_IN_GAME, POWER_NO_LAUNCHER, REBOOT_LABEL, SHUT_DOWN_LABEL};
use neptron::power::{POWER_OFF_EXIT_CODE, PowerRequest, REBOOT_EXIT_CODE, exit_code};
use neptron::{NextRun, Outcome};
use neptron_settings::{AUDIO_VOLUME, Level, Value};
use neptron_types::Control;
use neptron_video::{Phase, SAFE_MODE_FAILED_BOOTS, SafeReason};

use crate::rig::{Rig, in_child, test_name};

/// A game list run with the boot prompt kept, on the System panel.
fn on_the_system_panel(rig: &mut Rig) {
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    open_system(rig);
}

/// Opens `Settings > System` from home or the quick menu.
fn open_system(rig: &mut Rig) {
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "settings");
    rig.focus("System");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "system");
}

/// The reasons of the two rows of the System panel in the snapshot.
fn reasons(rig: &mut Rig) -> Vec<Option<String>> {
    let rows = &rig.shell().snapshot().settings.rows;
    [REBOOT_LABEL, SHUT_DOWN_LABEL]
        .iter()
        .map(|label| {
            rows.iter()
                .find(|r| r.label == *label)
                .unwrap_or_else(|| panic!("no row {label}"))
                .unavailable
                .clone()
        })
        .collect()
}

/// Confirms the row `label`: accept, right to its button, accept. Returns
/// the outcome of the run.
fn confirm(rig: &mut Rig, label: &str) -> Outcome {
    rig.focus(label);
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "confirm-action");
    assert_eq!(rig.focused().as_deref(), Some("Cancel"));
    rig.tap(&[Control::Right]);
    assert_eq!(rig.focused().as_deref(), Some(label));
    rig.hold(&[Control::A]);
    // Each frame that can end the run keeps its picture, so a test can
    // look at the last one.
    let mut outcome = None;
    for _ in 0..4 {
        rig.display.capture(rig.display.next_present_index());
        outcome = rig.shell().step();
        if outcome.is_some() {
            break;
        }
    }
    let outcome = outcome.expect("the run ends");
    rig.release(&[Control::A]);
    outcome
}

#[test]
#[ignore = "tier 2"]
fn confirm_reboot_ends_the_run_with_the_reboot_code() {
    in_child(
        &test_name(
            module_path!(),
            "confirm_reboot_ends_the_run_with_the_reboot_code",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            on_the_system_panel(&mut rig);
            assert_eq!(reasons(&mut rig), [None, None]);
            let modes = rig.display.mode_calls().len();
            let outcome = confirm(&mut rig, REBOOT_LABEL);
            assert_eq!(outcome, Outcome::Reboot);
            assert_eq!(exit_code(&outcome), Some(REBOOT_EXIT_CODE));
            // The last frame says what happens, through the normal present
            // in the mode on screen: no mode call.
            assert_eq!(rig.screen(), "confirm-action/working");
            let last = rig.display.next_present_index() - 1;
            let shown = rig.display.captured(last).expect("the last frame");
            assert_eq!(
                Some(shown),
                rig.display.screen(),
                "it is the frame on screen"
            );
            assert_eq!(rig.display.mode_calls().len(), modes);
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn confirm_shut_down_writes_the_settings_and_ends_with_the_power_off_code() {
    in_child(
        &test_name(
            module_path!(),
            "confirm_shut_down_writes_the_settings_and_ends_with_the_power_off_code",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            rig.focus("Settings");
            rig.tap(&[Control::A]);
            rig.focus("Audio");
            rig.tap(&[Control::A]);
            rig.focus("Volume");
            rig.tap(&[Control::Left]);
            rig.tap(&[Control::B]);
            rig.focus("System");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "system");
            let outcome = confirm(&mut rig, SHUT_DOWN_LABEL);
            assert_eq!(outcome, Outcome::PowerOff);
            assert_eq!(exit_code(&outcome), Some(POWER_OFF_EXIT_CODE));
            assert_eq!(
                rig.shell().config().get(Level::Global, AUDIO_VOLUME),
                Some(&Value::Int(75))
            );
            rig.finish();
            let file = std::fs::read_to_string(rig.paths.config().join("global.toml"))
                .expect("the global file");
            assert!(file.contains("volume = 75"), "{file}");
            assert!(
                !rig.paths.relaunch().exists(),
                "a power-off writes no relaunch file"
            );
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn cancel_and_back_change_nothing() {
    in_child(
        &test_name(module_path!(), "cancel_and_back_change_nothing"),
        |dir| {
            let mut rig = Rig::new(dir);
            on_the_system_panel(&mut rig);
            for label in [REBOOT_LABEL, SHUT_DOWN_LABEL] {
                rig.focus(label);
                rig.tap(&[Control::A]);
                assert_eq!(rig.screen(), "confirm-action");
                rig.tap(&[Control::A]);
                assert_eq!(rig.screen(), "system", "accept on Cancel");
                assert_eq!(rig.focused().as_deref(), Some(label));
                rig.tap(&[Control::A]);
                rig.tap(&[Control::Right]);
                rig.tap(&[Control::B]);
                assert_eq!(rig.screen(), "system", "back cancels");
                assert_eq!(rig.focused().as_deref(), Some(label));
            }
            // The frames panic if the run ends.
            rig.frames(10);
            rig.tap(&[Control::B]);
            assert_eq!(rig.screen(), "settings");
            assert_eq!(rig.focused().as_deref(), Some("System"));
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn in_a_game_both_rows_are_disabled() {
    in_child(
        &test_name(module_path!(), "in_a_game_both_rows_are_disabled"),
        |dir| {
            let mut rig = Rig::new(dir);
            let game = neptron::relaunch::GameRef {
                system: "nes".to_owned(),
                path: "nes/Test Game.nes".to_owned(),
            };
            rig.boot(Some(NextRun {
                game: Some(game),
                notice: None,
            }));
            rig.frames(30);
            rig.tap(&[Control::Select, Control::Start]);
            rig.frames(1);
            assert_eq!(rig.screen(), "quick-menu");
            open_system(&mut rig);
            let reason = Some(POWER_IN_GAME.to_owned());
            assert_eq!(reasons(&mut rig), [reason.clone(), reason]);
            for label in [REBOOT_LABEL, SHUT_DOWN_LABEL] {
                rig.focus(label);
                rig.tap(&[Control::A]);
                assert_eq!(rig.screen(), "system", "a disabled row asks nothing");
            }
            // A request that reaches the app anyway, as the intent of the
            // row would, does nothing in a game: the frames panic when the
            // run ends.
            for request in [PowerRequest::Reboot, PowerRequest::PowerOff] {
                rig.shell().request_power(request);
                rig.frames(10);
            }
            assert!(rig.shell().in_game());
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn with_no_launcher_both_rows_are_disabled() {
    in_child(
        &test_name(module_path!(), "with_no_launcher_both_rows_are_disabled"),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.power = false;
            on_the_system_panel(&mut rig);
            let reason = Some(POWER_NO_LAUNCHER.to_owned());
            assert_eq!(reasons(&mut rig), [reason.clone(), reason]);
            rig.focus(SHUT_DOWN_LABEL);
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "system");
            // A request that reaches the app anyway does nothing with no
            // power control: the frames panic when the run ends.
            for request in [PowerRequest::Reboot, PowerRequest::PowerOff] {
                rig.shell().request_power(request);
                rig.frames(10);
            }
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn quick_reboots_in_a_row_do_not_give_safe_mode() {
    in_child(
        &test_name(
            module_path!(),
            "quick_reboots_in_a_row_do_not_give_safe_mode",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            // A reboot before the good boot (10 s of picture) would count
            // as a failed boot without its own rule.
            for boot in 0..=SAFE_MODE_FAILED_BOOTS {
                rig.boot(None);
                rig.frames(2);
                assert!(
                    !matches!(
                        rig.shell().guard().phase(),
                        Phase::Safe(SafeReason::FailedBoots)
                    ),
                    "boot {boot}: {:?}",
                    rig.shell().guard().phase()
                );
                // The first boot keeps its picture, which makes it good;
                // the later ones show it with no prompt and reboot at once.
                if boot == 0 {
                    rig.keep();
                }
                open_system(&mut rig);
                assert_eq!(rig.shell().guard().boot_good(), boot == 0);
                assert_eq!(confirm(&mut rig, REBOOT_LABEL), Outcome::Reboot);
                rig.finish();
                assert!(
                    rig.state_file("video/guard.toml")
                        .contains("failed_boots = 0"),
                    "boot {boot}: {}",
                    rig.state_file("video/guard.toml")
                );
            }
            rig.boot(None);
            assert!(
                !matches!(rig.shell().guard().phase(), Phase::Safe(_)),
                "{:?}",
                rig.shell().guard().phase()
            );
            rig.finish();
        },
    );
}
