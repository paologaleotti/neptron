//! The screen simulator: what a screen of a given profile shows for a
//! game and its display settings.
//!
//! A case runs one core frame through `VideoEngine` on [`crate::FakeDisplay`]
//! with the policy settings that `neptron-video` builds from the setting
//! values, as the app does. The frame buffer of the mode is the wire; a
//! simple model of the screen gives the glass; the measures say where the
//! picture is, what is cut and what shape it has. A survey runs a matrix of
//! cases and flags the rows a person should look at.
//! `docs/spec/screen-simulator.md` states the model, the measures and the
//! flags. `cargo xtask screen` runs the command line of [`cli`] through the
//! example `screen` and writes the PNG files.

mod case;
pub mod cli;
mod glass;
mod image;
mod json;
mod measure;
mod pattern;
mod run;
mod survey;
mod wire;

pub use case::{
    Board, CaseError, DEFAULT_GPU, DEFAULT_TV_GPU, DisplaySpec, REFERENCE_NAMES, SETTING_KEYS,
    Screen, Settings, Source, panel_mode, parse_arcade_table, rate_text, screen,
};
pub use glass::{
    Band, GLASS_SIZE, Glass, TUBE_15KHZ_FIELD_MHZ, TUBE_ASPECT, TV_OVERSCAN_PERMILLE, Tube,
    TubeClass,
};
pub use image::{Rgb, Turn};
pub use json::{Json, object};
pub use measure::{
    Edges, Measures, ModeFacts, Names, Picture, Player, View, measure, milli_text, notice_parts,
    scan_letter, turned,
};
pub use pattern::test_pattern;
pub use run::{
    ARCADE_TABLE, Case, GlassOptions, Outcome, RunError, SurveyRun, Tables, class_of, pattern_of,
    player_turn, simulate, survey_rows,
};
pub use survey::{
    ASPECT_ERROR_MAX_PERMILLE, Counts, DISPLAYS, EXTRA_SOURCES, Flag, LABEL_ADVANCE_PX,
    LABEL_COLOR, LABEL_LINE_PX, LARGER_MARGIN_PERMILLE, LOOK_COLOR, Label, NOTE_COLOR,
    OFF_CENTRE_MAX_PX, REFRESHES, Row, SMALL_FILL_MIN_PERMILLE, STYLES, Severity, Tile,
    compare_rows, contact_sheet, counts, labels_json, look_pictures, markdown, own_flags,
    settings_matrix, sort_rows, to_json,
};
pub use wire::{SceneFacts, Shown, Wire, WireError, run as run_wire};
