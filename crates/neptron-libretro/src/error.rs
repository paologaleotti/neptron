//! Errors of the host calls.

use std::io;
use std::path::PathBuf;

use crate::geometry::GeometryError;
use crate::zip::ZipError;

/// Why [`Host::open`](crate::Host::open) failed.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// A core was opened in this process before. One process hosts one
    /// core; the application re-executes itself for the next game.
    #[error("a core was opened in this process before; one process hosts one core")]
    ProcessAlreadyUsed,
    /// A folder path of the configuration is not UTF-8 or holds a NUL.
    #[error("{path} is not a UTF-8 path without NUL bytes")]
    InvalidPath {
        /// The path.
        path: PathBuf,
    },
    /// The dynamic loader refused the file: it is not a library for this
    /// system.
    #[error("cannot load {path} as a library: {reason}")]
    Library {
        /// The core path.
        path: PathBuf,
        /// What the loader said.
        reason: String,
    },
    /// The library does not export a libretro entry point: it is not a
    /// core.
    #[error("{path} does not export {name}: not a libretro core")]
    MissingSymbol {
        /// The core path.
        path: PathBuf,
        /// The missing entry point.
        name: &'static str,
    },
    /// The core implements another version of the libretro API.
    #[error("the core has libretro API version {found}, the host has {expected}")]
    ApiVersion {
        /// What `retro_api_version` returned.
        found: u32,
        /// `RETRO_API_VERSION`.
        expected: u32,
    },
    /// `retro_get_system_info` gave a string with no NUL in the bound.
    #[error("retro_get_system_info gave an unterminated {field}")]
    InvalidSystemInfo {
        /// The field.
        field: &'static str,
    },
    /// The callback state did not come back from a call into the core.
    #[error("the host state was lost in a call into the core")]
    HostPoisoned,
}

/// Why [`Host::load_game`](crate::Host::load_game) failed.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// A game is loaded already. One process loads one game.
    #[error("a game is loaded already")]
    GameAlreadyLoaded,
    /// No content was given, and the core did not declare
    /// `SET_SUPPORT_NO_GAME`.
    #[error("the core needs content")]
    NoContent,
    /// The content path is not UTF-8 or holds a NUL.
    #[error("{path} is not a UTF-8 path without NUL bytes")]
    InvalidPath {
        /// The path.
        path: PathBuf,
    },
    /// The content file could not be read.
    #[error("cannot read {path}: {source}")]
    ReadContent {
        /// The content path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The content file is larger than
    /// [`MAX_CONTENT_BYTES`](crate::MAX_CONTENT_BYTES).
    #[error("{path} has {len} bytes, more than the limit of {limit} bytes")]
    ContentTooLarge {
        /// The content path.
        path: PathBuf,
        /// Its length.
        len: u64,
        /// The limit.
        limit: u64,
    },
    /// The zip file could not be opened or gave no usable entry.
    #[error("cannot load from {path}: {source}")]
    Zip {
        /// The archive path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: ZipError,
    },
    /// The core reads this content from a path on disk, and the MVP does
    /// not extract zip entries to files.
    #[error("the core needs a file path for {entry} in {path}; extract it from the archive")]
    ZipNeedsFullPath {
        /// The archive path.
        path: PathBuf,
        /// The entry that matched.
        entry: String,
    },
    /// `retro_load_game` returned false. The core log says why.
    #[error("the core refused the content")]
    Refused,
    /// `retro_get_system_av_info` gave a value that failed a check. The
    /// game was unloaded.
    #[error("the core gave invalid AV info: {0}")]
    InvalidAvInfo(#[source] GeometryError),
    /// The frame or audio buffer could not be allocated. The game was
    /// unloaded.
    #[error("cannot allocate {bytes} bytes for the frame and audio buffers")]
    OutOfMemory {
        /// The bytes asked for.
        bytes: usize,
    },
    /// The callback state did not come back from a call into the core.
    #[error("the host state was lost in a call into the core")]
    HostPoisoned,
}

/// Why a call during the game failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    /// No game is loaded.
    #[error("no game is loaded")]
    NoGame,
    /// The callback state did not come back from a call into the core.
    #[error("the host state was lost in a call into the core")]
    HostPoisoned,
}
