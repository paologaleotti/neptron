//! Timing check 6, frame pacing and audio, through the whole session: the
//! test core, the host, the engine, the audio pipeline, the fake display
//! with jitter and late vertical blanks, and the fake audio device.
//!
//! Asserts: the core runs exactly one frame per flip, no frame is dropped
//! or shown again except where the display was late, the audio queue stays
//! inside its bounds with no underrun and no overrun, rate control brings
//! the queue to its target, and the pitch error stays under 0.5%.
//!
//! The runs in the tier take 30 simulated seconds, about 4.7 time constants
//! of the rate control, as the random pairs of the audio crate do. The two
//! 100,000-frame runs are ignored tests: each takes about 67 s in the dev
//! profile, about 0.7 ms of test core, host, engine and resampler per
//! frame.
//!
//! ```sh
//! cargo xtask test -p neptron-session --run-ignored only -E 'test(hundred_thousand)'
//! ```

use neptron_session::RunRateSource;
use neptron_testkit::{AudioWrite, DisplayOptions, Jitter, LateFlips, Render};

use crate::harness::{DEVICE_RATE_HZ, Rig, Setup};
use crate::with_script;

/// The control span of the rate control at the device rate: 64 ms.
const SPAN_FRAMES: f64 = DEVICE_RATE_HZ as f64 * 0.064;
/// How long a late flip shows in the queue level: about 1.5 time
/// constants of the rate control.
const LATE_WINDOW_NS: u64 = 10_000_000_000;
/// The longest a late flip comes after its target: two periods at 50 Hz.
const LATE_FLIP_NS: u64 = 40_000_000;
/// The largest ratio deviation of the rate control: 0.5%.
const MAX_DEVIATION: f64 = 0.005;

fn script(fps_mhz: u32, sample_rate_hz: u32) -> String {
    format!(
        "neptron-testcore 1\npixel_format rgb565\ngeometry 256 224 1333\n\
         timing {fps_mhz} {sample_rate_hz}\n"
    )
}

/// Jitter of up to 0.5 ms on every vertical blank.
fn jitter(seed: u64) -> Jitter {
    Jitter::Seeded {
        seed,
        max_ns: 500_000,
    }
}

/// One flip in a thousand lands one vertical blank late.
fn late_flips(seed: u64) -> Option<LateFlips> {
    Some(LateFlips {
        seed,
        per_million: 1_000,
        max_vblanks: 1,
    })
}

struct Check {
    frames: u64,
    sample_rate_hz: u32,
    fps_mhz: u32,
    /// The level before each write, as a fraction of the span, after the
    /// first second.
    level_bounds: (f64, f64),
    /// The run is long enough for late flips to come close together. Each
    /// late flip takes one frame of audio (16.7 ms) from a 32 ms queue, and
    /// the law gives it back with a time constant of 6.4 s, so two late
    /// flips a few seconds apart run the queue dry. With this set, every
    /// underrun must follow two late flips in the 10 s before it.
    underruns_from_late_flips: bool,
    /// The mean level of the last 10 s must be inside these bounds. `None`
    /// for a long run with late flips, where a late flip can fall in the
    /// last 10 s; the jitter-only run proves the convergence, and the audio
    /// crate proves the recovery after a late flip.
    converged_bounds: Option<(f64, f64)>,
}

fn run_check(rig: &mut Rig, check: &Check) {
    for _ in 0..check.frames {
        rig.session.run_frame(None).expect("frame");
    }
    let counters = *rig.session.counters();
    assert_eq!(counters.core_frames, check.frames);
    assert_eq!(counters.flips, check.frames, "one flip per core frame");
    assert_eq!(counters.flipless_frames, 0);

    let frames = rig.display.frames();
    assert!(rig.display.dropped_frames().is_empty(), "no dropped frame");
    let late: u64 = frames.iter().filter_map(|f| f.late_by()).sum();
    assert_eq!(
        rig.display.repeated_vblanks(),
        late,
        "the only repeated blanks are the late flips of the display"
    );
    let hashes = rig.display.frame_hashes();
    if !hashes.is_empty() {
        assert!(
            hashes.windows(2).all(|pair| pair[0] != pair[1]),
            "each flip shows a new frame"
        );
    }

    let underruns = rig.audio.underruns();
    let late_ns: Vec<u64> = frames
        .iter()
        .filter(|f| f.late_by().is_some_and(|n| n > 0))
        .filter_map(|f| f.flip.map(|e| e.time_ns))
        .collect();
    if check.underruns_from_late_flips {
        for underrun in &underruns {
            let before = late_ns
                .iter()
                .filter(|t| {
                    // The flip that is late lands after the device ran dry.
                    **t <= underrun.time_ns + LATE_FLIP_NS
                        && **t + LATE_WINDOW_NS >= underrun.time_ns
                })
                .count();
            assert!(before >= 2, "{underrun:?} follows {before} late flips");
        }
    } else {
        assert!(underruns.is_empty(), "{underruns:?}");
    }
    assert!(rig.audio.overruns().is_empty());
    let writes = rig.audio.writes();
    let first_ns = writes.first().unwrap().time_ns;
    let levels: Vec<(u64, f64)> = writes
        .iter()
        .filter(|w| w.time_ns >= first_ns + 1_000_000_000)
        .map(|w: &AudioWrite| {
            let before = w.queued_after - w.taken_frames;
            (w.time_ns, before as f64 / SPAN_FRAMES)
        })
        .collect();
    let (low, high) = check.level_bounds;
    for (time_ns, level) in &levels {
        assert!(
            (low..=high).contains(level),
            "level {level:.3} of the span at {time_ns} ns"
        );
    }
    let end_ns = levels.last().unwrap().0;
    let tail: Vec<f64> = levels
        .iter()
        .filter(|(t, _)| *t + 10_000_000_000 >= end_ns)
        .map(|(_, l)| *l)
        .collect();
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    if let Some((low, high)) = check.converged_bounds {
        assert!(
            (low..=high).contains(&mean),
            "mean level {mean:.3} at the end"
        );
    }

    let stats = *rig.session.audio_stats().unwrap();
    assert!(stats.max_deviation <= MAX_DEVIATION + 1e-9, "{stats:?}");
    let run_rate = rig.session.run_rate_mhz().unwrap() as f64;
    let nominal = f64::from(DEVICE_RATE_HZ) * f64::from(check.fps_mhz)
        / (f64::from(check.sample_rate_hz) * run_rate);
    let mean_ratio = stats.output_frames as f64 / stats.input_frames as f64;
    let pitch_error = (mean_ratio / nominal - 1.0).abs();
    assert!(pitch_error < MAX_DEVIATION, "pitch error {pitch_error}");
}

fn setup(display: DisplayOptions) -> Setup {
    Setup {
        display,
        ..Setup::default()
    }
}

macro_rules! check6 {
    ($(#[$meta:meta])* $name:ident, $fps:expr, $rate:expr, $frames:expr, $display:expr, $levels:expr, $converged:expr) => {
        #[test]
        $(#[$meta])*
        fn $name() {
            with_script!(stringify!($name), &script($fps, $rate), |dir| {
                let mut rig = Rig::launch(dir, setup($display));
                let check = Check {
                    frames: $frames,
                    sample_rate_hz: $rate,
                    fps_mhz: $fps,
                    level_bounds: $levels,
                    underruns_from_late_flips: false,
                    converged_bounds: Some($converged),
                };
                run_check(&mut rig, &check);
            });
        }
    };
}

fn jittered(seed: u64) -> DisplayOptions {
    DisplayOptions {
        jitter: jitter(seed),
        ..DisplayOptions::default()
    }
}

fn jittered_and_late(seed: u64) -> DisplayOptions {
    DisplayOptions {
        render: Render::Off,
        jitter: jitter(seed),
        late_flips: late_flips(seed),
        ..DisplayOptions::default()
    }
}

// With exact rates, rate control holds the queue at its target.
check6!(
    check6_at_50_hz_with_jitter,
    50_000,
    44_100,
    1_500,
    jittered(0x50),
    (0.40, 0.60),
    (0.45, 0.55)
);
check6!(
    check6_at_59_94_hz_with_jitter,
    59_940,
    48_000,
    1_800,
    jittered(0x5994),
    (0.40, 0.60),
    (0.45, 0.55)
);
check6!(
    check6_at_60_10_hz_with_jitter,
    60_100,
    32_040,
    1_800,
    jittered(0x6010),
    (0.40, 0.60),
    (0.45, 0.55)
);

// A late flip takes one frame of audio from the queue; the queue never
// runs dry and the law brings it back.
check6!(
    check6_at_59_94_hz_with_late_flips,
    59_940,
    48_000,
    1_800,
    jittered_and_late(0xBAD),
    (0.15, 0.85),
    (0.40, 0.60)
);

/// The desktop window flips at the desktop rate, not at the mode's rate:
/// here 60.10 Hz under a 59.94 Hz game. The session measures the flip rate
/// and gives it to rate control, so the queue stays near its target and
/// the game runs at the display's rate.
#[test]
fn check6_measured_flip_rate_follows_the_display() {
    with_script!(
        "check6_measured_flip_rate_follows_the_display",
        &script(59_940, 48_000),
        |dir| {
            let setup = Setup {
                display: DisplayOptions {
                    refresh_mhz: Some(60_100),
                    jitter: jitter(0x77),
                    ..DisplayOptions::default()
                },
                run_rate: RunRateSource::Measured,
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            let check = Check {
                frames: 1_800,
                sample_rate_hz: 48_000,
                fps_mhz: 59_940,
                level_bounds: (0.25, 0.75),
                underruns_from_late_flips: false,
                converged_bounds: Some((0.45, 0.55)),
            };
            run_check(&mut rig, &check);
            // The meter averages 120 flip intervals (2 s); a jitter of up to
            // 0.5 ms at each end of that window moves the mean by up to
            // 1 ms in 2 s: 0.05%, 30 mHz at 60.1 Hz.
            let run_rate = rig.session.run_rate_mhz().unwrap();
            assert!(run_rate.abs_diff(60_100) <= 31, "run rate {run_rate}");
        }
    );
}

/// The script of the 100,000-frame runs. The frame is small, so the test
/// core draws it fast enough for the time limit of a test; the pacing and
/// the audio do not depend on the frame size.
const LONG_SCRIPT: &str = "neptron-testcore 1\npixel_format rgb565\ngeometry 128 112 1333\n\
                           timing 60100 32040\n";

fn long_check(late: bool) -> Check {
    // One late flip takes 0.26 of the span; one more before the law gave
    // it back takes the queue lower. With late flips the floor is the
    // underrun rule, not a level.
    let low = if late { 0.0 } else { 0.40 };
    Check {
        frames: 100_000,
        sample_rate_hz: 32_040,
        fps_mhz: 60_100,
        level_bounds: (low, if late { 0.85 } else { 0.60 }),
        underruns_from_late_flips: late,
        converged_bounds: (!late).then_some((0.45, 0.55)),
    }
}

/// 100,000 frames, about 28 simulated minutes, with jitter.
#[test]
#[ignore = "100,000 frames take about 67 s in the dev profile"]
fn hundred_thousand_frames_at_60_10_hz() {
    with_script!("hundred_thousand_frames_at_60_10_hz", LONG_SCRIPT, |dir| {
        let display = DisplayOptions {
            render: Render::Off,
            jitter: jitter(0x100_000),
            ..DisplayOptions::default()
        };
        let mut rig = Rig::launch(dir, setup(display));
        run_check(&mut rig, &long_check(false));
    });
}

/// 100,000 frames with jitter and one late flip in a thousand.
#[test]
#[ignore = "100,000 frames take about 67 s in the dev profile"]
fn hundred_thousand_frames_at_60_10_hz_with_late_flips() {
    with_script!(
        "hundred_thousand_frames_at_60_10_hz_with_late_flips",
        LONG_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, setup(jittered_and_late(0x100_000)));
            run_check(&mut rig, &long_check(true));
        }
    );
}
