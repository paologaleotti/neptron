//! Test fakes: a display that records every mode and scene, a fake audio sink, scripted input, a simulated clock and golden helpers.
//!
//! Everything here is deterministic. Time comes only from [`SimClock`].
//! No fake starts a thread, sleeps, reads the wall clock or uses a random
//! source without a fixed seed.

#![forbid(unsafe_code)]

mod clock;
pub mod golden;
mod image;

pub use clock::{ClockError, SimClock};
pub use golden::Golden;
pub use image::{ImageError, RgbImage};
