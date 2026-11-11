//! Integration tests of neptron-video: the engine on the fake display and
//! the simulated clock, and timing check 5.

#![forbid(unsafe_code)]

mod check5;
mod engine;
mod faults;
mod harness;
mod inputs;
mod scanlines;
mod stall;
