//! Integration tests of neptron-session: scripted sessions of the test core
//! through the real host, on the fake display, the fake audio, scripted
//! input and the simulated clock.

#![forbid(unsafe_code)]

mod faults;
mod harness;
mod launch;
mod session;
