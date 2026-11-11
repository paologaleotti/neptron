//! The error paths: refusals, display errors, and calls out of order. In
//! each, the engine reports, and the mode on screen is the last good one.

use neptron_backend::DisplayError;
use neptron_safety::Rejection;
use neptron_testkit::preset::monitor_preset;
use neptron_testkit::{FlipFault, ModeFault};
use neptron_types::Modeline;
use neptron_video::{Directive, Monitor, Outcome, Refusal, Trigger, Unshowable, VideoError};

use crate::harness::{MD_H32, MD_H40, Rig, SNES, frame, generic_15, mode_for, native_width};

fn refuse_all(mode: &Modeline) -> Result<(), Rejection> {
    Err(Rejection::NotAPanelMode {
        mode: *mode,
        panel_modes: 0,
    })
}

/// Goes from `MD_H40` to `MD_H32` through the debounce. The third frame of
/// `MD_H32` sets the mode.
fn switch_to_h32(rig: &mut Rig) -> Result<(), VideoError> {
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    rig.try_frame(MD_H32).map(drop)
}

#[test]
fn a_validator_that_refuses_everything_causes_no_commit() {
    let mut rig = Rig::generic_15(native_width());
    rig.engine.set_veto(Box::new(refuse_all));
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Kept);
    assert!(!rig.engine.muted());
    assert_eq!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::NoMode)
    );
    assert!(rig.fake.mode_calls().is_empty());
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.outcome, Outcome::Kept(Unshowable::NoWinner));
    assert!(!record.refused.is_empty());
    assert!(
        record
            .refused
            .iter()
            .all(|r| matches!(r.reason, Refusal::Validator(_)))
    );
    for _ in 0..5 {
        assert_eq!(rig.engine.begin_frame(SNES), Directive::Kept);
    }
    assert_eq!(rig.engine.trace().total(), 1, "one record per request");
}

#[test]
fn a_refused_winner_is_never_set_and_the_next_proposal_wins() {
    // A 31 kHz monitor has several rungs that validate for a 224-line game.
    let monitor = || Monitor::Profile(monitor_preset("pc_31_120").unwrap());
    let usual = mode_for(monitor(), native_width(), SNES);
    let mut rig = Rig::new(monitor(), native_width());
    rig.engine.set_veto(Box::new(move |mode: &Modeline| {
        if *mode == usual {
            refuse_all(mode)
        } else {
            Ok(())
        }
    }));
    rig.frame(SNES);
    assert!(rig.fake.mode_calls().iter().all(|c| c.requested != usual));
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.outcome, Outcome::Modeset);
    assert!(
        record
            .refused
            .iter()
            .any(|r| matches!(r.reason, Refusal::Validator(_)))
    );
}

#[test]
fn a_new_mode_with_no_winner_keeps_the_mode_on_screen() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let mode = rig.current_mode();
    rig.engine.set_veto(Box::new(
        move |m: &Modeline| {
            if *m == mode { Ok(()) } else { refuse_all(m) }
        },
    ));
    rig.frame(MD_H40);
    for _ in 0..5 {
        assert_eq!(rig.frame(MD_H32).directive, Directive::Kept);
    }
    assert_eq!(rig.modes(), [mode]);
    assert_eq!(
        rig.engine.trace().last().unwrap().outcome,
        Outcome::Kept(Unshowable::NoWinner)
    );
}

#[test]
fn a_device_error_in_set_mode_keeps_the_old_mode_and_blocks_the_source() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let old = rig.current_mode();
    rig.fake.fault_mode(
        1,
        ModeFault::Device {
            reason: "EIO".to_owned(),
        },
    );
    switch_to_h32(&mut rig).unwrap();
    assert!(!rig.engine.muted(), "the old mode never left the wire");
    assert_eq!(rig.current_mode(), old);
    assert_eq!(rig.fake.active_mode(), Some(old));
    let record = rig.engine.trace().last().unwrap();
    assert!(matches!(
        record.outcome,
        Outcome::SetModeFailed(DisplayError::Device { .. })
    ));
    for _ in 0..10 {
        assert_eq!(rig.frame(MD_H32).directive, Directive::Kept);
    }
    assert_eq!(rig.fake.mode_calls().len(), 2, "no retry storm");
    rig.frame(MD_H40);
    switch_to_h32(&mut rig).unwrap();
    assert_eq!(rig.fake.mode_calls().len(), 3, "a new request tries again");
    assert_ne!(rig.current_mode(), old);
}

#[test]
fn a_mode_the_driver_refuses_is_dropped_and_the_next_proposal_is_tried() {
    let refused = mode_for(generic_15(), native_width(), MD_H32);
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.fake.fault_requested_mode(
        refused,
        ModeFault::Refuse {
            reason: "test-only commit failed".to_owned(),
        },
    );
    switch_to_h32(&mut rig).unwrap();
    assert!(matches!(
        rig.engine.trace().last().unwrap().outcome,
        Outcome::SetModeFailed(DisplayError::ModeRefused { .. })
    ));
    let failed_frame = rig.engine.trace().last().unwrap().decided_frame;
    let step = rig.frame(MD_H32);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.trigger, Trigger::Retry);
    assert_eq!(record.decided_frame, failed_frame + 1, "no debounce");
    if step.directive == Directive::Modeset {
        assert_ne!(rig.current_mode(), refused);
        assert!(record.refused.iter().any(|r| r.reason == Refusal::Driver));
    } else {
        assert_eq!(step.directive, Directive::Kept);
    }
    for _ in 0..5 {
        rig.frame(MD_H32);
    }
    let asks = rig
        .fake
        .mode_calls()
        .iter()
        .filter(|c| c.requested == refused)
        .count();
    assert_eq!(asks, 1, "the refused mode is not asked again");
}

#[test]
fn a_monitor_change_forgets_the_modes_the_driver_refused() {
    let refused = mode_for(generic_15(), native_width(), MD_H40);
    let mut rig = Rig::generic_15(native_width());
    rig.fake.fault_mode(
        0,
        ModeFault::Refuse {
            reason: "busy".to_owned(),
        },
    );
    rig.engine.begin_frame(MD_H40);
    assert_eq!(
        rig.engine.present(Some(frame(&MD_H40)), None),
        Err(VideoError::NoMode)
    );
    rig.engine.set_monitor(generic_15());
    rig.frame(MD_H40);
    assert_eq!(rig.current_mode(), refused);
}

#[test]
fn a_read_back_of_another_size_restores_the_last_good_mode() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let good = rig.current_mode();
    let wrong = Modeline {
        hactive: good.hactive - 8,
        ..good
    };
    rig.fake.fault_mode(1, ModeFault::ReadBack(wrong));
    let asked = mode_for(generic_15(), native_width(), MD_H32);
    switch_to_h32(&mut rig).unwrap();
    let calls = rig.fake.mode_calls();
    assert_eq!(calls[1].requested, asked);
    assert_eq!(calls[2].requested, good, "the last good mode again");
    assert_eq!(rig.fake.active_mode(), Some(good));
    assert_eq!(rig.current_mode(), good);
    assert!(
        !rig.engine.muted(),
        "unmuted after the flip of the restored mode"
    );
    assert!(
        rig.engine
            .trace()
            .records()
            .any(|r| r.outcome == Outcome::ReadBackRefused { read_back: wrong })
    );
    rig.frame(MD_H32);
    assert_eq!(
        rig.engine.trace().last().unwrap().trigger,
        Trigger::Retry,
        "the engine tries the next proposal"
    );
}

#[test]
fn a_read_back_with_another_pixel_clock_is_kept() {
    let mut rig = Rig::generic_15(native_width());
    let asked = mode_for(generic_15(), native_width(), MD_H40);
    let rounded = Modeline {
        pclock_hz: asked.pclock_hz + 1,
        ..asked
    };
    rig.fake.fault_mode(0, ModeFault::ReadBack(rounded));
    rig.frame(MD_H40);
    let current = rig.engine.current().unwrap();
    assert_eq!(current.read_back, rounded);
    assert_eq!(*current.chosen.token.mode(), asked);
    for _ in 0..5 {
        assert_eq!(rig.frame(MD_H40).directive, Directive::Steady);
    }
}

#[test]
fn a_failed_restore_is_an_error_and_the_engine_decides_again() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let good = rig.current_mode();
    let wrong = Modeline {
        vactive: good.vactive - 2,
        ..good
    };
    rig.fake.fault_mode(1, ModeFault::ReadBack(wrong));
    rig.fake.fault_mode(
        2,
        ModeFault::Device {
            reason: "EIO".to_owned(),
        },
    );
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    rig.engine.begin_frame(MD_H32);
    let error = rig.engine.present(Some(frame(&MD_H32)), None).unwrap_err();
    assert!(matches!(error, VideoError::RestoreFailed { read_back, .. } if read_back == wrong));
    assert!(rig.engine.current().is_none(), "the mode is not known");
    assert!(rig.engine.current_refused());
    // The driver refused the only mode of this source, so the engine has
    // none, says so, and guesses nothing.
    assert_eq!(rig.try_frame(MD_H32), Err(VideoError::NoMode));
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.trigger, Trigger::Retry);
    assert!(matches!(record.outcome, Outcome::Kept(_)));
    rig.frame(MD_H40);
    assert_eq!(rig.current_mode(), good);
    assert_eq!(rig.fake.active_mode(), Some(good));
    assert!(!rig.engine.current_refused());
}

#[test]
fn a_failed_present_queues_nothing() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    rig.fake.fault_flip(
        1,
        FlipFault::PresentFails {
            reason: "ENOMEM".to_owned(),
        },
    );
    rig.engine.begin_frame(SNES);
    assert!(matches!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::Present(DisplayError::Device { .. }))
    ));
    assert_eq!(rig.engine.wait_flip(), Err(VideoError::NoFlipPending));
    rig.frame(SNES);
}

#[test]
fn a_failed_wait_loses_the_flip_and_the_next_frame_runs() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    rig.fake.fault_flip(1, FlipFault::Lost);
    rig.engine.begin_frame(SNES);
    rig.engine.present(Some(frame(&SNES)), None).unwrap();
    assert!(matches!(
        rig.engine.wait_flip(),
        Err(VideoError::WaitFlip(DisplayError::Device { .. }))
    ));
    assert_eq!(rig.engine.wait_flip(), Err(VideoError::NoFlipPending));
    rig.frame(SNES);
}

#[test]
fn a_mute_stays_until_the_new_mode_flips_even_when_a_flip_is_lost() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(index, FlipFault::Lost);
    rig.engine.begin_frame(MD_H32);
    rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert!(rig.engine.wait_flip().is_err());
    assert!(rig.engine.muted());
    assert_eq!(rig.frame(MD_H32).directive, Directive::Steady);
    assert!(!rig.engine.muted());
}

#[test]
fn calls_out_of_order_are_errors() {
    let mut rig = Rig::generic_15(native_width());
    assert_eq!(rig.engine.present(None, None), Err(VideoError::NoMode));
    assert_eq!(rig.engine.wait_flip(), Err(VideoError::NoFlipPending));
    rig.frame(SNES);
    assert_eq!(rig.engine.wait_flip(), Err(VideoError::NoFlipPending));
}

#[test]
fn a_failed_wait_before_a_modeset_keeps_the_modeset_for_the_next_frame() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.frame(MD_H32);
    rig.engine.begin_frame(MD_H32);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(
        index,
        FlipFault::WaitFails {
            reason: "EIO".to_owned(),
        },
    );
    rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert_eq!(rig.engine.begin_frame(MD_H32), Directive::Modeset);
    assert!(matches!(
        rig.engine.present(Some(frame(&MD_H32)), None),
        Err(VideoError::FlipBeforeModeset(_))
    ));
    assert_eq!(rig.modes().len(), 1);
    assert_eq!(rig.engine.begin_frame(MD_H32), Directive::Modeset);
    rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    rig.engine.wait_flip().unwrap();
    assert_eq!(rig.modes().len(), 2);
    assert!(!rig.engine.muted());
}

#[test]
fn a_restore_never_sets_a_mode_the_new_monitor_refuses() {
    let pc_31_120 = || Monitor::Profile(monitor_preset("pc_31_120").unwrap());
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.frame(SNES);
    let old = rig.current_mode();
    let new = mode_for(generic_15(), native_width(), SNES);
    rig.engine.set_monitor(generic_15());
    rig.fake.fault_mode(
        1,
        ModeFault::ReadBack(Modeline {
            hactive: new.hactive - 8,
            ..new
        }),
    );
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Modeset);
    assert_eq!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::NoMode)
    );
    let calls = rig.fake.mode_calls();
    assert_eq!(calls.len(), 2, "no restore of {old:?}");
    assert_eq!(calls[1].requested, new);
    assert!(rig.engine.current().is_none());
    assert!(rig.engine.current_refused());
    assert!(
        rig.engine
            .trace()
            .records()
            .any(|r| matches!(r.outcome, Outcome::Kept(Unshowable::CurrentRefused(_))))
    );
}

#[test]
fn the_read_back_of_a_restore_is_checked_like_a_modeset() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let good = rig.current_mode();
    let wrong = |m: Modeline| Modeline {
        vactive: m.vactive - 2,
        ..m
    };
    let asked = mode_for(generic_15(), native_width(), MD_H32);
    rig.fake.fault_mode(1, ModeFault::ReadBack(wrong(asked)));
    rig.fake.fault_mode(2, ModeFault::ReadBack(wrong(good)));
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    rig.engine.begin_frame(MD_H32);
    assert_eq!(
        rig.engine.present(Some(frame(&MD_H32)), None),
        Err(VideoError::NoMode)
    );
    assert_eq!(rig.fake.mode_calls()[2].requested, good);
    assert!(
        rig.engine.current().is_none(),
        "the restored mode did not hold"
    );
    assert!(rig.engine.current_refused());
    assert!(rig.engine.trace().records().any(|r| r.outcome
        == Outcome::ReadBackRefused {
            read_back: wrong(good)
        }));
}
