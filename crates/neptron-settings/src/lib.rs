//! Typed settings schema, three-level resolution (game, system, global, default) and TOML persistence.

#![forbid(unsafe_code)]

mod builtin;
mod schema;

pub use builtin::{
    DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY, DISPLAY_STYLE, NAME_MAX_LEN,
    PICTURE_STYLES, REFRESH_POLICIES, SCHEMA_VERSION, VERSION_KEY,
};
pub use schema::{
    Entry, Invalid, Kind, Level, Levels, Safety, Schema, SchemaError, Source, Value, ValueType,
};
