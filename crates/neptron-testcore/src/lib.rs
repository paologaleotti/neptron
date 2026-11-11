//! A libretro core that follows a script, for tests. It reads its script from
//! the file that `NEPTRON_TESTCORE_SCRIPT` names. The README of the crate
//! describes the script format.

#![forbid(unsafe_code)]

mod script;

pub use script::{
    Action, AudioMode, AvInfo, Event, Geometry, HEADER_KEYWORD, LoadError, MAX_ASPECT_MILLI,
    MAX_DIMENSION_PX, MAX_FPS_MHZ, MAX_SAMPLE_RATE_HZ, MAX_SAVE_RAM_BYTES, PixelFormat, SCRIPT_ENV,
    SCRIPT_VERSION, Script, ScriptError, Size, Timing,
};
