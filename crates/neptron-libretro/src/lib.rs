//! Safe libretro core host: load a core, environment callbacks, save RAM,
//! core options. `docs/spec/libretro-host.md` is the specification.
//!
//! [`Host`] loads one core per process, loads content, runs one frame at a
//! time and returns what the frame produced. It owns no display, audio
//! device or clock: the session decides what to do with the output.

// Only `ffi` may contain `unsafe`. Every other module denies it.
#![deny(unsafe_code)]

mod audio;
mod content;
mod crc32;
mod error;
#[allow(unsafe_code)]
mod ffi;
mod geometry;
mod host;
mod log;
mod output;
mod pad;
mod save;
mod state;
mod video;
mod zip;

pub use content::{ContentRequest, MAX_CONTENT_BYTES};
pub use error::{LoadError, OpenError, RunError};
pub use ffi::MAX_CORE_STRING_BYTES;
pub use geometry::{
    AvInfo, GameGeometry, GeometryError, MAX_ASPECT_MILLI, MAX_DIMENSION_PX, MAX_FPS_MHZ,
    MAX_SAMPLE_RATE_MHZ, MIN_FPS_MHZ, MIN_SAMPLE_RATE_MHZ, Timing,
};
pub use host::{Host, HostConfig, SystemInfo};
pub use log::{LOG_CAPACITY, LOG_TEXT_MAX_BYTES, LogLevel, LogRecord, LogSource};
pub use output::{Dropped, Event, FrameFault, FrameOutput, Refusal, Video};
pub use pad::{InputError, MAX_PORTS, Pad};
pub use save::{
    MAX_SAVE_RAM_BYTES, SaveFileError, read_save_file, save_file_path, write_save_file,
};
pub use state::{EVENT_CAPACITY, MAX_LOGGED_COMMANDS, MESSAGE_MAX_BYTES};
pub use video::MAX_PITCH_BYTES;
pub use zip::{MAX_CENTRAL_DIRECTORY_BYTES, ZipEntry, ZipError, read_zip_entry};
