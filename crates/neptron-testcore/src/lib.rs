//! A libretro core that follows a script, for tests. It reads its script from
//! the file that `NEPTRON_TESTCORE_SCRIPT` names. The README of the crate
//! describes the script format.
//!
//! The library also exports the frame and audio generators, so that a test
//! can work out what the core sends.

#![forbid(unsafe_code)]

mod audio;
mod frame;
mod script;

pub use audio::{frames_after, max_frames_per_run, sample};
pub use frame::{expected_hash, hash, pixel, render};

pub use script::{
    Action, AudioMode, AvInfo, Event, Geometry, HEADER_KEYWORD, LoadError, MAX_ASPECT_MILLI,
    MAX_DIMENSION_PX, MAX_FPS_MHZ, MAX_SAMPLE_RATE_HZ, MAX_SAVE_RAM_BYTES, PixelFormat, SCRIPT_ENV,
    SCRIPT_VERSION, Script, ScriptError, Size, Timing,
};
