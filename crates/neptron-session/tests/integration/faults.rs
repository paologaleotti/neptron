//! Display faults: the session follows what the engine reports. A wait
//! that fails or times out costs the frame, never a hang or a spin, and a
//! display that stops flipping (a flip that never completes stays queued)
//! ends the session after a bounded number of frames.

use neptron_safety::Rejection;
use neptron_session::{FLIPLESS_FRAMES_LIMIT, SessionError};
use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_testkit::{DEFAULT_FLIP_TIMEOUT_NS, FlipFault};
use neptron_types::{Control, GpuCaps, Modeline};
use neptron_video::{DEBOUNCE_FRAMES, Directive, VideoError};

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

/// A flip on time misses no field, from the first flip of the launch mode
/// on. A flip late by two fields counts 2, and so does the flip of a
/// frame whose wait timed out, by the fields it came after its target.
#[test]
fn a_late_flip_counts_the_fields_it_missed() {
    with_script!("a_late_flip_counts_the_fields_it_missed", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(120);
        assert_eq!(rig.session.counters().missed_fields, 0);
        let index = rig.display.next_present_index();
        rig.display
            .fault_flip(index, FlipFault::Late { vblanks: 2 });
        rig.run(60);
        assert_eq!(rig.session.counters().missed_fields, 2);
        let index = rig.display.next_present_index();
        rig.display
            .fault_flip(index, FlipFault::Slow { timeouts: 1 });
        rig.run(60);
        let counters = *rig.session.counters();
        let late: u64 = rig
            .display
            .frames()
            .iter()
            .filter_map(|f| f.late_by())
            .sum();
        assert_eq!(rig.display.repeated_vblanks(), late);
        assert_eq!(counters.missed_fields, late, "{counters:?}");
        assert!(late > 2, "the slow flip missed fields too");
        assert_eq!(
            counters.flipless_frames, 1,
            "frames_dropped keeps its meaning"
        );
    });
}

/// A late flip under the quick menu does not count: the core does not run.
#[test]
fn a_late_flip_under_the_quick_menu_is_not_counted() {
    with_script!(
        "a_late_flip_under_the_quick_menu_is_not_counted",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(10);
            rig.press(&[Control::Select, Control::Start]);
            rig.run(1);
            rig.release(&[Control::Select, Control::Start]);
            rig.run(1);
            assert!(rig.session.paused());
            let index = rig.display.next_present_index();
            rig.display
                .fault_flip(index, FlipFault::Late { vblanks: 3 });
            rig.run(10);
            assert_eq!(rig.display.repeated_vblanks(), 3);
            rig.session.resume();
            rig.run(30);
            assert_eq!(rig.session.counters().missed_fields, 0);
        }
    );
}

const INTERLACED_SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 448 1333
timing 60000 48000
";

/// An interlaced mode on a driver that flips once per frame: a flip lands
/// every second field, and the session counts no missed field in steady
/// play. A flip late by one frame misses the two fields of that frame.
#[test]
fn an_interlaced_mode_with_one_flip_per_frame_misses_no_field() {
    with_script!(
        "an_interlaced_mode_with_one_flip_per_frame_misses_no_field",
        INTERLACED_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            const { assert!(!CAPS_LOW_CLOCK.flip_per_field) };
            rig.run(120);
            assert!(rig.display.active_mode().unwrap().interlace);
            let flips = rig.display.flips();
            let last = &flips[flips.len() - 2..];
            assert_eq!(last[1].sequence - last[0].sequence, 2, "a flip per frame");
            assert_eq!(rig.session.counters().missed_fields, 0);
            let index = rig.display.next_present_index();
            rig.display
                .fault_flip(index, FlipFault::Late { vblanks: 1 });
            rig.run(30);
            assert_eq!(rig.session.counters().missed_fields, 2);
        }
    );
}

/// An interlaced mode on a driver that flips once per field: a flip lands
/// on every field, and the session counts no missed field.
#[test]
fn an_interlaced_mode_with_one_flip_per_field_misses_no_field() {
    with_script!(
        "an_interlaced_mode_with_one_flip_per_field_misses_no_field",
        INTERLACED_SCRIPT,
        |dir| {
            let setup = Setup {
                caps: GpuCaps {
                    flip_per_field: true,
                    ..CAPS_LOW_CLOCK
                },
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            rig.run(120);
            assert!(rig.display.active_mode().unwrap().interlace);
            let flips = rig.display.flips();
            let last = &flips[flips.len() - 2..];
            assert_eq!(last[1].sequence - last[0].sequence, 1, "a flip per field");
            assert_eq!(rig.session.counters().missed_fields, 0);
        }
    );
}

/// A modeset in the game starts the count again: a gap of blanks between
/// the last flip of the old mode and the first flip of the new one is not
/// a missed field. The fake display counts blanks on through a modeset, so
/// the test makes the gap: the first flip of the new mode lands 5 blanks
/// late. The frame after the last frame of the debounce sets the mode.
#[test]
fn a_modeset_in_the_game_misses_no_field() {
    let script = format!("{SCRIPT}at 300 av_info 256 224 1333 320 240 50007 32040\n");
    with_script!("a_modeset_in_the_game_misses_no_field", &script, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(10);
        let mut faulted = false;
        for _ in 0..600 {
            let index = rig.display.next_present_index();
            let report = rig.session.run_frame(None).unwrap();
            if faulted && index == rig.display.next_present_index() - 1 {
                faulted = false;
                assert_eq!(report.directive, Directive::Modeset);
            }
            let last_wait = Directive::Waiting {
                stable_frames: DEBOUNCE_FRAMES - 1,
            };
            if report.directive == last_wait {
                let next = rig.display.next_present_index();
                rig.display.fault_flip(next, FlipFault::Late { vblanks: 5 });
                faulted = true;
            }
        }
        assert_eq!(rig.display.mode_calls().len(), 2);
        assert!(rig.display.repeated_vblanks() >= 5, "the flip came late");
        assert_eq!(rig.session.counters().missed_fields, 0);
    });
}

/// A present that fails still ends the audio of its frame.
#[test]
fn the_audio_of_a_frame_goes_out_when_its_present_fails() {
    with_script!(
        "the_audio_of_a_frame_goes_out_when_its_present_fails",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let frames = rig.session.audio_stats().unwrap().frames;
            let index = rig.display.next_present_index();
            rig.display.fault_flip(
                index,
                FlipFault::PresentFails {
                    reason: "test".to_owned(),
                },
            );
            let report = rig.session.run_frame(None).unwrap();
            assert!(matches!(report.display_error, Some(VideoError::Present(_))));
            assert_eq!(rig.session.audio_stats().unwrap().frames, frames + 1);
        }
    );
}
