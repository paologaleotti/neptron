//! Integration tests of neptron-video: the engine on the fake display and
//! the simulated clock, timing check 5, and the scripted safety scenarios
//! of the guard.

#![forbid(unsafe_code)]

mod check5;
mod engine;
mod faults;
mod guard_confirm;
mod guard_rig;
mod harness;
mod inputs;
mod scanlines;
mod stall;
