//! Audio out: rubato resampler, dynamic rate control and the SDL3 sink.
//!
//! `docs/spec/pacing-audio.md` is the specification.

#![forbid(unsafe_code)]

mod control;
mod error;
mod rates;

pub use control::{CONTROL_SPAN_MS, MAX_RATE_DEVIATION_PPM, RateControl};
pub use error::AudioError;
pub use rates::{
    CoreTiming, MAX_CORE_FRAME_RATE_MHZ, MAX_CORE_SAMPLE_RATE_MHZ, MAX_DEVICE_RATE_HZ,
    MIN_CORE_FRAME_RATE_MHZ, MIN_CORE_SAMPLE_RATE_MHZ, MIN_DEVICE_RATE_HZ, check_device_rate,
};
