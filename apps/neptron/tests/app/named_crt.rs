//! A named CRT preset on a VGA output whose EDID reports a PC monitor of
//! 30 to 71 kHz: for a 15 kHz preset, a 15 kHz CRT behind an EDID dongle,
//! a KVM or a splitter. Every mode that reaches the display passes the
//! validator for the named preset: at an unanswered boot prompt, at the
//! revert of a menu change, in safe mode after failed boots, and at the
//! blind recovery. The safe default is the one the rule gives.

use std::path::Path;

use neptron_backend::Display;
use neptron_input::BLIND_RECOVERY_HOLD_NS;
use neptron_testkit::ModeRecord;
use neptron_testkit::preset::validated_mode;
use neptron_types::Control;
use neptron_video::{Phase, SAFE_MODE_FAILED_BOOTS, SafeReason};

use crate::output::{CRT_ANALOG, sysfs_with_vga};
use crate::panel::CONFIRM_WINDOW_FRAMES;
use crate::rig::{Rig, in_child, test_name};

/// The presets named for such a display, and the id of the safe default
/// that the rule gives for each on the 30 to 71 kHz EDID: the 15 kHz safe
/// start for a 15 kHz preset, the preset itself when it lies inside the
/// EDID, and their common part (field rates from 50 Hz) for `arcade_31`,
/// whose lowest field rate of 49.5 Hz is under that of the EDID.
const NAMED: [(&str, &str); 4] = [
    ("arcade_15", "generic_15"),
    ("generic_15", "generic_15"),
    ("pc_31_120", "pc_31_120"),
    ("arcade_31", "arcade_31@edid_npt_0071"),
];

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

/// Every mode of `calls`, and the mode on the wire, pass the validator
/// for the shipped preset `id` and the caps of the display. A step that
/// keeps the mode on the wire sends no new mode.
fn assert_valid(rig: &Rig, calls: &[ModeRecord], id: &str) {
    let caps = *rig.display.caps();
    for call in calls {
        if let Err(e) = validated_mode(&call.requested, id, &caps) {
            panic!("call {} is no mode of {id}: {e}", call.call);
        }
    }
    let wire = rig.display.active_mode().expect("a mode on the wire");
    if let Err(e) = validated_mode(&wire, id, &caps) {
        panic!("the mode on the wire is no mode of {id}: {e}");
    }
}

/// Every mode that reached the display passes the validator for the named
/// preset.
fn assert_inside(rig: &Rig, preset: &str) {
    let calls = rig.display.mode_calls();
    assert!(!calls.is_empty(), "{preset}: no mode reached the display");
    assert_valid(rig, &calls, preset);
}

/// The run's safe default is `safe`, and every mode sent from call `from`
/// on, in a safe state, passes the validator for it when it is a shipped
/// preset; a common part has no file, and its modes pass for the named
/// preset.
fn assert_safe(rig: &mut Rig, from: usize, preset: &str, safe: &str) {
    assert_eq!(
        rig.shell().guard().safe().monitor.profile().id,
        safe,
        "{preset}"
    );
    let calls = rig.display.mode_calls();
    let shipped = if safe.contains('@') { preset } else { safe };
    assert_valid(rig, &calls[from..], shipped);
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
            for (preset, safe) in NAMED {
                let mut rig = named_rig(dir, preset);
                rig.boot(None);
                let prompted = rig.shell().guard().pending().is_some();
                let from = rig.display.mode_calls().len();
                past_the_boot_prompt(&mut rig);
                rig.frames(10);
                assert_inside(&rig, preset);
                if prompted {
                    assert_safe(&mut rig, from, preset, safe);
                }
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
            for (preset, safe_id) in NAMED {
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
                let from = rig.display.mode_calls().len();
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
                let after = rig.display.mode_calls()[from..].to_vec();
                assert_valid(&rig, &after, preset);
                if safe {
                    assert_safe(&mut rig, from, preset, safe_id);
                }
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
            for (preset, safe) in NAMED {
                let mut rig = named_rig(dir, preset);
                for _ in 0..SAFE_MODE_FAILED_BOOTS {
                    rig.boot(None);
                    rig.frames(2);
                    // The run dies: no `finish`, so no clean exit.
                    rig.shell = None;
                }
                let from = rig.display.mode_calls().len();
                rig.boot(None);
                assert_eq!(
                    rig.shell().guard().phase(),
                    Phase::Safe(SafeReason::FailedBoots),
                    "{preset}"
                );
                rig.frames(10);
                assert_inside(&rig, preset);
                assert_safe(&mut rig, from, preset, safe);
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
            for (preset, safe) in NAMED {
                let mut rig = named_rig(dir, preset);
                rig.boot(None);
                rig.frames(2);
                if rig.shell().guard().pending().is_some() {
                    rig.keep();
                }
                assert_eq!(rig.shell().guard().phase(), Phase::Confirmed, "{preset}");
                let from = rig.display.mode_calls().len();
                rig.hold(&[Control::Select, Control::Start]);
                rig.until(HOLD_FRAMES, "the recovery", |r| {
                    r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
                });
                rig.release(&[Control::Select, Control::Start]);
                rig.frames(10);
                assert_inside(&rig, preset);
                assert_safe(&mut rig, from, preset, safe);
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
