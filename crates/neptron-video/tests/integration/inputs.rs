//! A monitor or settings change during a game, and the EDID limit.

use neptron_backend::DisplayError;
use neptron_platform::{EdidProfile, TimingFormula};
use neptron_safety::Rejection;
use neptron_testkit::preset::{CAPS_LOW_CLOCK, monitor_preset};
use neptron_testkit::{FlipFault, ModeFault};
use neptron_types::Modeline;
use neptron_video::{
    Directive, MODESET_ATTEMPTS, Monitor, Outcome, PictureStyle, PolicySettings, Refusal, Trigger,
    Unshowable, VideoError,
};

use crate::harness::{MD_H40, Rig, SNES, fixed_width, frame, generic_15, native_width};

fn pc_31_120() -> Monitor {
    Monitor::Profile(monitor_preset("pc_31_120").unwrap())
}

fn edid(pclock_max_hz: u32) -> Monitor {
    Monitor::Edid(EdidProfile {
        profile: monitor_preset("pc_31_120").unwrap(),
        pclock_max_hz,
        formula: TimingFormula::DefaultGtf,
    })
}

#[test]
fn a_monitor_change_in_a_game_switches_on_the_next_frame() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    rig.frame(SNES);
    let old = rig.current_mode();
    rig.engine.set_monitor(pc_31_120());
    let step = rig.frame(SNES);
    assert_eq!(step.directive, Directive::Modeset, "no debounce");
    assert!(step.muted_before_present);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.trigger, Trigger::Monitor);
    assert_eq!(record.decided_frame, 3);
    assert_eq!(record.took_frames(), Some(0));
    assert_ne!(rig.current_mode(), old);
    assert_eq!(rig.modes().len(), 2);
}

#[test]
fn a_settings_change_in_a_game_decides_on_the_next_frame() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.engine.set_settings(fixed_width());
    let step = rig.frame(MD_H40);
    assert_ne!(step.directive, Directive::Steady);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.trigger, Trigger::Settings);
    assert_eq!(record.decided_frame, 2);
    assert_eq!(rig.engine.odren().settings(), &fixed_width());
}

#[test]
fn the_same_settings_change_nothing() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    rig.engine.set_settings(native_width());
    assert_eq!(rig.frame(SNES).directive, Directive::Steady);
    assert_eq!(rig.engine.trace().total(), 1);
}

#[test]
fn a_style_change_takes_the_style_of_the_new_settings() {
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.frame(SNES);
    let x3 = PolicySettings {
        style: PictureStyle::X3,
        ..native_width()
    };
    rig.engine.set_settings(x3);
    rig.frame(SNES);
    let rows = rig.engine.style_rows(&SNES).unwrap();
    let x3_row = rows.iter().find(|r| r.style == PictureStyle::X3).unwrap();
    let current = rig.engine.current().unwrap();
    assert_eq!(x3_row.winner, Some(current.chosen.index));
}

#[test]
fn an_edid_monitor_limits_the_pixel_clock_of_the_caps() {
    let limit = 20_000_000;
    let mut rig = Rig::new(edid(limit), native_width());
    assert_eq!(rig.engine.odren().caps().pclock_max_hz, limit);
    rig.frame(SNES);
    let current = rig.engine.current().unwrap();
    assert!(current.chosen.token.mode().pclock_hz <= limit);
    rig.engine.set_monitor(generic_15());
    assert_eq!(
        rig.engine.odren().caps().pclock_max_hz,
        CAPS_LOW_CLOCK.pclock_max_hz
    );
}

#[test]
fn an_edid_limit_below_every_mode_keeps_the_screen_dark() {
    let mut rig = Rig::new(edid(2_000_000), native_width());
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Kept);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.outcome, Outcome::Kept(Unshowable::NoWinner));
    assert!(record.refused.iter().all(|r| matches!(
        r.reason,
        Refusal::Validator(Rejection::PixelClock {
            max_hz: 2_000_000,
            ..
        })
    )));
    assert!(rig.fake.mode_calls().is_empty());
}

#[test]
fn a_monitor_that_refuses_the_mode_on_screen_blanks_the_output_until_a_valid_mode() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    let mode = rig.current_mode();
    assert!(!rig.engine.current_refused());
    rig.engine.set_monitor(edid(2_000_000));
    assert!(
        rig.engine.current_refused(),
        "known at once, before a frame"
    );
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.trigger, Trigger::Monitor);
    assert!(matches!(
        record.outcome,
        Outcome::Kept(Unshowable::CurrentRefused(Refusal::Validator(_)))
    ));
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Blank);
    assert!(
        rig.fake.blank_calls().is_empty(),
        "begin_frame calls nothing"
    );
    assert_eq!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::NoMode)
    );
    let records: Vec<_> = rig.engine.trace().records().collect();
    let [.., no_winner, blanked] = records[..] else {
        panic!("{records:?}");
    };
    assert_eq!(no_winner.outcome, Outcome::Kept(Unshowable::NoWinner));
    assert_eq!(blanked.outcome, Outcome::Blanked);
    assert_eq!(blanked.previous, Some(mode));
    assert_eq!(rig.fake.blank_calls().len(), 1);
    assert_eq!(rig.fake.active_mode(), None);
    assert!(rig.engine.blanked());
    assert!(!rig.engine.current_refused());
    assert!(rig.engine.current().is_none());
    for _ in 0..5 {
        assert_eq!(rig.try_frame(SNES), Err(VideoError::NoMode));
    }
    assert_eq!(rig.fake.blank_calls().len(), 1, "one blank");
    assert!(rig.fake.mode_calls().len() == 1, "no mode while blank");

    rig.engine.set_monitor(generic_15());
    assert_eq!(rig.frame(SNES).directive, Directive::Modeset);
    assert!(!rig.engine.blanked());
    assert_eq!(rig.current_mode(), mode);
    assert_eq!(
        rig.fake.mode_calls().len(),
        2,
        "the same mode is set again after the blank"
    );
    assert_eq!(rig.fake.active_mode(), Some(mode));
}

#[test]
fn a_failed_blank_is_tried_again_up_to_the_bound() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    let mode = rig.current_mode();
    for call in 0..u64::from(MODESET_ATTEMPTS) {
        rig.fake.fault_blank(call, "EIO".to_owned());
    }
    rig.engine.set_monitor(edid(2_000_000));
    for attempt in 0..MODESET_ATTEMPTS {
        assert_eq!(rig.engine.begin_frame(SNES), Directive::Blank);
        rig.engine.present(Some(frame(&SNES)), None).unwrap();
        rig.engine.wait_flip().unwrap();
        let record = rig.engine.trace().last().unwrap();
        assert!(
            matches!(
                record.outcome,
                Outcome::BlankFailed(DisplayError::Device { .. })
            ),
            "attempt {attempt}: {record:?}"
        );
        assert!(rig.engine.current_refused());
        assert!(!rig.engine.blanked());
    }
    for _ in 0..5 {
        assert_eq!(rig.frame(SNES).directive, Directive::Steady);
    }
    assert_eq!(rig.fake.blank_calls().len(), MODESET_ATTEMPTS as usize);
    assert!(rig.engine.current_refused(), "the guard must act");
    assert_eq!(rig.fake.active_mode(), Some(mode));
    rig.engine.set_monitor(edid(2_000_000));
    assert_eq!(
        rig.engine.begin_frame(SNES),
        Directive::Blank,
        "a new count"
    );
    assert_eq!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::NoMode)
    );
    assert!(rig.engine.blanked());
}

#[test]
fn a_blank_waits_for_a_queued_flip_first() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(index, FlipFault::Slow { timeouts: 2 });
    rig.engine.begin_frame(SNES);
    rig.engine.present(Some(frame(&SNES)), None).unwrap();
    assert!(rig.engine.wait_flip().is_err());
    rig.engine.set_monitor(edid(2_000_000));
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Blank);
    assert!(matches!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::PendingFlip(_))
    ));
    assert!(rig.fake.blank_calls().is_empty(), "never busy");
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Blank);
    assert_eq!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::NoMode)
    );
    let calls = rig.fake.blank_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].result, Ok(()));
    assert!(rig.engine.blanked());
}

#[test]
fn a_veto_of_the_mode_on_screen_is_reported_until_a_valid_mode_is_set() {
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.frame(SNES);
    let mode = rig.current_mode();
    rig.engine.set_veto(Box::new(move |m: &Modeline| {
        if *m == mode {
            Err(Rejection::NotAPanelMode {
                mode: *m,
                panel_modes: 0,
            })
        } else {
            Ok(())
        }
    }));
    assert!(rig.engine.current_refused());
    assert_eq!(rig.frame(SNES).directive, Directive::Modeset);
    assert!(!rig.engine.current_refused());
    assert_ne!(rig.current_mode(), mode);
}

/// Every mode the display was asked for passes the validator of `monitor`.
fn every_set_mode_passes(rig: &Rig, monitor: &Monitor) {
    for call in rig.fake.mode_calls() {
        let caps = monitor.caps_for(&CAPS_LOW_CLOCK);
        assert!(
            neptron_safety::validate(&call.requested, monitor.profile(), &caps).is_ok(),
            "set_mode got {:?}",
            call.requested
        );
    }
}

#[test]
fn a_modeset_decided_before_a_monitor_change_is_never_set() {
    let mut rig = Rig::new(pc_31_120(), native_width());
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Modeset);
    rig.engine.set_monitor(generic_15());
    assert_eq!(
        rig.engine.present(Some(frame(&SNES)), None),
        Err(VideoError::NoMode)
    );
    assert!(rig.fake.mode_calls().is_empty());
    rig.frame(SNES);
    assert_eq!(rig.fake.mode_calls().len(), 1);
    every_set_mode_passes(&rig, &generic_15());
}

#[test]
fn a_modeset_decided_before_a_settings_or_veto_change_is_never_set() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let old = rig.current_mode();
    rig.engine.set_settings(fixed_width());
    assert_eq!(rig.engine.begin_frame(MD_H40), Directive::Modeset);
    let fixed = *rig.engine.odren().pending_modeset().unwrap().mode();
    rig.engine.set_veto(Box::new(move |m: &Modeline| {
        if *m == fixed {
            Err(Rejection::NotAPanelMode {
                mode: *m,
                panel_modes: 0,
            })
        } else {
            Ok(())
        }
    }));
    rig.engine.present(Some(frame(&MD_H40)), None).unwrap();
    rig.engine.wait_flip().unwrap();
    assert_eq!(rig.modes(), [old], "the stale token is dropped");
    rig.frame(MD_H40);
    assert!(
        !rig.modes().contains(&fixed),
        "the vetoed mode is never set"
    );
}

fn passes(monitor: &Monitor, mode: &Modeline) -> bool {
    let caps = monitor.caps_for(&CAPS_LOW_CLOCK);
    neptron_safety::validate(mode, monitor.profile(), &caps).is_ok()
}

#[test]
fn a_monitor_modeset_that_is_not_presented_is_decided_again() {
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.frame(SNES);
    rig.engine.set_monitor(generic_15());
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Modeset);
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Modeset);
    rig.engine.present(Some(frame(&SNES)), None).unwrap();
    rig.engine.wait_flip().unwrap();
    assert!(passes(&generic_15(), &rig.current_mode()));
    assert!(!rig.engine.current_refused());
}

#[test]
fn a_monitor_modeset_whose_flip_wait_fails_is_decided_again() {
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.frame(SNES);
    rig.engine.begin_frame(SNES);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(
        index,
        FlipFault::WaitFails {
            reason: "EIO".to_owned(),
        },
    );
    rig.engine.present(Some(frame(&SNES)), None).unwrap();
    rig.engine.set_monitor(generic_15());
    assert_eq!(rig.engine.begin_frame(SNES), Directive::Modeset);
    assert!(rig.engine.present(Some(frame(&SNES)), None).is_err());
    for _ in 0..3 {
        if rig.try_frame(SNES).is_ok() && passes(&generic_15(), &rig.current_mode()) {
            break;
        }
    }
    assert!(passes(&generic_15(), &rig.current_mode()));
    assert!(!rig.engine.current_refused());
}

#[test]
fn a_device_error_on_a_monitor_modeset_is_tried_again_up_to_the_bound() {
    let new = crate::harness::mode_for(generic_15(), native_width(), SNES);
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.frame(SNES);
    rig.fake.fault_requested_mode(
        new,
        ModeFault::Device {
            reason: "EIO".to_owned(),
        },
    );
    rig.engine.set_monitor(generic_15());
    for _ in 0..MODESET_ATTEMPTS {
        assert_eq!(rig.frame(SNES).directive, Directive::Modeset);
    }
    assert!(rig.engine.current_refused(), "the 31 kHz mode is still on");
    // The new monitor refuses the mode on screen, and the valid mode
    // cannot be set: the output goes off.
    assert_eq!(rig.try_frame(SNES), Err(VideoError::NoMode));
    assert!(rig.engine.blanked());
    for _ in 0..5 {
        assert_eq!(rig.try_frame(SNES), Err(VideoError::NoMode));
    }
    let asked = rig
        .fake
        .mode_calls()
        .iter()
        .filter(|c| c.requested == new)
        .count();
    // With no mode on screen the next frame is a launch: one more attempt,
    // then the source stays blocked.
    assert_eq!(asked, MODESET_ATTEMPTS as usize + 1, "no retry storm");
    assert_eq!(rig.fake.blank_calls().len(), 1);
    assert_eq!(rig.fake.active_mode(), None);
    rig.engine.set_monitor(pc_31_120());
    rig.frame(SNES);
    assert!(!rig.engine.blanked());
    assert!(passes(&pc_31_120(), &rig.current_mode()));
}

#[test]
fn a_device_error_on_a_monitor_modeset_then_success() {
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.frame(SNES);
    rig.fake.fault_mode(
        1,
        ModeFault::Device {
            reason: "EIO".to_owned(),
        },
    );
    rig.engine.set_monitor(generic_15());
    rig.frame(SNES);
    assert_eq!(rig.frame(SNES).directive, Directive::Modeset);
    assert!(passes(&generic_15(), &rig.current_mode()));
    assert!(!rig.engine.current_refused());
}
