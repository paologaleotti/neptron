//! A mode switch and a pause: mute, a stall with no flip, unmute.

use neptron_audio::{MUTE_COVER_MS, Resampling};
use neptron_backend::Clock;

use crate::harness::{
    EXACT_HIGH, EXACT_LOW, Session, Setup, Signal, deviation_bound, largest_step, measure,
    step_bound,
};

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

    // The first frame after the stall finds the queue empty and primes it
    // to the target. From the second frame on, the level before each write
    // is inside the bounds: the recovery takes one frame.
    let after: Vec<f64> = session
        .levels()
        .into_iter()
        .filter(|(time, _)| *time > resumed_ns)
        .map(|(_, level)| level as f64 / span)
        .collect();
    let low = after.iter().copied().fold(f64::MAX, f64::min);
    let high = after.iter().copied().fold(f64::MIN, f64::max);
    println!(
        "mode switch: levels before {:.3} to {:.3}, after {low:.3} to {high:.3} of the span, from the second frame",
        before.low, before.high
    );
    assert!(low >= EXACT_LOW && high <= EXACT_HIGH, "{low} to {high}");

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
    let after: Vec<f64> = session
        .levels()
        .into_iter()
        .filter(|(time, _)| *time > resumed_ns)
        .map(|(_, level)| level as f64 / span)
        .collect();
    let low = after.iter().copied().fold(f64::MAX, f64::min);
    let high = after.iter().copied().fold(f64::MIN, f64::max);
    assert!(low >= EXACT_LOW && high <= EXACT_HIGH, "{low} to {high}");
}

/// The muted frame from which the level before each write is back inside
/// the exact-rate bounds: the cover above the target (3264 frames) drains
/// by one frame of output (about 798 frames) in each muted frame.
const MUTED_FRAMES_TO_TARGET: usize = 5;

#[test]
fn a_mute_without_a_stall_trims_its_cover_back_to_the_target() {
    let mut session = tone_session(Resampling::Polynomial);
    session.run_seconds(2);
    session.pipeline.mute();
    let muted_ns = session.audio.writes().last().unwrap().time_ns;
    session.run_seconds(2);
    session.pipeline.unmute();
    session.run_frames(1);
    let unmuted_ns = session.audio.writes().last().unwrap().time_ns;
    session.run_seconds(2);
    let stats = session.stats();
    assert!(session.audio.underruns().is_empty());
    assert!(session.audio.overruns().is_empty());
    assert_eq!(stats.overrun_frames, 0);
    assert_eq!(stats.input_dropped_frames, 0);
    assert_eq!(stats.resampler_errors, 0);
    assert_eq!(stats.restarts, 1);
    assert!(
        stats.max_deviation <= deviation_bound(),
        "{}",
        stats.max_deviation
    );

    let cover = (48_000 * MUTE_COVER_MS / 1_000) as usize;
    let span = session.span_frames() as f64;
    let levels = session.levels();
    let muted: Vec<usize> = levels
        .iter()
        .filter(|(time, _)| *time > muted_ns && *time < unmuted_ns)
        .map(|(_, level)| *level)
        .collect();
    // The first muted frame finds the cover less one frame of output, and
    // no muted frame finds more than the cover.
    assert!(muted[0] + 1_000 >= cover, "{}", muted[0]);
    assert!(muted.iter().all(|l| *l <= cover), "{muted:?}");
    // Each muted frame drops its silence while the queue is above the
    // target, so the queue is back at the target after a few frames and
    // stays there, through the unmute.
    let settled: Vec<f64> = levels
        .iter()
        .filter(|(time, _)| *time > muted_ns)
        .skip(MUTED_FRAMES_TO_TARGET)
        .map(|(_, level)| *level as f64 / span)
        .collect();
    let low = settled.iter().copied().fold(f64::MAX, f64::min);
    let high = settled.iter().copied().fold(f64::MIN, f64::max);
    println!(
        "mute with no stall: levels from muted frame {MUTED_FRAMES_TO_TARGET} on {low:.3} to {high:.3} of the span"
    );
    assert!(low >= EXACT_LOW && high <= EXACT_HIGH, "{low} to {high}");
}

/// A TV standard change on the Pi 3 (NTSC to PAL): about 75 ms from the
/// last flip of the old mode to the first flip of the new one.
const TV_STANDARD_CHANGE_NS: u64 = 75 * MS;

#[test]
fn a_tv_standard_change_does_not_run_the_muted_queue_dry() {
    for gap_ns in [TV_STANDARD_CHANGE_NS, 90 * MS] {
        let mut session = tone_session(Resampling::Sinc);
        session.run_seconds(2);
        // The mute comes after the last flip of the old mode, and the next
        // frame ends at the first flip of the new one.
        let last_flip_ns = session.clock.now_ns();
        session.pipeline.mute();
        session.stall(gap_ns - (session.next_flip_ns() - last_flip_ns));
        session.run_frames(1);
        assert_eq!(session.clock.now_ns() - last_flip_ns, gap_ns);
        session.pipeline.unmute();
        session.run_seconds(3);

        assert!(
            session.audio.underruns().is_empty(),
            "{gap_ns} ns: {:?}",
            session.audio.underruns()
        );
        assert_eq!(session.stats().underruns, 0);
        assert_eq!(session.stats().restarts, 1);
        assert_eq!(session.stats().overrun_frames, 0);
        // No click and no old sound: the fade-out, silence, the fade-in.
        let samples = session.audio.samples();
        let step = largest_step(&samples);
        let bound = step_bound(AMPLITUDE, TONE_HZ, 48_000);
        assert!(step <= bound, "a step of {step}, bound {bound}");
    }
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
    let high = last.iter().copied().fold(f64::MIN, f64::max);
    println!("late flip: level after 30 s {low:.3} to {high:.3} of the span");
    // Back within 0.01 of the span from the target.
    assert!(low >= 0.49 && high <= 0.51, "{low} to {high}");
}
