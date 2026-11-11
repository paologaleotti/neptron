//! Typed settings schema, three-level resolution (game, system, global, default) and TOML persistence.

#![forbid(unsafe_code)]

mod builtin;
mod edit;
mod layer;
mod parse;
mod resolve;
mod schema;

pub use builtin::{
    DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY, DISPLAY_STYLE, NAME_MAX_LEN,
    PICTURE_STYLES, REFRESH_POLICIES, SCHEMA_VERSION, VERSION_KEY,
};
pub use edit::{Change, EditError, edit};
pub use layer::{Layer, SetError};
pub use parse::{Issue, IssueKind, Parsed, parse_layer};
pub use resolve::{LayerMismatch, Layers, Resolved, resolve, resolve_key};
pub use schema::{
    Entry, Invalid, Kind, Level, Levels, Safety, Schema, SchemaError, Source, Value, ValueType,
};
