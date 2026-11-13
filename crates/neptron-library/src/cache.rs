//! The index cache: one versioned binary file that holds an [`Index`].
//!
//! The cache is disposable. A file that is missing, damaged, truncated,
//! foreign or of another version is not used: [`load`] deletes it, and the
//! caller scans again and writes a new one. Nothing in this module panics on
//! the content of the file.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::catalog::Catalog;
use crate::index::Index;

mod crc32;
mod format;

pub use format::{CACHE_VERSION, HEADER_BYTES, MAGIC};

/// The largest cache file that [`read`] accepts and [`write`] writes, in
/// bytes.
///
/// A game takes about 140 bytes in the file with typical No-Intro names
/// (a 50-byte name, a 70-byte path and the size), so 16 MiB holds about
/// 120,000 games, more than a complete set of every cartridge system. Such
/// a file decodes to about 30 MiB. The worst case is a crafted file of
/// 16 MiB with the smallest valid games (20 bytes each): about 840,000
/// entries of 56 bytes, two small heap strings each and the sort buffer,
/// less than 160 MiB for a short time at start. That fits a device with
/// 512 MB of memory. A larger library gets no cache and is scanned at
/// each start.
pub const MAX_CACHE_BYTES: u64 = 16 * 1024 * 1024;

/// Why a cache file was not used or not written.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// The file could not be read or written.
    #[error("cache file {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error from the file system.
        source: io::Error,
    },
    /// The file is larger than [`MAX_CACHE_BYTES`].
    #[error("the cache file has more than {limit} bytes")]
    TooLarge {
        /// The limit in bytes.
        limit: u64,
    },
    /// The file is shorter than the header.
    #[error("the cache file has {len} bytes, less than the {header}-byte header")]
    Truncated {
        /// The length of the file in bytes.
        len: usize,
        /// The length of the header in bytes.
        header: usize,
    },
    /// The file does not start with [`MAGIC`].
    #[error("the file is not an index cache")]
    BadMagic,
    /// The file has another format version.
    #[error("cache version {found}, expected {expected}")]
    Version {
        /// The version in the file.
        found: u32,
        /// The version that this crate reads.
        expected: u32,
    },
    /// The payload length in the header does not equal the length of the file.
    #[error("the header gives a {declared}-byte payload, the file holds {actual} bytes")]
    Length {
        /// The payload length in the header.
        declared: u64,
        /// The payload length in the file.
        actual: u64,
    },
    /// The checksum of the payload does not match.
    #[error("cache checksum {stored:#010x}, computed {computed:#010x}")]
    Checksum {
        /// The checksum in the header.
        stored: u32,
        /// The checksum of the payload.
        computed: u32,
    },
    /// The payload does not follow the format.
    #[error("bad {what} at byte {offset} of the cache")]
    Malformed {
        /// Offset in the file.
        offset: usize,
        /// The field that is bad.
        what: &'static str,
    },
    /// The cache names a system that the catalog does not have.
    #[error("the cache names the unknown system `{system}`")]
    UnknownSystem {
        /// The system id.
        system: String,
    },
}

/// The result of [`load`].
#[derive(Debug)]
pub enum CacheLoad {
    /// The cache was valid.
    Loaded(Index),
    /// There is no cache file.
    Missing,
    /// The cache file was not valid and was deleted. The error says why.
    Discarded(CacheError),
}

/// The bytes of the cache file for `index`.
pub fn encode(index: &Index) -> Vec<u8> {
    format::encode(index)
}

/// The index in the bytes of a cache file. Every system in the cache must
/// be in `catalog`, and every game path must stay inside the folder of its
/// system. Any byte string gives an index or an error, never a panic, in
/// time linear in its length.
pub fn decode(bytes: &[u8], catalog: &Catalog) -> Result<Index, CacheError> {
    format::decode(bytes, catalog)
}

/// Reads and checks the cache file at `path` (see [`decode`]).
pub fn read(path: &Path, catalog: &Catalog) -> Result<Index, CacheError> {
    let io_error = |source| CacheError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(io_error)?
        .take(MAX_CACHE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > MAX_CACHE_BYTES {
        return Err(CacheError::TooLarge {
            limit: MAX_CACHE_BYTES,
        });
    }
    decode(&bytes, catalog)
}

/// Reads the cache file at `path`. A file that is not valid is deleted, so
/// the next start does not read it again.
pub fn load(path: &Path, catalog: &Catalog) -> CacheLoad {
    match read(path, catalog) {
        Ok(index) => CacheLoad::Loaded(index),
        Err(CacheError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            CacheLoad::Missing
        }
        Err(e) => {
            // The file is a cache. If it cannot be deleted, the next write
            // replaces it, and until then each read rejects it again.
            let _ = fs::remove_file(path);
            CacheLoad::Discarded(e)
        }
    }
}

/// Writes `index` to the cache file at `path`: a temporary file next to it,
/// fsync, rename, then fsync of the folder. A crash leaves the old file or
/// the new one. The folder must exist. An index larger than
/// [`MAX_CACHE_BYTES`] is not written.
pub fn write(path: &Path, index: &Index) -> Result<(), CacheError> {
    let bytes = encode(index);
    if bytes.len() as u64 > MAX_CACHE_BYTES {
        return Err(CacheError::TooLarge {
            limit: MAX_CACHE_BYTES,
        });
    }
    let tmp = temp_path(path);
    let io_error = |p: &Path| {
        let p = p.to_path_buf();
        move |source| CacheError::Io { path: p, source }
    };
    let result = write_synced(&tmp, &bytes)
        .map_err(io_error(&tmp))
        .and_then(|()| fs::rename(&tmp, path).map_err(io_error(path)));
    if result.is_err() {
        // The temporary file holds no user data; a file left behind is
        // replaced by the next write.
        let _ = fs::remove_file(&tmp);
    }
    result?;
    sync_parent(path).map_err(io_error(path))
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map_or_else(OsString::new, OsString::from);
    name.push(".tmp");
    path.with_file_name(name)
}

/// Removes what is at `path` first and then makes a new file there. A link
/// at `path` is removed, not followed, and `create_new` refuses any entry
/// that comes back, so the write never reaches a file outside the cache.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The rename is durable only when the folder entry is on disk too.
fn sync_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => File::open(dir)?.sync_all(),
        _ => File::open(".")?.sync_all(),
    }
}

#[cfg(test)]
mod tests;
