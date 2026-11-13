//! Typed settings schema, three-level resolution (game, system, global, default) and TOML persistence.
//!
//! Schema, resolution, parsing and editing are pure. Only [`Storage`] reads and writes files.
//!
//! ```
//! use neptron_settings::{Layers, Level, Schema, Source, Value, parse_layer, resolve_key};
//!
//! let schema = Schema::builtin();
//! let global = parse_layer(&schema, Level::Global, "[display]\nstyle = \"2x\"\n");
//! let game = parse_layer(&schema, Level::Game, "[display]\nstyle = \"9x\"\n");
//! assert_eq!(game.issues.len(), 1);
//!
//! let layers = Layers::try_new(Some(&global.layer), None, Some(&game.layer)).unwrap();
//! let style = resolve_key(&schema, &layers, "display.style").unwrap();
//! assert_eq!(style.value, Value::Text("2x".into()));
//! assert_eq!(style.source, Source::Global);
//! ```

#![forbid(unsafe_code)]

mod builtin;
mod edit;
mod layer;
mod parse;
mod resolve;
mod schema;
mod storage;

pub use builtin::{
    AUDIO_OUTPUT, AUDIO_OUTPUT_MAX_LEN, AUDIO_VOLUME, AUDIO_VOLUME_DEFAULT_PERCENT,
    AUDIO_VOLUME_MAX_PERCENT, DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY,
    DISPLAY_SCANLINE_STRENGTH, DISPLAY_SCANLINES, DISPLAY_STYLE, INPUT_MENU_HOLD_START_SECONDS,
    MENU_HOLD_START_MAX_SECONDS, NAME_MAX_LEN, PICTURE_STYLES, REFRESH_POLICIES, SCANLINE_MODES,
    SCANLINE_STRENGTH_DEFAULT_PERCENT, SCANLINE_STRENGTH_MAX_PERCENT, SCHEMA_VERSION, VERSION_KEY,
};
pub use edit::{Change, EditError, edit};
pub use layer::{Layer, SetError};
pub use parse::{Issue, IssueKind, Parsed, parse_layer};
pub use resolve::{LayerMismatch, Layers, Resolved, resolve, resolve_key};
pub use schema::{
    AppliesTo, DisplayClass, Entry, Invalid, Kind, Level, Levels, Safety, Schema, SchemaError,
    Source, Value, ValueType,
};
pub use storage::{Loaded, NEW_FILE_MODE, Storage, StorageError, Target, WriteStep};
