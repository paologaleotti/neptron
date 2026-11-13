//! Integration tests of neptron-session: scripted sessions of the test core
//! through the real host, on the fake display, the fake audio, scripted
//! input and the simulated clock, and timing check 6.

#![forbid(unsafe_code)]

mod faults;
mod harness;
mod launch;
mod session;
mod timing;
mod volume;
