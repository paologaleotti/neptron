//! A quiet tone through the pipeline and the SDL3 sink for one second.
//! It needs a sound device, so no tier runs it:
//!
//! ```sh
//! cargo xtask test -p neptron-audio --features sdl3 --run-ignored only
//! ```
//!
//! The device's queue paces the run: a core frame runs whenever the queue
//! is at or below its target. The test proves that the device opened and
//! took the samples; whether the tone is heard needs a person.

#![forbid(unsafe_code)]
#![cfg(feature = "sdl3")]

use std::f64::consts::TAU;
use std::time::{Duration, Instant};

use neptron_audio::{AudioPipeline, CoreTiming, Resampling, Sdl3Sink};
use neptron_backend::AudioSink;

const CORE: CoreTiming = CoreTiming {
    sample_rate_mhz: 32_040_500,
    frame_rate_mhz: 60_099,
};
/// -26 dBFS: quiet.
const AMPLITUDE: f64 = 0.05;
const TONE_HZ: f64 = 440.0;
/// The device must play one second within this time.
const DEADLINE: Duration = Duration::from_secs(5);

#[test]
#[ignore = "needs a sound device"]
fn a_tone_plays_through_the_sdl3_sink_for_one_second() {
    let sink = Sdl3Sink::open().unwrap();
    let rate_hz = sink.rate_hz();
    let device_buffer = sink.device_buffer_frames();
    let mut pipeline =
        AudioPipeline::new(sink, CORE, CORE.frame_rate_mhz, Resampling::Sinc).unwrap();
    let target = pipeline.control().target_frames();

    let started = Instant::now();
    let mut frames = 0u64;
    let mut input_frame = 0u64;
    let mut buffer = Vec::new();
    let mut levels = (usize::MAX, 0usize);
    loop {
        let stats = *pipeline.stats();
        let written = stats.output_frames + stats.silence_frames - stats.overrun_frames;
        let queued = pipeline.sink().queued_frames();
        let played = written.saturating_sub(queued as u64);
        if played >= u64::from(rate_hz) {
            break;
        }
        assert!(
            started.elapsed() < DEADLINE,
            "the device played {played} frames"
        );
        if queued > target {
            std::thread::yield_now();
            continue;
        }
        let count = (frames + 1) * 320_405_000 / 600_988 - frames * 320_405_000 / 600_988;
        buffer.clear();
        for _ in 0..count {
            let phase = TAU * TONE_HZ * input_frame as f64 / 32_040.5;
            let value = (AMPLITUDE * 32_768.0 * phase.sin()).round() as i16;
            buffer.extend_from_slice(&[value, value]);
            input_frame += 1;
        }
        pipeline.push_samples(&buffer);
        pipeline.end_frame();
        frames += 1;
        if frames > 1 {
            levels = (levels.0.min(queued), levels.1.max(queued));
        }
    }
    let stats = *pipeline.stats();
    println!(
        "device {rate_hz} Hz, device buffer {device_buffer:?} frames, {frames} core frames in {:?}, \
         queue {} to {} frames before a write, {stats:?}",
        started.elapsed(),
        levels.0,
        levels.1
    );
    assert_eq!(stats.overrun_frames, 0);
    assert_eq!(stats.resampler_errors, 0);
}
