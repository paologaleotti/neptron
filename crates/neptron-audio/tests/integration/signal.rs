//! The sound itself: no step larger than the signal allows, and the pitch
//! of a tone where it belongs.

use neptron_audio::Resampling;

use crate::harness::{
    Hz, SETTLE_NS, Session, Setup, Signal, assert_steady, frequency, largest_step, step_bound,
    upward_crossings,
};

const TONE_HZ: f64 = 440.0;
const AMPLITUDE: f64 = 0.25;
/// The run, in simulated seconds.
const RUN_S: u64 = 30;
/// The pitch is judged in windows of this many seconds of output.
const WINDOW_S: f64 = 1.0;
/// The largest pitch error in any window, as in timing check 6.
const WINDOW_PITCH_ERROR: f64 = 0.005;
/// The largest pitch error over the whole run that the movement of the
/// queue level does not explain. While the law moves the queue to its
/// fixed point, the device plays the frames the queue loses on top of the
/// frames written, so the mean pitch moves by that change over the frames
/// played.
const RUN_PITCH_ERROR: f64 = 0.000_1;

fn tone(setup: Setup) -> Setup {
    Setup {
        signal: Signal::Tone {
            freq_hz: TONE_HZ,
            amplitude: AMPLITUDE,
        },
        keep_samples: true,
        resampling: Resampling::Sinc,
        ..setup
    }
}

/// Asserts continuity and pitch over a run, and prints what it measured.
fn check_sound(session: &Session) {
    let setup = session.setup;
    let samples = session.audio.samples();
    let heard = setup.heard_hz(TONE_HZ);

    let step = largest_step(&samples);
    let bound = step_bound(AMPLITUDE, heard, setup.device_hz);
    assert!(step <= bound, "a step of {step}, bound {bound}");

    let rate = setup.device_rate_mhz() as f64 / 1_000.0;
    let crossings = upward_crossings(&samples);
    let end = crossings[crossings.len() - 1];
    let window = WINDOW_S * rate;
    let mut worst: f64 = 0.0;
    // The first second holds the priming silence and the fade-in.
    let mut from = rate;
    while from + window <= end {
        let measured = frequency(&crossings, from, from + window, rate);
        worst = worst.max((measured / heard - 1.0).abs());
        from += window;
    }
    let whole = frequency(&crossings, rate, end + 1.0, rate);
    let run_error = (whole / heard - 1.0).abs();
    let levels = session.levels();
    let start_level = levels
        .iter()
        .find(|(time, _)| *time >= SETTLE_NS)
        .map_or(0, |(_, level)| *level);
    let end_level = levels.last().map_or(0, |(_, level)| *level);
    let drift = start_level.abs_diff(end_level) as f64 / (end - rate);
    println!(
        "{TONE_HZ} Hz tone heard at {heard:.4} Hz, device {:+} ppm: largest step {step} (bound {bound}), \
         worst 1 s window pitch error {:.5}%, whole run {:.6}% ({whole:.4} Hz), \
         of which the level drift explains {:.6}%",
        setup.device_ppm,
        worst * 100.0,
        run_error * 100.0,
        drift * 100.0
    );
    assert!(worst <= WINDOW_PITCH_ERROR, "window pitch error {worst}");
    assert!(
        run_error <= RUN_PITCH_ERROR + drift,
        "run pitch error {run_error}, drift {drift}"
    );
}

#[test]
fn a_tone_keeps_its_pitch_and_has_no_clicks() {
    let mut session = Session::new(tone(Setup::snes(Resampling::Sinc)));
    session.run_seconds(RUN_S);
    assert_steady(&session, 0.25, 0.75);
    check_sound(&session);
}

#[test]
fn a_tone_keeps_its_pitch_on_a_device_0_1_percent_fast() {
    let mut session = Session::new(tone(Setup {
        device_ppm: 1_000,
        ..Setup::snes(Resampling::Sinc)
    }));
    session.run_seconds(RUN_S);
    assert_steady(&session, 0.25, 0.75);
    check_sound(&session);
}

#[test]
fn a_tone_keeps_its_pitch_on_a_device_0_1_percent_slow() {
    let mut session = Session::new(tone(Setup {
        device_ppm: -1_000,
        ..Setup::snes(Resampling::Sinc)
    }));
    session.run_seconds(RUN_S);
    assert_steady(&session, 0.25, 0.75);
    check_sound(&session);
}

/// A 50 Hz game with no region option, shown at 60 Hz.
fn forced_60(resampling: Resampling) -> Setup {
    Setup {
        core_rate: Hz::new(44_100, 1),
        core_fps: Hz::new(50, 1),
        display: Hz::new(60, 1),
        ..Setup::snes(resampling)
    }
}

#[test]
fn a_50_hz_game_forced_to_60_hz_stays_inside_the_bounds() {
    let mut session = Session::new(forced_60(Resampling::Polynomial));
    session.run_seconds(120);
    let measured = assert_steady(&session, 0.25, 0.75);
    println!(
        "forced 60 Hz: levels {:.3} to {:.3}, deviation after settling {:.5}",
        measured.low, measured.high, measured.settled_deviation
    );
}

#[test]
fn a_50_hz_game_forced_to_60_hz_plays_1_2_times_higher() {
    let mut session = Session::new(tone(forced_60(Resampling::Sinc)));
    assert!((session.setup.heard_hz(TONE_HZ) - 528.0).abs() < 1e-9);
    session.run_seconds(RUN_S);
    assert_steady(&session, 0.25, 0.75);
    check_sound(&session);
}
