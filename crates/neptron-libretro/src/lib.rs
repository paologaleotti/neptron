//! Safe libretro core host: load a core, environment callbacks, save RAM,
//! core options. `docs/spec/libretro-host.md` is the specification.

#![forbid(unsafe_code)]

mod crc32;
mod zip;

pub use zip::{MAX_CENTRAL_DIRECTORY_BYTES, ZipEntry, ZipError, read_zip_entry};
