//! The edges of the pacing: a GPU that makes the flips late, the return
//! to wait pacing and what the loop learns, long frames that come again
//! and again, a display whose flips take too long or carry no blank
//! count, and the safety paths in late pacing.

use neptron_safety::Rejection;
use neptron_session::{FLIPLESS_FRAMES_LIMIT, LATE_HOLD_FRAMES, SessionError};
use neptron_testkit::preset::monitor_preset;
use neptron_testkit::{DisplayOptions, FlipFault};
use neptron_types::{Control, Modeline};
use neptron_video::{Monitor, VideoError};

use crate::harness::{Rig, Setup};
use crate::pacing::interval_ns;
use crate::with_script;

const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 128 112 1333
timing 60000 48000
";

/// The first hold, in frames.
const HOLD_FRAMES: u64 = LATE_HOLD_FRAMES as u64;

/// The flips per blank of the flips after `from` in the display's record.
fn flip_rate(rig: &Rig, from: usize) -> f64 {
    let flips = rig.display.flips();
    let counted = &flips[from..];
    let span = counted.last().unwrap().sequence - counted[0].sequence;
    (counted.len() - 1) as f64 / span as f64
}

/// A core at 0.95 of the interval with a GPU that draws for 0.1 of it:
/// every commit is in time, every flip misses its blank. The loop sees the
/// late flips and goes to late pacing, where the GPU draws while the next
/// core frame runs: nearly the full rate, not half.
#[test]
fn flips_late_by_the_gpu_start_late_pacing() {
    with_script!("flips_late_by_the_gpu_start_late_pacing", SCRIPT, |dir| {
        let setup = Setup {
            display: DisplayOptions {
                gpu_ns: 1_666_667,
                ..DisplayOptions::default()
            },
            ..Setup::default()
        };
        let mut rig = Rig::launch(dir, setup);
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        rig.cost.set(interval * 95 / 100, 0, 0);
        let from = rig.display.flips().len() + 10;
        rig.run(1_200);
        let rate = flip_rate(&rig, from);
        assert!(rate >= 0.98, "presented rate {rate}");
        let counters = *rig.session.counters();
        assert!(counters.late_frames > 1_000, "{counters:?}");
        assert!(counters.late_flips >= 2, "{counters:?}");
        assert_eq!(counters.flipless_frames, 0);
        assert!(
            rig.audio.underruns().len() <= 1,
            "{:?}",
            rig.audio.underruns()
        );
    });
}

/// After one long frame, a fast core is back in wait pacing within the
/// hold, with an input delay of one interval.
fn back_after_a_spike(test: &str, permille: u64) {
    with_script!(test, SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        rig.cost.set(interval * 3 / 2, 0, 0);
        assert!(rig.session.run_frame(None).unwrap().deferred);
        rig.cost.set(interval * permille / 1_000, 0, 0);
        let mut late_frames = 1;
        for _ in 0..4 * HOLD_FRAMES {
            late_frames += u64::from(rig.session.run_frame(None).unwrap().deferred);
        }
        assert!(late_frames <= HOLD_FRAMES + 2, "{late_frames} late frames");
        rig.press(&[Control::A]);
        rig.run(1);
        let flips = rig.display.flip_times();
        let last = flips[flips.len() - 1] - flips[flips.len() - 2];
        assert_eq!(rig.session.counters().input_to_flip_ns, last);
    });
}

#[test]
fn a_fast_core_is_back_at_one_interval_after_a_spike() {
    back_after_a_spike("a_fast_core_is_back_at_one_interval_after_a_spike", 500);
}

#[test]
fn a_core_at_0_95_is_back_at_one_interval_after_a_spike() {
    back_after_a_spike("a_core_at_0_95_is_back_at_one_interval_after_a_spike", 950);
}

/// Every flip takes 700 ms: two bounded waits of 350 ms time out, and the
/// present of the third frame drains it. A drained flip is no progress of
/// the display, so the session ends after the limit of flipless frames.
#[test]
fn flips_that_take_700_ms_end_the_session() {
    with_script!("flips_that_take_700_ms_end_the_session", SCRIPT, |dir| {
        let setup = Setup {
            display: DisplayOptions {
                flip_timeout_ns: 350_000_000,
                ..DisplayOptions::default()
            },
            ..Setup::default()
        };
        let mut rig = Rig::launch(dir, setup);
        rig.run(10);
        let first = rig.display.next_present_index();
        for index in first..first + 100 {
            rig.display
                .fault_flip(index, FlipFault::Slow { timeouts: 2 });
        }
        let mut frames = 0;
        let error = loop {
            frames += 1;
            match rig.session.run_frame(None) {
                Ok(_) => assert!(frames <= 3 * FLIPLESS_FRAMES_LIMIT, "no end"),
                Err(error) => break error,
            }
        };
        assert_eq!(frames, FLIPLESS_FRAMES_LIMIT);
        assert!(matches!(error, SessionError::DisplayStalled { .. }));
        assert!(
            rig.session.counters().flips > 10,
            "drained flips still count"
        );
    });
}

/// The deferred wait of a late frame fails: the frame still ends its
/// audio, and the next present drains the flip.
#[test]
fn a_failed_deferred_wait_ends_the_audio_of_its_frame() {
    with_script!(
        "a_failed_deferred_wait_ends_the_audio_of_its_frame",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 13 / 10, 0, 0);
            rig.run(30);
            let index = rig.display.next_present_index();
            rig.display.fault_flip(
                index,
                FlipFault::WaitFails {
                    reason: "test".to_owned(),
                },
            );
            assert!(rig.session.run_frame(None).unwrap().deferred);
            let audio_frames = rig.session.audio_stats().unwrap().frames;
            let report = rig.session.run_frame(None).unwrap();
            assert!(matches!(
                report.display_error,
                Some(VideoError::WaitFlip(_))
            ));
            assert_eq!(
                rig.session.audio_stats().unwrap().frames,
                audio_frames + 1,
                "the frame ended its audio"
            );
            rig.run(30);
            assert_eq!(rig.session.counters().flipless_frames, 1);
        }
    );
}

/// Fast and slow frames in turn, 0.5 and 1.5 intervals: late pacing, every
/// frame shown, the sound with no storm of underruns.
#[test]
fn fast_and_slow_frames_in_turn() {
    with_script!("fast_and_slow_frames_in_turn", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        let from = rig.display.flips().len() + 10;
        for frame in 0..600 {
            let cost = if frame % 2 == 0 {
                interval / 2
            } else {
                interval * 3 / 2
            };
            rig.cost.set(cost, 0, 0);
            rig.session.run_frame(None).unwrap();
        }
        let rate = flip_rate(&rig, from);
        assert!(rate >= 0.99, "presented rate {rate}");
        assert_eq!(rig.session.counters().flipless_frames, 0);
        assert!(
            rig.audio.underruns().len() <= 1,
            "{:?}",
            rig.audio.underruns()
        );
        assert!(rig.audio.overruns().is_empty());
    });
}

/// The display guard reverts a mode by giving the engine a monitor again;
/// here a change of monitor and its revert, in late pacing. Each modeset
/// waits for the flip before, and the game goes on in late pacing.
#[test]
fn a_monitor_change_and_its_revert_in_late_pacing() {
    with_script!(
        "a_monitor_change_and_its_revert_in_late_pacing",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 13 / 10, 0, 0);
            rig.run(60);
            for preset in ["pc_31_120", "generic_15"] {
                let profile = monitor_preset(preset).unwrap();
                rig.session
                    .video_mut()
                    .set_monitor(Monitor::Profile(profile));
                for _ in 0..60 {
                    let report = rig.session.run_frame(None).unwrap();
                    assert!(report.display_error.is_none(), "{report:?}");
                }
            }
            assert_eq!(rig.display.mode_calls().len(), 3);
            assert!(rig.display.mode_calls().iter().all(|m| m.result.is_ok()));
            assert!(rig.session.run_frame(None).unwrap().deferred);
            assert_eq!(rig.session.counters().flipless_frames, 0);
        }
    );
}

/// The output goes off in late pacing and comes back: no display error,
/// no stall, and late pacing comes back with the mode.
#[test]
fn the_output_off_and_back_in_late_pacing() {
    with_script!("the_output_off_and_back_in_late_pacing", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        rig.cost.set(interval * 5 / 2, 0, 0);
        rig.run(200);
        rig.session
            .video_mut()
            .set_veto(Box::new(|mode: &Modeline| {
                Err(Rejection::NotAPanelMode {
                    mode: *mode,
                    panel_modes: 0,
                })
            }));
        for _ in 0..20 {
            let report = rig.session.run_frame(None).unwrap();
            assert!(
                report.display_error.is_none() || report.blanked,
                "{report:?}"
            );
        }
        assert_eq!(rig.display.active_mode(), None, "the output is off");
        rig.session
            .video_mut()
            .set_veto(Box::new(|_: &Modeline| Ok(())));
        let mut late = 0;
        for _ in 0..60 {
            late += u32::from(rig.session.run_frame(None).unwrap().deferred);
        }
        assert!(late > 50, "late pacing again: {late}");
        assert_eq!(rig.session.counters().flipless_frames, 0);
    });
}

/// Runs `frames` frames with the cost of frame `k` given by `cost(k)`, and
/// returns the real rate of core frames over the last 240, in mHz, and the
/// pacing changes.
pub fn run_pattern(rig: &mut Rig, frames: u64, cost: impl Fn(u64) -> u64) -> (u64, u32) {
    let mut changes = 0;
    let mut start = (0, 0);
    for k in 0..frames {
        if k == frames - 240 {
            start = (rig.clock_ns(), k);
        }
        rig.cost.set(cost(k), 0, 0);
        let report = rig.session.run_frame(None).unwrap();
        changes += u32::from(report.pacing.is_some());
    }
    let span = rig.clock_ns() - start.0;
    let rate = (frames - start.1) * 1_000_000_000_000 / span;
    (rate, changes)
}

/// A core at 0.95 with a GPU that makes its flips late, over 72 s: after
/// the first entry into late pacing, no more tries, no more missed fields
/// and no more underruns.
#[test]
fn a_core_late_by_its_gpu_reaches_a_quiet_state() {
    with_script!(
        "a_core_late_by_its_gpu_reaches_a_quiet_state",
        SCRIPT,
        |dir| {
            let setup = Setup {
                display: DisplayOptions {
                    gpu_ns: 1_666_667,
                    ..DisplayOptions::default()
                },
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 95 / 100, 0, 0);
            rig.run(300);
            let missed = rig.session.counters().missed_fields;
            let underruns = rig.audio.underruns().len();
            let mut changes = 0;
            for _ in 0..4_000 {
                changes += u32::from(rig.session.run_frame(None).unwrap().pacing.is_some());
            }
            assert_eq!(changes, 0, "no more tries");
            assert_eq!(rig.session.counters().missed_fields, missed);
            assert_eq!(rig.audio.underruns().len(), underruns);
        }
    );
}

/// A spike every 180 frames (3 s): the hold grows until the loop stays in late
/// pacing, so the pacing changes a few times and then never again in 5
/// minutes.
#[test]
fn a_spike_every_180_frames_reaches_a_stable_pacing() {
    with_script!(
        "a_spike_every_180_frames_reaches_a_stable_pacing",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            let spikes = |k: u64| {
                if k % 180 == 179 {
                    interval * 3
                } else {
                    interval / 2
                }
            };
            let (_, early) = run_pattern(&mut rig, 3_600, spikes);
            assert!(early <= 10, "{early} changes in the first minute");
            let (_, late) = run_pattern(&mut rig, 14_400, spikes);
            assert_eq!(late, 0, "no change in the next 4 minutes");
        }
    );
}

/// A driver whose flip events carry a vertical blank count of 0: the
/// loop sees the late flips from their times, and a core late by its GPU
/// still gets nearly the full rate.
#[test]
fn a_frozen_vblank_count_still_shows_late_flips() {
    with_script!(
        "a_frozen_vblank_count_still_shows_late_flips",
        SCRIPT,
        |dir| {
            let setup = Setup {
                display: DisplayOptions {
                    gpu_ns: 1_666_667,
                    frozen_vblank_count: true,
                    ..DisplayOptions::default()
                },
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 95 / 100, 0, 0);
            let (rate, _) = run_pattern(&mut rig, 600, |_| interval * 95 / 100);
            let field_rate = 1_000_000_000_000 / interval;
            assert!(rate * 100 >= field_rate * 98, "rate {rate} of {field_rate}");
        }
    );
}

/// The quick menu resets the pacing: what the loop learned about a core
/// late by its GPU is forgotten, so after the resume it enters late pacing
/// once more, by two late flips, and then stays quiet.
#[test]
fn the_quick_menu_resets_what_the_pacing_learned() {
    with_script!(
        "the_quick_menu_resets_what_the_pacing_learned",
        SCRIPT,
        |dir| {
            let setup = Setup {
                display: DisplayOptions {
                    gpu_ns: 1_666_667,
                    ..DisplayOptions::default()
                },
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 95 / 100, 0, 0);
            rig.run(600);
            rig.cost.set(0, 0, 0);
            rig.open_menu();
            rig.run(10);
            rig.session.resume();
            rig.cost.set(interval * 95 / 100, 0, 0);
            let late_flips = rig.session.counters().late_flips;
            let (_, changes) = run_pattern(&mut rig, 1_200, |_| interval * 95 / 100);
            assert_eq!(changes, 1, "one entry after the resume");
            // The two late flips that enter late pacing, and the flip of the
            // frame that started at the second one.
            assert_eq!(rig.session.counters().late_flips - late_flips, 3);
        }
    );
}
