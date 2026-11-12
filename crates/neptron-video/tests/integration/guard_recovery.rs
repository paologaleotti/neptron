//! Scripted safety scenarios: the blind recovery in each state.
//!
//! `neptron-input` turns the 5 s hold of Select and Start into one
//! `Hotkey::BlindRecovery` and tests the hold in simulated time. These
//! scenarios start where that hotkey reaches the guard. The recovery must
//! work with no picture, so no scenario reads the screen first.

use neptron_testkit::ModeFault;
use neptron_video::{GuardEvent, GuardRecord, MODESET_ATTEMPTS, Phase, SafeReason};

use crate::guard_rig::{FRAME_NS, GuardRig, config, confirmed, crt_15, crt_31, dead_edid, mode_of};

/// Recovers, then checks that the safe default is on the wire and will be
/// at the next boot.
fn assert_recovers(rig: &mut GuardRig) {
    let saves = rig.saves.len();
    rig.recover();
    assert_eq!(
        rig.events.last().map(|(_, e)| *e),
        Some(GuardEvent::SafeMode(SafeReason::Recovery))
    );
    assert_eq!(rig.guard.phase(), Phase::Safe(SafeReason::Recovery));
    assert_eq!(rig.guard.pending(), None, "any dialog closes");
    for _ in 0..3 {
        rig.frame();
    }
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
    assert!(!rig.engine.blanked());
    assert!(!rig.engine.current_refused());

    let record = rig.guard.record().clone();
    assert!(record.safe_mode);
    if saves < rig.saves.len() {
        assert_eq!(rig.saves[saves], record);
    }
    let next = GuardRig::boot(record, crt_31(), crt_15());
    assert!(matches!(next.guard.phase(), Phase::Safe(_)));
}

#[test]
fn recovery_from_a_confirmed_configuration() {
    let mut rig = GuardRig::confirmed_on(crt_31(), crt_15());
    rig.run_for(10 * FRAME_NS);
    assert_recovers(&mut rig);
    assert_eq!(
        rig.saves.last().unwrap().last_known_good,
        Some(crt_31().monitor)
    );
}

#[test]
fn recovery_while_a_confirmation_is_open() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(crt_31());
    rig.frame();
    assert_recovers(&mut rig);
    rig.run_until(prompt.deadline_ns + FRAME_NS);
    assert!(
        !rig.event_list()
            .iter()
            .any(|e| matches!(e, GuardEvent::Reverted(_))),
        "the closed prompt does not revert later"
    );
    assert_eq!(rig.guard.phase(), Phase::Safe(SafeReason::Recovery));
}

#[test]
fn recovery_while_the_output_is_off() {
    let mut rig = GuardRig::confirmed_on(crt_31(), crt_15());
    rig.propose(config(dead_edid()));
    rig.frame();
    assert!(rig.engine.blanked());
    assert_recovers(&mut rig);
}

#[test]
fn recovery_while_a_refused_mode_stays_after_failed_blanks() {
    let mut rig = GuardRig::confirmed_on(crt_31(), crt_15());
    for call in 0..u64::from(MODESET_ATTEMPTS) {
        rig.fake.fault_blank(call, "EIO".to_owned());
    }
    rig.propose(config(dead_edid()));
    for _ in 0..MODESET_ATTEMPTS + 1 {
        rig.frame();
    }
    assert!(rig.engine.current_refused());
    assert_eq!(
        rig.wire(),
        Some(mode_of(&crt_31())),
        "the refused mode stays"
    );
    assert_recovers(&mut rig);
}

#[test]
fn recovery_in_safe_mode_after_failed_boots() {
    let record = GuardRecord {
        failed_boots: 5,
        ..confirmed(crt_31().monitor)
    };
    let mut rig = GuardRig::boot(record, crt_31(), crt_15());
    rig.frame();
    assert_recovers(&mut rig);
}

#[test]
fn recovery_at_boot_before_the_first_frame() {
    let mut rig = GuardRig::boot(confirmed(crt_31().monitor), crt_31(), crt_15());
    assert_recovers(&mut rig);
}

#[test]
fn recovery_after_device_errors_blocked_the_source() {
    let mut rig = GuardRig::confirmed_on(crt_31(), crt_15());
    let first = rig.fake.next_mode_call();
    for call in first..first + u64::from(MODESET_ATTEMPTS) + 1 {
        rig.fake.fault_mode(
            call,
            ModeFault::Device {
                reason: "EIO".to_owned(),
            },
        );
    }
    // Behind the guard's back, so that the engine blocks the source with
    // the guard still in its confirmed state.
    rig.engine.set_monitor(crt_15().monitor);
    for _ in 0..MODESET_ATTEMPTS + 2 {
        rig.frame();
    }
    assert_ne!(rig.wire(), Some(mode_of(&crt_15())));
    assert_recovers(&mut rig);
}
