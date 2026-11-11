//! Test fakes: a display that records every mode and scene, a fake audio sink, scripted input, a simulated clock and golden helpers.
//!
//! Everything here is deterministic. Time comes only from [`SimClock`].
//! No fake starts a thread, sleeps, reads the wall clock or uses a random
//! source without a fixed seed.

#![forbid(unsafe_code)]

mod audio;
mod clock;
mod display;
pub mod golden;
mod hash;
mod image;
pub mod preset;
mod rng;

pub use audio::{AudioOptions, AudioWrite, FakeAudio, Underrun};
pub use clock::{ClockError, SimClock};
pub use display::{
    DEFAULT_FLIP_TIMEOUT_NS, DisplayOptions, FakeDisplay, FlipFault, FrameRecord, Jitter,
    LateFlips, ModeFault, ModeRecord, ModeSwitch, Render, SceneSummary,
};
pub use golden::Golden;
pub use hash::frame_hash;
pub use image::{ImageError, RgbImage};
