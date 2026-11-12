//! The frontend: the wiring of the crates, and no logic of its own.
//!
//! A run is one process. It shows the game list, or it runs one game,
//! because a process holds one core.

#![forbid(unsafe_code)]

pub mod atomic;
pub mod options;
pub mod paths;
pub mod presets;
pub mod record;
pub mod relaunch;

pub use paths::Paths;
pub use relaunch::{NextRun, RELAUNCH_EXIT_CODE};
