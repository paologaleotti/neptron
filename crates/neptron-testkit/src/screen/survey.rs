//! The survey: a matrix of cases, the flags that mark a row for a person
//! with their severity, the sorted table, and the contact sheet of each
//! display with its labels. Pure.

use std::collections::BTreeMap;
use std::fmt;

use super::image::Rgb;
use super::json::{Json, object};
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
/// menus offer give two pictures, or the menus show a choice with no
/// effect.
const DISTINCT_STYLES: [&str; 5] = ["native", "2x", "3x", "4x", "fit"];

/// A picture that fills less than this of the visible width and of the
/// visible height is small, in per mille.
pub const SMALL_FILL_MIN_PERMILLE: u32 = 700;
/// The largest aspect error of the placement that is not flagged, in per
/// mille.
pub const ASPECT_ERROR_MAX_PERMILLE: i64 = 30;
/// The largest distance of the centre of the picture from the centre of
/// the mode that is not flagged, in mode pixels.
pub const OFF_CENTRE_MAX_PX: i64 = 1;
/// A picture of another row is larger than Fill when its area on the
/// glass is above the area of Fill by more than this, in per mille of the
/// area of Fill.
pub const LARGER_MARGIN_PERMILLE: u64 = 5;

/// How much a flag asks of a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// A likely fault: look at the picture.
    Look,
    /// Expected by design, and the policy says so.
    Note,
}

/// Why a row is marked. A flag is a hint for a person, not a test
/// failure.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    /// Nothing shows. Look.
    NoPicture,
    /// Source lines or columns are cut. Look when the notice of the policy
    /// does not name the cut lines; note when it does.
    Cropped {
        /// The notice names the cut.
        noticed: bool,
    },
    /// The lines of a progressive tube mode are scaled by a fraction:
    /// some source lines show taller than others. Look.
    UnevenLines,
    /// A picture mode that promises whole steps gives a scale that is not
    /// whole. Look.
    NotWhole,
    /// The placement gives the picture a shape more than
    /// [`ASPECT_ERROR_MAX_PERMILLE`] off its aspect. Look.
    Aspect,
    /// Fill gives a picture no larger than Native, where another setting
    /// gives a larger one. Look.
    FillNotLarger,
    /// The picture is off the centre of the mode. Look.
    OffCentre,
    /// The picture fills less than [`SMALL_FILL_MIN_PERMILLE`] of the
    /// width and of the height. Note for `native` or when the notice of
    /// the policy names the small picture; else look.
    Small {
        /// The picture mode or the notice states it.
        stated: bool,
    },
    /// Two picture modes that the menus both offer give the same picture.
    /// Note.
    SameAs(String),
    /// The game runs at another speed, and the status line says so. Note.
    SpeedAlert,
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
            Self::Cropped { .. } => "cropped".into(),
            Self::UnevenLines => "uneven_lines".into(),
            Self::NotWhole => "not_whole".into(),
            Self::Aspect => "aspect".into(),
            Self::FillNotLarger => "fill_not_larger".into(),
            Self::OffCentre => "off_centre".into(),
            Self::Small { .. } => "small".into(),
            Self::SameAs(other) => format!("same_as_{other}"),
            Self::SpeedAlert => "speed_alert".into(),
        }
    }

    /// How much the flag asks of a person.
    pub fn severity(&self) -> Severity {
        match self {
            Self::Cropped { noticed: true }
            | Self::Small { stated: true }
            | Self::SameAs(_)
            | Self::SpeedAlert => Severity::Note,
            Self::NoPicture
            | Self::Cropped { noticed: false }
            | Self::UnevenLines
            | Self::NotWhole
            | Self::Aspect
            | Self::FillNotLarger
            | Self::OffCentre
            | Self::Small { stated: false } => Severity::Look,
        }
    }
}

impl Row {
    /// The row has a flag to look at.
    pub fn look(&self) -> bool {
        self.flags.iter().any(|f| f.severity() == Severity::Look)
    }

    /// The flags of one severity, by name, joined by `sep`.
    pub fn flag_names(&self, severity: Severity, sep: &str) -> String {
        self.flags
            .iter()
            .filter(|f| f.severity() == severity)
            .map(Flag::name)
            .collect::<Vec<_>>()
            .join(sep)
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
        let noticed =
            u32::from(p.notice_cropped_lines) == c.top + c.bottom && c.left + c.right == 0;
        flags.push(Flag::Cropped { noticed });
    }
    let tube = measures.class != "panel";
    if tube && !p.mode.interlace && p.uneven_lines {
        flags.push(Flag::UnevenLines);
    }
    if WHOLE_STYLES.contains(&style) && !p.scale_y_whole {
        flags.push(Flag::NotWhole);
    }
    if p.aspect_error_permille.abs() > ASPECT_ERROR_MAX_PERMILLE {
        flags.push(Flag::Aspect);
    }
    if off_centre(p) {
        flags.push(Flag::OffCentre);
    }
    if p.fill_w_permille < SMALL_FILL_MIN_PERMILLE && p.fill_h_permille < SMALL_FILL_MIN_PERMILLE {
        let named = p
            .notices
            .iter()
            .any(|n| n.starts_with("padded_fill") || n.starts_with("small_fill"));
        flags.push(Flag::Small {
            stated: style == "native" || named,
        });
    }
    if p.speed_alert {
        flags.push(Flag::SpeedAlert);
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
/// against Native and every other row, and two offered picture modes with
/// the same picture.
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
        if let (Some(native), Some(fit)) = (find(rows, "native"), find(rows, "fit"))
            && let (Some(n), Some(f)) = (area(&rows[native]), area(&rows[fit]))
        {
            let largest = members.iter().filter_map(|&i| area(&rows[i])).max();
            let larger = largest.is_some_and(|l| l * 1_000 > f * (1_000 + LARGER_MARGIN_PERMILLE));
            if f <= n && larger {
                rows[fit].flags.push(Flag::FillNotLarger);
            }
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

/// The rows to look at first, then the rows with notes only, then the
/// rest; within a group, the most flags first, then the order of the run.
pub fn sort_rows(rows: &mut [Row]) {
    rows.sort_by_key(|r| {
        let group = if r.look() {
            0
        } else if r.flags.is_empty() {
            2
        } else {
            1
        };
        (group, std::cmp::Reverse(r.flags.len()))
    });
}

/// The rows of one display: how many, how many to look at, how many with
/// notes only.
pub fn counts(rows: &[Row], display: &str) -> (usize, usize, usize) {
    let mine: Vec<&Row> = rows
        .iter()
        .filter(|r| r.measures.display == display)
        .collect();
    let look = mine.iter().filter(|r| r.look()).count();
    let note = mine
        .iter()
        .filter(|r| !r.look() && !r.flags.is_empty())
        .count();
    (mine.len(), look, note)
}

/// The survey as Markdown: the counts per display, the rows to look at,
/// then every other row.
pub fn markdown(rows: &[Row], displays: &[String]) -> String {
    let mut out = String::from("# Screen survey\n\n");
    out.push_str(
        "Made by `cargo xtask screen survey`. A flag is a hint for a person, not a test failure. \
         `docs/spec/screen-simulator.md` defines each measure and flag. A `look` flag is a \
         likely fault; a `note` flag is expected by design and stated by the policy.\n\n",
    );
    out.push_str("| display | rows | look | note only |\n| --- | --- | --- | --- |\n");
    for display in displays {
        let (all, look, note) = counts(rows, display);
        out.push_str(&format!("| {display} | {all} | {look} | {note} |\n"));
    }
    let header = Measures::HEADER.replacen("| display |", "| look | note | display |", 1);
    let rule = format!("|{}\n", " --- |".repeat(header.matches('|').count() - 1));
    let table = |title: &str, keep: &dyn Fn(&Row) -> bool| {
        let mut t = format!("\n## {title}\n\n{header}\n{rule}");
        for row in rows.iter().filter(|r| keep(r)) {
            let dash = |s: String| if s.is_empty() { "-".to_owned() } else { s };
            t.push_str(&format!(
                "| {} | {} {}\n",
                dash(row.flag_names(Severity::Look, ", ")),
                dash(row.flag_names(Severity::Note, ", ")),
                row.measures.row()
            ));
        }
        t
    };
    out.push_str(&table("Rows to look at", &|r| r.look()));
    out.push_str(&table("Other rows", &|r| !r.look()));
    out
}

/// The survey as JSON: one object per row, with its flags.
pub fn to_json(rows: &[Row]) -> Json {
    Json::Array(
        rows.iter()
            .map(|r| {
                let mut m = r.measures.to_json();
                for (key, severity) in [("look", Severity::Look), ("note", Severity::Note)] {
                    let names: Vec<String> = r
                        .flags
                        .iter()
                        .filter(|f| f.severity() == severity)
                        .map(Flag::name)
                        .collect();
                    m.insert(key, names.into());
                }
                m
            })
            .collect(),
    )
}

/// The gap between two tiles of a contact sheet, in pixels.
const SHEET_GUTTER_PX: u32 = 6;
/// The height of one label line, in pixels: the 12-pixel UI font that
/// xtask draws.
pub const LABEL_LINE_PX: u32 = 12;
/// The width of one character of the label font, in pixels.
pub const LABEL_ADVANCE_PX: u32 = 6;
/// The label lines under each tile.
const LABEL_LINES: u32 = 3;
const SHEET_BACKGROUND: [u8; 3] = [0x30, 0x30, 0x30];
const NO_PICTURE_TILE: [u8; 3] = [0x50, 0x10, 0x10];
/// The frame around the tile of a row to look at.
const LOOK_FRAME: [u8; 3] = [0xFF, 0x50, 0x20];
/// The colour of the text of a label.
pub const LABEL_COLOR: [u8; 3] = [0xFF, 0xFF, 0xFF];
/// The colour of the flags to look at.
pub const LOOK_COLOR: [u8; 3] = [0xFF, 0x80, 0x40];
/// The colour of the notes.
pub const NOTE_COLOR: [u8; 3] = [0x90, 0xC0, 0xFF];

/// One tile of a contact sheet: the glass, or `None` for no picture.
pub struct Tile<'a> {
    /// The row of the tile.
    pub row: &'a Row,
    /// The glass picture.
    pub glass: Option<&'a Rgb>,
}

/// One line of text on a contact sheet, for the writer to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// The left edge, in pixels.
    pub x: u32,
    /// The top edge, in pixels.
    pub y: u32,
    /// The text.
    pub text: String,
    /// The colour.
    pub color: [u8; 3],
}

/// A contact sheet: `tiles` in rows of `columns`, each tile `tile` pixels
/// with three label lines under it: the source and the settings, the mode
/// and the size of the picture, the flags. A tile to look at has an orange
/// frame. The labels are returned to be drawn by the writer, which has
/// the font.
pub fn contact_sheet(tiles: &[Tile<'_>], columns: usize, tile: (u32, u32)) -> (Rgb, Vec<Label>) {
    let (tw, th) = tile;
    let cell_h = th + LABEL_LINES * LABEL_LINE_PX + 2;
    let columns = columns.max(1);
    let rows = tiles.len().div_ceil(columns).max(1) as u32;
    let cols = columns.min(tiles.len().max(1)) as u32;
    let width = cols * tw + (cols + 1) * SHEET_GUTTER_PX;
    let height = rows * cell_h + (rows + 1) * SHEET_GUTTER_PX;
    let mut sheet = Rgb::new(width, height, SHEET_BACKGROUND);
    let mut labels = Vec::new();
    let chars = (tw / LABEL_ADVANCE_PX) as usize;
    for (i, t) in tiles.iter().enumerate() {
        let x = SHEET_GUTTER_PX + (i % columns) as u32 * (tw + SHEET_GUTTER_PX);
        let y = SHEET_GUTTER_PX + (i / columns) as u32 * (cell_h + SHEET_GUTTER_PX);
        if t.row.look() {
            frame(&mut sheet, x, y, tw, th);
        }
        match t.glass {
            Some(glass) => {
                let dx = tw.saturating_sub(glass.width) / 2;
                let dy = th.saturating_sub(glass.height) / 2;
                sheet.paste(glass, x + dx, y + dy);
            }
            None => {
                sheet.paste(&Rgb::new(tw, th, NO_PICTURE_TILE), x, y);
            }
        }
        let m = &t.row.measures;
        let line1 = format!("{} {}", m.source, m.settings);
        let line2 = match &m.picture {
            Some(p) => format!(
                "{}x{}{} {} {}x{}",
                p.mode.hactive,
                p.mode.vactive,
                scan_letter(p.mode.interlace, p.mode.doublescan),
                hz_text(p.mode.field_rate_mhz),
                p.dest.w,
                p.dest.h
            ),
            None => "NO PICTURE".to_owned(),
        };
        let look = t.row.flag_names(Severity::Look, " ");
        let (line3, color3) = if look.is_empty() {
            (t.row.flag_names(Severity::Note, " "), NOTE_COLOR)
        } else {
            (look, LOOK_COLOR)
        };
        let ly = y + th + 2;
        for (n, (text, color)) in [(line1, LABEL_COLOR), (line2, LABEL_COLOR), (line3, color3)]
            .into_iter()
            .enumerate()
        {
            labels.push(Label {
                x,
                y: ly + n as u32 * LABEL_LINE_PX,
                text: text.chars().take(chars).collect(),
                color,
            });
        }
    }
    (sheet, labels)
}

/// A 2-pixel frame just outside a tile.
fn frame(sheet: &mut Rgb, x: u32, y: u32, w: u32, h: u32) {
    for d in 1..=2 {
        for i in 0..w + 2 * d {
            sheet.put(x + i - d, y - d, LOOK_FRAME);
            sheet.put(x + i - d, y + h + d - 1, LOOK_FRAME);
        }
        for j in 0..h + 2 * d {
            sheet.put(x - d, y + j - d, LOOK_FRAME);
            sheet.put(x + w + d - 1, y + j - d, LOOK_FRAME);
        }
    }
}

/// A field rate as `60.05Hz`.
fn hz_text(mhz: u64) -> String {
    let text = milli_text(mhz);
    format!("{}Hz", &text[..text.len().saturating_sub(1)])
}

/// The labels as JSON, for the writer.
pub fn labels_json(labels: &[Label]) -> Json {
    Json::Array(
        labels
            .iter()
            .map(|l| {
                object([
                    ("x", l.x.into()),
                    ("y", l.y.into()),
                    ("text", (&l.text).into()),
                    ("color", l.color.to_vec().into()),
                ])
            })
            .collect(),
    )
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
            glass_aspect_error_permille: 0,
            uneven_lines: false,
            notice_cropped_lines: 0,
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
            class: "crt".into(),
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
        p.uneven_lines = true;
        p.dest.x = 10;
        p.speed_alert = true;
        let r = row("2x", Some(p));
        assert_eq!(
            r.flags,
            [
                Flag::Cropped { noticed: false },
                Flag::UnevenLines,
                Flag::NotWhole,
                Flag::Aspect,
                Flag::OffCentre,
                Flag::Small { stated: false },
                Flag::SpeedAlert,
            ]
        );
        assert!(r.look());
        assert_eq!(r.flag_names(Severity::Note, " "), "speed_alert");
        assert_eq!(row("auto", None).flags, [Flag::NoPicture]);
    }

    #[test]
    fn a_crop_the_notice_names_and_a_small_native_picture_are_notes() {
        let mut p = picture();
        p.cropped.top = 8;
        p.cropped.bottom = 8;
        p.notice_cropped_lines = 16;
        p.fill_w_permille = 500;
        p.fill_h_permille = 600;
        let r = row("native", Some(p.clone()));
        assert_eq!(
            r.flags,
            [
                Flag::Cropped { noticed: true },
                Flag::Small { stated: true }
            ]
        );
        assert!(!r.look());
        p.notices = vec!["padded_fill=600permille".into()];
        assert!(!row("auto", Some(p)).look(), "the notice names it");
    }

    #[test]
    fn uneven_lines_count_only_on_a_progressive_tube_mode() {
        let mut p = picture();
        p.uneven_lines = true;
        assert_eq!(row("auto", Some(p.clone())).flags, [Flag::UnevenLines]);
        let mut panel = row("auto", Some(p.clone()));
        panel.measures.class = "panel".into();
        assert!(own_flags("auto", &panel.measures).is_empty());
        p.mode.interlace = true;
        assert!(row("auto", Some(p)).flags.is_empty());
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
    fn fill_no_larger_than_native_and_twin_modes_are_flagged() {
        let mut small = picture();
        small.fill_w_permille = 600;
        small.dest.w = 192;
        small.dest.x = 64;
        let mut rows = vec![
            row("auto", Some(picture())),
            row("fit", Some(small.clone())),
            row("native", Some(small)),
            row("2x", Some(picture())),
        ];
        rows[3].measures.offer = "declined: NotShownHere".into();
        compare_rows(&mut rows);
        assert!(
            rows[1].flags.contains(&Flag::FillNotLarger),
            "auto is larger"
        );
        assert!(rows[1].flags.contains(&Flag::SameAs("native".into())));
        assert!(rows[1].look());
        assert!(!rows[3].flags.iter().any(|f| matches!(f, Flag::SameAs(_))));
        sort_rows(&mut rows);
        assert!(rows[0].look());
        assert!(rows.last().unwrap().flags.is_empty());
        assert_eq!(counts(&rows, "d"), (4, 1, 1));
    }

    #[test]
    fn fill_equal_to_native_is_not_flagged_when_nothing_is_larger() {
        let mut rows = vec![row("fit", Some(picture())), row("native", Some(picture()))];
        compare_rows(&mut rows);
        assert_eq!(rows[0].flags, [Flag::SameAs("native".into())]);
        assert!(!rows[0].look());
    }

    #[test]
    fn the_matrix_has_every_mode_and_every_rate() {
        let m = settings_matrix();
        assert_eq!(m.len(), 11);
        assert_eq!(m[0], ("auto", "exact"));
        assert_eq!(m[10], ("auto", "force_60"));
    }

    #[test]
    fn the_sheet_has_room_for_every_tile_and_three_labels_each() {
        let mut rows: Vec<Row> = (0..5).map(|_| row("auto", Some(picture()))).collect();
        rows[1].flags.push(Flag::OffCentre);
        let glass = Rgb::new(40, 30, [1; 3]);
        let tiles: Vec<Tile<'_>> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| Tile {
                row: r,
                glass: (i != 2).then_some(&glass),
            })
            .collect();
        let (sheet, labels) = contact_sheet(&tiles, 3, (40, 30));
        assert_eq!(sheet.width, 3 * 40 + 4 * SHEET_GUTTER_PX);
        assert_eq!(sheet.height, 2 * (30 + 38) + 3 * SHEET_GUTTER_PX);
        assert_eq!(sheet.get(SHEET_GUTTER_PX, SHEET_GUTTER_PX), [1; 3]);
        assert_eq!(
            sheet.get(2 * SHEET_GUTTER_PX + 40 - 1, SHEET_GUTTER_PX - 1),
            LOOK_FRAME,
            "the tile to look at has a frame"
        );
        assert_eq!(labels.len(), 15);
        assert_eq!(labels[5].text, "off_ce", "40 pixels hold 6 characters");
        assert_eq!(labels[5].color, LOOK_COLOR);
        assert_eq!(labels_json(&labels).as_array().unwrap().len(), 15);
        let md = markdown(&rows, &["d".into()]);
        assert!(md.contains("| d | 5 | 1 | 0 |"), "{md}");
        assert_eq!(to_json(&rows).as_array().unwrap().len(), 5);
    }
}
