//! A mode switch and a pause: mute, a stall with no flip, unmute.

use neptron_audio::Resampling;

use crate::harness::{Session, Setup, Signal, assert_steady, largest_step, measure, step_bound};

const MS: u64 = 1_000_000;
/// The blocking modeset of the test: 100 ms with no flip.
const SWITCH_NS: u64 = 100 * MS;
const TONE_HZ: f64 = 440.0;
const AMPLITUDE: f64 = 0.25;

fn tone_session(resampling: Resampling) -> Session {
    Session::new(Setup {
        signal: Signal::Tone {
            freq_hz: TONE_HZ,
            amplitude: AMPLITUDE,
        },
        keep_samples: true,
        ..Setup::snes(resampling)
    })
}

#[test]
fn a_muted_mode_switch_recovers_at_the_first_frame_after_it() {
    let mut session = tone_session(Resampling::Sinc);
    session.run_seconds(5);
    let before = measure(&session);
    let span = session.span_frames() as f64;

    session.pipeline.mute();
    session.run_frames(1);
    let samples_before = session.audio.samples();
    let stall_start = session.audio.writes().last().unwrap().time_ns;
    // The device runs dry on silence only.
    assert_eq!(samples_before[samples_before.len() - 2..], [0, 0]);

    session.stall(SWITCH_NS);
    session.pipeline.unmute();
    session.run_frames(1);
    let resumed_ns = session.audio.writes().last().unwrap().time_ns;
    session.run_seconds(10);

    // The only underrun is inside the stall, before the first frame after
    // it.
    let underruns = session.audio.underruns();
    assert_eq!(underruns.len(), 1, "{underruns:?}");
    assert!(underruns[0].time_ns > stall_start && underruns[0].time_ns < resumed_ns);
    assert_eq!(session.stats().restarts, 2);
    assert_eq!(session.stats().overrun_frames, 0);

    // From the second frame after the stall, every level is inside the
    // steady bounds: the recovery takes one frame.
    let after: Vec<f64> = session
        .levels()
        .into_iter()
        .filter(|(time, _)| *time > resumed_ns)
        .map(|(_, level)| level as f64 / span)
        .collect();
    let low = after.iter().copied().fold(f64::MAX, f64::min);
    let high = after.iter().copied().fold(f64::MIN, f64::max);
    println!(
        "mode switch: levels before {:.3} to {:.3}, after {low:.3} to {high:.3} of the span, from the first frame",
        before.low, before.high
    );
    assert!(low >= 0.25 && high <= 0.75, "{low} to {high}");

    // No click: the fades keep each step inside the bound of a sine.
    let samples = session.audio.samples();
    let step = largest_step(&samples);
    let bound = step_bound(AMPLITUDE, TONE_HZ, 48_000);
    println!("mode switch: largest step {step}, bound {bound}");
    assert!(step <= bound, "a step of {step}, bound {bound}");
}

#[test]
fn a_pause_drains_silence_and_the_resume_primes_again() {
    let mut session = tone_session(Resampling::Polynomial);
    session.run_seconds(2);
    session.pipeline.mute();
    session.stall(3_000 * MS);
    session.pipeline.unmute();
    session.run_frames(1);
    let resumed_ns = session.audio.writes().last().unwrap().time_ns;
    session.run_seconds(5);
    assert_eq!(session.audio.underruns().len(), 1);
    assert_eq!(session.stats().restarts, 2);
    let span = session.span_frames() as f64;
    let low = session
        .levels()
        .into_iter()
        .filter(|(time, _)| *time > resumed_ns)
        .map(|(_, level)| level as f64 / span)
        .fold(f64::MAX, f64::min);
    assert!(low >= 0.25, "{low}");
}

#[test]
fn a_mute_without_a_stall_keeps_the_queue_and_the_law() {
    let mut session = tone_session(Resampling::Polynomial);
    session.run_seconds(2);
    session.pipeline.mute();
    session.run_seconds(2);
    session.pipeline.unmute();
    session.run_seconds(2);
    assert_steady(&session, 0.25, 0.75);
}

#[test]
fn an_unmuted_stall_that_empties_the_queue_restarts_it() {
    let mut session = tone_session(Resampling::Polynomial);
    session.run_seconds(2);
    session.stall(SWITCH_NS);
    session.run_seconds(2);
    assert_eq!(session.audio.underruns().len(), 1);
    assert_eq!(session.stats().restarts, 2);
    assert_eq!(session.stats().overrun_frames, 0);
}

#[test]
fn a_late_flip_is_recovered_by_the_law() {
    let mut session = tone_session(Resampling::Polynomial);
    session.run_seconds(2);
    // One missed vblank: one frame of audio less in that time.
    session.stall(16_639_267);
    session.run_seconds(30);
    assert!(session.audio.underruns().is_empty());
    let span = session.span_frames() as f64;
    let last: Vec<f64> = session
        .levels()
        .into_iter()
        .rev()
        .take(60)
        .map(|(_, level)| level as f64 / span)
        .collect();
    let low = last.iter().copied().fold(f64::MAX, f64::min);
    println!("late flip: level after 30 s {low:.3} of the span");
    assert!(low >= 0.49, "{low}");
}
