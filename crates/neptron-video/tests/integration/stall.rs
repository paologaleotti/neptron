//! Waits that time out while the flip is still queued, as the bounded wait
//! of the `Display` contract allows.

use neptron_testkit::FlipFault;
use neptron_video::{Directive, Outcome, VideoError};

use crate::harness::{MD_H32, MD_H40, Rig, frame, native_width};

#[test]
fn a_timed_out_wait_keeps_the_flip_queued_and_the_next_wait_gets_it() {
    let mut rig = Rig::generic_15(native_width());
    rig.engine.begin_frame(MD_H40);
    rig.fake.fault_flip(0, FlipFault::Slow { timeouts: 1 });
    rig.engine.present(Some(frame(&MD_H40)), None).unwrap();
    assert!(matches!(
        rig.engine.wait_flip(),
        Err(VideoError::WaitFlip(_))
    ));
    let event = rig.engine.wait_flip().unwrap();
    assert_eq!(rig.fake.flips().last(), Some(&event));
}

#[test]
fn a_timed_out_wait_before_a_modeset_sets_no_mode_and_blocks_nothing() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.engine.begin_frame(MD_H32);
    rig.engine.begin_frame(MD_H32);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(index, FlipFault::Slow { timeouts: 1 });
    rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert_eq!(rig.engine.begin_frame(MD_H32), Directive::Modeset);
    assert!(matches!(
        rig.engine.present(Some(frame(&MD_H32)), None),
        Err(VideoError::PendingFlip(_))
    ));
    assert_eq!(
        rig.fake.mode_calls().len(),
        1,
        "no set_mode while the flip is queued"
    );
    assert_eq!(rig.engine.begin_frame(MD_H32), Directive::Modeset);
    let presented = rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert!(presented.drained_flip.is_some());
    assert_eq!(presented.modeset, Some(Outcome::Modeset));
    rig.engine.wait_flip().unwrap();
    assert!(rig.fake.mode_calls().iter().all(|c| c.result.is_ok()));
    assert_eq!(rig.fake.mode_switches().len(), 2);
}

#[test]
fn a_flip_that_needs_many_waits_is_never_given_up() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(index, FlipFault::Slow { timeouts: 5 });
    rig.engine.begin_frame(MD_H40);
    rig.engine.present(Some(frame(&MD_H40)), None).unwrap();
    assert!(rig.engine.wait_flip().is_err());
    for _ in 0..4 {
        assert!(matches!(
            rig.try_frame(MD_H40),
            Err(VideoError::PendingFlip(_))
        ));
    }
    assert_eq!(
        rig.fake.next_present_index(),
        index + 1,
        "no present while the flip is queued"
    );
    let step = rig.frame(MD_H40);
    assert_eq!(step.directive, Directive::Steady);
    let frames = rig.fake.frames();
    assert!(frames[usize::try_from(index).unwrap()].flip.is_some());
    assert!(frames.iter().all(|f| f.present.is_ok()), "never busy");
}

#[test]
fn a_lost_flip_stays_queued_and_every_frame_reports_it() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(index, FlipFault::Lost);
    rig.engine.begin_frame(MD_H40);
    rig.engine.present(Some(frame(&MD_H40)), None).unwrap();
    assert!(matches!(
        rig.engine.wait_flip(),
        Err(VideoError::WaitFlip(_))
    ));
    for _ in 0..5 {
        assert!(matches!(
            rig.try_frame(MD_H40),
            Err(VideoError::PendingFlip(_))
        ));
    }
    assert!(
        matches!(rig.engine.wait_flip(), Err(VideoError::WaitFlip(_))),
        "the flip is still queued"
    );
    assert_eq!(rig.fake.next_present_index(), index + 1);
    assert!(rig.fake.frames().iter().all(|f| f.present.is_ok()));
    assert_eq!(rig.fake.mode_calls().len(), 1);
}

#[test]
fn a_mute_stays_until_the_new_mode_flips_after_a_slow_wait() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    let index = rig.fake.next_present_index();
    rig.fake.fault_flip(index, FlipFault::Slow { timeouts: 1 });
    assert_eq!(rig.engine.begin_frame(MD_H32), Directive::Modeset);
    rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert!(rig.engine.wait_flip().is_err());
    assert!(rig.engine.muted());
    rig.engine.begin_frame(MD_H32);
    let presented = rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert!(presented.drained_flip.is_some());
    assert!(!rig.engine.muted(), "the first flip in the new mode came");
    rig.engine.wait_flip().unwrap();
}
