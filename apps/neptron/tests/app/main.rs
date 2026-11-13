//! Tier 2 of the frontend: the real frontend on the fakes of
//! `neptron-testkit` (display, audio, input, simulated clock), the built
//! test core through the real host, and the UI crawler.
//!
//! The runs are long, so each test is ignored in tier 1 and runs in tier 2:
//! `cargo xtask verify --tier 2`, or alone with
//! `cargo xtask test -p neptron --run-ignored only`.

#![forbid(unsafe_code)]

mod audio;
mod boot;
mod continued;
mod control;
mod crawler;
mod end_to_end;
mod faults;
mod output;
mod panel;
mod recovery;
mod rig;
