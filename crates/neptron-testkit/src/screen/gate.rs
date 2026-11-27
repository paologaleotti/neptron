//! The gate: every stored case of the simulator, one verdict each. A case
//! fails when the shape of the test card on the glass is off by more than
//! 1 % from its reference (the width of the real console where the table
//! has its clock, else the aspect of the core), unless it names an
//! accepted value with its reason; when its lines cut, its fill, the
//! centre of its picture on the glass or its speed differ from the values
//! stored for it; when it drops a line; when it does not stand upright;
//! when the two fields of an interlaced mode do not carry their own source
//! lines; or when two measures of the simulator disagree.
//! `docs/spec/screen-simulator.md` states the rules.

use neptron_types::CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ;

use super::cli::{GOLDEN_COUNT, GoldenCase, golden_case};
use super::fields::{field_picture, flicker_filter};
use super::glass::Glass;
use super::image::{Rgb, Turn};
use super::measure::{Edges, Measures, Picture, milli_text, scan_letter};
use super::run::{GlassOptions, RunError, Tables, measure_card, simulate_with_reference_card};
use super::sheet::{Cell, card_legend, sheet};
use super::survey::{LABEL_COLOR, LOOK_COLOR, Label, NOTE_COLOR};

/// The largest error of the glass shape against the reference of the
/// case, in per mille: 1 %.
pub const SHAPE_MAX_PERMILLE: i64 = 10;
/// An accepted value holds while the measure is within this of it, in per
/// mille: the reading of the circle on the glass is good to about 0.2 %.
pub const ACCEPTED_MATCH_PERMILLE: i64 = 3;
/// Field 2 lies half a line of a field below field 1, in thousandths of
/// that line, within [`FIELD_OFFSET_TOLERANCE_MILLI`].
pub const FIELD_OFFSET_MILLI: i64 = 500;
/// The tolerance of [`FIELD_OFFSET_MILLI`].
pub const FIELD_OFFSET_TOLERANCE_MILLI: i64 = 10;
/// The speed of the mode read back and the speed of the timing of the
/// policy agree within this, in parts per million.
pub const SPEED_AGREE_PPM: i64 = 500;
/// The shape read from the light and the shape worked out from the
/// geometry of the glass model agree within this, in per mille, where the
/// lines scale by a whole step.
pub const MODEL_AGREE_PERMILLE: i64 = 3;
/// The circle spans at least this part of the lines of `dest`, in per
/// mille: 0.8 of the frame upright, 0.6 turned (192 of 320 lines).
const CIRCLE_LINES_MIN_PERMILLE: u64 = 600;
/// The lowest line rate of a 31 kHz class, in millihertz: a CRT mode
/// between the 15 kHz class and this is a 25 kHz mode.
pub const CRT_31KHZ_MIN_LINE_RATE_MHZ: u64 = 28_000_000;
/// The fill of a case holds while it is within this of its stored value,
/// on each axis, in per mille of the glass.
pub const FILL_MATCH_PERMILLE: u32 = 3;
/// The centre of the picture holds while it is within this of its stored
/// place, on each axis, in per mille of the glass.
pub const CENTRE_MATCH_PERMILLE: u32 = 3;
/// The speed of a case holds while it is within this of its stored value,
/// in parts per million: 0.1 %.
pub const SPEED_MATCH_PPM: i64 = 1_000;

/// The cases of the gate that have no golden picture: the 25 kHz class.
pub const GATE_CASES: [GoldenCase; 4] = [
    ("snes_256-arcade_25", "arcade_25", None, "snes_256", &[]),
    ("ps1_320-arcade_25", "arcade_25", None, "ps1_320", &[]),
    ("ps1_480i-arcade_25", "arcade_25", None, "ps1_480i", &[]),
    ("rtype-arcade_25", "arcade_25", None, "rtype", &[]),
];

/// A shape that the product gives today and that a case accepts: the
/// error against the reference of the case, why, and the open point that
/// holds the question, if one does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accepted {
    /// The case name.
    pub case: &'static str,
    /// The glass shape error against the reference of the case, in per
    /// mille.
    pub error_permille: i64,
    /// Why the product gives this shape today.
    pub reason: &'static str,
    /// The open point that holds the question, or `None` when it is a
    /// limit of the hardware or of the model.
    pub open_point: Option<&'static str>,
}

/// The values of a stored case that the gate holds it to: what a person
/// looked at, so a change fails until a person looks again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pinned {
    /// The case name.
    pub case: &'static str,
    /// The source lines and columns cut: top, bottom, left, right.
    pub cut: [u32; 4],
    /// The fill of the glass, across and down, in per mille.
    pub fill: (u32, u32),
    /// The centre of the lit picture on the glass, across and down, in per
    /// mille.
    pub centre: (u32, u32),
    /// The speed of the game on the mode, in parts per million.
    pub speed_ppm: i64,
    /// The status line shows the speed alert.
    pub speed_alert: bool,
}

const PAL_SAMPLES: &str =
    "whole samples for each game pixel on the fixed 13.5 MHz clock of a composite output";
const TUBE_MODEL: &str = "tube model, to measure on the bench";
const TV_CLAMP: &str =
    "NTSC TV, Auto: a 384-pixel board is clamped to 704 samples and 16 of its 256 lines are cut";
const CORE_ASPECT: &str = "on a CRT of 25 kHz or more and on a panel the policy gives the 4:3 of the core, not the width of the real console";

/// An accepted value with no open point.
const fn limit(case: &'static str, error_permille: i64, reason: &'static str) -> Accepted {
    Accepted {
        case,
        error_permille,
        reason,
        open_point: None,
    }
}

/// An accepted value under OP-122: a question of the product.
const fn open(case: &'static str, error_permille: i64, reason: &'static str) -> Accepted {
    Accepted {
        case,
        error_permille,
        reason,
        open_point: Some("OP-122"),
    }
}

/// The accepted values of the gate. They name what the product or the
/// model gives today and do not say that the shape is right.
pub const ACCEPTED: &[Accepted] = &[
    limit("snes-generic_15-auto", 27, TUBE_MODEL),
    open("rtype-ntsc-auto", -63, TV_CLAMP),
    open("psx-pc_31_120-auto", 94, CORE_ASPECT),
    limit("ddonpach-generic_15-auto", -25, TUBE_MODEL),
    open("snes-panel_1080-auto", 21, CORE_ASPECT),
    limit("ddonpach-arcade_15-o0", -49, TUBE_MODEL),
    limit("pwrinst2-arcade_15-o90", -49, TUBE_MODEL),
    limit("ps1_320-generic_15", 26, TUBE_MODEL),
    limit("ps1_512-generic_15", 27, TUBE_MODEL),
    limit("ps1_480i-generic_15", 29, TUBE_MODEL),
    limit("ps1_480i-generic_15-pi5", 27, TUBE_MODEL),
    limit("ps1_320-arcade_15", 26, TUBE_MODEL),
    limit("ps1_480i-arcade_15", -12, TUBE_MODEL),
    open("ps1_320-pc_31_120", 94, CORE_ASPECT),
    open("ps1_480i-pc_31_120", 94, CORE_ASPECT),
    limit("ps1_320-pal", -15, PAL_SAMPLES),
    limit("ps1_480i-pal", -15, PAL_SAMPLES),
    open("ps1_480i-panel_1080", 94, CORE_ASPECT),
    limit("snes_256-generic_15", 27, TUBE_MODEL),
    limit("snes_512-generic_15", 27, TUBE_MODEL),
    limit("md_320-generic_15", 27, TUBE_MODEL),
    limit("md_256-generic_15", 27, TUBE_MODEL),
    limit("snes_256-arcade_15", 27, TUBE_MODEL),
    limit("snes_512-arcade_15", 27, TUBE_MODEL),
    limit("md_320-arcade_15", 27, TUBE_MODEL),
    limit("md_256-arcade_15", 27, TUBE_MODEL),
    limit("snes_256-pal", -15, PAL_SAMPLES),
    limit("md_320-pal", -15, PAL_SAMPLES),
    limit("md_256-pal", -15, PAL_SAMPLES),
    open("snes_256-arcade_25", 21, CORE_ASPECT),
    open("ps1_320-arcade_25", 95, CORE_ASPECT),
    open("ps1_480i-arcade_25", 92, CORE_ASPECT),
];

/// A stored case.
#[allow(clippy::too_many_arguments)]
const fn pin(
    case: &'static str,
    cut: [u32; 4],
    fill: (u32, u32),
    centre: (u32, u32),
    speed_ppm: i64,
    speed_alert: bool,
) -> Pinned {
    Pinned {
        case,
        cut,
        fill,
        centre,
        speed_ppm,
        speed_alert,
    }
}

/// The values of every stored case, read from the run that a person
/// looked at.
pub const PINNED: &[Pinned] = &[
    pin(
        "snes-generic_15-auto",
        [0, 0, 0, 0],
        (1000, 932),
        (500, 499),
        0,
        false,
    ),
    pin(
        "rtype-ntsc-auto",
        [8, 8, 0, 0],
        (1000, 1000),
        (500, 500),
        91890,
        true,
    ),
    pin(
        "psx-pc_31_120-auto",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "psx_480i-ntsc-auto",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "ddonpach-generic_15-auto",
        [0, 0, 0, 0],
        (562, 992),
        (500, 505),
        0,
        false,
    ),
    pin(
        "snes-panel_1080-auto",
        [0, 0, 0, 0],
        (750, 1000),
        (500, 500),
        -1647,
        true,
    ),
    pin(
        "snes-ntsc-fit",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        -748,
        false,
    ),
    pin(
        "ddonpach-pc_31_120-fit",
        [0, 0, 0, 0],
        (562, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "rtype-ntsc-fit",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        91890,
        true,
    ),
    pin(
        "rtype-pal-fit",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        -89454,
        true,
    ),
    pin(
        "ddonpach-arcade_15-o0",
        [0, 0, 0, 0],
        (563, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "ddonpach-arcade_15-o90",
        [0, 0, 0, 0],
        (999, 975),
        (500, 513),
        0,
        false,
    ),
    pin(
        "pwrinst2-arcade_15-o0",
        [0, 0, 0, 0],
        (999, 975),
        (500, 513),
        0,
        false,
    ),
    pin(
        "pwrinst2-arcade_15-o90",
        [0, 0, 0, 0],
        (563, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "ddonpach-pc_31_120-o0",
        [0, 0, 0, 0],
        (351, 625),
        (499, 500),
        0,
        false,
    ),
    pin(
        "ddonpach-pc_31_120-o90",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "pwrinst2-pc_31_120-o0",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "pwrinst2-pc_31_120-o90",
        [0, 0, 0, 0],
        (351, 625),
        (499, 500),
        0,
        false,
    ),
    pin(
        "ddonpach-ntsc-o0",
        [0, 0, 0, 0],
        (417, 741),
        (497, 502),
        41529,
        true,
    ),
    pin(
        "ddonpach-ntsc-o90",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        43509,
        true,
    ),
    pin(
        "pwrinst2-ntsc-o0",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        43509,
        true,
    ),
    pin(
        "pwrinst2-ntsc-o90",
        [0, 0, 0, 0],
        (417, 741),
        (497, 502),
        41529,
        true,
    ),
    pin(
        "ddonpach-panel_1080-o0",
        [0, 0, 0, 0],
        (375, 889),
        (500, 500),
        42571,
        true,
    ),
    pin(
        "ddonpach-panel_1080-o90",
        [0, 0, 0, 0],
        (750, 1000),
        (500, 500),
        42571,
        true,
    ),
    pin(
        "pwrinst2-panel_1080-o0",
        [0, 0, 0, 0],
        (750, 1000),
        (500, 500),
        42571,
        true,
    ),
    pin(
        "pwrinst2-panel_1080-o90",
        [0, 0, 0, 0],
        (375, 889),
        (500, 500),
        42571,
        true,
    ),
    pin(
        "ps1_320-generic_15",
        [0, 0, 0, 0],
        (1000, 995),
        (500, 501),
        0,
        false,
    ),
    pin(
        "ps1_512-generic_15",
        [0, 0, 0, 0],
        (1000, 995),
        (500, 501),
        0,
        false,
    ),
    pin(
        "ps1_480i-generic_15",
        [0, 0, 0, 0],
        (1000, 997),
        (500, 499),
        0,
        false,
    ),
    pin(
        "ps1_480i-generic_15-pi5",
        [0, 0, 0, 0],
        (1000, 995),
        (500, 501),
        0,
        false,
    ),
    pin(
        "ps1_320-arcade_15",
        [0, 0, 0, 0],
        (1000, 995),
        (500, 501),
        0,
        false,
    ),
    pin(
        "ps1_480i-arcade_15",
        [0, 0, 0, 0],
        (1000, 998),
        (500, 501),
        0,
        false,
    ),
    pin(
        "ps1_320-pc_31_120",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "ps1_480i-pc_31_120",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "vga_480p-pc_31_120",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "ps1_320-ntsc",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        3811,
        false,
    ),
    pin(
        "ps1_480i-ntsc",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "ps1_320-pal",
        [0, 0, 0, 0],
        (1000, 926),
        (500, 500),
        -162905,
        true,
    ),
    pin(
        "ps1_480i-pal",
        [0, 0, 0, 0],
        (992, 926),
        (503, 500),
        -165832,
        true,
    ),
    pin(
        "ps1_480i-panel_1080",
        [0, 0, 0, 0],
        (667, 889),
        (500, 500),
        1001,
        true,
    ),
    pin(
        "snes_256-generic_15",
        [0, 0, 0, 0],
        (1000, 932),
        (500, 499),
        0,
        false,
    ),
    pin(
        "snes_512-generic_15",
        [0, 0, 0, 0],
        (1000, 932),
        (500, 499),
        0,
        false,
    ),
    pin(
        "md_320-generic_15",
        [0, 0, 0, 0],
        (1000, 931),
        (500, 499),
        0,
        false,
    ),
    pin(
        "md_256-generic_15",
        [0, 0, 0, 0],
        (1000, 931),
        (500, 499),
        0,
        false,
    ),
    pin(
        "snes_256-arcade_15",
        [0, 0, 0, 0],
        (1000, 932),
        (500, 499),
        0,
        false,
    ),
    pin(
        "snes_512-arcade_15",
        [0, 0, 0, 0],
        (1000, 932),
        (500, 499),
        0,
        false,
    ),
    pin(
        "md_320-arcade_15",
        [0, 0, 0, 0],
        (1000, 931),
        (500, 499),
        0,
        false,
    ),
    pin(
        "md_256-arcade_15",
        [0, 0, 0, 0],
        (1000, 931),
        (500, 499),
        0,
        false,
    ),
    pin(
        "snes_256-ntsc",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        -748,
        false,
    ),
    pin(
        "snes_512-ntsc",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        -748,
        false,
    ),
    pin(
        "md_320-ntsc",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        2202,
        false,
    ),
    pin(
        "md_256-ntsc",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        2202,
        false,
    ),
    pin(
        "snes_256-pal",
        [0, 0, 0, 0],
        (1000, 864),
        (500, 500),
        -166708,
        true,
    ),
    pin(
        "snes_512-pal",
        [0, 0, 0, 0],
        (1000, 864),
        (500, 500),
        -166708,
        true,
    ),
    pin(
        "md_320-pal",
        [0, 0, 0, 0],
        (1000, 864),
        (500, 500),
        -164246,
        true,
    ),
    pin(
        "md_256-pal",
        [0, 0, 0, 0],
        (1000, 864),
        (500, 500),
        -164246,
        true,
    ),
    pin(
        "snes_256-arcade_25",
        [0, 0, 0, 0],
        (583, 583),
        (500, 500),
        -1647,
        true,
    ),
    pin(
        "ps1_320-arcade_25",
        [0, 0, 0, 0],
        (624, 623),
        (500, 499),
        0,
        false,
    ),
    pin(
        "ps1_480i-arcade_25",
        [0, 0, 0, 0],
        (1000, 1000),
        (500, 500),
        0,
        false,
    ),
    pin(
        "rtype-arcade_25",
        [0, 0, 0, 0],
        (640, 640),
        (500, 500),
        0,
        false,
    ),
];

/// The screen types of the gate, one contact sheet each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenType {
    /// A CRT in a mode below 20 kHz.
    Crt15,
    /// A CRT in a mode from 20 kHz to 28 kHz.
    Crt25,
    /// A CRT in a mode at 28 kHz or more.
    Crt31,
    /// An NTSC TV.
    Ntsc,
    /// A PAL TV.
    Pal,
    /// A flat panel.
    Panel,
}

/// The verdict of a case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every rule holds.
    Pass,
    /// Every rule holds but the shape, which is the accepted value.
    Accepted(Accepted),
    /// The reasons the case fails.
    Fail(Vec<String>),
}

/// One case of the gate.
#[derive(Debug, Clone)]
pub struct GateRow {
    /// The case name.
    pub name: String,
    /// The screen type of its sheet.
    pub screen: ScreenType,
    /// Its measures.
    pub measures: Measures,
    /// Its verdict.
    pub verdict: Verdict,
}

/// The pictures of the two fields of an interlaced case: field 1, field 2,
/// the two fields as the eye sees them, and the same with the model of
/// the flicker filter.
#[derive(Debug, Clone)]
pub struct FieldPictures {
    /// The case name.
    pub name: String,
    /// The pictures, in that order.
    pub pictures: [Rgb; 4],
}

/// The result of the gate: the rows, the glass of each row, and the field
/// pictures of the interlaced cases.
#[derive(Debug)]
pub struct GateRun {
    /// One row per case.
    pub rows: Vec<GateRow>,
    /// The glass of each row at the tile size, as the player sees it.
    pub tiles: Vec<Option<Rgb>>,
    /// The fields of each interlaced case.
    pub fields: Vec<FieldPictures>,
}

/// The size of each field picture, in pixels.
pub const FIELD_PICTURE_SIZE: (u32, u32) = (720, 540);

/// Every stored case: the golden cases, then [`GATE_CASES`].
pub fn gate_cases() -> Vec<GoldenCase> {
    (0..GOLDEN_COUNT)
        .map(golden_case)
        .chain(GATE_CASES)
        .collect()
}

/// Runs every stored case, measures the card on the glass, and gives each
/// its verdict. The glass of each row is `tile` pixels.
///
/// # Errors
///
/// [`RunError`] when a case does not run.
pub fn run_gate(tables: &Tables, tile: (u32, u32)) -> Result<GateRun, RunError> {
    run_cases(tables, &gate_cases(), tile)
}

/// [`run_gate`] for the cases `cases`.
///
/// # Errors
///
/// [`RunError`] when a case does not run.
pub fn run_cases(
    tables: &Tables,
    cases: &[GoldenCase],
    tile: (u32, u32),
) -> Result<GateRun, RunError> {
    let options = GlassOptions {
        overscan_permille: None,
        size: tile,
        turn: Turn::R0,
    };
    let mut run = GateRun {
        rows: Vec::new(),
        tiles: Vec::new(),
        fields: Vec::new(),
    };
    for &(name, display, gpu, source, sets) in cases {
        let mut case = tables.named_case_on(display, gpu, source, sets)?;
        // The glass turns after it is drawn: a screen on its side draws
        // into the box turned, so the picture fits the tile once turned.
        let size = if case.settings.orientation()?.screen.across() {
            (tile.1, tile.1 * tile.1 / tile.0.max(1))
        } else {
            tile
        };
        let (mut outcome, frame) =
            simulate_with_reference_card(&mut case, &GlassOptions { size, ..options })?;
        measure_card(&case, &frame, None, &mut outcome);
        if let Some(shown) = outcome.wire.shown.as_ref().filter(|s| s.mode.interlace) {
            let glass = Glass::of(&case.screen.profile, &shown.mode, None);
            let size = glass.picture_size(FIELD_PICTURE_SIZE);
            let fb = &shown.frame_buffer;
            let filtered = flicker_filter(fb, shown.scene.dest);
            run.fields.push(FieldPictures {
                name: name.to_owned(),
                pictures: [
                    field_picture(&glass, &shown.mode, fb, size, 1),
                    field_picture(&glass, &shown.mode, fb, size, 2),
                    glass.render(&shown.mode, fb, size),
                    glass.render(&shown.mode, &filtered, size),
                ],
            });
        }
        let measures = outcome.measures;
        run.rows.push(GateRow {
            name: name.to_owned(),
            screen: screen_type(&measures),
            verdict: verdict(name, &measures, ACCEPTED, PINNED),
            measures,
        });
        run.tiles.push(outcome.glass_image);
    }
    Ok(run)
}

/// The screen type of the case of `m`.
pub fn screen_type(m: &Measures) -> ScreenType {
    match m.class.as_str() {
        "panel" => ScreenType::Panel,
        "tv" if m.display == "pal" => ScreenType::Pal,
        "tv" => ScreenType::Ntsc,
        _ => match m.picture.as_ref().map(|p| p.mode.line_rate_mhz) {
            Some(rate) if rate >= CRT_31KHZ_MIN_LINE_RATE_MHZ => ScreenType::Crt31,
            Some(rate) if rate >= CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ => ScreenType::Crt25,
            _ => ScreenType::Crt15,
        },
    }
}

/// The verdict of case `name` with measures `m`, the accepted values
/// `accepted` and the stored values `pinned`.
pub fn verdict(name: &str, m: &Measures, accepted: &[Accepted], pinned: &[Pinned]) -> Verdict {
    let Some(p) = &m.picture else {
        return Verdict::Fail(vec!["no picture".into()]);
    };
    let mut why = Vec::new();
    let accept = accepted.iter().find(|a| a.case == name);
    match (p.glass_shape, accept) {
        (None, _) => why.push("no test card on the glass".to_owned()),
        (Some(shape), Some(a)) => {
            let error = shape.verdict_error_permille();
            if (error - a.error_permille).abs() > ACCEPTED_MATCH_PERMILLE {
                why.push(format!(
                    "shape {} is not the accepted {}",
                    signed(error),
                    signed(a.error_permille)
                ));
            }
        }
        (Some(shape), None) => {
            let error = shape.verdict_error_permille();
            if error.abs() > SHAPE_MAX_PERMILLE {
                why.push(format!(
                    "shape {} ({})",
                    signed(error),
                    shape.verdict_reference()
                ));
            }
        }
    }
    match pinned.iter().find(|v| v.case == name) {
        Some(v) => why.extend(differences(p, v)),
        None => why.push(format!("no stored values; measured {}", pin_text(name, p))),
    }
    if p.dropped_lines > 0 {
        why.push(format!("{} lines dropped", p.dropped_lines));
    }
    if p.player.upright == Some(false) {
        why.push("not upright".into());
    }
    if let Some(f) = p.fields {
        let off = (f.field_offset_milli - FIELD_OFFSET_MILLI).abs();
        if !f.own_lines || off > FIELD_OFFSET_TOLERANCE_MILLI {
            why.push(format!(
                "fields do not pair: offset {} of a line",
                milli_text(u64::try_from(f.field_offset_milli).unwrap_or(0))
            ));
        }
        match f.source_lines {
            Some(lines) if lines.own => {}
            Some(lines) => why.push(format!(
                "the fields do not carry their own source lines: {} lines in both",
                lines.shared
            )),
            None => why.push("the source lines of the fields are not read".into()),
        }
    }
    why.extend(disagreements(p));
    match (why.is_empty(), accept) {
        (true, Some(a)) => Verdict::Accepted(*a),
        (true, None) => Verdict::Pass,
        (false, _) => Verdict::Fail(why),
    }
}

/// Where the picture `p` differs from the stored values `v`.
fn differences(p: &Picture, v: &Pinned) -> Vec<String> {
    let mut out = Vec::new();
    let cut = [
        p.cropped.top,
        p.cropped.bottom,
        p.cropped.left,
        p.cropped.right,
    ];
    if cut != v.cut {
        out.push(format!("lines cut {cut:?}, stored {:?}", v.cut));
    }
    let fill = (p.fill_w_permille, p.fill_h_permille);
    if fill.0.abs_diff(v.fill.0) > FILL_MATCH_PERMILLE
        || fill.1.abs_diff(v.fill.1) > FILL_MATCH_PERMILLE
    {
        out.push(format!("fill {fill:?}, stored {:?}", v.fill));
    }
    match p.glass_centre_permille {
        Some(c)
            if c.0.abs_diff(v.centre.0) <= CENTRE_MATCH_PERMILLE
                && c.1.abs_diff(v.centre.1) <= CENTRE_MATCH_PERMILLE => {}
        Some(c) => out.push(format!("centre {c:?}, stored {:?}", v.centre)),
        None => out.push("no lit picture on the glass".into()),
    }
    if (p.mode_speed_ppm - v.speed_ppm).abs() > SPEED_MATCH_PPM || p.speed_alert != v.speed_alert {
        out.push(format!(
            "speed {} ppm (alert {}), stored {} ppm (alert {})",
            p.mode_speed_ppm, p.speed_alert, v.speed_ppm, v.speed_alert
        ));
    }
    out
}

/// The values of `p` as the source text of a [`Pinned`] entry for `name`,
/// so a new case can be stored after a person has looked at it.
pub fn pin_text(name: &str, p: &Picture) -> String {
    let c = &p.cropped;
    let centre = p.glass_centre_permille.unwrap_or_default();
    format!(
        "pin(\"{name}\", [{}, {}, {}, {}], ({}, {}), ({}, {}), {}, {})",
        c.top,
        c.bottom,
        c.left,
        c.right,
        p.fill_w_permille,
        p.fill_h_permille,
        centre.0,
        centre.1,
        p.mode_speed_ppm,
        p.speed_alert
    )
}

/// The places where two measures of the simulator disagree: the crop of
/// the scene against the sides of the border in the frame buffer, the
/// part the glass model hides against the sides of the border on the
/// glass, and the speed of the timing against the speed of the mode.
fn disagreements(p: &Picture) -> Vec<String> {
    let mut out = Vec::new();
    let sides = |e: &Edges<u32>| [e.top, e.bottom, e.left, e.right];
    let seen = |e: &Edges<bool>| [e.top, e.bottom, e.left, e.right];
    let names = ["top", "bottom", "left", "right"];
    if let Some(wire) = &p.border_on_wire {
        for ((cut, shows), side) in sides(&p.cropped).into_iter().zip(seen(wire)).zip(names) {
            if (cut == 0) != shows {
                out.push(format!("crop {cut} at the {side}, border shows: {shows}"));
            }
        }
    }
    if let Some(glass) = &p.border_on_glass {
        let cut = sides(&p.cropped);
        let hidden = sides(&p.glass_hidden);
        for (((cut, hidden), shows), side) in
            cut.into_iter().zip(hidden).zip(seen(glass)).zip(names)
        {
            let wrong = cut == 0 && ((hidden == 0 && !shows) || (hidden >= 2 && shows));
            if wrong {
                out.push(format!(
                    "glass hides {hidden} at the {side}, border shows: {shows}"
                ));
            }
        }
    }
    if let Some(shape) = p.glass_shape {
        let off = (shape.core_error_permille - p.model_aspect_error_permille).abs();
        // With lines scaled by a fraction, the edge of the circle shows on
        // the nearest mode line, so the light can differ from the geometry
        // by one mode line over the height of the circle.
        let circle_lines = u64::from(p.dest.h) * CIRCLE_LINES_MIN_PERMILLE / 1000;
        let quantum = if p.scale_y_whole {
            0
        } else {
            i64::try_from(1000_u64.div_ceil(circle_lines.max(1))).unwrap_or(i64::MAX)
        };
        if off > MODEL_AGREE_PERMILLE + quantum {
            out.push(format!(
                "shape from the light {}, from the model {}",
                signed(shape.core_error_permille),
                signed(p.model_aspect_error_permille)
            ));
        }
    }
    if (p.mode_speed_ppm - p.speed_ppm).abs() > SPEED_AGREE_PPM {
        out.push(format!(
            "speed of the mode {} ppm, of the timing {} ppm",
            p.mode_speed_ppm, p.speed_ppm
        ));
    }
    out
}

fn signed(permille: i64) -> String {
    let sign = if permille < 0 { "-" } else { "+" };
    let a = permille.abs();
    format!("{sign}{}.{}%", a / 10, a % 10)
}

impl ScreenType {
    /// Every type, in sheet order.
    pub const ALL: [Self; 6] = [
        Self::Crt15,
        Self::Crt25,
        Self::Crt31,
        Self::Ntsc,
        Self::Pal,
        Self::Panel,
    ];

    /// The file name of its sheet.
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::Crt15 => "crt_15khz",
            Self::Crt25 => "crt_25khz",
            Self::Crt31 => "crt_31khz",
            Self::Ntsc => "ntsc",
            Self::Pal => "pal",
            Self::Panel => "panel",
        }
    }
}

impl Verdict {
    /// `PASS`, `ACCEPTED` or `FAIL`.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Accepted(_) => "ACCEPTED",
            Self::Fail(_) => "FAIL",
        }
    }

    /// The verdict with its reason.
    pub fn text(&self) -> String {
        match self {
            Self::Pass => "PASS".into(),
            Self::Accepted(a) => match a.open_point {
                Some(op) => format!("ACCEPTED {} ({op}): {}", signed(a.error_permille), a.reason),
                None => format!("ACCEPTED {}: {}", signed(a.error_permille), a.reason),
            },
            Self::Fail(why) => format!("FAIL: {}", why.join("; ")),
        }
    }
}

/// The header of [`row_text`].
pub const HEADER: &str = "| case | display | mode | ref | shape (a) core | shape (b) console | fill w/h | lines cut t/b/l/r | dropped | flicker / filtered | verdict |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |";

/// One row of the gate table.
pub fn row_text(row: &GateRow) -> String {
    let m = &row.measures;
    let Some(p) = &m.picture else {
        return format!(
            "| {} | {} | no picture | | | | | | | | {} |",
            row.name,
            m.display,
            row.verdict.text()
        );
    };
    let (core, console) = shape_texts(p);
    let flicker = p.fields.map_or("-".to_owned(), |f| {
        format!(
            "{} / {}",
            permille(f.flicker_permille),
            permille(f.filtered_flicker_permille)
        )
    });
    format!(
        "| {} | {} | {} | {} | {core} | {console} | {} / {} | {}/{}/{}/{} | {} | {flicker} | {} |",
        row.name,
        m.display,
        mode_text(p),
        reference_text(p),
        permille(p.fill_w_permille),
        permille(p.fill_h_permille),
        p.cropped.top,
        p.cropped.bottom,
        p.cropped.left,
        p.cropped.right,
        p.dropped_lines,
        row.verdict.text()
    )
}

/// The glass shape error against reference (a) and against reference
/// (b), as text.
fn shape_texts(p: &Picture) -> (String, String) {
    match p.glass_shape {
        None => ("-".into(), "-".into()),
        Some(s) => (
            signed(s.core_error_permille),
            s.console.map_or("-".into(), |c| {
                format!("{} ({})", signed(c.error_permille), c.name)
            }),
        ),
    }
}

/// The reference of the card and the verdict of `p`.
fn reference_text(p: &Picture) -> &'static str {
    p.glass_shape.map_or("-", |s| s.verdict_reference())
}

fn permille(p: u32) -> String {
    format!("{}.{}%", p / 10, p % 10)
}

fn mode_text(p: &Picture) -> String {
    let rate = milli_text(p.mode.field_rate_mhz);
    format!(
        "{}x{}{} {}Hz",
        p.mode.hactive,
        p.mode.vactive,
        scan_letter(p.mode.interlace, p.mode.doublescan),
        &rate[..rate.len() - 1]
    )
}

/// The whole table of the gate, with a count line at the end.
pub fn table(rows: &[GateRow]) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for row in rows {
        out.push_str(&row_text(row));
        out.push('\n');
    }
    let count = |word: &str| rows.iter().filter(|r| r.verdict.word() == word).count();
    out.push_str(&format!(
        "\n{} cases: {} pass, {} accepted, {} fail\n",
        rows.len(),
        count("PASS"),
        count("ACCEPTED"),
        count("FAIL")
    ));
    out
}

/// The colour of a verdict on a sheet.
const PASS_COLOR: [u8; 3] = [0x80, 0xE0, 0x80];

/// The contact sheet of screen type `screen`: the glass of each of its
/// rows with the case name, the mode, the shape against both references
/// and the verdict under it. `None` when no case has that type.
pub fn gate_sheet(
    run: &GateRun,
    screen: ScreenType,
    tile: (u32, u32),
) -> Option<(Rgb, Vec<Label>)> {
    let cells: Vec<Cell<'_>> = run
        .rows
        .iter()
        .zip(&run.tiles)
        .filter(|(row, _)| row.screen == screen)
        .map(|(row, glass)| gate_cell(row, glass.as_ref()))
        .collect();
    if cells.is_empty() {
        return None;
    }
    Some(sheet(&cells, 4, tile, 4, &card_legend()))
}

fn gate_cell<'a>(row: &GateRow, glass: Option<&'a Rgb>) -> Cell<'a> {
    let (color, mark) = match row.verdict {
        Verdict::Pass => (PASS_COLOR, None),
        Verdict::Accepted(_) => (NOTE_COLOR, Some(NOTE_COLOR)),
        Verdict::Fail(_) => (LOOK_COLOR, Some(LOOK_COLOR)),
    };
    let m = &row.measures;
    let (mode, shape, extra) = match &m.picture {
        Some(p) => {
            let (core, console) = shape_texts(p);
            let flicker = p.fields.map_or(String::new(), |f| {
                format!(
                    " flicker {}/{}",
                    permille(f.flicker_permille),
                    permille(f.filtered_flicker_permille)
                )
            });
            let shape = match p.glass_shape {
                Some(g) if g.console.is_some() => {
                    format!("ref: console {console}  (core 4:3 {core})")
                }
                Some(_) => format!("ref: core 4:3 {core}"),
                None => "ref: no card read".to_owned(),
            };
            (
                format!("{} {}", m.display, mode_text(p)),
                shape,
                format!(
                    "cut {} drop {}{flicker}",
                    p.cropped.top + p.cropped.bottom + p.cropped.left + p.cropped.right,
                    p.dropped_lines
                ),
            )
        }
        None => (m.display.clone(), "no picture".into(), String::new()),
    };
    Cell {
        picture: glass,
        lines: vec![
            (format!("{} {}", row.verdict.word(), row.name), color),
            (mode, LABEL_COLOR),
            (shape, LABEL_COLOR),
            (extra, LABEL_COLOR),
        ],
        mark,
    }
}

#[cfg(test)]
mod tests {
    use neptron_types::{Filter, Orientation, Rect, Rotation};

    use super::super::card::GlassShape;
    use super::super::measure::Measures;
    use super::super::redraw;
    use super::super::run::finish;
    use super::super::wire::Shown;
    use super::*;
    use crate::golden::workspace_root;

    /// Runs the stored cases `range` of [`gate_cases`] and checks that none
    /// fails and that a case is accepted exactly when it has a value in
    /// [`ACCEPTED`].
    fn assert_cases(range: std::ops::Range<usize>) -> GateRun {
        let tables = Tables::load(&workspace_root()).unwrap();
        let cases = gate_cases();
        let run = run_cases(&tables, &cases[range], (64, 48)).unwrap();
        for row in &run.rows {
            assert!(
                !matches!(row.verdict, Verdict::Fail(_)),
                "{}: {}",
                row.name,
                row.verdict.text()
            );
            let named = ACCEPTED.iter().any(|a| a.case == row.name);
            assert_eq!(
                matches!(row.verdict, Verdict::Accepted(_)),
                named,
                "{}",
                row.name
            );
        }
        run
    }

    use super::super::cli::{GOLDEN_CASES, INTERLACED_CASES, TATE_CASES, WIDTH_CASES};

    #[test]
    fn the_reference_cases_pass_the_gate() {
        assert_cases(0..GOLDEN_CASES.len());
    }

    #[test]
    fn the_tate_cases_pass_the_gate() {
        let first = GOLDEN_CASES.len();
        assert_cases(first..first + TATE_CASES.len());
    }

    /// The interlaced cases pass, and their fields pair: field 2 half a
    /// line below field 1, and each carries its own source lines. The
    /// flicker of the card is 3.4 % to 3.5 % of white on a 480i mode, and
    /// 1.7 % to 1.8 % with the filter model.
    #[test]
    fn the_interlaced_cases_pass_the_gate_and_their_fields_pair() {
        let first = GOLDEN_CASES.len() + TATE_CASES.len();
        let run = assert_cases(first..first + INTERLACED_CASES.len());
        let interlaced: Vec<_> = run
            .rows
            .iter()
            .filter_map(|r| {
                r.measures
                    .picture
                    .as_ref()?
                    .fields
                    .map(|f| (r.name.as_str(), f))
            })
            .collect();
        let names: Vec<&str> = interlaced.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            ["ps1_480i-generic_15-pi5", "ps1_480i-ntsc", "ps1_480i-pal"]
        );
        let flicker = [(35, 18), (35, 18), (34, 17)];
        for ((name, f), flicker) in interlaced.into_iter().zip(flicker) {
            assert_eq!(f.field_offset_milli, FIELD_OFFSET_MILLI, "{name}");
            assert!(f.own_lines, "{name}");
            assert_eq!(f.source_lines.map(|l| (l.own, l.shared)), Some((true, 0)));
            assert_eq!(
                (f.flicker_permille, f.filtered_flicker_permille),
                flicker,
                "{name}"
            );
        }
    }

    #[test]
    fn the_width_cases_pass_the_gate() {
        let first = GOLDEN_CASES.len() + TATE_CASES.len() + INTERLACED_CASES.len();
        assert_cases(first..first + WIDTH_CASES.len());
    }

    #[test]
    fn the_25_khz_cases_pass_the_gate() {
        assert_cases(GOLDEN_COUNT..GOLDEN_COUNT + GATE_CASES.len());
    }

    /// Every accepted value names one stored case, once, and every stored
    /// case has its stored values, once.
    #[test]
    fn every_accepted_value_names_one_stored_case() {
        let cases = gate_cases();
        for case in &cases {
            assert_eq!(
                PINNED.iter().filter(|v| v.case == case.0).count(),
                1,
                "{}",
                case.0
            );
        }
        assert_eq!(PINNED.len(), cases.len());
        for a in ACCEPTED {
            assert_eq!(
                cases.iter().filter(|c| c.0 == a.case).count(),
                1,
                "{}",
                a.case
            );
            assert_eq!(
                ACCEPTED.iter().filter(|b| b.case == a.case).count(),
                1,
                "{}",
                a.case
            );
            assert!(a.error_permille.abs() > SHAPE_MAX_PERMILLE, "{}", a.case);
        }
    }

    /// The measures of the stored case `name` after `change` makes the
    /// placement of the engine wrong on purpose, with the card of its
    /// reference. With `redraw_fb` the frame buffer is drawn again for the
    /// changed scene, as a renderer would; without it, the scene and the
    /// picture disagree.
    fn mutated(name: &str, redraw_fb: bool, change: impl FnOnce(&mut Shown, &Rgb)) -> Measures {
        let tables = Tables::load(&workspace_root()).unwrap();
        let (_, display, gpu, source, sets) =
            gate_cases().into_iter().find(|c| c.0 == name).unwrap();
        let mut case = tables.named_case_on(display, gpu, source, sets).unwrap();
        let options = GlassOptions {
            overscan_permille: None,
            size: (64, 48),
            turn: Turn::R0,
        };
        let (outcome, frame) = simulate_with_reference_card(&mut case, &options).unwrap();
        let mut wire = outcome.wire;
        let shown = wire.shown.as_mut().unwrap();
        change(shown, &frame);
        if redraw_fb {
            shown.frame_buffer = redraw::draw(&shown.mode, &frame, &shown.scene).unwrap();
        }
        let mut outcome = finish(&case, &frame, &options, Orientation::NORMAL, wire);
        measure_card(&case, &frame, None, &mut outcome);
        outcome.measures
    }

    fn picture(m: &Measures) -> &Picture {
        m.picture.as_ref().unwrap()
    }

    /// The verdict of the gate for the stored case `name`.
    fn judged(name: &str, m: &Measures) -> Verdict {
        verdict(name, m, ACCEPTED, PINNED)
    }

    /// A stored case judged on the console reference; the mutations below
    /// change its placement.
    const CASE: &str = "ps1_320-ntsc";

    fn fails(m: &Measures) -> Vec<String> {
        match judged(CASE, m) {
            Verdict::Fail(why) => why,
            other => panic!("the gate does not fail: {other:?}"),
        }
    }

    #[test]
    fn the_case_as_the_engine_places_it_passes() {
        let m = mutated(CASE, true, |_, _| {});
        assert_eq!(judged(CASE, &m), Verdict::Pass);
        let shape = picture(&m).glass_shape.unwrap();
        assert_eq!(shape.verdict_reference(), "console");
    }

    /// A picture squeezed by 5 % across, on the card of the console: the
    /// shape fails.
    #[test]
    fn a_squeeze_of_5_percent_fails_the_gate() {
        let m = mutated(CASE, true, |s, _| {
            let w = s.scene.dest.w * 95 / 100;
            s.scene.dest.x += i32::try_from((s.scene.dest.w - w) / 2).unwrap();
            s.scene.dest.w = w;
            s.scene.filter = Filter::SharpBilinear;
        });
        let why = fails(&m);
        assert!(why.iter().any(|w| w.starts_with("shape")), "{why:?}");
    }

    /// A picture shrunk by 5 % on both axes, near its shape: the fill
    /// fails.
    #[test]
    fn a_shrink_of_5_percent_fails_the_gate() {
        let m = mutated(CASE, true, |s, _| {
            let d = s.scene.dest;
            let (w, h) = (d.w * 95 / 100, d.h * 95 / 100);
            s.scene.dest = Rect {
                x: d.x + i32::try_from((d.w - w) / 2).unwrap(),
                y: d.y + i32::try_from((d.h - h) / 2).unwrap(),
                w,
                h,
            };
            s.scene.filter = Filter::SharpBilinear;
        });
        let why = fails(&m);
        assert!(why.iter().any(|w| w.starts_with("fill")), "{why:?}");
    }

    /// A new crop of 8 lines at the top and the bottom that the border of
    /// the frame buffer agrees with: the lines cut fail.
    #[test]
    fn a_new_crop_of_8_lines_fails_the_gate() {
        let m = mutated(CASE, true, |s, _| cut_scene(s));
        let p = picture(&m);
        assert!(disagreements(p).is_empty(), "{:?}", disagreements(p));
        let why = fails(&m);
        assert!(why.iter().any(|w| w.starts_with("lines cut")), "{why:?}");
    }

    /// The picture moved 8 lines down. On NTSC it leaves the mode at the
    /// bottom, so the lines cut fail; on PAL it stays in the field of 288
    /// lines, so the centre of the picture on the glass fails.
    #[test]
    fn a_shift_of_8_lines_fails_the_gate() {
        let m = mutated(CASE, true, |s, _| s.scene.dest.y += 8);
        let why = fails(&m);
        assert!(why.iter().any(|w| w.starts_with("lines cut")), "{why:?}");
        let pal = "ps1_320-pal";
        let m = mutated(pal, true, |s, _| s.scene.dest.y += 8);
        let Verdict::Fail(why) = judged(pal, &m) else {
            panic!("passes");
        };
        assert!(why.iter().any(|w| w.starts_with("centre")), "{why:?}");
        assert!(!why.iter().any(|w| w.starts_with("lines cut")), "{why:?}");
    }

    /// A picture turned upside down fails.
    #[test]
    fn a_turned_picture_fails_the_gate() {
        let m = mutated(CASE, true, |s, _| s.scene.rotation = Rotation::R180);
        assert_eq!(picture(&m).player.upright, Some(false));
        assert!(fails(&m).iter().any(|w| w == "not upright"));
    }

    /// 240 lines pressed into 224 with nearest drop 16 lines and fail.
    #[test]
    fn a_dropped_line_fails_the_gate() {
        let m = mutated(CASE, true, |s, _| {
            s.scene.dest.h = 224;
            s.scene.filter = Filter::Nearest;
        });
        assert_eq!(picture(&m).dropped_lines, 16);
        assert!(fails(&m).iter().any(|w| w == "16 lines dropped"));
    }

    /// A 240-line picture doubled into the 480i mode: both fields carry
    /// the same source lines, and the case fails.
    #[test]
    fn a_240p_frame_doubled_into_480i_fails_the_pairing() {
        let name = "ps1_480i-ntsc";
        let m = mutated(name, true, |s, _| {
            s.scene.crop = Rect {
                h: s.scene.crop.h / 2,
                ..s.scene.crop
            };
        });
        let lines = picture(&m).fields.unwrap().source_lines.unwrap();
        assert!(!lines.own);
        assert_eq!(lines.shared, 240);
        let Verdict::Fail(why) = judged(name, &m) else {
            panic!("passes");
        };
        assert!(
            why.iter().any(|w| w.contains("own source lines")),
            "{why:?}"
        );
    }

    /// The crop drawn but not said by the scene: the crop and the border
    /// of the frame buffer disagree, and the case fails.
    #[test]
    fn a_scene_that_hides_its_crop_fails_the_gate() {
        let m = mutated(CASE, false, |s, card| {
            let mut wrong = s.scene;
            cut_scene_facts(&mut wrong);
            s.frame_buffer = redraw::draw(&s.mode, card, &wrong).unwrap();
        });
        assert!(fails(&m).iter().any(|w| w.contains("border shows")));
    }

    fn cut_scene(s: &mut Shown) {
        cut_scene_facts(&mut s.scene);
    }

    fn cut_scene_facts(s: &mut super::super::wire::SceneFacts) {
        s.crop = Rect {
            y: 8,
            h: s.crop.h - 16,
            ..s.crop
        };
    }

    /// A mode read back at another rate than the timing of the policy
    /// says: the two speeds disagree, and the speed differs from the stored
    /// one.
    #[test]
    fn a_mode_at_another_rate_fails_the_gate() {
        let m = mutated(CASE, false, |s, _| {
            s.mode.pclock_hz = s.mode.pclock_hz * 5 / 6;
        });
        let why = fails(&m);
        assert!(
            why.iter().any(|w| w.starts_with("speed of the mode")),
            "{why:?}"
        );
        assert!(why.iter().any(|w| w.starts_with("speed ")), "{why:?}");
    }

    fn measures(shape: i64) -> Measures {
        let mut p = super::super::survey::tests::picture();
        p.model_aspect_error_permille = shape;
        p.glass_centre_permille = Some((500, 500));
        p.glass_shape = Some(GlassShape {
            shown_milli: 1_333,
            core_milli: 1_333,
            core_error_permille: shape,
            console: None,
            border_error_permille: None,
        });
        Measures {
            display: "ntsc".into(),
            class: "tv".into(),
            gpu: "g".into(),
            source: "s".into(),
            settings: "auto/exact".into(),
            offer: "offered".into(),
            picture: Some(p),
            no_picture: None,
        }
    }

    const ACCEPT: Accepted = Accepted {
        case: "c",
        error_permille: -91,
        reason: "r",
        open_point: Some("OP-122"),
    };

    /// The stored values of the case "c" of [`measures`].
    const PIN: [Pinned; 1] = [pin("c", [0; 4], (1000, 1000), (500, 500), 0, false)];

    #[test]
    fn a_shape_off_by_more_than_one_percent_fails() {
        assert_eq!(verdict("c", &measures(10), &[], &PIN), Verdict::Pass);
        assert_eq!(verdict("c", &measures(-10), &[], &PIN), Verdict::Pass);
        assert!(matches!(
            verdict("c", &measures(11), &[], &PIN),
            Verdict::Fail(_)
        ));
    }

    #[test]
    fn an_accepted_value_holds_only_while_the_shape_is_that_value() {
        assert_eq!(
            verdict("c", &measures(-90), &[ACCEPT], &PIN),
            Verdict::Accepted(ACCEPT)
        );
        assert!(matches!(
            verdict("c", &measures(-60), &[ACCEPT], &PIN),
            Verdict::Fail(_)
        ));
        assert!(
            matches!(
                verdict("c", &measures(0), &[ACCEPT], &PIN),
                Verdict::Fail(_)
            ),
            "a fixed shape makes the acceptance stale"
        );
        assert!(matches!(
            verdict("d", &measures(-90), &[ACCEPT], &PIN),
            Verdict::Fail(_)
        ));
    }

    #[test]
    fn a_dropped_line_a_wrong_turn_and_no_card_fail() {
        let mut m = measures(0);
        m.picture.as_mut().unwrap().dropped_lines = 2;
        assert!(matches!(verdict("c", &m, &[], &PIN), Verdict::Fail(_)));
        let mut m = measures(0);
        m.picture.as_mut().unwrap().player.upright = Some(false);
        assert!(matches!(verdict("c", &m, &[], &PIN), Verdict::Fail(_)));
        let mut m = measures(0);
        m.picture.as_mut().unwrap().glass_shape = None;
        assert!(matches!(verdict("c", &m, &[], &PIN), Verdict::Fail(_)));
    }

    /// A scene that says nothing is cut while the border does not show in
    /// the frame buffer: the simulator disagrees with itself, and the case
    /// fails.
    #[test]
    fn two_measures_that_disagree_fail() {
        let mut m = measures(0);
        m.picture.as_mut().unwrap().border_on_wire = Some(Edges {
            top: false,
            bottom: true,
            left: true,
            right: true,
        });
        let Verdict::Fail(why) = verdict("c", &m, &[], &PIN) else {
            panic!("passes");
        };
        assert!(why[0].contains("top"), "{why:?}");
        let mut m = measures(0);
        m.picture.as_mut().unwrap().mode_speed_ppm = -166_000;
        assert!(matches!(verdict("c", &m, &[], &PIN), Verdict::Fail(_)));
    }
}
