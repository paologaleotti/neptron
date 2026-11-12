//! Display faults: the session follows what the engine reports. A wait
//! that fails or times out costs the frame, never a hang or a spin, and a
//! display that stops flipping (a flip that never completes stays queued)
//! ends the session after a bounded number of frames.

use neptron_safety::Rejection;
use neptron_session::{FLIPLESS_FRAMES_LIMIT, SessionError};
use neptron_testkit::{DEFAULT_FLIP_TIMEOUT_NS, FlipFault};
use neptron_types::Modeline;
use neptron_video::{Directive, VideoError};

use crate::harness::{Rig, Setup};
use crate::with_script;

const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
timing 60000 48000
";

/// A flip whose first two bounded waits time out stays queued: the first
/// frame's wait fails, the next frame's present finds it still pending,
/// and the third wait drains it.
#[test]
fn a_slow_flip_costs_two_frames_and_the_session_goes_on() {
    with_script!(
        "a_slow_flip_costs_two_frames_and_the_session_goes_on",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(20);
            let index = rig.display.next_present_index();
            rig.display
                .fault_flip(index, FlipFault::Slow { timeouts: 2 });
            let report = rig.session.run_frame(None).unwrap();
            assert_eq!(report.flips, 0);
            assert!(matches!(
                report.display_error,
                Some(VideoError::WaitFlip(_))
            ));
            let report = rig.session.run_frame(None).unwrap();
            assert_eq!(report.flips, 0);
            assert!(matches!(
                report.display_error,
                Some(VideoError::PendingFlip(_))
            ));
            rig.run(60);
            let counters = *rig.session.counters();
            assert_eq!(counters.flipless_frames, 2);
            assert_eq!(counters.core_frames, 82, "the core ran in every frame");
            assert_eq!(counters.flips, 80);
            assert!(rig.audio.overruns().is_empty());
        }
    );
}

#[test]
fn a_failed_present_or_wait_costs_one_frame() {
    with_script!("a_failed_present_or_wait_costs_one_frame", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(10);
        let index = rig.display.next_present_index();
        rig.display.fault_flip(
            index,
            FlipFault::PresentFails {
                reason: "test".to_owned(),
            },
        );
        let report = rig.session.run_frame(None).unwrap();
        assert!(matches!(report.display_error, Some(VideoError::Present(_))));
        rig.run(10);
        let index = rig.display.next_present_index();
        rig.display.fault_flip(
            index,
            FlipFault::WaitFails {
                reason: "test".to_owned(),
            },
        );
        let report = rig.session.run_frame(None).unwrap();
        assert!(matches!(
            report.display_error,
            Some(VideoError::WaitFlip(_))
        ));
        rig.run(10);
        assert_eq!(rig.session.counters().flipless_frames, 2);
        assert_eq!(rig.session.counters().flips, 30);
    });
}

#[test]
fn a_display_that_stops_flipping_ends_the_session_in_bounded_time() {
    with_script!(
        "a_display_that_stops_flipping_ends_the_session_in_bounded_time",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(10);
            let first = rig.display.next_present_index();
            for index in first..first + 100 {
                rig.display.fault_flip(index, FlipFault::Lost);
            }
            let start_ns = neptron_backend::Clock::now_ns(&rig.clock);
            let mut frames = 0;
            let error = loop {
                frames += 1;
                match rig.session.run_frame(None) {
                    Ok(report) => assert_eq!(report.flips, 0),
                    Err(error) => break error,
                }
                assert!(frames <= FLIPLESS_FRAMES_LIMIT, "no end");
            };
            assert_eq!(frames, FLIPLESS_FRAMES_LIMIT);
            assert!(matches!(
                error,
                SessionError::DisplayStalled {
                    frames: FLIPLESS_FRAMES_LIMIT,
                    ..
                }
            ));
            let waited = neptron_backend::Clock::now_ns(&rig.clock) - start_ns;
            assert!(
                waited <= u64::from(FLIPLESS_FRAMES_LIMIT) * DEFAULT_FLIP_TIMEOUT_NS,
                "each frame waits at most one flip timeout: {waited} ns"
            );
            assert!(rig.session.ended());
            assert!(matches!(
                rig.session.run_frame(None),
                Err(SessionError::Ended)
            ));
        }
    );
}

/// The inputs refuse the mode on screen and no mode is valid: the engine
/// turns the output off. The core waits with the game frozen and the audio
/// muted, the frames do not count as a stall, and the game goes on when
/// the inputs give a valid mode again.
#[test]
fn with_the_output_off_the_core_waits_and_the_session_goes_on() {
    with_script!(
        "with_the_output_off_the_core_waits_and_the_session_goes_on",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(20);
            let frozen = rig.session.counters().core_frames;
            rig.session
                .video_mut()
                .set_veto(Box::new(|mode: &Modeline| {
                    Err(Rejection::NotAPanelMode {
                        mode: *mode,
                        panel_modes: 0,
                    })
                }));
            let report = rig.session.run_frame(None).unwrap();
            assert_eq!(report.directive, Directive::Blank);
            assert!(report.blanked);
            let audio_frames = rig.session.audio_stats().unwrap().frames;
            for _ in 0..2 * FLIPLESS_FRAMES_LIMIT {
                let report = rig.session.run_frame(None).unwrap();
                assert!(report.blanked);
                assert!(!report.ran_core);
                assert_eq!(report.flips, 0);
            }
            assert_eq!(rig.display.active_mode(), None, "the output is off");
            assert_eq!(rig.session.counters().core_frames, frozen + 1);
            assert_eq!(rig.session.audio_stats().unwrap().frames, audio_frames);
            assert_eq!(rig.session.counters().flipless_frames, 0);

            rig.session
                .video_mut()
                .set_veto(Box::new(|_: &Modeline| Ok(())));
            let report = rig.session.run_frame(None).unwrap();
            assert!(!report.blanked);
            assert_eq!(report.flips, 1);
            rig.run(10);
            assert_eq!(rig.session.counters().core_frames, frozen + 1 + 10);
            assert!(rig.display.active_mode().is_some());
        }
    );
}
