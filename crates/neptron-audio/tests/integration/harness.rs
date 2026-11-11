//! A session on the fakes: a display-locked loop that runs one core frame
//! per simulated flip and gives its audio to the pipeline.

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use neptron_audio::{
    AudioPipeline, AudioStats, CoreTiming, FADE_MS, MAX_RATE_DEVIATION_PPM, Resampling,
};
use neptron_backend::Clock;
use neptron_testkit::{AudioOptions, FakeAudio, SimClock};

/// Nanoseconds in one second.
const NS_PER_S: u128 = 1_000_000_000;
/// The sink queue: 128 ms, as the SDL3 sink.
const SINK_CAPACITY_MS: u64 = 128;

/// A rate in hertz as an exact fraction `num / den`.
#[derive(Debug, Clone, Copy)]
pub struct Hz {
    pub num: u64,
    pub den: u64,
}

impl Hz {
    pub const fn new(num: u64, den: u64) -> Self {
        Self { num, den }
    }

    /// The rate in millihertz, rounded, as the session gives it.
    pub fn mhz(self) -> u64 {
        ((u128::from(self.num) * 1_000 + u128::from(self.den) / 2) / u128::from(self.den)) as u64
    }

    pub fn as_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// This rate off by `ppm` parts per million.
    pub fn skewed(self, ppm: i64) -> Self {
        let num = i128::from(self.num) * (1_000_000 + i128::from(ppm));
        Self {
            num: num as u64,
            den: self.den * 1_000_000,
        }
    }
}

/// The SNES: 60.0988 Hz video, 32 040.5 Hz audio.
pub const SNES_FPS: Hz = Hz::new(600_988, 10_000);
pub const SNES_RATE: Hz = Hz::new(320_405, 10);

/// What the core plays.
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    /// A cheap integer wave, different on each channel, for long runs.
    Ramp,
    /// A sine of `freq_hz` at `amplitude` of full scale on both channels.
    Tone { freq_hz: f64, amplitude: f64 },
}

#[derive(Debug, Clone, Copy)]
pub struct Setup {
    pub core_rate: Hz,
    pub core_fps: Hz,
    /// The true flip rate of the display.
    pub display: Hz,
    pub device_hz: u32,
    /// The device crystal error, in parts per million.
    pub device_ppm: i64,
    /// Give the display rate to the pipeline as the run rate.
    pub tell_display: bool,
    pub resampling: Resampling,
    pub signal: Signal,
    pub keep_samples: bool,
}

impl Setup {
    /// The SNES on a 48 kHz device, the display at the core's rate.
    pub fn snes(resampling: Resampling) -> Self {
        Self {
            core_rate: SNES_RATE,
            core_fps: SNES_FPS,
            display: SNES_FPS,
            device_hz: 48_000,
            device_ppm: 0,
            tell_display: true,
            resampling,
            signal: Signal::Ramp,
            keep_samples: false,
        }
    }

    pub fn device_rate_mhz(&self) -> u64 {
        let nominal = i128::from(self.device_hz) * 1_000;
        (nominal * (1_000_000 + i128::from(self.device_ppm)) / 1_000_000) as u64
    }

    /// The frequency of a tone as the device plays it, in real time: the
    /// game runs at the display rate.
    pub fn heard_hz(&self, tone_hz: f64) -> f64 {
        tone_hz * self.display.as_f64() / self.core_fps.as_f64()
    }
}

pub struct Session {
    pub setup: Setup,
    pub clock: SimClock,
    pub audio: FakeAudio,
    pub pipeline: AudioPipeline<FakeAudio>,
    /// Core frames run.
    frames: u64,
    /// Time added to the flip times by stalls.
    offset_ns: u64,
    /// Core audio frames made.
    input_frames: u64,
    buffer: Vec<i16>,
    /// The largest `|ratio / nominal - 1|` of the frames after
    /// [`SETTLE_NS`].
    pub settled_deviation: f64,
}

/// Levels and deviations are judged after this time: the law moves from
/// the target to its fixed point first.
pub const SETTLE_NS: u64 = 1_000_000_000;

impl Session {
    pub fn new(setup: Setup) -> Self {
        let clock = SimClock::new();
        let audio = FakeAudio::with_options(
            clock.clone(),
            setup.device_hz,
            AudioOptions {
                capacity_frames: (u64::from(setup.device_hz) * SINK_CAPACITY_MS / 1_000) as usize,
                device_rate_mhz: Some(setup.device_rate_mhz()),
                keep_samples: setup.keep_samples,
            },
        );
        let core = CoreTiming {
            sample_rate_mhz: setup.core_rate.mhz(),
            frame_rate_mhz: setup.core_fps.mhz(),
        };
        let mut pipeline = AudioPipeline::new(audio.clone(), core, setup.resampling).unwrap();
        if setup.tell_display {
            pipeline.set_run_rate_mhz(setup.display.mhz()).unwrap();
        }
        Self {
            setup,
            clock,
            audio,
            pipeline,
            frames: 0,
            offset_ns: 0,
            input_frames: 0,
            buffer: Vec::new(),
            settled_deviation: 0.0,
        }
    }

    /// Runs `count` frames, one per flip.
    pub fn run_frames(&mut self, count: u64) {
        for _ in 0..count {
            self.run_frame();
        }
    }

    /// Runs the frames of `seconds` of display time.
    pub fn run_seconds(&mut self, seconds: u64) {
        let frames = u128::from(seconds) * u128::from(self.setup.display.num)
            / u128::from(self.setup.display.den);
        self.run_frames(frames as u64);
    }

    /// No flip for `ns`: a blocking modeset, a pause.
    pub fn stall(&mut self, ns: u64) {
        self.offset_ns += ns;
    }

    /// The time of the next flip.
    pub fn next_flip_ns(&self) -> u64 {
        let display = self.setup.display;
        let ns = u128::from(self.frames + 1) * NS_PER_S * u128::from(display.den)
            / u128::from(display.num);
        self.offset_ns + ns as u64
    }

    fn run_frame(&mut self) {
        self.clock.advance_to(self.next_flip_ns()).unwrap();
        self.frames += 1;
        let count = self.audio_frames_up_to(self.frames) - self.audio_frames_up_to(self.frames - 1);
        self.buffer.clear();
        for _ in 0..count {
            let [left, right] = self.sample(self.input_frames);
            self.buffer.push(left);
            self.buffer.push(right);
            self.input_frames += 1;
        }
        self.pipeline.push_samples(&self.buffer);
        self.pipeline.end_frame();
        if self.clock.now_ns() >= SETTLE_NS {
            let deviation =
                (self.pipeline.stats().ratio / self.pipeline.nominal_ratio() - 1.0).abs();
            self.settled_deviation = self.settled_deviation.max(deviation);
        }
    }

    /// Core audio frames in the first `frames` core frames.
    fn audio_frames_up_to(&self, frames: u64) -> u64 {
        let rate = self.setup.core_rate;
        let fps = self.setup.core_fps;
        let num = u128::from(frames) * u128::from(rate.num) * u128::from(fps.den);
        (num / (u128::from(rate.den) * u128::from(fps.num))) as u64
    }

    fn sample(&self, index: u64) -> [i16; 2] {
        match self.setup.signal {
            Signal::Ramp => {
                let left = (index.wrapping_mul(97) % 65_536) as i64 - 32_768;
                let right = (index.wrapping_mul(61) % 40_000) as i64 - 20_000;
                [left as i16, right as i16]
            }
            Signal::Tone { freq_hz, amplitude } => {
                let phase = TAU * freq_hz * index as f64 / self.setup.core_rate.as_f64();
                let value = (amplitude * 32_768.0 * phase.sin()).round() as i16;
                [value, value]
            }
        }
    }

    pub fn stats(&self) -> AudioStats {
        *self.pipeline.stats()
    }

    pub fn span_frames(&self) -> usize {
        self.pipeline.control().span_frames()
    }

    /// The queue level just before the first write of each frame, with the
    /// time of that write, from the fake device's records.
    pub fn levels(&self) -> Vec<(u64, usize)> {
        let mut first = BTreeMap::new();
        for write in self.audio.writes() {
            let before = write.queued_after - write.taken_frames;
            first.entry(write.time_ns).or_insert(before);
        }
        first.into_iter().collect()
    }
}

/// What a run measured.
#[derive(Debug, Clone, Copy)]
pub struct Measured {
    /// Lowest and highest level before a write, as fractions of the span.
    pub low: f64,
    pub high: f64,
    /// The largest deviation of the whole run, the first frame included.
    pub max_deviation: f64,
    /// The largest deviation after [`SETTLE_NS`].
    pub settled_deviation: f64,
}

/// The deviation bound `d`, with room for the rounding of one `f64`
/// multiplication.
pub fn deviation_bound() -> f64 {
    f64::from(MAX_RATE_DEVIATION_PPM) / 1_000_000.0 * (1.0 + 1e-12)
}

/// Asserts a steady run: no underrun, no overrun, no lost input, the
/// deviation inside `d`, and every level after [`SETTLE_NS`] inside `[low,
/// high]` times the span.
pub fn assert_steady(session: &Session, low: f64, high: f64) -> Measured {
    let stats = session.stats();
    assert!(
        session.audio.underruns().is_empty(),
        "{:?}",
        session.audio.underruns().first()
    );
    assert!(
        session.audio.overruns().is_empty(),
        "{:?}",
        session.audio.overruns().first()
    );
    assert_eq!(stats.overrun_frames, 0);
    assert_eq!(stats.input_dropped_frames, 0);
    assert_eq!(stats.resampler_errors, 0);
    assert_eq!(stats.restarts, 1);
    assert!(
        stats.max_deviation <= deviation_bound(),
        "{}",
        stats.max_deviation
    );
    let measured = measure(session);
    assert!(
        measured.low >= low && measured.high <= high,
        "levels {:.3} to {:.3} of the span, bounds {low} to {high}",
        measured.low,
        measured.high
    );
    measured
}

/// The level range after [`SETTLE_NS`] and the largest deviations.
pub fn measure(session: &Session) -> Measured {
    let span = session.span_frames() as f64;
    let levels: Vec<f64> = session
        .levels()
        .into_iter()
        .filter(|(time, _)| *time >= SETTLE_NS)
        .map(|(_, level)| level as f64 / span)
        .collect();
    assert!(!levels.is_empty());
    Measured {
        low: levels.iter().copied().fold(f64::MAX, f64::min),
        high: levels.iter().copied().fold(f64::MIN, f64::max),
        max_deviation: session.stats().max_deviation,
        settled_deviation: session.settled_deviation,
    }
}

/// Positions of the upward zero crossings of the left channel, in output
/// frames, interpolated between the two samples.
pub fn upward_crossings(samples: &[i16]) -> Vec<f64> {
    let left: Vec<f64> = samples.iter().step_by(2).map(|s| f64::from(*s)).collect();
    left.windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] < 0.0 && pair[1] >= 0.0)
        .map(|(index, pair)| index as f64 + pair[0] / (pair[0] - pair[1]))
        .collect()
}

/// The frequency of the crossings between output frames `from` and `to`,
/// at a device that plays `rate` frames per second.
pub fn frequency(crossings: &[f64], from: f64, to: f64, rate: f64) -> f64 {
    let inside: Vec<f64> = crossings
        .iter()
        .copied()
        .filter(|position| *position >= from && *position < to)
        .collect();
    let first = inside[0];
    let last = inside[inside.len() - 1];
    (inside.len() - 1) as f64 / (last - first) * rate
}

/// The largest step between two consecutive samples of one channel.
pub fn largest_step(samples: &[i16]) -> i32 {
    let mut largest = 0;
    for channel in 0..2 {
        let values: Vec<i32> = samples
            .iter()
            .skip(channel)
            .step_by(2)
            .map(|s| i32::from(*s))
            .collect();
        for pair in values.windows(2) {
            largest = largest.max((pair[1] - pair[0]).abs());
        }
    }
    largest
}

/// The largest step a sine of `amplitude` (of full scale) at `heard_hz`
/// may take at `device_hz`, with the ratio up to `d` off, plus the slope
/// of a fade over [`FADE_MS`], plus two steps of rounding.
pub fn step_bound(amplitude: f64, heard_hz: f64, device_hz: u32) -> i32 {
    let peak = amplitude * 32_768.0;
    let sine = peak * TAU * heard_hz / f64::from(device_hz) * (1.0 + deviation_bound());
    let fade_frames = f64::from(device_hz) * f64::from(FADE_MS) / 1_000.0;
    (sine + peak / fade_frames).ceil() as i32 + 2
}
