//! A libretro core that follows a script, for tests. It reads its script from
//! the file that `NEPTRON_TESTCORE_SCRIPT` names. The README of the crate
//! describes the script format.
//!
//! The library also exports the frame and audio generators, so that a test
//! can work out what the core sends.
//!
//! The crate builds as a dynamic library (`cdylib`) that exports the 25
//! libretro entry points, and as a Rust library for tests.

// Only `ffi` may contain `unsafe`. Every other module forbids it.
#![deny(unsafe_code)]

mod audio;
#[allow(unsafe_code)]
mod ffi;
mod frame;
mod gl_card;
mod model;
mod options;
mod script;

pub use audio::{frames_after, max_frames_per_run, sample};
pub use frame::{expected_hash, hash, pixel, render, render_shape, shape_pixel};
pub use gl_card::{
    CARD_RECTS, ClearRect, GL_BLEND, GL_COLOR_BUFFER_BIT, GL_FRAMEBUFFER, GL_FUNCTIONS,
    GL_NO_ERROR, GL_SCISSOR_TEST, gl_card, gl_card_pixel,
};
pub use model::{Core, PADDING_BYTE, Request, SaveRam, Step, UNKNOWN_ENVIRONMENT_COMMAND, Video};
pub use options::{
    CORE_OPTIONS, CoreOption, LOAD_OPTION, SCALE_OPTION, VALUE_NOT_LISTED, VALUE_NOT_SET,
    declaration, option, option_index, scale_of, value_code,
};

pub use script::{
    Action, AudioMode, AvInfo, DISCS_VERSION, Event, Geometry, HEADER_KEYWORD, HW_RENDER_VERSION,
    HwContext, HwRender, LOAD_LINES_VERSION, LoadError, LoadOutcome, LogLevel, MAX_ASPECT_MILLI,
    MAX_DIMENSION_PX, MAX_DISCS, MAX_FPS_MHZ, MAX_GL_VERSION, MAX_SAMPLE_RATE_HZ,
    MAX_SAVE_RAM_BYTES, MIN_FPS_MHZ, OptionTiming, PATTERN_VERSION, Pattern, PixelFormat,
    SCRIPT_ENV, SCRIPT_VERSION, Script, ScriptError, Size, TEXT_ACTIONS_VERSION, Timing,
};
