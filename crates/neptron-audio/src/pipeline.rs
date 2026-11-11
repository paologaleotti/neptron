//! The audio pipeline of a running core: input buffer, resampler, rate
//! control, fades and the sink.

mod output;
mod resample;
#[cfg(test)]
mod tests;

use neptron_backend::AudioSink;

use crate::rates::check_device_rate;
use crate::{AudioError, CoreTiming, RateControl};

/// The most core audio one frame may bring, in milliseconds at the core's
/// sample rate. Samples beyond it in one frame are dropped and counted.
pub const MAX_INPUT_PER_FRAME_MS: u32 = 100;

/// The length of a fade in or out, in milliseconds of output frames.
pub const FADE_MS: u32 = 5;

/// The interpolation of the resampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Resampling {
    /// A 128-tap windowed sinc filter with linear interpolation between 128
    /// oversampled points: band-limited, 2 ms of delay at 48 kHz.
    #[default]
    Sinc,
    /// Cubic polynomial interpolation with no anti-alias filter: about four
    /// times cheaper, and for a slow CPU or a long test in the dev profile.
    Polynomial,
}

/// Counters of an [`AudioPipeline`] since it was made.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AudioStats {
    /// Calls of [`AudioPipeline::end_frame`].
    pub frames: u64,
    /// Core frames (stereo pairs) taken by [`AudioPipeline::push_samples`].
    pub input_frames: u64,
    /// Core frames dropped because one frame brought more than
    /// [`MAX_INPUT_PER_FRAME_MS`] of audio.
    pub input_dropped_frames: u64,
    /// Frames the resampler gave.
    pub output_frames: u64,
    /// Frames of silence written to prime or to top up the queue.
    pub silence_frames: u64,
    /// Frames the sink did not take because its queue was full.
    pub overrun_frames: u64,
    /// Times the queue was found empty and primed again, the first frame
    /// included.
    pub restarts: u64,
    /// Calls in which the resampler refused its input or its ratio. The
    /// frame's audio is lost; it does not happen with the buffers that
    /// [`AudioPipeline::new`] makes.
    pub resampler_errors: u64,
    /// The queue level before the last write, in output frames.
    pub level_frames: usize,
    /// The resample ratio of the last frame, in output frames per input
    /// frame.
    pub ratio: f64,
    /// The largest `|ratio / nominal - 1|` so far.
    pub max_deviation: f64,
}

/// Resamples a core's audio to a sink and keeps the sink's queue at its
/// target with dynamic rate control.
///
/// The session calls [`push_samples`](Self::push_samples) from the core's
/// audio callbacks and [`end_frame`](Self::end_frame) once per core frame.
/// Neither call allocates, does I/O or takes a lock of its own: every
/// buffer is made in [`new`](Self::new).
pub struct AudioPipeline<S> {
    sink: S,
    core: CoreTiming,
    device_rate_hz: u32,
    run_rate_mhz: u64,
    nominal: f64,
    control: RateControl,
    resampler: resample::Stereo,
    /// Interleaved core audio of the current frame, as `f32`.
    input: Vec<f32>,
    input_frames: usize,
    /// Interleaved resampler output.
    output: Vec<f32>,
    /// Interleaved output for the sink.
    samples: Vec<i16>,
    /// Zeros, for priming and topping up: the target level of frames.
    silence: Vec<i16>,
    fade_frames: usize,
    /// Output frames still to fade in.
    fade_in_left: usize,
    muted: bool,
    /// The last frame written, for the fade-out of a mute.
    last: [i16; 2],
    stats: AudioStats,
}

impl<S: AudioSink> AudioPipeline<S> {
    /// A pipeline from a core with `core` timing to `sink`, run at the
    /// core's own frame rate until [`set_run_rate_mhz`](Self::set_run_rate_mhz).
    /// It makes the resampler and every buffer.
    ///
    /// # Errors
    ///
    /// [`AudioError::CoreSampleRate`], [`AudioError::CoreFrameRate`] or
    /// [`AudioError::DeviceRate`] for a rate outside the limits, and
    /// [`AudioError::Resampler`] when rubato refuses the resampler.
    pub fn new(sink: S, core: CoreTiming, resampling: Resampling) -> Result<Self, AudioError> {
        core.check()?;
        let device_rate_hz = sink.rate_hz();
        check_device_rate(device_rate_hz)?;
        let run_rate_mhz = core.frame_rate_mhz;
        let nominal = core.nominal_ratio(device_rate_hz, run_rate_mhz);
        let max_input_frames = frames_in_ms_mhz(core.sample_rate_mhz, MAX_INPUT_PER_FRAME_MS);
        let resampler = resample::Stereo::new(resampling, nominal, max_input_frames)?;
        let max_output_frames = resampler.max_output_frames();
        let control = RateControl::new(device_rate_hz);
        let fade_frames = frames_in_ms_mhz(u64::from(device_rate_hz) * 1_000, FADE_MS).max(1);
        Ok(Self {
            sink,
            core,
            device_rate_hz,
            run_rate_mhz,
            nominal,
            control,
            resampler,
            input: vec![0.0; max_input_frames * 2],
            input_frames: 0,
            output: vec![0.0; max_output_frames * 2],
            samples: vec![0; max_output_frames.max(fade_frames) * 2],
            silence: vec![0; control.target_frames() * 2],
            fade_frames,
            fade_in_left: 0,
            muted: false,
            last: [0; 2],
            stats: AudioStats {
                ratio: nominal,
                ..AudioStats::default()
            },
        })
    }

    /// Sets the run rate: core frames the session runs per second of real
    /// time, in millihertz. The nominal ratio follows from the next frame.
    ///
    /// # Errors
    ///
    /// [`AudioError::RunRatio`] when the rate is less than half or more than
    /// twice the core's frame rate. The run rate does not change.
    pub fn set_run_rate_mhz(&mut self, run_rate_mhz: u64) -> Result<(), AudioError> {
        self.core.check_run_rate(run_rate_mhz)?;
        self.run_rate_mhz = run_rate_mhz;
        self.nominal = self.core.nominal_ratio(self.device_rate_hz, run_rate_mhz);
        Ok(())
    }

    /// Takes interleaved stereo samples from the core, left first. It may be
    /// called any number of times in a frame. An odd last sample is not
    /// taken.
    pub fn push_samples(&mut self, stereo: &[i16]) {
        let offered = stereo.len() / 2;
        let room = self.input.len() / 2 - self.input_frames;
        let taken = offered.min(room);
        let start = self.input_frames * 2;
        let target = &mut self.input[start..start + taken * 2];
        for (to, from) in target.iter_mut().zip(stereo) {
            *to = output::to_float(*from);
        }
        self.input_frames += taken;
        self.stats.input_frames += taken as u64;
        self.stats.input_dropped_frames += (offered - taken) as u64;
    }

    /// Ends a core frame: reads the queue level, sets the ratio, resamples
    /// the frame's audio and writes it to the sink.
    pub fn end_frame(&mut self) {
        let level = self.sink.queued_frames();
        self.stats.frames += 1;
        self.stats.level_frames = level;
        let factor = self.control.factor(level);
        let ratio = self.nominal * factor;
        self.stats.ratio = ratio;
        self.stats.max_deviation = self.stats.max_deviation.max((factor - 1.0).abs());
        if self.resampler.set_ratio(ratio).is_err() {
            self.stats.resampler_errors += 1;
        }

        let mut level = level;
        if level == 0 && !self.muted {
            level += self.write_silence(self.control.target_frames());
            self.stats.restarts += 1;
            self.fade_in_left = self.fade_frames;
        }

        let frames = self.resample();
        self.stats.output_frames += frames as u64;
        if self.muted {
            self.samples[..frames * 2].fill(0);
        } else {
            self.convert(frames);
        }
        level += self.write_samples(frames);
        if self.muted {
            self.top_up(level);
        }
    }

    /// Mutes the output now: writes a fade from the last sample to zero,
    /// then silence up to the target level. While muted, each frame writes
    /// silence of the length of its audio, and the rate control keeps
    /// running.
    pub fn mute(&mut self) {
        if self.muted {
            return;
        }
        self.muted = true;
        let frames = output::fade_out(self.last, &mut self.samples, self.fade_frames);
        let written = self.write_samples(frames);
        self.last = [0; 2];
        let level = self.sink.queued_frames().max(written);
        self.top_up(level);
    }

    /// Ends a mute: the next frame fades in.
    pub fn unmute(&mut self) {
        if !self.muted {
            return;
        }
        self.muted = false;
        self.fade_in_left = self.fade_frames;
    }

    /// True between [`mute`](Self::mute) and [`unmute`](Self::unmute).
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// The counters since the pipeline was made.
    pub fn stats(&self) -> &AudioStats {
        &self.stats
    }

    /// The nominal ratio: output frames per input frame at the run rate,
    /// before rate control.
    pub fn nominal_ratio(&self) -> f64 {
        self.nominal
    }

    /// The run rate in use, in millihertz.
    pub fn run_rate_mhz(&self) -> u64 {
        self.run_rate_mhz
    }

    /// The rate control law in use.
    pub fn control(&self) -> &RateControl {
        &self.control
    }

    /// The delay of the resampler, in output frames.
    pub fn resampler_delay_frames(&self) -> usize {
        self.resampler.delay_frames()
    }

    /// The sink.
    pub fn sink(&self) -> &S {
        &self.sink
    }

    /// Gives the sink back.
    pub fn into_sink(self) -> S {
        self.sink
    }
}

impl<S: AudioSink> AudioPipeline<S> {
    /// Resamples the frame's input into `output` and empties the input.
    fn resample(&mut self) -> usize {
        let input = &self.input[..self.input_frames * 2];
        let frames = match self.resampler.process(input, &mut self.output) {
            Ok(frames) => frames,
            Err(_) => {
                self.stats.resampler_errors += 1;
                0
            }
        };
        self.input_frames = 0;
        frames
    }

    /// Converts `frames` output frames to `samples`, with the fade-in.
    fn convert(&mut self, frames: usize) {
        if frames == 0 {
            return;
        }
        self.fade_in_left = output::convert(
            &self.output[..frames * 2],
            &mut self.samples[..frames * 2],
            self.fade_frames,
            self.fade_in_left,
        );
        let end = frames * 2;
        self.last = [self.samples[end - 2], self.samples[end - 1]];
    }

    /// Writes the first `frames` frames of `samples`. Returns the frames
    /// the sink took.
    fn write_samples(&mut self, frames: usize) -> usize {
        write(&mut self.sink, &mut self.stats, &self.samples[..frames * 2])
    }

    /// Writes `frames` frames of silence, at most the target level.
    fn write_silence(&mut self, frames: usize) -> usize {
        let frames = frames.min(self.silence.len() / 2);
        let taken = write(&mut self.sink, &mut self.stats, &self.silence[..frames * 2]);
        self.stats.silence_frames += taken as u64;
        taken
    }

    /// Writes silence up to the target level from `level`.
    fn top_up(&mut self, level: usize) {
        let missing = self.control.target_frames().saturating_sub(level);
        if missing > 0 {
            self.write_silence(missing);
        }
    }
}

/// Writes `samples` to `sink` and counts what it refused.
fn write<S: AudioSink>(sink: &mut S, stats: &mut AudioStats, samples: &[i16]) -> usize {
    if samples.is_empty() {
        return 0;
    }
    let offered = samples.len() / 2;
    let taken = sink.write(samples).min(offered);
    stats.overrun_frames += (offered - taken) as u64;
    taken
}

/// Frames in `ms` milliseconds at `rate_mhz`, rounded up.
fn frames_in_ms_mhz(rate_mhz: u64, ms: u32) -> usize {
    let frames = (u128::from(rate_mhz) * u128::from(ms)).div_ceil(1_000_000);
    usize::try_from(frames).unwrap_or(usize::MAX)
}
