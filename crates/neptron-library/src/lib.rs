//! ROM scan, index cache, and core and system metadata.
//!
//! See the crate README for the scan rules, the sort order and the cache format.

#![forbid(unsafe_code)]

pub mod cache;
mod catalog;
mod changes;
mod folders;
mod index;
mod scan;
#[cfg(test)]
mod test_dir;
mod worker;

pub use catalog::{Catalog, CatalogError, CoreDef, DEFINITION_FORMAT, Source, SystemDef};
pub use changes::{Changes, changes};
pub use folders::{FolderStamp, MTIME_TRUST_WINDOW_NS, Mtime, trusted};
pub use index::{Game, Index, MAX_TEXT_BYTES, SystemGames, compare_names};
pub use scan::{
    MAX_FOLDER_DEPTH, MAX_REPORTED_ENTRIES, ScanError, ScanOutcome, ScanProgress, ScanReport,
    Unreadable, ensure_system_folders, refresh, scan,
};
pub use worker::ScanWorker;
