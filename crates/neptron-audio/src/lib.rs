//! Audio out: rubato resampler, dynamic rate control and the SDL3 sink.
//!
//! `docs/spec/pacing-audio.md` is the specification.

#![forbid(unsafe_code)]

mod control;
mod error;
mod gain;
mod meter;
mod pipeline;
mod rates;
#[cfg(feature = "sdl3")]
mod sdl;

pub use control::{CONTROL_SPAN_MS, MAX_RATE_DEVIATION_PPM, RateControl};
pub use error::AudioError;
pub use gain::{GAIN_UNITY_Q24, VOLUME_MAX_PERCENT, VOLUME_RAMP_MS, gain_q24, scale};
pub use meter::{FLIP_METER_INTERVALS, FLIP_METER_MIN_INTERVALS, FlipRateMeter};
pub use pipeline::{AudioPipeline, AudioStats, FADE_MS, MAX_INPUT_PER_FRAME_MS, Resampling};
pub use rates::{
    CoreTiming, MAX_CORE_FRAME_RATE_MHZ, MAX_CORE_SAMPLE_RATE_MHZ, MAX_DEVICE_RATE_HZ,
    MIN_CORE_FRAME_RATE_MHZ, MIN_CORE_SAMPLE_RATE_MHZ, MIN_DEVICE_RATE_HZ, check_device_rate,
};
#[cfg(feature = "sdl3")]
pub use sdl::{DEVICE_BUFFER_FRAMES, SINK_CAPACITY_MS, Sdl3Sink, SinkError};
