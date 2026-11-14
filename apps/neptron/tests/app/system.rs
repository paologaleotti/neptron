//! The System panel: reboot and shut down end the run cleanly with the
//! outcome that the launcher turns into a reboot or a power-off.

use neptron::pages::{
    FORMAT_NEXT_RESTART, FORMAT_NEXT_RESTART_NOTICE, FORMAT_NOT_SAVED, FORMAT_NOT_SAVED_NOTICE,
    FORMAT_SHARE_LABEL, POWER_IN_GAME, POWER_NO_LAUNCHER, REBOOT_LABEL, SHUT_DOWN_LABEL,
};
use neptron::power::{POWER_OFF_EXIT_CODE, PowerRequest, REBOOT_EXIT_CODE, exit_code};
use neptron::{NextRun, Outcome};
use neptron_settings::{AUDIO_VOLUME, Level, Value};
use neptron_testkit::ModeFault;
use neptron_types::Control;
use neptron_types::Modeline;
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

/// The frames that [`confirm`] waits for the end of the run at most.
const FRAMES_TO_THE_END: u32 = 1_000;

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
    // A format ends the run only when the writer thread has written the
    // request file, a few frames later.
    let mut outcome = None;
    for _ in 0..FRAMES_TO_THE_END {
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

/// Taps OK on each notice, so the run shows the screen under it.
fn dismiss_notices(rig: &mut Rig) {
    for _ in 0..8 {
        if rig.screen() != "notice" {
            return;
        }
        rig.tap(&[Control::A]);
    }
}

/// A rig under `dir` whose global monitor is the 31 kHz arcade preset.
fn with_monitor_31(dir: &std::path::Path) -> Rig {
    let rig = Rig::new(dir);
    let config = rig.paths.config();
    std::fs::create_dir_all(&config).expect("the config folder");
    std::fs::write(
        config.join("global.toml"),
        "schema_version = 1\n\n[display]\nmonitor = \"arcade_31\"\n",
    )
    .expect("the settings");
    rig
}

fn refused() -> ModeFault {
    ModeFault::Refuse {
        reason: "a driver that lost the 31 kHz modes".to_owned(),
    }
}

/// Every 31 kHz mode that the engine tries for the 31 kHz arcade preset,
/// on a rig of its own under `dir`: each boot shows the next one, until
/// the driver has refused them all and the boot shows the safe default.
/// The guard record goes before each boot, so no boot counts.
fn modes_at_31_khz(dir: &std::path::Path) -> Vec<Modeline> {
    let mut rig = with_monitor_31(dir);
    let mut modes = Vec::new();
    for _ in 0..16 {
        // A record from the boot before; absence is fine.
        let _ = std::fs::remove_file(rig.paths.state.join("video/guard.toml"));
        rig.boot(None);
        rig.frames(90);
        let mode = rig
            .display
            .active_mode()
            .filter(|m| m.line_rate_mhz() > 20_000_000);
        rig.finish();
        let Some(mode) = mode else {
            return modes;
        };
        rig.display.fault_requested_mode(mode, refused());
        modes.push(mode);
    }
    panic!("the engine tries more than 16 modes: {modes:?}");
}

#[test]
#[ignore = "tier 2"]
fn reboots_of_boots_with_no_valid_mode_still_reach_safe_mode() {
    in_child(
        &test_name(
            module_path!(),
            "reboots_of_boots_with_no_valid_mode_still_reach_safe_mode",
        ),
        |dir| {
            let mut rig = with_monitor_31(dir);
            // The first boot keeps the 31 kHz monitor; then the driver
            // refuses every mode of it, so each later boot gives no picture
            // with the last known good, and the guard holds it failed.
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.finish();
            for mode in modes_at_31_khz(&dir.join("discover")) {
                rig.display.fault_requested_mode(mode, refused());
            }
            for boot in 1..=SAFE_MODE_FAILED_BOOTS {
                rig.boot(None);
                rig.until(300, "the safe default", |r| {
                    r.shell().guard().phase() == Phase::Safe(SafeReason::NoValidMode)
                });
                rig.frames(2);
                dismiss_notices(&mut rig);
                open_system(&mut rig);
                assert_eq!(confirm(&mut rig, REBOOT_LABEL), Outcome::Reboot);
                rig.finish();
                assert!(
                    rig.state_file("video/guard.toml")
                        .contains(&format!("failed_boots = {boot}")),
                    "boot {boot}: the failed boot stays counted: {}",
                    rig.state_file("video/guard.toml")
                );
            }
            rig.boot(None);
            assert_eq!(
                rig.shell().guard().phase(),
                Phase::Safe(SafeReason::FailedBoots)
            );
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_second_request_after_the_outcome_changes_nothing() {
    in_child(
        &test_name(
            module_path!(),
            "a_second_request_after_the_outcome_changes_nothing",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            on_the_system_panel(&mut rig);
            // Both in one frame: the run has its outcome after the first.
            rig.shell().request_power(PowerRequest::Reboot);
            rig.shell().request_power(PowerRequest::PowerOff);
            assert_eq!(rig.until_end(1), Outcome::Reboot);
            rig.finish();
        },
    );
}

/// A status file with a format offer for a partition of 56.6 GB, as the
/// storage script writes it.
pub const OFFER: &str = "state = \"partition\"\nshare = \"ram\"\n\
share_reason = \"the share partition /dev/sda4 holds no file system that blkid finds and has no /state/share-pending, so it is left untouched\"\n\
share_fix = \"format\"\nshare_fix_device = \"/dev/sda4\"\nshare_fix_start = 10240\n\
share_fix_size = 110546944\nshare_fix_type = \"7\"\n";

/// The request file that the confirm of [`OFFER`] writes.
const REQUEST: &str = "device=/dev/sda4\nstart=10240\nsize=110546944\ntype=7\n";

/// Writes `status` as the status file of the storage script.
pub fn storage_status(rig: &Rig, status: &str) {
    std::fs::create_dir_all(&rig.paths.run).expect("the run folder");
    std::fs::write(rig.paths.run.join("storage.toml"), status).expect("the status");
}

/// The labels of the rows of the System panel in the snapshot.
fn labels(rig: &mut Rig) -> Vec<String> {
    rig.shell()
        .snapshot()
        .settings
        .rows
        .iter()
        .map(|r| r.label.clone())
        .collect()
}

/// A game list run with an offer in the status file, the boot prompt kept
/// and the notices dismissed, on the System panel.
fn offered(rig: &mut Rig) {
    storage_status(rig, OFFER);
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    dismiss_notices(rig);
    open_system(rig);
}

#[test]
#[ignore = "tier 2"]
fn an_offer_shows_the_format_row_and_its_notice_says_where() {
    in_child(
        &test_name(
            module_path!(),
            "an_offer_shows_the_format_row_and_its_notice_says_where",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            storage_status(&rig, OFFER);
            rig.boot(None);
            let notices: Vec<String> = rig
                .shell()
                .snapshot()
                .notices
                .iter()
                .map(|n| n.text.clone())
                .collect();
            assert!(
                notices
                    .iter()
                    .any(|n| n.ends_with("Settings > System can format it.")),
                "{notices:?}"
            );
            rig.frames(2);
            rig.keep();
            dismiss_notices(&mut rig);
            open_system(&mut rig);
            assert_eq!(
                labels(&mut rig),
                [REBOOT_LABEL, SHUT_DOWN_LABEL, FORMAT_SHARE_LABEL]
            );
            rig.focus(FORMAT_SHARE_LABEL);
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "confirm-action");
            assert_eq!(rig.focused().as_deref(), Some("Cancel"));
            let tree = rig.tree();
            let texts: Vec<&str> = tree.rows.iter().map(|r| r.label.as_str()).collect();
            assert!(texts.contains(&"Format the share partition?"), "{texts:?}");
            rig.tap(&[Control::B]);
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn with_no_offer_the_format_row_is_absent() {
    in_child(
        &test_name(module_path!(), "with_no_offer_the_format_row_is_absent"),
        |dir| {
            let mut rig = Rig::new(dir);
            // A share in RAM for a reason that a format cannot fix.
            storage_status(
                &rig,
                "state = \"partition\"\nshare = \"ram\"\nshare_reason = \"no room\"\n",
            );
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            dismiss_notices(&mut rig);
            open_system(&mut rig);
            assert_eq!(labels(&mut rig), [REBOOT_LABEL, SHUT_DOWN_LABEL]);
            // A request that reaches the app anyway does nothing: the
            // frames panic when the run ends.
            rig.shell().request_share_format();
            rig.frames(10);
            rig.finish();
            assert!(!rig.paths.share_format_request().exists());
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn confirm_format_writes_the_request_and_ends_with_the_reboot_code() {
    in_child(
        &test_name(
            module_path!(),
            "confirm_format_writes_the_request_and_ends_with_the_reboot_code",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            offered(&mut rig);
            assert!(!rig.paths.share_format_request().exists());
            let outcome = confirm(&mut rig, FORMAT_SHARE_LABEL);
            assert_eq!(outcome, Outcome::Reboot);
            assert_eq!(
                std::fs::read_to_string(rig.paths.share_format_request()).expect("the request"),
                REQUEST,
                "the run ends only after the request is written"
            );
            assert_eq!(exit_code(&outcome), Some(REBOOT_EXIT_CODE));
            assert_eq!(rig.screen(), "confirm-action/working");
            let tree = rig.tree();
            assert!(
                tree.rows
                    .iter()
                    .any(|r| r.label == "Formatting at the next start..."),
                "{tree:?}"
            );
            rig.finish();
            assert_eq!(
                std::fs::read_to_string(rig.paths.share_format_request()).expect("the request"),
                REQUEST
            );
            assert!(!rig.paths.relaunch().exists());
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn cancel_of_the_format_writes_nothing() {
    in_child(
        &test_name(module_path!(), "cancel_of_the_format_writes_nothing"),
        |dir| {
            let mut rig = Rig::new(dir);
            offered(&mut rig);
            rig.focus(FORMAT_SHARE_LABEL);
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "confirm-action");
            // Accept on Cancel, the focus at the start.
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "system");
            rig.tap(&[Control::A]);
            rig.tap(&[Control::Right]);
            rig.tap(&[Control::B]);
            assert_eq!(rig.screen(), "system");
            assert_eq!(rig.focused().as_deref(), Some(FORMAT_SHARE_LABEL));
            rig.frames(10);
            rig.finish();
            assert!(!rig.paths.share_format_request().exists());
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn in_a_game_the_format_row_is_disabled() {
    in_child(
        &test_name(module_path!(), "in_a_game_the_format_row_is_disabled"),
        |dir| {
            let mut rig = Rig::new(dir);
            storage_status(&rig, OFFER);
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
            // The notices of the start show over the quick menu; in a game
            // the UI reads a press two frames after it.
            for _ in 0..16 {
                if rig.screen() != "notice" {
                    break;
                }
                rig.tap(&[Control::A]);
                rig.frames(3);
            }
            assert_eq!(rig.screen(), "quick-menu");
            open_system(&mut rig);
            let row = rig
                .shell()
                .snapshot()
                .settings
                .rows
                .iter()
                .find(|r| r.label == FORMAT_SHARE_LABEL)
                .cloned()
                .expect("the format row");
            assert_eq!(row.unavailable.as_deref(), Some(POWER_IN_GAME));
            rig.focus(FORMAT_SHARE_LABEL);
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "system", "a disabled row asks nothing");
            rig.shell().request_share_format();
            rig.frames(10);
            assert!(rig.shell().in_game());
            rig.finish();
            assert!(!rig.paths.share_format_request().exists());
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn with_no_launcher_the_format_row_is_disabled() {
    in_child(
        &test_name(
            module_path!(),
            "with_no_launcher_the_format_row_is_disabled",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.power = false;
            storage_status(&rig, OFFER);
            rig.boot(None);
            let notices: Vec<String> = rig
                .shell()
                .snapshot()
                .notices
                .iter()
                .map(|n| n.text.clone())
                .collect();
            assert!(
                notices
                    .iter()
                    .any(|n| n.starts_with("The share partition is not in use")),
                "{notices:?}"
            );
            assert!(
                !notices
                    .iter()
                    .any(|n| n.contains("Settings > System can format it")),
                "the row cannot run here: {notices:?}"
            );
            rig.finish();
            offered(&mut rig);
            let rows = rig.shell().snapshot().settings.rows.clone();
            let row = rows
                .iter()
                .find(|r| r.label == FORMAT_SHARE_LABEL)
                .expect("the format row");
            assert_eq!(row.unavailable.as_deref(), Some(POWER_NO_LAUNCHER));
            rig.focus(FORMAT_SHARE_LABEL);
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "system");
            rig.shell().request_share_format();
            rig.frames(10);
            rig.finish();
            assert!(!rig.paths.share_format_request().exists());
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_request_that_is_not_saved_keeps_the_run_and_says_so() {
    in_child(
        &test_name(
            module_path!(),
            "a_request_that_is_not_saved_keeps_the_run_and_says_so",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            offered(&mut rig);
            // A folder with a file in the place of the request: the rename
            // of the written file fails.
            let request = rig.paths.share_format_request();
            std::fs::create_dir_all(&request).expect("the folder");
            std::fs::write(request.join("keep"), b"").expect("a file");
            rig.focus(FORMAT_SHARE_LABEL);
            rig.tap(&[Control::A]);
            rig.tap(&[Control::Right]);
            rig.tap(&[Control::A]);
            // The frames panic when the run ends.
            rig.until(1_000, "the notice", |r| {
                r.shell()
                    .snapshot()
                    .notices
                    .iter()
                    .any(|n| n.text == FORMAT_NOT_SAVED_NOTICE)
            });
            rig.frames(10);
            dismiss_notices(&mut rig);
            assert_eq!(rig.screen(), "system");
            let row = rig
                .shell()
                .snapshot()
                .settings
                .rows
                .iter()
                .find(|r| r.label == FORMAT_SHARE_LABEL)
                .cloned()
                .expect("the format row");
            assert_eq!(row.unavailable.as_deref(), Some(FORMAT_NOT_SAVED));
            rig.shell().request_share_format();
            rig.frames(10);
            rig.finish();
            assert!(request.is_dir(), "nothing replaced the folder");
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_written_request_with_no_reboot_says_when_it_runs() {
    in_child(
        &test_name(
            module_path!(),
            "a_written_request_with_no_reboot_says_when_it_runs",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            offered(&mut rig);
            rig.shell().request_share_format();
            // A display change opens its confirmation before the writer
            // gives back the result: the run takes no reboot then.
            rig.shell()
                .set_from_protocol(
                    "display.style",
                    Level::Global,
                    &serde_json::Value::String("2x".to_owned()),
                )
                .expect("the change");
            assert!(rig.shell().guard().pending().is_some());
            // The frames panic when the run ends.
            rig.until(1_000, "the notice", |r| {
                r.shell()
                    .snapshot()
                    .notices
                    .iter()
                    .any(|n| n.text == FORMAT_NEXT_RESTART_NOTICE)
            });
            assert_eq!(
                std::fs::read_to_string(rig.paths.share_format_request()).expect("the request"),
                REQUEST
            );
            let row = rig
                .shell()
                .snapshot()
                .settings
                .rows
                .iter()
                .find(|r| r.label == FORMAT_SHARE_LABEL)
                .cloned()
                .expect("the format row");
            assert_eq!(row.unavailable.as_deref(), Some(FORMAT_NEXT_RESTART));
            rig.shell().request_share_format();
            rig.frames(10);
            rig.finish();
        },
    );
}
