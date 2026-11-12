//! Scripted safety scenarios: last known good, failed boots and safe mode.

use neptron_video::{
    CONFIRM_WINDOW_NS, GOOD_BOOT_PICTURE_NS, GuardEvent, GuardRecord, NO_PICTURE_LIMIT_NS, Phase,
    PictureStyle, RevertReason, SAFE_MODE_FAILED_BOOTS, SafeReason,
};

use crate::guard_rig::{
    FRAME_NS, GuardRig, config, confirmed, crt_15, crt_31, dead_edid, mode_of, pc_31_120,
    with_style,
};

#[test]
fn a_boot_on_the_last_known_good_needs_no_confirmation() {
    let mut rig = GuardRig::boot(confirmed(pc_31_120()), crt_31(), crt_15());
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(rig.event_list(), []);
    assert_eq!(
        rig.saves.len(),
        1,
        "the boot is counted before the first mode"
    );
    assert_eq!(rig.saves[0].failed_boots, 1);
    rig.frame();
    assert_eq!(rig.wire(), Some(mode_of(&crt_31())));
}

#[test]
fn a_stable_picture_makes_the_boot_good() {
    let record = GuardRecord {
        failed_boots: 1,
        ..confirmed(pc_31_120())
    };
    let mut rig = GuardRig::boot(record, crt_31(), crt_15());
    rig.run_until(GOOD_BOOT_PICTURE_NS - FRAME_NS);
    assert!(!rig.guard.boot_good());
    rig.run_for(2 * FRAME_NS);
    assert!(rig.guard.boot_good());
    assert_eq!(rig.event_list(), [GuardEvent::GoodBoot]);
    let last = rig.saves.last().unwrap();
    assert_eq!(last.failed_boots, 0);
    assert_eq!(last.last_known_good, Some(pc_31_120()));
}

#[test]
fn two_failed_boots_start_the_third_in_safe_mode() {
    let wanted = crt_31();
    let safe = crt_15();
    let mut record = confirmed(pc_31_120());
    for boot in 1..=SAFE_MODE_FAILED_BOOTS {
        // The frontend dies before the picture is stable.
        let mut rig = GuardRig::boot(record, wanted.clone(), safe.clone());
        rig.run_for(GOOD_BOOT_PICTURE_NS / 2);
        assert_eq!(rig.guard.phase(), Phase::Confirmed);
        record = rig.saves.last().unwrap().clone();
        assert_eq!(record.failed_boots, boot);
    }

    let mut rig = GuardRig::boot(record, wanted, safe.clone());
    assert_eq!(rig.guard.phase(), Phase::Safe(SafeReason::FailedBoots));
    assert_eq!(
        rig.event_list(),
        [GuardEvent::SafeMode(SafeReason::FailedBoots)]
    );
    let saved = rig.saves.last().unwrap();
    assert!(saved.safe_mode);
    assert_eq!(
        saved.last_known_good,
        Some(pc_31_120()),
        "kept for the user"
    );
    rig.run_for(2 * GOOD_BOOT_PICTURE_NS);
    assert_eq!(rig.wire(), Some(mode_of(&safe)));
    assert!(
        !rig.guard.boot_good(),
        "safe mode is not a confirmed picture"
    );
}

#[test]
fn safe_mode_stays_until_the_user_confirms() {
    let record = GuardRecord {
        failed_boots: SAFE_MODE_FAILED_BOOTS,
        ..confirmed(pc_31_120())
    };
    let mut rig = GuardRig::boot(record, crt_31(), crt_15());
    rig.run_for(GOOD_BOOT_PICTURE_NS + FRAME_NS);
    let after_good_picture = rig.saves.last().unwrap().clone();
    let mut next = GuardRig::boot(after_good_picture, crt_31(), crt_15());
    assert_eq!(next.guard.phase(), Phase::Safe(SafeReason::FailedBoots));

    let prompt = next.propose(crt_31());
    next.frame();
    let step = next
        .guard
        .confirm(prompt.id, next.now(), &next.engine)
        .unwrap();
    let saved = step.save.unwrap();
    assert!(!saved.safe_mode);
    assert_eq!(saved.failed_boots, 0);
    let after = GuardRig::boot(saved, crt_31(), crt_15());
    assert_eq!(after.guard.phase(), Phase::Confirmed);
}

#[test]
fn a_change_in_safe_mode_that_reverts_goes_back_to_safe_mode() {
    let record = GuardRecord {
        failed_boots: SAFE_MODE_FAILED_BOOTS,
        ..GuardRecord::default()
    };
    let mut rig = GuardRig::boot(record, crt_31(), crt_15());
    let prompt = rig.propose(crt_31());
    rig.run_until(prompt.deadline_ns + FRAME_NS);
    assert_eq!(rig.guard.phase(), Phase::Safe(SafeReason::FailedBoots));
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
}

#[test]
fn settings_that_differ_from_the_last_known_good_wait_for_a_confirmation() {
    let mut rig = GuardRig::boot(confirmed(pc_31_120()), crt_15(), crt_15());
    let Some(GuardEvent::PromptOpened(prompt)) = rig.event_list().first().cloned() else {
        panic!("a prompt at boot");
    };
    rig.run_until(prompt.deadline_ns + FRAME_NS);
    assert!(
        rig.event_list().contains(&GuardEvent::Reverted {
            reason: RevertReason::Timeout,
            rewrite: Some(crt_31()),
        }),
        "the settings still hold the unconfirmed monitor: the app writes the revert target"
    );
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(
        rig.guard.active().monitor,
        pc_31_120(),
        "the last known good"
    );
    assert_eq!(rig.wire(), Some(mode_of(&crt_31())));
}

#[test]
fn a_first_boot_with_nothing_confirmed_falls_back_to_the_safe_default() {
    let mut rig = GuardRig::boot(GuardRecord::default(), crt_31(), crt_15());
    let prompt = rig.guard.pending().unwrap();
    assert_eq!(prompt.deadline_ns, CONFIRM_WINDOW_NS);
    rig.run_until(prompt.deadline_ns + FRAME_NS);
    assert_eq!(rig.guard.phase(), Phase::Safe(SafeReason::NothingConfirmed));
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
    assert!(
        rig.saves.iter().all(|r| r.last_known_good.is_none()),
        "nothing unconfirmed is saved"
    );
    assert!(!rig.saves.last().unwrap().safe_mode);
}

#[test]
fn a_last_known_good_with_no_valid_mode_gives_way_to_the_safe_default() {
    let mut rig = GuardRig::boot(confirmed(dead_edid()), config(dead_edid()), crt_15());
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    rig.frame();
    assert_eq!(rig.wire(), None, "no mode is set for it");
    rig.run_for(NO_PICTURE_LIMIT_NS + 2 * FRAME_NS);
    assert_eq!(
        rig.event_list(),
        [GuardEvent::SafeMode(SafeReason::NoValidMode)]
    );
    assert_eq!(rig.wire(), Some(mode_of(&crt_15())));
    assert!(!rig.guard.boot_good(), "the boot stays failed");
    assert!(
        !rig.saves.last().unwrap().safe_mode,
        "the next boot tries again"
    );
}

#[test]
fn a_game_setting_follows_with_no_confirmation() {
    let mut rig = GuardRig::confirmed_on(crt_31(), crt_15());
    let x2 = with_style(crt_31(), PictureStyle::X2).settings;
    let step = rig.guard.follow_settings(x2, &mut rig.engine);
    assert_eq!(step, Default::default());
    assert_eq!(rig.guard.active().settings, x2);
    assert_eq!(rig.engine.odren().settings(), &x2);
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
}

#[test]
fn safe_mode_keeps_the_settings_of_the_safe_default() {
    let record = GuardRecord {
        failed_boots: SAFE_MODE_FAILED_BOOTS,
        ..GuardRecord::default()
    };
    let mut rig = GuardRig::boot(record, crt_31(), crt_15());
    let x2 = with_style(crt_15(), PictureStyle::X2).settings;
    let step = rig.guard.follow_settings(x2, &mut rig.engine);
    assert_eq!(step, Default::default());
    assert_eq!(rig.engine.odren().settings(), &crt_15().settings);
}

#[test]
fn a_boot_prompt_comes_back_on_each_boot_until_the_settings_are_written() {
    let record = confirmed(pc_31_120());
    let mut written = crt_15();
    for boot in 0..3 {
        let mut rig = GuardRig::boot(record.clone(), written.clone(), crt_15());
        if boot == 2 {
            assert_eq!(rig.guard.phase(), Phase::Confirmed, "no prompt any more");
            assert_eq!(rig.wire(), None);
            rig.frame();
            assert_eq!(rig.wire(), Some(mode_of(&crt_31())));
            break;
        }
        let prompt = rig
            .guard
            .pending()
            .expect("the unconfirmed monitor asks again");
        rig.run_until(prompt.deadline_ns + FRAME_NS);
        let Some(GuardEvent::Reverted {
            rewrite: Some(target),
            ..
        }) = rig.event_list().last().cloned()
        else {
            panic!("the revert of a boot prompt asks for the settings");
        };
        if boot == 1 {
            written = target;
        }
    }
}

#[test]
fn a_change_that_replaces_a_boot_prompt_still_asks_for_the_settings() {
    let mut rig = GuardRig::boot(confirmed(pc_31_120()), crt_15(), crt_15());
    rig.frame();
    let prompt = rig.propose(with_style(crt_15(), PictureStyle::X2));
    let step = rig.guard.decline(prompt.id, &mut rig.engine).unwrap();
    assert_eq!(
        step.event,
        Some(GuardEvent::Reverted {
            reason: RevertReason::Declined,
            rewrite: Some(crt_31()),
        })
    );
    assert_eq!(rig.guard.active().monitor, pc_31_120());
}

#[test]
fn a_game_setting_during_a_boot_prompt_is_not_what_the_app_writes() {
    let mut rig = GuardRig::boot(confirmed(pc_31_120()), crt_15(), crt_15());
    rig.frame();
    let game = with_style(crt_31(), PictureStyle::X2).settings;
    let step = rig.guard.follow_settings(game, &mut rig.engine);
    assert_eq!(
        step.event,
        Some(GuardEvent::Reverted {
            reason: RevertReason::Superseded,
            rewrite: Some(crt_31()),
        }),
        "the revert target with the global settings, not the game's"
    );
    assert_eq!(rig.guard.active().settings, game);
}
