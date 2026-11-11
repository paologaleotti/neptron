//! ROM scan, index cache, core and system metadata, gamelist import.

#![forbid(unsafe_code)]

mod catalog;
mod index;
mod scan;
#[cfg(test)]
mod test_dir;
mod worker;

pub use catalog::{Catalog, CatalogError, CoreDef, DEFINITION_FORMAT, Source, SystemDef};
pub use index::{Game, Index, MAX_TEXT_BYTES, SystemGames, compare_names};
pub use scan::{
    MAX_FOLDER_DEPTH, MAX_REPORTED_ENTRIES, ScanError, ScanOutcome, ScanProgress, ScanReport,
    Unreadable, ensure_system_folders, scan,
};
pub use worker::ScanWorker;
