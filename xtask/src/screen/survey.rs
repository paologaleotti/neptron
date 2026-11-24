//! The survey: a matrix of cases, the flags that mark a suspicious row,
//! the sorted table and the contact sheet of each display. Pure.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Value, json};

use super::image::Rgb;
use super::measure::{Measures, Picture, milli_text, scan_letter};

/// The displays of the default survey.
pub const DISPLAYS: [&str; 7] = [
    "generic_15",
    "arcade_15",
    "pc_31_120",
    "ntsc",
    "pal",
    "1920x1080@60",
    "2560x1440@120",
];

/// The sources of the default survey besides the reference sources: R-Type
/// and two vertical boards of the arcade table.
pub const EXTRA_SOURCES: [&str; 3] = ["rtype", "gunbird", "pacman"];

/// The picture modes, by their value of `display.style`.
pub const STYLES: [&str; 8] = ["auto", "native", "2x", "3x", "4x", "sharp", "speed", "fit"];
/// The refresh choices, by their value of `display.refresh_policy`.
pub const REFRESHES: [&str; 4] = ["exact", "exact_x2", "force_50", "force_60"];

/// The picture modes that promise whole steps on the vertical axis.
const WHOLE_STYLES: [&str; 5] = ["native", "2x", "3x", "4x", "sharp"];
/// The picture modes that name one picture each: two of them that the
/// menus offer should not give the same picture.
const DISTINCT_STYLES: [&str; 5] = ["native", "2x", "3x", "4x", "fit"];

/// A picture that fills less than this of the visible width and of the
/// visible height is small, in per mille.
pub const SMALL_FILL_MIN_PERMILLE: u32 = 700;
/// The largest aspect error that is not flagged, in per mille.
pub const ASPECT_ERROR_MAX_PERMILLE: i64 = 30;
/// The largest distance of the centre of the picture from the centre of
/// the mode that is not flagged, in mode pixels.
pub const OFF_CENTRE_MAX_PX: i64 = 1;
/// Fill is flagged when its area on the glass is smaller than the area of
/// Auto by more than this, in per mille of the area of Auto.
pub const FILL_SMALLER_MARGIN_PERMILLE: u64 = 5;

/// Why a row is suspicious. A flag is a hint for a person, not a test
/// failure.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    /// Nothing shows, and no picture mode falls back to a picture.
    NoPicture,
    /// Source lines or columns are cut.
    Cropped,
    /// The picture fills less than [`SMALL_FILL_MIN_PERMILLE`] of the
    /// width and of the height.
    Small,
    /// The aspect error is above [`ASPECT_ERROR_MAX_PERMILLE`].
    Aspect,
    /// A picture mode that promises whole steps scales the lines by a
    /// fraction.
    NotWhole,
    /// Fill gives a smaller picture than Auto.
    FillSmallerThanAuto,
    /// Two picture modes that the menus offer give the same picture.
    SameAs(String),
    /// The picture is off the centre of the mode.
    OffCentre,
}

/// One row of the survey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The value of `display.style`.
    pub style: String,
    /// The value of `display.refresh_policy`.
    pub refresh: String,
    /// The measures.
    pub measures: Measures,
    /// The flags.
    pub flags: Vec<Flag>,
}

impl Flag {
    /// The short name in the tables and on the contact sheets.
    pub fn name(&self) -> String {
        match self {
            Self::NoPicture => "no_picture".into(),
            Self::Cropped => "cropped".into(),
            Self::Small => "small".into(),
            Self::Aspect => "aspect".into(),
            Self::NotWhole => "not_whole".into(),
            Self::FillSmallerThanAuto => "fill_smaller_than_auto".into(),
            Self::SameAs(other) => format!("same_as_{other}"),
            Self::OffCentre => "off_centre".into(),
        }
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// The settings of the default survey: every picture mode with the
/// refresh rate at Auto, then Auto with every other refresh rate.
pub fn settings_matrix() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<_> = STYLES.iter().map(|s| (*s, REFRESHES[0])).collect();
    out.extend(REFRESHES.iter().skip(1).map(|r| (STYLES[0], *r)));
    out
}

/// The flags of one row by itself.
pub fn own_flags(style: &str, measures: &Measures) -> Vec<Flag> {
    let Some(p) = &measures.picture else {
        return vec![Flag::NoPicture];
    };
    let mut flags = Vec::new();
    let c = &p.cropped;
    if c.top + c.bottom + c.left + c.right > 0 {
        flags.push(Flag::Cropped);
    }
    if p.fill_w_permille < SMALL_FILL_MIN_PERMILLE && p.fill_h_permille < SMALL_FILL_MIN_PERMILLE {
        flags.push(Flag::Small);
    }
    if p.aspect_error_permille.abs() > ASPECT_ERROR_MAX_PERMILLE {
        flags.push(Flag::Aspect);
    }
    if WHOLE_STYLES.contains(&style) && !p.scale_y_whole {
        flags.push(Flag::NotWhole);
    }
    if off_centre(p) {
        flags.push(Flag::OffCentre);
    }
    flags
}

fn off_centre(p: &Picture) -> bool {
    let off = |start: i32, len: u32, mode: u16| {
        (2 * i64::from(start) + i64::from(len) - i64::from(mode)).abs() > 2 * OFF_CENTRE_MAX_PX
    };
    off(p.dest.x, p.dest.w, p.mode.hactive) || off(p.dest.y, p.dest.h, p.mode.vactive)
}

/// Adds the flags that compare rows of the same display and source: Fill
/// against Auto, and two offered picture modes with the same picture.
pub fn compare_rows(rows: &mut [Row]) {
    let mut groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (i, row) in rows.iter().enumerate() {
        let key = (row.measures.display.clone(), row.measures.source.clone());
        groups.entry(key).or_default().push(i);
    }
    for members in groups.values() {
        let find = |rows: &[Row], style: &str| {
            members
                .iter()
                .copied()
                .find(|&i| rows[i].style == style && rows[i].refresh == REFRESHES[0])
        };
        if let (Some(auto), Some(fit)) = (find(rows, "auto"), find(rows, "fit"))
            && let (Some(a), Some(f)) = (area(&rows[auto]), area(&rows[fit]))
            && f * 1_000 < a * (1_000 - FILL_SMALLER_MARGIN_PERMILLE)
        {
            rows[fit].flags.push(Flag::FillSmallerThanAuto);
        }
        let distinct: Vec<usize> = DISTINCT_STYLES
            .iter()
            .filter_map(|s| find(rows, s))
            .collect();
        for (n, &i) in distinct.iter().enumerate() {
            for &j in &distinct[n + 1..] {
                if same_offered_picture(&rows[i], &rows[j]) {
                    let (si, sj) = (rows[i].style.clone(), rows[j].style.clone());
                    rows[i].flags.push(Flag::SameAs(sj));
                    rows[j].flags.push(Flag::SameAs(si));
                }
            }
        }
    }
}

/// The area of the picture on the glass, in millionths.
fn area(row: &Row) -> Option<u64> {
    let p = row.measures.picture.as_ref()?;
    Some(u64::from(p.fill_w_permille) * u64::from(p.fill_h_permille))
}

fn same_offered_picture(a: &Row, b: &Row) -> bool {
    let offered = |r: &Row| r.measures.offer == "offered";
    match (&a.measures.picture, &b.measures.picture) {
        (Some(pa), Some(pb)) => {
            offered(a)
                && offered(b)
                && pa.mode == pb.mode
                && pa.dest == pb.dest
                && pa.crop == pb.crop
        }
        _ => false,
    }
}

/// The rows with the most flags first; the order of the run within the
/// same count.
pub fn sort_rows(rows: &mut [Row]) {
    rows.sort_by_key(|r| std::cmp::Reverse(r.flags.len()));
}

/// The survey as Markdown: the count of flagged rows per display, then
/// every row.
pub fn markdown(rows: &[Row], displays: &[String]) -> String {
    let mut out = String::from("# Screen survey\n\n");
    out.push_str(
        "Made by `cargo xtask screen survey`. A flag is a hint for a person, not a test failure. \
         `docs/spec/screen-simulator.md` defines each measure and flag.\n\n",
    );
    out.push_str("| display | rows | flagged | ");
    let names = [
        "no_picture",
        "cropped",
        "small",
        "aspect",
        "not_whole",
        "fill_smaller_than_auto",
        "same_as",
        "off_centre",
    ];
    out.push_str(&names.join(" | "));
    out.push_str(" |\n|");
    out.push_str(&" --- |".repeat(names.len() + 3));
    out.push('\n');
    for display in displays {
        let mine: Vec<&Row> = rows
            .iter()
            .filter(|r| &r.measures.display == display)
            .collect();
        let flagged = mine.iter().filter(|r| !r.flags.is_empty()).count();
        out.push_str(&format!("| {display} | {} | {flagged} |", mine.len()));
        for name in names {
            let n = mine
                .iter()
                .filter(|r| r.flags.iter().any(|f| f.name().starts_with(name)))
                .count();
            out.push_str(&format!(" {n} |"));
        }
        out.push('\n');
    }
    out.push_str("\n## Rows, the most flags first\n\n");
    out.push_str(&Measures::HEADER.replacen("| display |", "| flags | display |", 1));
    out.push('\n');
    out.push('|');
    out.push_str(&" --- |".repeat(Measures::HEADER.matches('|').count()));
    out.push('\n');
    for row in rows {
        let flags = if row.flags.is_empty() {
            "-".to_owned()
        } else {
            row.flags
                .iter()
                .map(Flag::name)
                .collect::<Vec<_>>()
                .join(", ")
        };
        out.push_str(&format!("| {flags} {}\n", row.measures.row()));
    }
    out
}

/// The survey as JSON: one object per row, with its flags.
pub fn to_json(rows: &[Row]) -> Value {
    Value::Array(
        rows.iter()
            .map(|r| {
                let mut m = r.measures.to_json();
                m["flags"] = json!(r.flags.iter().map(Flag::name).collect::<Vec<_>>());
                m
            })
            .collect(),
    )
}

/// The gap between two tiles of a contact sheet, in pixels.
const SHEET_GUTTER_PX: u32 = 8;
/// The height of one label line, in pixels.
const LABEL_LINE_PX: u32 = 12;
/// The label lines under each tile.
const LABEL_LINES: u32 = 3;
const SHEET_BACKGROUND: [u8; 3] = [0x30, 0x30, 0x30];
const LABEL_COLOR: [u8; 3] = [0xFF, 0xFF, 0xFF];
const FLAG_COLOR: [u8; 3] = [0xFF, 0x80, 0x40];
const NO_PICTURE_TILE: [u8; 3] = [0x50, 0x10, 0x10];

/// One tile of a contact sheet: the glass, or `None` for no picture.
pub struct Tile<'a> {
    /// The row of the tile.
    pub row: &'a Row,
    /// The glass picture.
    pub glass: Option<&'a Rgb>,
}

/// A contact sheet: `tiles` in rows of `columns`, each tile `tile` pixels
/// with three label lines under it: the source and the settings, the mode,
/// the flags.
pub fn contact_sheet(tiles: &[Tile<'_>], columns: usize, tile: (u32, u32)) -> Rgb {
    let (tw, th) = tile;
    let cell_h = th + LABEL_LINES * LABEL_LINE_PX + 2;
    let columns = columns.max(1);
    let rows = tiles.len().div_ceil(columns).max(1) as u32;
    let cols = columns.min(tiles.len().max(1)) as u32;
    let width = cols * tw + (cols + 1) * SHEET_GUTTER_PX;
    let height = rows * cell_h + (rows + 1) * SHEET_GUTTER_PX;
    let mut sheet = Rgb::new(width, height, SHEET_BACKGROUND);
    let chars = (tw / neptron_ui::font::ADVANCE) as usize;
    for (i, t) in tiles.iter().enumerate() {
        let x = SHEET_GUTTER_PX + (i % columns) as u32 * (tw + SHEET_GUTTER_PX);
        let y = SHEET_GUTTER_PX + (i / columns) as u32 * (cell_h + SHEET_GUTTER_PX);
        match t.glass {
            Some(glass) => {
                let dx = tw.saturating_sub(glass.width) / 2;
                let dy = th.saturating_sub(glass.height) / 2;
                sheet.paste(glass, x + dx, y + dy);
            }
            None => {
                sheet.paste(&Rgb::new(tw, th, NO_PICTURE_TILE), x, y);
                sheet.text(x + 4, y + 4, "NO PICTURE", LABEL_COLOR);
            }
        }
        let m = &t.row.measures;
        let line1 = format!("{} {}", m.source, m.settings);
        let line2 = match &m.picture {
            Some(p) => format!(
                "{}x{}{} {}Hz {}x{}",
                p.mode.hactive,
                p.mode.vactive,
                scan_letter(p.mode.interlace, p.mode.doublescan),
                milli_text(p.mode.field_rate_mhz),
                p.dest.w,
                p.dest.h
            ),
            None => m.offer.clone(),
        };
        let line3 = t
            .row
            .flags
            .iter()
            .map(Flag::name)
            .collect::<Vec<_>>()
            .join(" ");
        let ly = y + th + 2;
        sheet.text(x, ly, &clip(&line1, chars), LABEL_COLOR);
        sheet.text(x, ly + LABEL_LINE_PX, &clip(&line2, chars), LABEL_COLOR);
        sheet.text(x, ly + 2 * LABEL_LINE_PX, &clip(&line3, chars), FLAG_COLOR);
    }
    sheet
}

fn clip(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

#[cfg(test)]
mod tests {
    use neptron_types::{Rect, Rotation};

    use super::super::measure::{Edges, ModeFacts};
    use super::*;

    fn picture() -> Picture {
        Picture {
            mode: ModeFacts {
                hactive: 320,
                vactive: 240,
                htotal: 400,
                vtotal: 262,
                pclock_hz: 6_400_000,
                line_rate_mhz: 16_000_000,
                field_rate_mhz: 61_068,
                interlace: false,
                doublescan: false,
            },
            rung: "native".into(),
            crop: Rect {
                x: 0,
                y: 0,
                w: 320,
                h: 240,
            },
            dest: Rect {
                x: 0,
                y: 0,
                w: 320,
                h: 240,
            },
            rotation: Rotation::R0,
            filter: "nearest".into(),
            scale_x_milli: 1_000,
            scale_x_whole: true,
            scale_y_milli: 1_000,
            scale_y_whole: true,
            cropped: Edges::default(),
            borders: Edges::default(),
            fill_w_permille: 1_000,
            fill_h_permille: 1_000,
            aspect_shown_milli: 1_333,
            aspect_asked_milli: 1_333,
            aspect_error_permille: 0,
            glass_hidden: Edges::default(),
            overscan_hidden: None,
            speed_ppm: 0,
            speed_alert: false,
            scanlines: "none".into(),
            notices: Vec::new(),
        }
    }

    fn row(style: &str, picture: Option<Picture>) -> Row {
        let measures = Measures {
            display: "d".into(),
            gpu: "g".into(),
            source: "s".into(),
            settings: format!("{style}/exact"),
            offer: "offered".into(),
            picture,
            no_picture: None,
        };
        Row {
            style: style.into(),
            refresh: "exact".into(),
            flags: own_flags(style, &measures),
            measures,
        }
    }

    #[test]
    fn a_good_picture_has_no_flag() {
        assert!(row("auto", Some(picture())).flags.is_empty());
    }

    #[test]
    fn a_bad_picture_raises_every_own_flag() {
        let mut p = picture();
        p.cropped.top = 8;
        p.fill_w_permille = 500;
        p.fill_h_permille = 600;
        p.aspect_error_permille = -62;
        p.scale_y_whole = false;
        p.dest.x = 10;
        let flags = row("2x", Some(p)).flags;
        assert_eq!(
            flags,
            [
                Flag::Cropped,
                Flag::Small,
                Flag::Aspect,
                Flag::NotWhole,
                Flag::OffCentre
            ]
        );
        assert_eq!(row("auto", None).flags, [Flag::NoPicture]);
    }

    #[test]
    fn the_limits_are_not_flagged() {
        let mut p = picture();
        p.fill_w_permille = SMALL_FILL_MIN_PERMILLE;
        p.fill_h_permille = 100;
        p.aspect_error_permille = ASPECT_ERROR_MAX_PERMILLE;
        p.dest.w = 319;
        assert!(row("auto", Some(p.clone())).flags.is_empty());
        p.scale_y_whole = false;
        assert!(
            row("auto", Some(p)).flags.is_empty(),
            "auto promises no whole step"
        );
    }

    #[test]
    fn fill_smaller_than_auto_and_twin_modes_are_flagged() {
        let mut small = picture();
        small.fill_w_permille = 900;
        small.dest.w = 288;
        small.dest.x = 16;
        let mut rows = vec![
            row("auto", Some(picture())),
            row("fit", Some(small)),
            row("native", Some(picture())),
            row("2x", Some(picture())),
        ];
        rows[3].measures.offer = "declined: NotShownHere".into();
        compare_rows(&mut rows);
        assert_eq!(rows[1].flags, [Flag::FillSmallerThanAuto]);
        assert!(rows[2].flags.is_empty(), "2x is declined, so it may match");
        assert!(rows[0].flags.is_empty(), "auto is not a distinct mode");
        rows[3].measures.offer = "offered".into();
        compare_rows(&mut rows);
        assert_eq!(rows[2].flags, [Flag::SameAs("2x".into())]);
        sort_rows(&mut rows);
        assert!(!rows[0].flags.is_empty());
        assert!(rows.last().unwrap().flags.is_empty());
    }

    #[test]
    fn the_matrix_has_every_mode_and_every_rate() {
        let m = settings_matrix();
        assert_eq!(m.len(), 11);
        assert_eq!(m[0], ("auto", "exact"));
        assert_eq!(m[10], ("auto", "force_60"));
    }

    #[test]
    fn the_sheet_has_room_for_every_tile() {
        let rows: Vec<Row> = (0..5).map(|_| row("auto", Some(picture()))).collect();
        let glass = Rgb::new(40, 30, [1; 3]);
        let tiles: Vec<Tile<'_>> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| Tile {
                row: r,
                glass: (i != 2).then_some(&glass),
            })
            .collect();
        let sheet = contact_sheet(&tiles, 3, (40, 30));
        assert_eq!(sheet.width, 3 * 40 + 4 * SHEET_GUTTER_PX);
        assert_eq!(sheet.height, 2 * (30 + 38) + 3 * SHEET_GUTTER_PX);
        assert_eq!(sheet.get(SHEET_GUTTER_PX, SHEET_GUTTER_PX), [1; 3]);
        let md = markdown(&rows, &["d".into()]);
        assert!(md.contains("| d | 5 | 0 |"), "{md}");
        assert_eq!(to_json(&rows).as_array().unwrap().len(), 5);
    }
}
