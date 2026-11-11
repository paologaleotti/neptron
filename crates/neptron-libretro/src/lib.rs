//! Safe libretro core host: load a core, environment callbacks, save RAM,
//! core options. `docs/spec/libretro-host.md` is the specification.

#![forbid(unsafe_code)]

mod crc32;
mod geometry;
mod save;
mod zip;

pub use geometry::{
    AvInfo, GameGeometry, GeometryError, MAX_ASPECT_MILLI, MAX_DIMENSION_PX, MAX_FPS_MHZ,
    MAX_SAMPLE_RATE_MHZ, MIN_FPS_MHZ, MIN_SAMPLE_RATE_MHZ, Timing,
};
pub use save::{
    MAX_SAVE_RAM_BYTES, SaveFileError, read_save_file, save_file_path, write_save_file,
};
pub use zip::{MAX_CENTRAL_DIRECTORY_BYTES, ZipEntry, ZipError, read_zip_entry};
