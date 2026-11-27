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

mod card;
mod case;
pub mod cli;
mod consoles;
mod fields;
mod gate;
mod glass;
mod image;
mod json;
mod measure;
mod options;
mod pattern;
mod redraw;
mod run;
mod sheet;
mod survey;
mod wire;

pub use card::{
    CardSize, ConsoleShape, Expected, GlassShape, border_seen, card_size, glass_shape, lit_centre,
};
pub use case::{
    Board, CaseError, DEFAULT_GPU, DEFAULT_TV_GPU, DisplaySpec, REFERENCE_NAMES, SETTING_KEYS,
    Screen, Settings, Source, panel_mode, parse_arcade_table, rate_text, screen,
};
pub use consoles::{
    Console, LINE_525_MHZ, LINE_625_MHZ, MEGA_DRIVE, NEO_GEO, NES, NES_PAL, PLAYSTATION, Region,
    SNES, console_of,
};
pub use fields::{
    Fields, SourceLines, field_picture, flicker, flicker_filter, line_card, pairing, source_lines,
};
pub use gate::{
    ACCEPTED, ACCEPTED_MATCH_PERMILLE, Accepted, CRT_31KHZ_MIN_LINE_RATE_MHZ, FIELD_OFFSET_MILLI,
    FIELD_OFFSET_TOLERANCE_MILLI, FIELD_PICTURE_SIZE, FieldPictures, GATE_CASES, GateRow, GateRun,
    HEADER as GATE_HEADER, SHAPE_MAX_PERMILLE, SPEED_AGREE_PPM, ScreenType, Verdict, gate_cases,
    gate_sheet, row_text, run_cases, run_gate, screen_type, table as gate_table, verdict,
};
pub use gate::{
    CENTRE_MATCH_PERMILLE, FILL_MATCH_PERMILLE, PINNED, Pinned, SPEED_MATCH_PPM, pin_text,
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
pub use options::{OPTION_SOURCES, OptionCell, TvOption, option_cells, option_lines, option_sheet};
pub use pattern::{test_pattern, test_pattern_for};
pub use redraw::draw;
pub use run::{
    ARCADE_TABLE, CARD_MEASURE_SIZE, Case, GlassOptions, Outcome, RunError, SurveyRun, Tables,
    card_of, class_of, console_pixel_shape, finish, measure_card, pattern_of, player_turn,
    simulate, simulate_with_reference_card, survey_rows,
};
pub use sheet::{Cell, card_legend, sheet};
pub use survey::{
    ASPECT_ERROR_MAX_PERMILLE, Counts, DISPLAYS, EXTRA_SOURCES, Flag, LABEL_ADVANCE_PX,
    LABEL_COLOR, LABEL_LINE_PX, LARGER_MARGIN_PERMILLE, LOOK_COLOR, Label, NOTE_COLOR,
    OFF_CENTRE_MAX_PX, REFRESHES, Row, SMALL_FILL_MIN_PERMILLE, STYLES, Severity, Tile,
    compare_rows, contact_sheet, counts, labels_json, look_pictures, markdown, own_flags,
    settings_matrix, sort_rows, to_json,
};
pub use wire::{SceneFacts, Shown, Wire, WireError, run as run_wire};
