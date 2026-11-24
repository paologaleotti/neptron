//! The command line of the screen simulator: one case, the survey and the
//! goldens. It writes PPM and JSON files under `target/screen/`; `cargo
//! xtask screen` turns the PPM files into PNG files and draws the labels
//! of the contact sheets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::case::{DisplaySpec, REFERENCE_NAMES, Settings, Source};
use super::glass::GLASS_SIZE;
use super::image::{Rgb, Turn};
use super::measure::Measures;
use super::run::{Case, GlassOptions, RunError, Tables, pattern_of, simulate, survey_rows};
use super::survey::{self, Tile};
use crate::golden::{Golden, GoldenError, decode_ppm, encode_ppm, workspace_root};
use crate::{ImageError, RgbImage};

/// The output directory, relative to the workspace root.
pub const OUT_DIR: &str = "target/screen";
/// The golden images of the reference cases, relative to the workspace
/// root.
pub const GOLDEN_DIR: &str = "crates/neptron-testkit/tests/golden/screen";
/// The glass size of a golden: small, to keep the repository small.
pub const GOLDEN_SIZE: (u32, u32) = (240, 180);
/// The box of each glass picture on a contact sheet of the survey, in
/// pixels: 32 characters of the label font across.
pub const SURVEY_TILE: (u32, u32) = (192, 144);
/// The size of the glass of a row to look at, in pixels.
pub const LOOK_SIZE: (u32, u32) = (720, 540);
/// The size of each 1:1 crop of `--exact` on a panel, in pixels.
const EXACT_CROP: (u32, u32) = (480, 270);

/// The reference cases with a golden glass: name, display, source and
/// settings.
pub const GOLDEN_CASES: [(&str, &str, &str, &[&str]); 6] = [
    ("snes-generic_15-auto", "generic_15", "snes", &[]),
    ("rtype-ntsc-auto", "ntsc", "rtype", &[]),
    ("psx-pc_31_120-auto", "pc_31_120", "psx", &[]),
    ("psx_480i-ntsc-auto", "ntsc", "psx_480i", &[]),
    ("ddonpach-generic_15-auto", "generic_15", "ddonpach", &[]),
    ("snes-panel_1080-auto", "1920x1080@60", "snes", &[]),
];

const USAGE: &str = "\
usage: screen --display D --source S [--gpu ID] [--set KEY=VALUE]... [--aspect A]
              [--rotate 0|90|180|270] [--interlaced] [--arcade] [--frame FILE.ppm]
              [--overscan PERCENT] [--size WxH] [--turn 0|90|180|270] [--exact] [--out NAME]
       screen survey [--display D]... [--source S]...
       screen golden [--bless]";

/// An error of the command line.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CliError {
    /// The arguments are wrong.
    #[error("{0}\n{USAGE}")]
    Usage(String),
    /// A case does not run.
    #[error(transparent)]
    Run(#[from] RunError),
    /// The frame file is not a PPM of the source size.
    #[error("{path}: {reason}")]
    Frame {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// A picture has a size that does not match its pixels.
    #[error(transparent)]
    Image(#[from] ImageError),
    /// A file cannot be written.
    #[error("{path}: {source}")]
    Write {
        /// The file.
        path: PathBuf,
        /// The error of the operating system.
        source: std::io::Error,
    },
}

/// Runs the command line `args` (the arguments after the program name).
///
/// # Errors
///
/// [`CliError`] for bad arguments, a case that does not run, or a file
/// that cannot be written. A golden that differs is no error: the exit
/// code says it.
pub fn run(args: &[&str]) -> Result<ExitCode, CliError> {
    let root = workspace_root();
    let tables = Tables::load(&root)?;
    match args.split_first() {
        Some((&"survey", rest)) => run_survey(&root, &tables, rest),
        Some((&"golden", rest)) => run_golden(&root, &tables, rest),
        _ => run_case(&root, &tables, args),
    }
}

fn run_case(root: &Path, tables: &Tables, args: &[&str]) -> Result<ExitCode, CliError> {
    let parsed = Args::parse(args)?;
    let case = parsed.case(tables)?;
    let frame = match &parsed.frame {
        Some(path) => read_frame(path)?,
        None => pattern_of(&case.source),
    };
    let outcome = simulate(&case, &frame, &parsed.glass)?;
    let name = parsed.out.clone().unwrap_or_else(|| case_dir_name(&case));
    let dir = root.join(OUT_DIR).join(name);
    write_ppm(&dir.join("frame.ppm"), &frame)?;
    if let Some(shown) = &outcome.wire.shown {
        write_ppm(&dir.join("wire.ppm"), &shown.frame_buffer)?;
        if parsed.exact {
            let fb = &shown.frame_buffer;
            let (cw, ch) = EXACT_CROP;
            let dest = shown.scene.dest;
            let corner = fb.crop(dest.x.max(0) as u32, dest.y.max(0) as u32, cw, ch);
            let centre = fb.crop(
                (fb.width / 2).saturating_sub(cw / 2),
                (fb.height / 2).saturating_sub(ch / 2),
                cw,
                ch,
            );
            write_ppm(&dir.join("exact-corner.ppm"), &corner)?;
            write_ppm(&dir.join("exact-centre.ppm"), &centre)?;
        }
    }
    if let Some(image) = &outcome.glass_image {
        write_ppm(&dir.join("glass.ppm"), image)?;
    }
    let json = outcome.measures.to_json().pretty();
    write(&dir.join("measures.json"), json.as_bytes())?;
    println!("{}", Measures::HEADER);
    println!("{}", outcome.measures.row());
    println!("{}", dir.display());
    Ok(ExitCode::SUCCESS)
}

/// The survey: every display, source and setting of the matrix. Writes
/// `survey.md`, `survey.json`, one contact sheet per display with its
/// labels, and the glass of each row to look at under `look/<display>/`.
fn run_survey(root: &Path, tables: &Tables, args: &[&str]) -> Result<ExitCode, CliError> {
    let (displays, sources) = survey_lists(args)?;
    let dir = root.join(OUT_DIR).join("survey");
    let look_dir = dir.join("look");
    if look_dir.exists() {
        // The rows to look at change from run to run; old pictures would
        // mislead. The directory is in target/ and holds no user data.
        fs::remove_dir_all(&look_dir).map_err(|source| CliError::Write {
            path: look_dir.clone(),
            source,
        })?;
    }
    let (mut rows, tiles, names) = survey_rows(tables, &displays, &sources, SURVEY_TILE, |d| {
        eprintln!("screen: survey {d} done");
    })?;
    let columns = survey::settings_matrix().len();
    for name in &names {
        let sheet_tiles: Vec<Tile<'_>> = rows
            .iter()
            .zip(&tiles)
            .filter(|(r, _)| &r.measures.display == name)
            .map(|(row, glass)| Tile {
                row,
                glass: glass.as_ref(),
            })
            .collect();
        let (sheet, labels) = survey::contact_sheet(&sheet_tiles, columns, SURVEY_TILE);
        let stem = file_stem(name);
        write_ppm(&dir.join(format!("{stem}.ppm")), &sheet)?;
        let json = survey::labels_json(&labels).compact();
        write(&dir.join(format!("{stem}.labels.json")), json.as_bytes())?;
    }
    let options = GlassOptions {
        overscan_permille: None,
        size: LOOK_SIZE,
        turn: Turn::R0,
    };
    for row in rows.iter().filter(|r| r.look()) {
        let m = &row.measures;
        let style = format!("display.style={}", row.style);
        let refresh = format!("display.refresh_policy={}", row.refresh);
        let case = tables.named_case(&m.display, &m.source, &[&style, &refresh])?;
        let outcome = simulate(&case, &pattern_of(&case.source), &options)?;
        if let Some(glass) = &outcome.glass_image {
            let file = format!("{}-{}-{}.ppm", m.source, row.style, row.refresh);
            write_ppm(&look_dir.join(file_stem(&m.display)).join(file), glass)?;
        }
    }
    survey::sort_rows(&mut rows);
    write(
        &dir.join("survey.md"),
        survey::markdown(&rows, &names).as_bytes(),
    )?;
    let json = survey::to_json(&rows).pretty();
    write(&dir.join("survey.json"), json.as_bytes())?;
    for name in &names {
        let c = survey::counts(&rows, name);
        println!(
            "{name}: {} rows, {} to look at ({} pictures), {} with notes only",
            c.rows, c.look_rows, c.look_pictures, c.note_rows
        );
    }
    println!("{}", dir.join("survey.md").display());
    Ok(ExitCode::SUCCESS)
}

/// The goldens: compares the glass of each reference case with its golden,
/// or with `--bless` writes it. The glass of each case is also written to
/// `target/screen/golden/` to look at before a bless.
fn run_golden(root: &Path, tables: &Tables, args: &[&str]) -> Result<ExitCode, CliError> {
    let bless = match args {
        [] => false,
        ["--bless"] => true,
        _ => return Err(CliError::Usage("screen golden [--bless]".into())),
    };
    let golden = Golden::new(
        root.join(GOLDEN_DIR),
        root.join(OUT_DIR).join("golden-actual"),
    );
    let mut failed = 0;
    for (i, (name, ..)) in GOLDEN_CASES.iter().enumerate() {
        let (image, _) = golden_glass(tables, i)?;
        write_ppm(
            &root
                .join(OUT_DIR)
                .join("golden")
                .join(format!("{name}.ppm")),
            &image,
        )?;
        let rgb = to_rgb_image(&image)?;
        match golden.check(name, &rgb) {
            Ok(()) => println!("same     {name}"),
            Err(_) if bless => {
                write(
                    &root.join(GOLDEN_DIR).join(format!("{name}.ppm")),
                    &encode_ppm(&rgb),
                )?;
                println!("blessed  {name}");
            }
            Err(GoldenError::Missing { .. } | GoldenError::Mismatch { .. }) => {
                failed += 1;
                println!("FAILED   {name}");
            }
            Err(other) => {
                failed += 1;
                println!("FAILED   {name}: {other}");
            }
        }
    }
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// The glass of golden case `index`, at [`GOLDEN_SIZE`], and its measures.
///
/// # Errors
///
/// [`RunError`] when the case does not run.
pub fn golden_glass(tables: &Tables, index: usize) -> Result<(Rgb, Measures), RunError> {
    let (_, display, source, sets) = GOLDEN_CASES[index % GOLDEN_CASES.len()];
    let case = tables.named_case(display, source, sets)?;
    let options = GlassOptions {
        overscan_permille: None,
        size: GOLDEN_SIZE,
        turn: Turn::R0,
    };
    let outcome = simulate(&case, &pattern_of(&case.source), &options)?;
    let image = outcome
        .glass_image
        .unwrap_or_else(|| Rgb::new(GOLDEN_SIZE.0, GOLDEN_SIZE.1, [0; 3]));
    Ok((image, outcome.measures))
}

fn to_rgb_image(image: &Rgb) -> Result<RgbImage, ImageError> {
    RgbImage::from_rgb(image.width, image.height, image.rgb.clone())
}

/// The displays and the sources of a survey: the defaults, or the ones
/// given with `--display` and `--source`.
fn survey_lists(args: &[&str]) -> Result<(Vec<String>, Vec<String>), CliError> {
    let mut displays = Vec::new();
    let mut sources = Vec::new();
    let mut it = args.iter();
    while let Some(&arg) = it.next() {
        let list = match arg {
            "--display" => &mut displays,
            "--source" => &mut sources,
            other => return Err(CliError::Usage(format!("unknown argument {other:?}"))),
        };
        let value = it
            .next()
            .ok_or_else(|| CliError::Usage(format!("{arg} needs a value")))?;
        list.push((*value).to_owned());
    }
    if displays.is_empty() {
        displays = survey::DISPLAYS.iter().map(|d| (*d).to_owned()).collect();
    }
    if sources.is_empty() {
        sources = REFERENCE_NAMES
            .iter()
            .chain(&survey::EXTRA_SOURCES)
            .map(|s| (*s).to_owned())
            .collect();
    }
    Ok((displays, sources))
}

/// A file name for a display name: `1920x1080@60` is `1920x1080_60`.
fn file_stem(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The directory name of a case: display, source and settings, with only
/// `[a-z0-9_.-]`.
fn case_dir_name(case: &Case) -> String {
    let raw = format!("{}-{}-{}", case.screen.name, case.source.name, case.label);
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn read_frame(path: &Path) -> Result<Rgb, CliError> {
    let bad = |reason: String| CliError::Frame {
        path: path.to_path_buf(),
        reason,
    };
    let bytes = fs::read(path).map_err(|e| bad(e.to_string()))?;
    let image = decode_ppm(&bytes).map_err(|e| bad(e.to_string()))?;
    Rgb::from_rgb(image.width(), image.height(), image.bytes().to_vec())
        .ok_or_else(|| bad("the pixel data does not match the size".into()))
}

fn write_ppm(path: &Path, image: &Rgb) -> Result<(), CliError> {
    write(path, &encode_ppm(&to_rgb_image(image)?))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let err = |source| CliError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(err)?;
    }
    fs::write(path, bytes).map_err(err)
}

/// The arguments of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    display: Option<String>,
    source: Option<String>,
    gpu: Option<String>,
    sets: Vec<String>,
    aspect: Option<String>,
    rotate: Option<String>,
    interlaced: bool,
    arcade: bool,
    frame: Option<PathBuf>,
    glass: GlassOptions,
    exact: bool,
    out: Option<String>,
}

impl Args {
    fn parse(args: &[&str]) -> Result<Self, CliError> {
        let mut out = Self {
            display: None,
            source: None,
            gpu: None,
            sets: Vec::new(),
            aspect: None,
            rotate: None,
            interlaced: false,
            arcade: false,
            frame: None,
            glass: GlassOptions {
                overscan_permille: None,
                size: GLASS_SIZE,
                turn: Turn::R0,
            },
            exact: false,
            out: None,
        };
        let mut it = args.iter();
        while let Some(&arg) = it.next() {
            let mut value = || {
                it.next()
                    .map(|v| (*v).to_owned())
                    .ok_or_else(|| CliError::Usage(format!("{arg} needs a value")))
            };
            match arg {
                "--display" => out.display = Some(value()?),
                "--source" => out.source = Some(value()?),
                "--gpu" => out.gpu = Some(value()?),
                "--set" => out.sets.push(value()?),
                "--aspect" => out.aspect = Some(value()?),
                "--rotate" => out.rotate = Some(value()?),
                "--interlaced" => out.interlaced = true,
                "--arcade" => out.arcade = true,
                "--frame" => out.frame = Some(PathBuf::from(value()?)),
                "--overscan" => {
                    let v = value()?;
                    let percent: f64 = v
                        .parse()
                        .ok()
                        .filter(|p: &f64| (0.0..50.0).contains(p))
                        .ok_or_else(|| CliError::Usage(format!("--overscan {v}: 0 to 49.9")))?;
                    out.glass.overscan_permille = Some((percent * 10.0).round() as u32);
                }
                "--size" => out.glass.size = parse_size(&value()?)?,
                "--turn" => {
                    let v = value()?;
                    out.glass.turn =
                        v.parse().ok().and_then(Turn::from_degrees).ok_or_else(|| {
                            CliError::Usage(format!("--turn {v}: 0, 90, 180 or 270"))
                        })?;
                }
                "--exact" => out.exact = true,
                "--out" => out.out = Some(value()?),
                other => return Err(CliError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        Ok(out)
    }

    fn case(&self, tables: &Tables) -> Result<Case, CliError> {
        let display = self
            .display
            .as_deref()
            .ok_or_else(|| CliError::Usage("--display is missing".into()))?;
        let source = self
            .source
            .as_deref()
            .ok_or_else(|| CliError::Usage("--source is missing".into()))?;
        let spec = DisplaySpec::parse(display).map_err(RunError::from)?;
        let screen = tables.screen(&spec, self.gpu.as_deref())?;
        let source = if source.contains('@') {
            Source::by_hand(
                source,
                self.aspect.as_deref(),
                self.rotate.as_deref(),
                self.interlaced,
                self.arcade,
            )
            .map_err(RunError::from)?
        } else {
            let mut named = Source::named(source, &tables.boards).map_err(RunError::from)?;
            named.arcade |= self.arcade;
            named
        };
        let defaults = Settings::defaults(&tables.schema);
        let mut settings = defaults.clone();
        for set in &self.sets {
            settings.set(set).map_err(RunError::from)?;
        }
        Ok(Case {
            screen,
            source,
            label: settings.label(&defaults),
            settings,
        })
    }
}

fn parse_size(text: &str) -> Result<(u32, u32), CliError> {
    text.split_once('x')
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .filter(|(w, h): &(u32, u32)| *w > 0 && *h > 0 && *w <= 8_192 && *h <= 8_192)
        .ok_or_else(|| CliError::Usage(format!("--size {text}: WIDTHxHEIGHT")))
}

#[cfg(test)]
mod tests {
    use neptron_types::Rect;

    use super::super::measure::Edges;
    use super::super::survey::Flag;
    use super::*;

    #[test]
    fn the_arguments_parse() {
        let a = Args::parse(&[
            "--display",
            "ntsc",
            "--source",
            "rtype",
            "--set",
            "style=fit",
            "--overscan",
            "7.5",
            "--turn",
            "90",
            "--size",
            "360x270",
        ])
        .unwrap();
        assert_eq!(a.display.as_deref(), Some("ntsc"));
        assert_eq!(a.sets, ["style=fit"]);
        assert_eq!(a.glass.overscan_permille, Some(75));
        assert_eq!(a.glass.turn, Turn::R90);
        assert_eq!(a.glass.size, (360, 270));
        for bad in [
            &["--turn", "45"][..],
            &["--size", "0x3"],
            &["--overscan", "60"],
            &["--bogus"],
            &["--display"],
        ] {
            assert!(
                matches!(Args::parse(bad), Err(CliError::Usage(_))),
                "{bad:?}"
            );
        }
        assert!(matches!(
            survey_lists(&["--bogus"]),
            Err(CliError::Usage(_))
        ));
        assert_eq!(file_stem("1920x1080@60"), "1920x1080_60");
    }

    /// The expected measures of a golden case.
    struct Expected {
        mode: (u16, u16),
        dest: (i32, i32, u32, u32),
        cropped: (u32, u32, u32, u32),
        fill: (u32, u32),
        aspect_error_permille: i64,
        uneven: bool,
    }

    const fn e(
        mode: (u16, u16),
        dest: (i32, i32, u32, u32),
        cropped: (u32, u32, u32, u32),
        fill: (u32, u32),
        aspect_error_permille: i64,
        uneven: bool,
    ) -> Expected {
        Expected {
            mode,
            dest,
            cropped,
            fill,
            aspect_error_permille,
            uneven,
        }
    }

    /// The measures of [`GOLDEN_CASES`], in its order, computed by hand
    /// from the placement and the glass model, then read back.
    const EXPECTED: [Expected; 6] = [
        // 256 at 5x fills the 1280 line, 224 of the 240 lines of the
        // window: 93.2 % high. The placement fills the mode: no error.
        e(
            (1280, 224),
            (0, 0, 1280, 224),
            (0, 0, 0, 0),
            (1000, 932),
            0,
            false,
        ),
        // 256 lines in 240p: 8 cut at each edge; 704 samples for a frame
        // that asks 750: 6.2 % narrow.
        e(
            (720, 240),
            (8, 0, 704, 240),
            (8, 8, 0, 0),
            (1000, 1000),
            -62,
            false,
        ),
        // 2x at 31 kHz, 480 lines of the 507 of the window.
        e(
            (640, 480),
            (0, 0, 640, 480),
            (0, 0, 0, 0),
            (1000, 947),
            0,
            false,
        ),
        // 480i 1:1, 640 samples: a console picture against the console
        // width of 644 samples, 0.6 % narrow.
        e(
            (720, 480),
            (40, 0, 640, 480),
            (0, 0, 0, 0),
            (1000, 1000),
            -6,
            false,
        ),
        // A vertical board with no interlace on the PC: 320 lines fitted
        // into 248p at 0.775, uneven.
        e(
            (1440, 248),
            (315, 0, 810, 248),
            (0, 0, 0, 0),
            (562, 992),
            0,
            true,
        ),
        // The panel fills its height at 4:3, 1080 over 224 lines.
        e(
            (1920, 1080),
            (240, 0, 1440, 1080),
            (0, 0, 0, 0),
            (750, 1000),
            0,
            true,
        ),
    ];

    #[test]
    fn the_reference_cases_match_their_goldens_and_measures() {
        let root = workspace_root();
        let tables = Tables::load(&root).unwrap();
        let golden = Golden::new(
            root.join(GOLDEN_DIR),
            root.join(OUT_DIR).join("golden-actual"),
        );
        for (i, expected) in EXPECTED.iter().enumerate() {
            let name = GOLDEN_CASES[i].0;
            let (image, m) = golden_glass(&tables, i).unwrap();
            golden
                .check(name, &to_rgb_image(&image).unwrap())
                .unwrap_or_else(|e| panic!("{e}"));
            let p = m
                .picture
                .as_ref()
                .unwrap_or_else(|| panic!("{name}: no picture"));
            assert_eq!((p.mode.hactive, p.mode.vactive), expected.mode, "{name}");
            let (x, y, w, h) = expected.dest;
            assert_eq!(p.dest, Rect { x, y, w, h }, "{name}");
            let (top, bottom, left, right) = expected.cropped;
            assert_eq!(
                p.cropped,
                Edges {
                    top,
                    bottom,
                    left,
                    right
                },
                "{name}"
            );
            assert_eq!(
                (p.fill_w_permille, p.fill_h_permille),
                expected.fill,
                "{name}"
            );
            assert_eq!(
                p.aspect_error_permille, expected.aspect_error_permille,
                "{name}"
            );
            assert_eq!(p.uneven_lines, expected.uneven, "{name}");
        }
    }

    #[test]
    fn rtype_on_ntsc_crops_16_lines_with_a_notice_and_fill_changes_nothing() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let (_, auto) = golden_glass(&tables, 1).unwrap();
        let p = auto.picture.clone().unwrap();
        let hidden = p.overscan_hidden.unwrap();
        assert_eq!((hidden.top, hidden.bottom), (12, 12));
        assert_eq!(
            (hidden.left, hidden.right),
            (19, 19),
            "35.2 of 704 samples, 384 wide"
        );
        assert_eq!(p.notices, ["speed_change", "cropped_lines=16"]);
        assert_eq!(p.notice_cropped_lines, 16);
        assert!(p.speed_alert);
        let fill = tables.named_case("ntsc", "rtype", &["style=fit"]).unwrap();
        let options = GlassOptions {
            overscan_permille: None,
            size: GOLDEN_SIZE,
            turn: Turn::R0,
        };
        let out = simulate(&fill, &pattern_of(&fill.source), &options).unwrap();
        assert_eq!(out.measures.picture, auto.picture);
    }

    #[test]
    fn a_fitted_272_line_board_on_ntsc_has_uneven_lines() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let screen = tables
            .screen(&DisplaySpec::parse("ntsc").unwrap(), None)
            .unwrap();
        let source = Source::by_hand("384x272@55", None, None, false, true).unwrap();
        let defaults = Settings::defaults(&tables.schema);
        let mut settings = defaults.clone();
        settings.set("style=fit").unwrap();
        let case = Case {
            screen,
            label: settings.label(&defaults),
            source,
            settings,
        };
        let options = GlassOptions {
            overscan_permille: None,
            size: GOLDEN_SIZE,
            turn: Turn::R0,
        };
        let out = simulate(&case, &pattern_of(&case.source), &options).unwrap();
        let p = out.measures.picture.as_ref().unwrap();
        assert_eq!(p.scale_y_milli, 882, "240 lines for 272");
        assert!(p.uneven_lines);
        let flags = survey::own_flags("fit", &out.measures);
        assert!(flags.contains(&Flag::UnevenLines), "{flags:?}");
    }

    #[test]
    fn the_survey_runs_and_notes_the_crop_of_rtype_on_a_tv() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let (rows, tiles, names) = survey_rows(
            &tables,
            &["ntsc".into()],
            &["rtype".into()],
            (64, 48),
            |_| {},
        )
        .unwrap();
        assert_eq!(names, ["ntsc"]);
        assert_eq!(rows.len(), survey::settings_matrix().len());
        assert_eq!(tiles.len(), rows.len());
        let auto = &rows[0];
        assert_eq!(
            (auto.style.as_str(), auto.refresh.as_str()),
            ("auto", "exact")
        );
        assert!(auto.flags.contains(&Flag::Cropped { noticed: true }));
        assert!(auto.flags.contains(&Flag::Aspect), "{:?}", auto.flags);
        assert!(auto.look(), "the 704-sample clamp is a finding");
        let fit = rows.iter().find(|r| r.style == "fit").unwrap();
        assert!(fit.flags.contains(&Flag::SameAs("native".into())));
        assert!(
            !fit.flags.contains(&Flag::FillNotLarger),
            "nothing is larger"
        );
    }
}
