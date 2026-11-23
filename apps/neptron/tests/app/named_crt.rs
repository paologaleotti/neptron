//! A named 15 kHz preset on a VGA output whose EDID reports a PC monitor
//! of 30 to 71 kHz: a 15 kHz CRT behind an EDID dongle, a KVM or a
//! splitter. No mode above the highest line rate of the preset reaches the
//! display: not at an unanswered boot prompt, at the revert of a menu
//! change, in safe mode after failed boots, or at the blind recovery.

use std::path::Path;

use neptron_input::BLIND_RECOVERY_HOLD_NS;
use neptron_testkit::preset::monitor_preset;
use neptron_types::{Control, MonitorKind};
use neptron_video::{Phase, SAFE_MODE_FAILED_BOOTS, SafeReason};

use crate::output::{CRT_ANALOG, sysfs_with_vga};
use crate::panel::CONFIRM_WINDOW_FRAMES;
use crate::rig::{Rig, in_child, test_name};

/// The 15 kHz presets that an owner names for such a display.
const NAMED: [&str; 2] = ["arcade_15", "generic_15"];

/// The frames of the blind recovery hold at 60 Hz, and a margin.
const HOLD_FRAMES: u32 = (BLIND_RECOVERY_HOLD_NS / 16_000_000) as u32 + 30;

/// A rig under `dir/preset` with a VGA output that reports the PC-range
/// analog EDID, and `display.monitor = preset` in the global settings.
fn named_rig(dir: &Path, preset: &str) -> Rig {
    let root = dir.join(preset);
    std::fs::create_dir_all(&root).expect("the folder");
    let mut rig = Rig::new(&root);
    rig.output = "VGA-1".to_owned();
    sysfs_with_vga(&rig.paths.sysfs, CRT_ANALOG);
    let config = rig.paths.config();
    std::fs::create_dir_all(&config).expect("the config folder");
    std::fs::write(
        config.join("global.toml"),
        format!("schema_version = 1\n\n[display]\nmonitor = \"{preset}\"\n"),
    )
    .expect("the settings");
    rig
}

/// The highest line rate of the shipped preset `id`, in millihertz.
fn highest_line_rate_mhz(id: &str) -> u64 {
    match monitor_preset(id).expect("the preset").kind {
        MonitorKind::Crt { ranges } => ranges.iter().map(|r| r.h_max_mhz).max().expect("a range"),
        MonitorKind::FlatPanel { .. } | MonitorKind::Tv { .. } => panic!("{id} is no CRT"),
    }
}

/// Every mode that reached the display has a line rate inside `preset`.
fn assert_inside(rig: &Rig, preset: &str) {
    let max = highest_line_rate_mhz(preset);
    let calls = rig.display.mode_calls();
    assert!(!calls.is_empty(), "{preset}: no mode reached the display");
    for call in calls {
        assert!(
            call.requested.line_rate_mhz() <= max,
            "{preset}: call {} sent {} mHz, above {max} mHz: {:?}",
            call.call,
            call.requested.line_rate_mhz(),
            call.requested
        );
    }
}

/// Waits for the end of the boot prompt, if one opened.
fn past_the_boot_prompt(rig: &mut Rig) {
    rig.frames(2);
    if matches!(rig.shell().guard().phase(), Phase::Pending(_)) {
        rig.frames(CONFIRM_WINDOW_FRAMES);
    }
    assert!(
        !matches!(rig.shell().guard().phase(), Phase::Pending(_)),
        "the boot prompt ended"
    );
}

#[test]
#[ignore = "tier 2"]
fn an_unanswered_boot_prompt_stays_inside_the_named_preset() {
    in_child(
        &test_name(
            module_path!(),
            "an_unanswered_boot_prompt_stays_inside_the_named_preset",
        ),
        |dir| {
            for preset in NAMED {
                let mut rig = named_rig(dir, preset);
                rig.boot(None);
                past_the_boot_prompt(&mut rig);
                rig.frames(10);
                assert_inside(&rig, preset);
                rig.finish();
                let settings = std::fs::read_to_string(rig.paths.config().join("global.toml"))
                    .expect("the settings");
                assert!(
                    !settings.contains("monitor = \"auto\""),
                    "{preset}: the revert names auto: {settings}"
                );

                // The next boot starts from what the revert wrote.
                rig.boot(None);
                past_the_boot_prompt(&mut rig);
                rig.frames(10);
                assert_inside(&rig, preset);
                rig.finish();
            }
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_reverted_menu_change_stays_inside_the_named_preset() {
    in_child(
        &test_name(
            module_path!(),
            "a_reverted_menu_change_stays_inside_the_named_preset",
        ),
        |dir| {
            for preset in NAMED {
                let mut rig = named_rig(dir, preset);
                rig.boot(None);
                past_the_boot_prompt(&mut rig);
                rig.scanned();
                while rig.screen() == "notice" {
                    rig.tap(&[Control::A]);
                }
                rig.focus("Settings");
                rig.tap(&[Control::A]);
                rig.focus("Display");
                rig.tap(&[Control::A]);
                rig.wait_for_picture_answers();
                // On a confirmed monitor a picture mode applies with no
                // prompt; in safe mode it waits for a Keep and reverts to
                // the safe default.
                let safe = matches!(rig.shell().guard().phase(), Phase::Safe(_));
                rig.focus("Picture mode");
                rig.tap(&[Control::A]);
                rig.focus("Native");
                rig.tap(&[Control::A]);
                assert_eq!(
                    matches!(rig.shell().guard().phase(), Phase::Pending(_)),
                    safe,
                    "{preset}: {:?}",
                    rig.shell().guard().phase()
                );
                rig.frames(CONFIRM_WINDOW_FRAMES);
                assert!(!matches!(rig.shell().guard().phase(), Phase::Pending(_)));
                rig.frames(10);
                assert_inside(&rig, preset);
                rig.finish();
            }
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn safe_mode_after_failed_boots_stays_inside_the_named_preset() {
    in_child(
        &test_name(
            module_path!(),
            "safe_mode_after_failed_boots_stays_inside_the_named_preset",
        ),
        |dir| {
            for preset in NAMED {
                let mut rig = named_rig(dir, preset);
                for _ in 0..SAFE_MODE_FAILED_BOOTS {
                    rig.boot(None);
                    rig.frames(2);
                    // The run dies: no `finish`, so no clean exit.
                    rig.shell = None;
                }
                rig.boot(None);
                assert_eq!(
                    rig.shell().guard().phase(),
                    Phase::Safe(SafeReason::FailedBoots),
                    "{preset}"
                );
                rig.frames(10);
                assert_inside(&rig, preset);
                rig.finish();
            }
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn the_blind_recovery_stays_inside_the_named_preset() {
    in_child(
        &test_name(
            module_path!(),
            "the_blind_recovery_stays_inside_the_named_preset",
        ),
        |dir| {
            for preset in NAMED {
                let mut rig = named_rig(dir, preset);
                rig.boot(None);
                rig.frames(2);
                if rig.shell().guard().pending().is_some() {
                    rig.keep();
                }
                assert_eq!(rig.shell().guard().phase(), Phase::Confirmed, "{preset}");
                rig.hold(&[Control::Select, Control::Start]);
                rig.until(HOLD_FRAMES, "the recovery", |r| {
                    r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
                });
                rig.release(&[Control::Select, Control::Start]);
                rig.frames(10);
                assert_inside(&rig, preset);
                rig.finish();

                // The next boot starts in safe mode, on the same default.
                rig.boot(None);
                assert_eq!(
                    rig.shell().guard().phase(),
                    Phase::Safe(SafeReason::Recovery),
                    "{preset}"
                );
                rig.frames(10);
                assert_inside(&rig, preset);
                rig.finish();
            }
        },
    );
}
