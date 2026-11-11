//! ROM scan, index cache, core and system metadata, gamelist import.

#![forbid(unsafe_code)]

mod catalog;
mod index;

pub use catalog::{Catalog, CatalogError, CoreDef, DEFINITION_FORMAT, Source, SystemDef};
pub use index::{Game, Index, MAX_TEXT_BYTES, SystemGames, compare_names};
