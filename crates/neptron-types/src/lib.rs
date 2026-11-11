//! Shared plain data: modeline, CRT range, monitor profile, GPU capabilities,
//! source geometry, frame, scene and backend events. Plain data with serde,
//! no I/O.
//!
//! Every contract value is an integer: frequencies in millihertz (or hertz
//! where the name says `_hz`), times in nanoseconds.

#![forbid(unsafe_code)]

mod modeline;

pub use modeline::Modeline;
