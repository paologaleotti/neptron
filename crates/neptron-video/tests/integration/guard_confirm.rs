//! Scripted safety scenarios: confirm-or-revert.

use neptron_testkit::{FlipFault, ModeFault};
use neptron_types::{Modeline, Rotation, SourceGeometry};
use neptron_video::{
    CONFIRM_WINDOW_NS, ConfirmError, DisplayConfig, GuardEvent, Health, MODESET_ATTEMPTS,
    NO_PICTURE_LIMIT_NS, Outcome, PENDING_LIMIT_NS, Phase, PictureStyle, Prompt, PromptId,
    RevertReason, SafeReason, Unshowable, VideoError,
};

use crate::guard_rig::{
    FRAME_NS, GuardRig, arcade_15_25_31, config, confirmed, crt_15, crt_31, dead_edid, mode_of,
    pc_31_120, reverted, with_style,
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
    let reverted = rig.time_of(reverted(RevertReason::Timeout));
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
            .any(|e| matches!(e, GuardEvent::Reverted { .. }))
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
fn a_confirmation_before_a_frame_of_the_change_is_refused() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let old = rig.wire();
    let multi = with_style(config(arcade_15_25_31()), PictureStyle::X2);
    let prompt = rig.propose(multi.clone());
    assert!(
        Health::of(&rig.engine).picture,
        "the old mode is valid for the new monitor, so the engine shows a picture"
    );
    assert_eq!(
        rig.guard.confirm(prompt.id, rig.now(), &rig.engine),
        Err(ConfirmError::NotShown)
    );
    assert_eq!(rig.guard.phase(), Phase::Pending(prompt));
    assert!(rig.engine.odren().decision_due());

    rig.frame();
    assert!(!rig.engine.odren().decision_due());
    assert_ne!(rig.wire(), old, "the frame set the mode of the change");
    assert_eq!(rig.wire(), Some(mode_of(&multi)));
    assert!(rig.guard.confirm(prompt.id, rig.now(), &rig.engine).is_ok());
}

#[test]
fn a_declined_change_reverts_at_once() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(crt_31());
    rig.frame();
    let step = rig.guard.decline(prompt.id, &mut rig.engine).unwrap();
    assert_eq!(step.event, Some(reverted(RevertReason::Declined)));
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
    assert_eq!(step.event, Some(reverted(RevertReason::Superseded)));
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
    assert_eq!(step.event, Some(reverted(RevertReason::Superseded)));
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
    let reverted = rig.time_of(reverted(RevertReason::NoPicture)).unwrap();
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
    assert!(events.contains(&reverted(RevertReason::Timeout)));
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
    assert!(events.contains(&reverted(RevertReason::Timeout)));
    assert!(events.contains(&GuardEvent::SafeMode(SafeReason::NoValidMode)));
    assert_eq!(rig.wire(), None, "the refused 31 kHz mode left the wire");
    assert!(rig.engine.blanked());
    let calls = rig.fake.next_mode_call() - first;
    assert!(
        calls <= 2 * u64::from(MODESET_ATTEMPTS + 1),
        "no retry storm: {calls} set_mode calls"
    );
}

#[test]
fn a_propose_loop_with_no_picture_still_reverts() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let start = rig.now();
    let styles = [PictureStyle::X2, PictureStyle::X3];
    for round in 0..8 {
        rig.propose(with_style(config(dead_edid()), styles[round % 2]));
        rig.run_for(NO_PICTURE_LIMIT_NS / 2);
        if rig.guard.phase() == Phase::Confirmed {
            break;
        }
    }
    let reverted = rig
        .time_of(reverted(RevertReason::NoPicture))
        .expect("the loop does not hold a change with no picture");
    assert!(reverted <= start + NO_PICTURE_LIMIT_NS + FRAME_SLACK_NS);
    rig.frame();
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
}

#[test]
fn a_propose_loop_cannot_hold_a_change_past_the_pending_limit() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let start = rig.now();
    let styles = [PictureStyle::X2, PictureStyle::Native];
    let mut round = 0;
    while rig.guard.phase() != Phase::Confirmed || round == 0 {
        let prompt = rig.propose(with_style(crt_31(), styles[round % 2]));
        assert!(prompt.deadline_ns <= start + PENDING_LIMIT_NS);
        rig.run_for(CONFIRM_WINDOW_NS / 2);
        round += 1;
        assert!(rig.now() < start + 2 * PENDING_LIMIT_NS, "no end");
    }
    let reverted = rig.time_of(reverted(RevertReason::Timeout)).unwrap();
    assert!(reverted >= start + PENDING_LIMIT_NS);
    assert!(reverted <= start + PENDING_LIMIT_NS + FRAME_SLACK_NS);
    assert_eq!(rig.guard.active(), &crt_15());
}

#[test]
fn a_settings_loop_with_no_picture_still_gives_way_to_the_safe_default() {
    let dead = config(dead_edid());
    let mut rig = GuardRig::boot(confirmed(dead_edid()), dead.clone(), crt_15());
    let start = rig.now();
    let styles = [PictureStyle::X2, PictureStyle::X3];
    for round in 0..8 {
        let step = rig.guard.follow_settings(
            with_style(dead.clone(), styles[round % 2]).settings,
            &mut rig.engine,
        );
        rig.take(step);
        rig.run_for(NO_PICTURE_LIMIT_NS / 2);
    }
    let safe = rig
        .time_of(GuardEvent::SafeMode(SafeReason::NoValidMode))
        .expect("the loop does not hold a configuration with no picture");
    assert!(safe <= start + NO_PICTURE_LIMIT_NS + FRAME_SLACK_NS);
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
}

#[test]
fn time_that_goes_backwards_reverts_a_pending_change() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    rig.propose(crt_31());
    rig.run_for(CONFIRM_WINDOW_NS / 2);
    let back = rig.now() - CONFIRM_WINDOW_NS / 4;
    let step = rig.guard.tick(back, &mut rig.engine);
    assert_eq!(step.event, Some(reverted(RevertReason::ClockWentBack)));
    rig.frame();
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
}

#[test]
fn time_that_goes_backwards_does_not_hold_off_the_no_picture_limit() {
    let dead = config(dead_edid());
    let mut rig = GuardRig::boot(confirmed(dead_edid()), dead, crt_15());
    rig.frame();
    let start = rig.now();
    let late = start + NO_PICTURE_LIMIT_NS / 2;
    let step = rig.guard.tick(late, &mut rig.engine);
    assert_eq!(step.event, None);
    // The clock falls back to before the watch began, then goes on.
    let step = rig.guard.tick(start - FRAME_NS, &mut rig.engine);
    assert_eq!(step.event, None);
    let step = rig.guard.tick(start + NO_PICTURE_LIMIT_NS, &mut rig.engine);
    assert_eq!(
        step.event,
        Some(GuardEvent::SafeMode(SafeReason::NoValidMode)),
        "the time before the jump still counts"
    );
}

#[test]
fn a_flip_that_never_completes_cannot_be_confirmed_and_the_deadline_reverts() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let prompt = rig.propose(crt_31());
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(index, FlipFault::Lost);
    rig.frame();
    assert_eq!(rig.wire(), Some(mode_of(&crt_31())), "the mode is set");
    assert!(rig.engine.muted(), "its first flip never came");
    assert!(matches!(
        rig.guard.confirm(prompt.id, rig.now(), &rig.engine),
        Err(ConfirmError::NoPicture { .. })
    ));

    rig.run_until(prompt.deadline_ns + FRAME_NS);
    assert!(rig.event_list().contains(&reverted(RevertReason::Timeout)));
    assert!(
        rig.errors
            .iter()
            .any(|e| matches!(e, VideoError::PendingFlip(_)))
    );
    assert_eq!(rig.engine.odren().monitor(), &crt_15().monitor);
    assert!(
        rig.engine.current_refused(),
        "the stuck flip keeps the 31 kHz mode; the session must open the display again"
    );
}

/// Proposes `to` from a confirmed `from`, and makes every modeset of the
/// change fail on the device, so the engine keeps the old mode.
fn device_errors_on_the_change(from: DisplayConfig, to: DisplayConfig) -> (GuardRig, Prompt) {
    let mut rig = GuardRig::confirmed_on(from.clone(), from);
    let prompt = rig.propose(to);
    let first = rig.fake.next_mode_call();
    for call in first..first + u64::from(MODESET_ATTEMPTS) {
        rig.fake.fault_mode(
            call,
            ModeFault::Device {
                reason: "EIO".to_owned(),
            },
        );
    }
    rig.run_for(CONFIRM_WINDOW_NS / 2);
    (rig, prompt)
}

#[test]
fn the_old_mode_left_by_device_errors_cannot_be_confirmed() {
    // The winner is 256x448 at about 31.4 kHz; the 15 kHz mode of
    // generic_15 also passes the arcade monitor.
    let to = with_style(config(arcade_15_25_31()), PictureStyle::X2);
    let old = mode_of(&crt_15());
    let (mut rig, prompt) = device_errors_on_the_change(crt_15(), to.clone());
    assert_eq!(rig.wire(), Some(old), "the device kept the old mode");
    assert_ne!(mode_of(&to), old);
    assert!(
        Health::of(&rig.engine).picture,
        "the old mode passes the new inputs"
    );
    assert!(!rig.engine.odren().decision_due());
    assert!(!rig.engine.odren().settled());
    assert_eq!(
        rig.guard.confirm(prompt.id, rig.now(), &rig.engine),
        Err(ConfirmError::NotShown)
    );
    rig.run_until(prompt.deadline_ns + FRAME_NS);
    assert!(rig.event_list().contains(&reverted(RevertReason::Timeout)));
    assert_eq!(rig.guard.record().last_known_good, Some(crt_15().monitor));
}

#[test]
fn a_change_with_no_winner_that_keeps_the_old_mode_cannot_be_confirmed() {
    let mut rig = GuardRig::confirmed_on(crt_15(), crt_15());
    let old = rig.wire();
    // A turned game has no winner (`Kept(Rotated)`): the old mode stays,
    // and the new settings still accept it.
    rig.source = SourceGeometry {
        rotation: Rotation::R90,
        ..rig.source
    };
    let prompt = rig.propose(with_style(crt_15(), PictureStyle::X2));
    rig.frame();
    rig.frame();
    assert!(matches!(
        rig.engine.trace().last().unwrap().outcome,
        Outcome::Kept(Unshowable::Rotated(_))
    ));
    assert_eq!(rig.wire(), old);
    assert!(Health::of(&rig.engine).picture);
    assert_eq!(
        rig.guard.confirm(prompt.id, rig.now(), &rig.engine),
        Err(ConfirmError::NotShown)
    );
}
