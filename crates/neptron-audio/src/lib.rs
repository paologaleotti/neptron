//! Audio out: rubato resampler, dynamic rate control and the SDL3 sink.
//!
//! `docs/spec/pacing-audio.md` is the specification.

#![forbid(unsafe_code)]

mod control;

pub use control::{CONTROL_SPAN_MS, MAX_RATE_DEVIATION_PPM, RateControl};
