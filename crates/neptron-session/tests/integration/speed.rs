//! The sound of a late core: the run rate at the measured speed, its
//! bounded change, the mute below half speed, and a long frame of a fast
//! core that changes nothing.

use neptron_backend::AudioSink;
use neptron_safety::Rejection;
use neptron_session::RUN_RATE_SLEW_PERMILLE_PER_S;
use neptron_types::Modeline;

use crate::harness::{Rig, Setup};
use crate::pacing::{RUN_FRAMES, SCRIPT, interval_ns, run_with_cost};
use crate::pacing_edges::run_pattern;
use crate::with_script;

/// The control span of the rate control at 48 kHz: 64 ms.
const SPAN_FRAMES: u64 = 3_072;

/// The seconds after a change of scene within which the level before a
/// write is back within half the target of the target.
const QUEUE_SETTLE_S: u64 = 3;

/// The most frames the queue holds: the 128 ms of the sink, as
/// `Sdl3Sink` keeps. More is an overrun, dropped by the sink.
const QUEUED_MAX_FRAMES: u64 = 6_144;

/// One second of the simulated clock.
const SECOND_NS: u64 = 1_000_000_000;

/// A late core at `permille` of the interval plays its sound at the
/// measured rate, within 0.5%, with at most `entry_underruns` underruns,
/// all in the first second of late pacing.
fn plays(test: &str, permille: u64, entry_underruns: usize) {
    with_script!(test, SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let start_ns = rig.clock_ns();
        run_with_cost(&mut rig, permille, 0, RUN_FRAMES);
        assert!(rig.audio.overruns().is_empty());
        assert!(!rig.session.sound_too_slow());
        let underruns = rig.audio.underruns();
        assert!(underruns.len() <= entry_underruns, "{underruns:?}");
        assert!(
            underruns.iter().all(|u| u.time_ns <= start_ns + SECOND_NS),
            "{underruns:?}"
        );
        let mode_rate = rig.display.active_mode().unwrap().field_rate_mhz();
        let measured = mode_rate as f64 * 1_000.0 / permille as f64;
        let run_rate = rig.session.run_rate_mhz().unwrap() as f64;
        assert!(
            (run_rate / measured - 1.0).abs() < 0.005,
            "run rate {run_rate} for {measured}"
        );
    });
}

#[test]
fn a_core_two_percent_too_slow_plays_at_its_speed() {
    plays("a_core_two_percent_too_slow_plays_at_its_speed", 1_020, 0);
}

#[test]
fn a_core_four_percent_too_slow_plays_at_its_speed() {
    plays("a_core_four_percent_too_slow_plays_at_its_speed", 1_040, 0);
}

#[test]
fn a_core_at_1_3_intervals_plays_at_its_speed() {
    plays("a_core_at_1_3_intervals_plays_at_its_speed", 1_300, 1);
}

#[test]
fn a_core_at_2_intervals_plays_at_its_speed() {
    plays("a_core_at_2_intervals_plays_at_its_speed", 2_000, 4);
}

/// A core at 2.5 intervals runs below half its rate: the sound fades out,
/// the core's frames are idle frames of the pipeline, and only the entry
/// costs underruns.
#[test]
fn a_core_below_half_speed_is_muted() {
    with_script!("a_core_below_half_speed_is_muted", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        run_with_cost(&mut rig, 2_500, 0, RUN_FRAMES);
        assert!(rig.session.sound_too_slow());
        let stats = *rig.session.audio_stats().unwrap();
        assert!(stats.underruns <= 4, "{stats:?}");
        rig.run(60);
        let after = *rig.session.audio_stats().unwrap();
        assert_eq!(after.frames, stats.frames, "no core audio");
        assert_eq!(after.underruns, stats.underruns);
    });
}

/// A cost that jitters around one interval: no underrun, no overrun.
#[test]
fn a_core_that_jitters_at_the_limit_plays_without_underrun() {
    with_script!(
        "a_core_that_jitters_at_the_limit_plays_without_underrun",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            run_with_cost(&mut rig, 1_000, 50, RUN_FRAMES);
            assert!(
                rig.audio.underruns().is_empty(),
                "{:?}",
                rig.audio.underruns()
            );
            assert!(rig.audio.overruns().is_empty());
        }
    );
}

/// One frame of a fast core takes `spike_ns`, then 240 frames take no
/// time. The audio keeps the run rate of the mode: no mute, no change of
/// rate, no overrun, at most one underrun, and the queue at its target.
fn a_long_frame(test: &str, spike: impl Fn(u64) -> u64) {
    with_script!(test, SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(60);
        let mode_rate = rig.session.run_rate_mhz().unwrap();
        rig.cost.set(spike(interval_ns(&rig, 1)), 0, 0);
        rig.run(1);
        rig.cost.set(0, 0, 0);
        for _ in 0..240 {
            rig.session.run_frame(None).unwrap();
            assert!(!rig.session.sound_too_slow(), "no mute");
            assert_eq!(rig.session.run_rate_mhz(), Some(mode_rate));
        }
        assert!(rig.audio.overruns().is_empty(), "no overrun");
        assert!(
            rig.audio.underruns().len() <= 1,
            "{:?}",
            rig.audio.underruns()
        );
        let level = rig.session.audio_stats().unwrap().level_frames as u64;
        assert!(
            (SPAN_FRAMES * 2 / 5..=SPAN_FRAMES * 3 / 5).contains(&level),
            "level {level} of {SPAN_FRAMES}"
        );
    });
}

#[test]
fn a_frame_of_two_and_a_half_fields_keeps_the_audio_rate() {
    a_long_frame(
        "a_frame_of_two_and_a_half_fields_keeps_the_audio_rate",
        |interval| interval * 5 / 2,
    );
}

#[test]
fn a_frame_of_300_ms_keeps_the_audio_rate() {
    a_long_frame("a_frame_of_300_ms_keeps_the_audio_rate", |_| 300_000_000);
}

/// A pattern of costs over 60 s: after the window fills, the run rate is
/// the real rate of core frames within 0.5%, and after the first 3 s at
/// most `late_underruns` underruns come: no steady stream.
fn assert_measured(test: &str, late_underruns: usize, cost: fn(u64, u64) -> u64) {
    with_script!(test, SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let start_ns = rig.clock_ns();
        let interval = interval_ns(&rig, 1);
        let (truth, changes) = run_pattern(&mut rig, 3_600, |k| cost(k, interval));
        assert!(changes <= 2, "{changes} pacing changes");
        let run_rate = rig.session.run_rate_mhz().unwrap();
        let error = run_rate.abs_diff(truth) as f64 / truth as f64;
        assert!(error < 0.005, "run rate {run_rate} for {truth}");
        let underruns = rig.audio.underruns();
        let late: Vec<_> = underruns
            .iter()
            .filter(|u| u.time_ns > start_ns + 3 * SECOND_NS)
            .collect();
        assert!(late.len() <= late_underruns, "{late:?}");
        assert!(rig.audio.overruns().is_empty());
    });
}

#[test]
fn frames_of_0_97_and_1_05_in_turn_are_measured() {
    assert_measured(
        "frames_of_0_97_and_1_05_in_turn_are_measured",
        0,
        |k, interval| {
            if k % 2 == 0 {
                interval * 97 / 100
            } else {
                interval * 105 / 100
            }
        },
    );
}

#[test]
fn a_spike_of_3_5_intervals_every_4_frames_is_measured() {
    assert_measured(
        "a_spike_of_3_5_intervals_every_4_frames_is_measured",
        0,
        |k, interval| {
            if k % 4 == 3 {
                interval * 7 / 2
            } else {
                interval
            }
        },
    );
}

#[test]
fn a_fast_core_with_a_spike_of_3_intervals_every_10_frames_is_measured() {
    assert_measured(
        "a_fast_core_with_a_spike_of_3_intervals_every_10_frames_is_measured",
        0,
        |k, interval| {
            if k % 10 == 9 {
                interval * 3
            } else {
                interval / 2
            }
        },
    );
}

#[test]
fn a_fast_core_with_a_spike_every_10_frames_is_measured() {
    assert_measured(
        "a_fast_core_with_a_spike_every_10_frames_is_measured",
        // A frame of 4 intervals, 67 ms, is longer than the 64 ms span of
        // the rate control: the deepest dips of the queue touch zero for
        // about 1 ms, about every 5 s.
        15,
        |k, interval| {
            if k % 10 == 9 {
                interval * 4
            } else {
                interval / 2
            }
        },
    );
}

/// The run rate moves by at most [`RUN_RATE_SLEW_PERMILLE_PER_S`] of the
/// mode's rate per second of the frame, at the start, during and at the end of a
/// slow phase, and the queue stays inside its span.
#[test]
fn the_run_rate_changes_by_a_bounded_step() {
    with_script!("the_run_rate_changes_by_a_bounded_step", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        let mode_rate = rig.session.run_rate_mhz().unwrap();
        let mut last = (mode_rate, rig.clock_ns());
        for k in 0..900 {
            let cost = if (200..500).contains(&k) {
                interval * 13 / 10
            } else {
                interval / 2
            };
            rig.cost.set(cost, 0, 0);
            rig.session.run_frame(None).unwrap();
            let rate = rig.session.run_rate_mhz().unwrap();
            let now = rig.clock_ns();
            let bound =
                mode_rate * RUN_RATE_SLEW_PERMILLE_PER_S * (now - last.1) / 1_000_000_000_000 + 1;
            assert!(
                rate.abs_diff(last.0) <= bound,
                "frame {k}: {} to {rate}",
                last.0
            );
            last = (rate, now);
        }
        assert!(rig.audio.overruns().is_empty());
    });
}

/// The core hovers at half its rate: the sound mutes below 50%, stays muted
/// at 51%, and comes back above 52%.
#[test]
fn the_mute_at_half_speed_has_a_hysteresis() {
    with_script!("the_mute_at_half_speed_has_a_hysteresis", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        // The cost of a frame for a speed of `permille` of the core's
        // 60 Hz.
        let cost = |permille: u64| 1_000_000_000_000 * 1_000 / (60_000 * permille);
        rig.cost.set(cost(490), 0, 0);
        rig.run(300);
        assert!(rig.session.sound_too_slow(), "muted at 49%");
        rig.cost.set(cost(510), 0, 0);
        for _ in 0..300 {
            rig.session.run_frame(None).unwrap();
            assert!(rig.session.sound_too_slow(), "still muted at 51%");
        }
        rig.cost.set(cost(550), 0, 0);
        rig.run(300);
        assert!(!rig.session.sound_too_slow(), "sound at 55%");
    });
}

/// The output goes off in late pacing and comes back: no display error,
/// no stall, the sound of a core below half speed is not held muted while
/// no mode is set, and late pacing comes back with the mode.
#[test]
fn the_mute_ends_while_the_output_is_off() {
    with_script!("the_mute_ends_while_the_output_is_off", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        rig.cost.set(interval * 5 / 2, 0, 0);
        rig.run(200);
        assert!(rig.session.sound_too_slow());
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
        assert!(!rig.session.sound_too_slow(), "no measure with no mode");
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

/// One scene of [`scenes_of_killer_instinct`]: the cost of a frame, in
/// permille of the interval, with its jitter, and the length.
struct Scene {
    permille: u64,
    jitter_permille: u64,
    seconds: u64,
}

/// The pattern measured on the PC with Killer Instinct in its attract mode:
/// light scenes near one interval, heavy scenes at 1.5 intervals.
const SCENES: [Scene; 4] = [
    Scene {
        permille: 970,
        jitter_permille: 30,
        seconds: 12,
    },
    Scene {
        permille: 1_500,
        jitter_permille: 50,
        seconds: 17,
    },
    Scene {
        permille: 970,
        jitter_permille: 30,
        seconds: 12,
    },
    Scene {
        permille: 1_500,
        jitter_permille: 50,
        seconds: 17,
    },
];

/// Scenes that move between one and 1.5 intervals: in each second the
/// queue stays under its bound, and from [`QUEUE_SETTLE_S`] after each
/// change the level before a write is near its target. A change of scene costs at most one
/// underrun, and nothing overruns.
#[test]
fn scenes_of_killer_instinct_keep_the_queue_near_its_target() {
    with_script!(
        "scenes_of_killer_instinct_keep_the_queue_near_its_target",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(60);
            let interval = interval_ns(&rig, 1);
            let target = SPAN_FRAMES / 2;
            let mut samples = Vec::new();
            let mut faults = Vec::new();
            for (index, scene) in SCENES.iter().enumerate() {
                rig.cost.set(
                    interval * scene.permille / 1_000,
                    interval * scene.jitter_permille / 1_000,
                    index as u64,
                );
                let start = rig.clock_ns();
                let underruns = rig.audio.underruns().len();
                let mut next = start + SECOND_NS;
                while rig.clock_ns() < start + scene.seconds * SECOND_NS {
                    rig.session.run_frame(None).unwrap();
                    if rig.clock_ns() < next {
                        continue;
                    }
                    next += SECOND_NS;
                    let queued = rig.session.audio_sink().unwrap().queued_frames() as u64;
                    let level = rig.session.audio_stats().unwrap().level_frames as u64;
                    let second = (rig.clock_ns() - start) / SECOND_NS;
                    samples.push((index, second, queued, level));
                    if queued > QUEUED_MAX_FRAMES {
                        faults.push(format!("scene {index} second {second}: {queued} queued"));
                    }
                    if second >= QUEUE_SETTLE_S && !(target / 2..=target * 3 / 2).contains(&level) {
                        faults.push(format!("scene {index} second {second}: level {level}"));
                    }
                }
                let new = rig.audio.underruns().len() - underruns;
                if new > 1 {
                    faults.push(format!("scene {index}: {new} underruns"));
                }
            }
            if !rig.audio.overruns().is_empty() {
                faults.push(format!("overruns {:?}", rig.audio.overruns()));
            }
            assert!(faults.is_empty(), "{faults:#?}\n{samples:?}");
        }
    );
}
