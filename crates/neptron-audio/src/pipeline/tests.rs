use neptron_testkit::{AudioOptions, FakeAudio, SimClock};

use super::*;

const MS: u64 = 1_000_000;
const SNES: CoreTiming = CoreTiming {
    sample_rate_mhz: 32_040_500,
    frame_rate_mhz: 60_099,
};

fn fake(clock: &SimClock, rate_hz: u32, capacity_frames: usize) -> FakeAudio {
    FakeAudio::with_options(
        clock.clone(),
        rate_hz,
        AudioOptions {
            capacity_frames,
            device_rate_mhz: None,
            keep_samples: true,
        },
    )
}

fn pipeline(clock: &SimClock) -> (AudioPipeline<FakeAudio>, FakeAudio) {
    let audio = fake(clock, 48_000, 6_144);
    let pipeline = AudioPipeline::new(audio.clone(), SNES, 60_099, Resampling::Polynomial).unwrap();
    (pipeline, audio)
}

#[test]
fn rates_outside_the_limits_are_refused() {
    let clock = SimClock::new();
    let slow_device = fake(&clock, 4_000, 1_000);
    assert_eq!(
        AudioPipeline::new(slow_device, SNES, 60_099, Resampling::Sinc).err(),
        Some(AudioError::DeviceRate {
            rate_hz: 4_000,
            min_hz: 8_000,
            max_hz: 384_000
        })
    );
    let bad_core = CoreTiming {
        frame_rate_mhz: 5_000,
        ..SNES
    };
    assert!(matches!(
        AudioPipeline::new(
            fake(&clock, 48_000, 1_000),
            bad_core,
            5_000,
            Resampling::Sinc
        ),
        Err(AudioError::CoreFrameRate { .. })
    ));
}

#[test]
fn a_refused_run_rate_keeps_the_old_one() {
    let clock = SimClock::new();
    let (mut pipeline, _) = pipeline(&clock);
    assert_eq!(pipeline.run_rate_mhz(), 60_099);
    let nominal = pipeline.nominal_ratio();
    assert!((nominal - 48_000.0 / 32_040.5).abs() < 1e-12);
    assert_eq!(
        pipeline.set_run_rate_mhz(29_000),
        Err(AudioError::RunRatio {
            run_rate_mhz: 29_000,
            frame_rate_mhz: 60_099
        })
    );
    assert_eq!(pipeline.run_rate_mhz(), 60_099);
    assert_eq!(pipeline.nominal_ratio(), nominal);
    assert!(pipeline.set_run_rate_mhz(0).is_err());
    assert_eq!(pipeline.nominal_ratio(), nominal);
    assert!(pipeline.nominal_ratio().is_finite());
    assert_eq!(pipeline.set_run_rate_mhz(72_119), Ok(()));
    assert!(pipeline.nominal_ratio() < nominal);
}

#[test]
fn the_first_frame_primes_the_queue_with_silence_then_fades_in() {
    let clock = SimClock::new();
    let (mut pipeline, audio) = pipeline(&clock);
    pipeline.push_samples(&[8_000; 533 * 2]);
    pipeline.end_frame();

    let writes = audio.writes();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].offered_frames, 1_536);
    let stats = *pipeline.stats();
    assert_eq!(stats.restarts, 1);
    assert_eq!(stats.silence_frames, 1_536);
    assert_eq!(stats.level_frames, 0);
    assert_eq!(stats.input_frames, 533);
    // An empty queue asks for the largest ratio.
    assert!((stats.ratio / pipeline.nominal_ratio() - 1.005).abs() < 1e-12);

    let samples = audio.samples();
    assert!(samples[..1_536 * 2].iter().all(|s| *s == 0));
    let audio_part = &samples[1_536 * 2..];
    assert_eq!(audio_part.len() / 2, stats.output_frames as usize);
    // 240 frames of fade at 48 kHz, then the full level.
    assert_eq!(pipeline.fade_frames, 240);
    assert_eq!(audio_part[0], 0);
    assert!(audio_part[2 * 100] > 0 && audio_part[2 * 100] < 8_000);
    assert_eq!(audio_part[2 * 400], 8_000);
}

#[test]
fn a_queue_that_ran_dry_is_primed_again() {
    let clock = SimClock::new();
    let (mut pipeline, audio) = pipeline(&clock);
    pipeline.push_samples(&[100; 533 * 2]);
    pipeline.end_frame();
    clock.advance(200 * MS);
    assert_eq!(audio.underruns().len(), 1);
    pipeline.push_samples(&[100; 533 * 2]);
    pipeline.end_frame();
    assert_eq!(pipeline.stats().restarts, 2);
    assert_eq!(pipeline.stats().silence_frames, 2 * 1_536);
}

#[test]
fn input_beyond_the_frame_limit_is_dropped_and_counted() {
    let clock = SimClock::new();
    let (mut pipeline, _) = pipeline(&clock);
    // 100 ms at 32 040.5 Hz is 3205 frames, rounded up.
    pipeline.push_samples(&[0; 3_000 * 2]);
    pipeline.push_samples(&[0; 300 * 2 + 1]);
    assert_eq!(pipeline.stats().input_frames, 3_205);
    assert_eq!(pipeline.stats().input_dropped_frames, 95);
    pipeline.end_frame();
    pipeline.push_samples(&[0; 10 * 2]);
    assert_eq!(pipeline.stats().input_frames, 3_215);
}

#[test]
fn a_full_sink_counts_the_frames_it_refused() {
    let clock = SimClock::new();
    let audio = fake(&clock, 48_000, 2_000);
    let mut pipeline =
        AudioPipeline::new(audio.clone(), SNES, 60_099, Resampling::Polynomial).unwrap();
    pipeline.push_samples(&[0; 1_000 * 2]);
    pipeline.end_frame();
    let stats = *pipeline.stats();
    let written = audio.written_frames();
    assert_eq!(written, 2_000);
    assert_eq!(
        stats.overrun_frames,
        stats.silence_frames + stats.output_frames - written
    );
    assert!(stats.overrun_frames > 0);
    assert_eq!(audio.overruns().len(), 1);
}

#[test]
fn mute_fades_out_from_the_last_sample_and_tops_up_with_silence() {
    let clock = SimClock::new();
    let (mut pipeline, audio) = pipeline(&clock);
    pipeline.push_samples(&[10_000; 800 * 2]);
    pipeline.end_frame();
    let before = audio.samples().len();
    assert_eq!(audio.samples()[before - 2..], [10_000, 10_000]);

    clock.advance(10 * MS);
    pipeline.mute();
    assert!(pipeline.is_muted());
    let samples = audio.samples();
    let fade = &samples[before..before + 240 * 2];
    assert_eq!(fade[0], 9_958);
    assert_eq!(fade[fade.len() - 1], 0);
    assert!(fade.windows(4).step_by(2).all(|w| w[2] <= w[0]));
    assert!(samples[before + 240 * 2..].iter().all(|s| *s == 0));

    // Muted frames still run the law and write silence of their length.
    let silence_before = pipeline.stats().silence_frames;
    let written_before = audio.written_frames();
    pipeline.push_samples(&[10_000; 533 * 2]);
    pipeline.end_frame();
    let output = pipeline.stats().output_frames;
    assert!(audio.samples()[samples.len()..].iter().all(|s| *s == 0));
    assert!(audio.written_frames() >= written_before + 533);
    assert!(pipeline.stats().silence_frames >= silence_before);
    assert!(output > 0);

    let written = audio.written_frames();
    pipeline.mute();
    assert_eq!(audio.written_frames(), written);
}

#[test]
fn unmute_fades_the_next_frame_in() {
    let clock = SimClock::new();
    let (mut pipeline, audio) = pipeline(&clock);
    pipeline.push_samples(&[10_000; 800 * 2]);
    pipeline.end_frame();
    pipeline.mute();
    pipeline.unmute();
    pipeline.unmute();
    assert!(!pipeline.is_muted());
    let before = audio.samples().len();
    clock.advance(16 * MS);
    pipeline.push_samples(&[10_000; 800 * 2]);
    pipeline.end_frame();
    let samples = audio.samples();
    let part = &samples[before..];
    assert_eq!(part[0], 0);
    assert!(
        part[2 * 120] > 3_000 && part[2 * 120] < 7_000,
        "{}",
        part[240]
    );
    assert_eq!(part[2 * 300], 10_000);
}

#[test]
fn the_sinc_filter_delays_by_95_frames_from_32_khz_to_48_khz() {
    let clock = SimClock::new();
    let audio = fake(&clock, 48_000, 6_144);
    let pipeline = AudioPipeline::new(audio, SNES, 60_099, Resampling::Sinc).unwrap();
    assert_eq!(pipeline.resampler_delay_frames(), 95);
    let audio = fake(&clock, 48_000, 6_144);
    let pipeline = AudioPipeline::new(audio, SNES, 60_099, Resampling::Polynomial).unwrap();
    assert!(pipeline.resampler_delay_frames() <= 2);
}

#[test]
fn a_frame_without_audio_writes_nothing_but_the_priming() {
    let clock = SimClock::new();
    let (mut pipeline, audio) = pipeline(&clock);
    pipeline.end_frame();
    assert_eq!(audio.written_frames(), 1_536);
    pipeline.end_frame();
    assert_eq!(audio.written_frames(), 1_536);
    assert_eq!(pipeline.stats().frames, 2);
    assert_eq!(pipeline.stats().output_frames, 0);
    assert_eq!(pipeline.into_sink().written_frames(), 1_536);
}

#[test]
fn the_run_rate_at_construction_is_checked_and_used() {
    let clock = SimClock::new();
    assert_eq!(
        AudioPipeline::new(fake(&clock, 48_000, 6_144), SNES, 0, Resampling::Sinc).err(),
        Some(AudioError::RunRatio {
            run_rate_mhz: 0,
            frame_rate_mhz: 60_099
        })
    );
    let forced =
        AudioPipeline::new(fake(&clock, 48_000, 6_144), SNES, 72_119, Resampling::Sinc).unwrap();
    assert_eq!(forced.run_rate_mhz(), 72_119);
    let expected = 48_000.0 * 60_099.0 / (32_040.5 * 72_119.0);
    assert!((forced.nominal_ratio() - expected).abs() < 1e-12);
}

#[test]
fn the_run_rate_can_cross_its_whole_range_after_construction() {
    for resampling in [Resampling::Sinc, Resampling::Polynomial] {
        let clock = SimClock::new();
        let audio = fake(&clock, 48_000, 6_144);
        let mut pipeline = AudioPipeline::new(audio, SNES, 30_050, resampling).unwrap();
        for run in [120_198, 30_050, 120_198] {
            pipeline.set_run_rate_mhz(run).unwrap();
            for _ in 0..3 {
                clock.advance(16 * MS);
                pipeline.push_samples(&[100; 533 * 2]);
                pipeline.end_frame();
            }
        }
        assert_eq!(pipeline.stats().resampler_errors, 0, "{resampling:?}");
        assert!(pipeline.stats().output_frames > 0);
    }
}

/// The left samples the device took after the priming silence.
fn audio_part(audio: &FakeAudio) -> Vec<i16> {
    audio.samples()[1_536 * 2..]
        .iter()
        .step_by(2)
        .copied()
        .collect()
}

#[test]
fn a_pipeline_at_half_volume_writes_an_eighth_of_each_sample() {
    let clock = SimClock::new();
    let (pipeline, audio) = pipeline(&clock);
    let mut pipeline = pipeline.with_volume_percent(50);
    assert_eq!(pipeline.volume_percent(), 50);
    for _ in 0..3 {
        pipeline.push_samples(&[8_000; 533 * 2]);
        pipeline.end_frame();
        clock.advance(16 * MS);
    }
    let left = audio_part(&audio);
    assert_eq!(left[0], 0, "the fade-in still starts at zero");
    assert!(
        left[400..].iter().all(|s| *s == 1_000),
        "{:?}",
        &left[400..410]
    );
}

#[test]
fn a_volume_change_ramps_with_no_step_and_reaches_silence() {
    let clock = SimClock::new();
    let (mut pipeline, audio) = pipeline(&clock);
    for frame in 0..8 {
        if frame == 4 {
            pipeline.set_volume_percent(0);
        }
        pipeline.push_samples(&[8_000; 533 * 2]);
        pipeline.end_frame();
        clock.advance(16 * MS);
    }
    let left = audio_part(&audio);
    // After the fade-in the level is 8000; the 20 ms ramp falls 8000 / 960
    // per frame.
    let steady = &left[400..];
    let largest = steady
        .windows(2)
        .map(|w| (i32::from(w[1]) - i32::from(w[0])).abs())
        .max()
        .unwrap();
    assert!(largest <= 8_000 / 960 + 1, "a step of {largest}");
    let falling = steady.iter().filter(|s| **s > 0 && **s < 8_000).count();
    assert!((955..=965).contains(&falling), "{falling} frames of ramp");
    assert!(left.contains(&8_000));
    assert_eq!(left.last(), Some(&0));
    assert_eq!(pipeline.volume_percent(), 0);
}

#[test]
fn the_mute_and_the_volume_are_separate() {
    let clock = SimClock::new();
    let (pipeline, _) = pipeline(&clock);
    let mut pipeline = pipeline.with_volume_percent(40);
    pipeline.push_samples(&[8_000; 533 * 2]);
    pipeline.end_frame();
    pipeline.mute();
    assert_eq!(pipeline.volume_percent(), 40);
    pipeline.set_volume_percent(70);
    assert!(pipeline.is_muted());
    pipeline.unmute();
    assert_eq!(pipeline.volume_percent(), 70);
    pipeline.set_volume_percent(500);
    assert_eq!(pipeline.volume_percent(), 100);
}

/// A sink whose device is lost after `lost_after` writes: it then takes
/// nothing and reports a full queue, as `Sdl3Sink` does on an SDL error.
struct StuckSink {
    inner: FakeAudio,
    writes: usize,
    lost_after: usize,
}

impl AudioSink for StuckSink {
    fn rate_hz(&self) -> u32 {
        self.inner.rate_hz()
    }
    fn queued_frames(&self) -> usize {
        if self.writes >= self.lost_after {
            6_144
        } else {
            self.inner.queued_frames()
        }
    }
    fn write(&mut self, stereo: &[i16]) -> usize {
        self.writes += 1;
        if self.writes > self.lost_after {
            0
        } else {
            self.inner.write(stereo)
        }
    }
}

/// A sink whose device is lost and drops what it gets, as an SDL zombie
/// device does, but with no queue left at all.
struct DroppingSink {
    rate_hz: u32,
}

impl AudioSink for DroppingSink {
    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }
    fn queued_frames(&self) -> usize {
        0
    }
    fn write(&mut self, stereo: &[i16]) -> usize {
        stereo.len() / 2
    }
}

#[test]
fn a_device_lost_with_a_full_queue_costs_frames_and_never_blocks() {
    let clock = SimClock::new();
    let sink = StuckSink {
        inner: fake(&clock, 48_000, 6_144),
        writes: 0,
        lost_after: 10,
    };
    let mut pipeline = AudioPipeline::new(sink, SNES, 60_099, Resampling::Polynomial).unwrap();
    for frame in 0..600 {
        if frame == 300 {
            pipeline.set_volume_percent(30);
            pipeline.mute();
            pipeline.unmute();
        }
        pipeline.push_samples(&[8_000; 533 * 2]);
        pipeline.end_frame();
        clock.advance(16 * MS);
    }
    let stats = pipeline.stats();
    assert_eq!(stats.frames, 600);
    assert!(stats.overrun_frames > 500 * 700, "{stats:?}");
    assert_eq!(stats.level_frames, 6_144);
}

#[test]
fn a_device_lost_that_drops_its_queue_restarts_each_frame_and_goes_on() {
    let sink = DroppingSink { rate_hz: 48_000 };
    let mut pipeline = AudioPipeline::new(sink, SNES, 60_099, Resampling::Polynomial).unwrap();
    for _ in 0..600 {
        pipeline.push_samples(&[8_000; 533 * 2]);
        pipeline.end_frame();
    }
    let stats = pipeline.stats();
    assert_eq!(stats.frames, 600);
    assert_eq!(stats.restarts, 600);
    assert_eq!(stats.overrun_frames, 0);
}

#[test]
fn a_pipeline_at_100_is_bit_exact_with_no_volume_stage() {
    let run = |volume: Option<u32>| {
        let clock = SimClock::new();
        let (pipeline, audio) = pipeline(&clock);
        let mut pipeline = match volume {
            Some(v) => pipeline.with_volume_percent(v),
            None => pipeline,
        };
        assert_eq!(pipeline.volume_percent(), 100);
        for frame in 0..30i16 {
            pipeline.push_samples(&[frame * 1_000 - 15_000; 533 * 2]);
            pipeline.end_frame();
            clock.advance(16 * MS);
        }
        audio.samples()
    };
    let default = run(None);
    assert_eq!(run(Some(100)), default);
    let clock = SimClock::new();
    let (pipeline, audio) = pipeline(&clock);
    let mut pipeline = pipeline.with_volume_percent(100);
    pipeline.set_volume_percent(100);
    for frame in 0..30i16 {
        pipeline.push_samples(&[frame * 1_000 - 15_000; 533 * 2]);
        pipeline.end_frame();
        clock.advance(16 * MS);
    }
    assert_eq!(
        audio.samples(),
        default,
        "a change to 100 at 100 has no ramp"
    );
}
