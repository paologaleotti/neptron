//! ROM scan, index cache, core and system metadata, gamelist import.

#![forbid(unsafe_code)]

mod catalog;

pub use catalog::{Catalog, CatalogError, CoreDef, DEFINITION_FORMAT, Source, SystemDef};
