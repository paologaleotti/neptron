//! Scripted safety scenarios: confirm-or-revert.

use neptron_testkit::ModeFault;
use neptron_types::Modeline;
use neptron_video::{
    CONFIRM_WINDOW_NS, ConfirmError, GuardEvent, MODESET_ATTEMPTS, NO_PICTURE_LIMIT_NS, Phase,
    PictureStyle, PromptId, RevertReason, SafeReason,
};

use crate::guard_rig::{
    FRAME_NS, GuardRig, config, crt_15, crt_31, dead_edid, mode_of, pc_31_120, with_style,
};

/// Two frames: the watch starts at the first frame that sees the state.
const FRAME_SLACK_NS: u64 = 2 * FRAME_NS;

#[test]
fn a_change_with_no_confirmation_reverts_at_the_deadline() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let saves = rig.saves.len();
    let prompt = rig.propose(crt_31());
    assert_eq!(prompt.deadline_ns, rig.now() + CONFIRM_WINDOW_NS);
    rig.frame();
    assert_eq!(
        rig.wire(),
        Some(mode_of(&crt_31())),
        "the change applies at once"
    );

    rig.run_before(prompt.deadline_ns);
    assert_eq!(
        rig.guard.phase(),
        Phase::Pending(prompt),
        "no revert before the deadline"
    );
    assert_eq!(rig.wire(), Some(mode_of(&crt_31())));

    rig.run_until(prompt.deadline_ns + 1);
    let reverted = rig.time_of(GuardEvent::Reverted(RevertReason::Timeout));
    assert!(reverted.is_some_and(|t| t >= prompt.deadline_ns));
    rig.frame();
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(rig.guard.active(), &crt_15());
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
    assert_eq!(rig.saves.len(), saves, "a revert writes nothing");
}

#[test]
fn a_confirmed_change_stays_and_becomes_last_known_good() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(crt_31());
    rig.run_for(CONFIRM_WINDOW_NS / 2);
    let step = rig
        .guard
        .confirm(prompt.id, rig.now(), &rig.engine)
        .unwrap();
    assert_eq!(step.event, Some(GuardEvent::Confirmed));
    let record = step.save.unwrap();
    assert_eq!(record.last_known_good, Some(pc_31_120()));
    assert_eq!(record.failed_boots, 0);
    assert!(!record.safe_mode);

    rig.run_for(2 * CONFIRM_WINDOW_NS);
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(rig.wire(), Some(mode_of(&crt_31())));
    assert!(
        !rig.event_list()
            .iter()
            .any(|e| matches!(e, GuardEvent::Reverted(_)))
    );
}

#[test]
fn a_confirmation_that_does_not_match_is_refused() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let asked = PromptId(99);
    assert_eq!(
        rig.guard.confirm(asked, rig.now(), &rig.engine),
        Err(ConfirmError::NoPrompt { asked })
    );
    let prompt = rig.propose(crt_31());
    rig.frame();
    assert_eq!(
        rig.guard.confirm(asked, rig.now(), &rig.engine),
        Err(ConfirmError::WrongPrompt {
            asked,
            open: prompt.id
        })
    );
    assert_eq!(
        rig.guard
            .confirm(prompt.id, prompt.deadline_ns, &rig.engine),
        Err(ConfirmError::Expired {
            deadline_ns: prompt.deadline_ns,
            now_ns: prompt.deadline_ns
        })
    );
    assert_eq!(rig.guard.phase(), Phase::Pending(prompt), "nothing changed");
    assert_eq!(
        rig.guard.decline(asked, &mut rig.engine),
        Err(ConfirmError::WrongPrompt {
            asked,
            open: prompt.id
        })
    );
}

#[test]
fn a_confirmation_before_the_new_mode_flips_is_refused() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(crt_31());
    let Err(ConfirmError::NoPicture { health }) =
        rig.guard.confirm(prompt.id, rig.now(), &rig.engine)
    else {
        panic!("the old mode is refused by the new monitor");
    };
    assert!(health.refused);
    rig.frame();
    assert!(rig.guard.confirm(prompt.id, rig.now(), &rig.engine).is_ok());
}

#[test]
fn a_declined_change_reverts_at_once() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(crt_31());
    rig.frame();
    let step = rig.guard.decline(prompt.id, &mut rig.engine).unwrap();
    assert_eq!(
        step.event,
        Some(GuardEvent::Reverted(RevertReason::Declined))
    );
    assert_eq!(step.save, None);
    rig.frame();
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
    assert_eq!(rig.guard.pending(), None);
}

#[test]
fn a_new_change_while_one_is_pending_reverts_to_the_confirmed_one() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let first = rig.propose(crt_31());
    rig.run_for(CONFIRM_WINDOW_NS / 2);
    let second = rig.propose(with_style(crt_31(), PictureStyle::X2));
    assert_ne!(second.id, first.id, "a new prompt");
    assert_eq!(
        second.deadline_ns,
        rig.now() + CONFIRM_WINDOW_NS,
        "a new deadline"
    );
    assert!(
        rig.guard.confirm(first.id, rig.now(), &rig.engine).is_err(),
        "the old prompt is gone"
    );

    rig.run_until(first.deadline_ns + 1);
    assert_eq!(
        rig.guard.phase(),
        Phase::Pending(second),
        "the old deadline is gone"
    );
    rig.run_until(second.deadline_ns + 1);
    rig.frame();
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(
        rig.guard.active(),
        &crt_15(),
        "the confirmed one, not the first change"
    );
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
}

#[test]
fn choosing_the_confirmed_configuration_ends_the_pending_change() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    rig.propose(crt_31());
    rig.frame();
    let step = rig.guard.propose(crt_15(), rig.now(), &mut rig.engine);
    assert_eq!(
        step.event,
        Some(GuardEvent::Reverted(RevertReason::Superseded))
    );
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    let again = rig.guard.propose(crt_15(), rig.now(), &mut rig.engine);
    assert_eq!(again.event, None, "no change, no prompt");
}

#[test]
fn a_settings_change_with_no_confirmation_ends_a_pending_change() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    rig.propose(crt_31());
    rig.frame();
    let x3 = with_style(crt_15(), PictureStyle::X3);
    let step = rig.guard.follow_settings(x3.settings, &mut rig.engine);
    assert_eq!(
        step.event,
        Some(GuardEvent::Reverted(RevertReason::Superseded))
    );
    assert_eq!(
        rig.guard.active(),
        &x3,
        "the confirmed monitor, the new settings"
    );
    assert_eq!(rig.engine.odren().settings(), &x3.settings);
    rig.frame();
    assert_eq!(rig.wire(), Some(mode_of(&x3)));
}

#[test]
fn a_blank_during_a_pending_change_reverts_after_the_limit() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(config(dead_edid()));
    rig.frame();
    assert!(rig.engine.blanked(), "no mode exists for the change");
    assert_eq!(rig.wire(), None);
    let blank_at = rig.now();
    assert!(matches!(
        rig.guard.confirm(prompt.id, rig.now(), &rig.engine),
        Err(ConfirmError::NoPicture { .. })
    ));

    rig.run_for(NO_PICTURE_LIMIT_NS + FRAME_SLACK_NS);
    let reverted = rig
        .time_of(GuardEvent::Reverted(RevertReason::NoPicture))
        .unwrap();
    assert!(reverted >= blank_at + NO_PICTURE_LIMIT_NS - FRAME_SLACK_NS);
    assert!(reverted < prompt.deadline_ns, "before the deadline");
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
    assert!(!rig.engine.blanked());
}

#[test]
fn a_revert_target_that_the_inputs_now_refuse_is_never_set() {
    let mut rig = GuardRig::confirmed_on(crt_31(), crt_15());
    rig.propose(crt_15());
    rig.frame();
    // A limit of the driver or the GPU arrives during the change: every
    // 31 kHz mode is refused from now on, also the revert target's.
    rig.engine.set_veto(Box::new(|m: &Modeline| {
        if m.line_rate_mhz() > 20_000_000 {
            Err(neptron_safety::Rejection::NotAPanelMode {
                mode: *m,
                panel_modes: 0,
            })
        } else {
            Ok(())
        }
    }));
    let calls_before = rig.fake.mode_calls().len();
    rig.run_for(CONFIRM_WINDOW_NS + NO_PICTURE_LIMIT_NS + FRAME_SLACK_NS);

    let events = rig.event_list();
    assert!(events.contains(&GuardEvent::Reverted(RevertReason::Timeout)));
    assert!(events.contains(&GuardEvent::SafeMode(SafeReason::NoValidMode)));
    let mode_31 = mode_of(&crt_31());
    assert!(
        rig.fake.mode_calls()[calls_before..]
            .iter()
            .all(|c| c.requested != mode_31),
        "the refused revert target is never set"
    );
    assert!(
        !rig.fake.blank_calls().is_empty(),
        "the output was off first"
    );
    assert_eq!(rig.guard.phase(), Phase::Safe(SafeReason::NoValidMode));
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
}

#[test]
fn device_errors_during_a_revert_end_with_the_output_off() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(crt_31());
    rig.run_before(prompt.deadline_ns);
    let first = rig.fake.next_mode_call();
    for call in first..first + 20 {
        rig.fake.fault_mode(
            call,
            ModeFault::Device {
                reason: "EIO".to_owned(),
            },
        );
    }
    rig.run_for(4 * NO_PICTURE_LIMIT_NS);

    let events = rig.event_list();
    assert!(events.contains(&GuardEvent::Reverted(RevertReason::Timeout)));
    assert!(events.contains(&GuardEvent::SafeMode(SafeReason::NoValidMode)));
    assert_eq!(rig.wire(), None, "the refused 31 kHz mode left the wire");
    assert!(rig.engine.blanked());
    let calls = rig.fake.next_mode_call() - first;
    assert!(
        calls <= 2 * u64::from(MODESET_ATTEMPTS + 1),
        "no retry storm: {calls} set_mode calls"
    );
}
