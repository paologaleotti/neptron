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
pub const GOLDEN_CASES: [(&str, &str, &str, &[&str]); 10] = [
    ("snes-generic_15-auto", "generic_15", "snes", &[]),
    ("rtype-ntsc-auto", "ntsc", "rtype", &[]),
    ("psx-pc_31_120-auto", "pc_31_120", "psx", &[]),
    ("psx_480i-ntsc-auto", "ntsc", "psx_480i", &[]),
    ("ddonpach-generic_15-auto", "generic_15", "ddonpach", &[]),
    ("snes-panel_1080-auto", "1920x1080@60", "snes", &[]),
    ("snes-ntsc-fit", "ntsc", "snes", &["style=fit"]),
    (
        "ddonpach-pc_31_120-fit",
        "pc_31_120",
        "ddonpach",
        &["style=fit"],
    ),
    ("rtype-ntsc-fit", "ntsc", "rtype", &["style=fit"]),
    ("rtype-pal-fit", "pal", "rtype", &["style=fit"]),
];

/// The reference cases of TATE mode with a golden glass: DoDonPachi (a
/// vertical board) and Power Instinct 2 (a horizontal board of the same
/// size and rate) on a 15 kHz arcade tube, a PC monitor, an NTSC TV and a
/// 1080p panel, with the screen at 0 and at 90. Name, display, source and
/// settings, as [`GOLDEN_CASES`].
pub const TATE_CASES: [(&str, &str, &str, &[&str]); 16] = [
    (
        "ddonpach-arcade_15-o0",
        "arcade_15",
        "ddonpach",
        &["orientation=0"],
    ),
    (
        "ddonpach-arcade_15-o90",
        "arcade_15",
        "ddonpach",
        &["orientation=90"],
    ),
    (
        "pwrinst2-arcade_15-o0",
        "arcade_15",
        "pwrinst2",
        &["orientation=0"],
    ),
    (
        "pwrinst2-arcade_15-o90",
        "arcade_15",
        "pwrinst2",
        &["orientation=90"],
    ),
    (
        "ddonpach-pc_31_120-o0",
        "pc_31_120",
        "ddonpach",
        &["orientation=0"],
    ),
    (
        "ddonpach-pc_31_120-o90",
        "pc_31_120",
        "ddonpach",
        &["orientation=90"],
    ),
    (
        "pwrinst2-pc_31_120-o0",
        "pc_31_120",
        "pwrinst2",
        &["orientation=0"],
    ),
    (
        "pwrinst2-pc_31_120-o90",
        "pc_31_120",
        "pwrinst2",
        &["orientation=90"],
    ),
    ("ddonpach-ntsc-o0", "ntsc", "ddonpach", &["orientation=0"]),
    ("ddonpach-ntsc-o90", "ntsc", "ddonpach", &["orientation=90"]),
    ("pwrinst2-ntsc-o0", "ntsc", "pwrinst2", &["orientation=0"]),
    ("pwrinst2-ntsc-o90", "ntsc", "pwrinst2", &["orientation=90"]),
    (
        "ddonpach-panel_1080-o0",
        "1920x1080@60",
        "ddonpach",
        &["orientation=0"],
    ),
    (
        "ddonpach-panel_1080-o90",
        "1920x1080@60",
        "ddonpach",
        &["orientation=90"],
    ),
    (
        "pwrinst2-panel_1080-o0",
        "1920x1080@60",
        "pwrinst2",
        &["orientation=0"],
    ),
    (
        "pwrinst2-panel_1080-o90",
        "1920x1080@60",
        "pwrinst2",
        &["orientation=90"],
    ),
];

/// The reference cases of 480i and 480p systems with a golden glass
/// (`docs/spec/interlaced-systems.md`): the PlayStation at 240p and 480i
/// and a VGA frame on a 15 kHz CRT with the bench PC (no interlace) and
/// with the Raspberry Pi 5 (interlace), on a 15 kHz arcade tube, a PC
/// monitor, an NTSC and a PAL TV and a 1080p panel. Name, display, GPU
/// entry (`None` for the default of the display), source and settings.
pub const INTERLACED_CASES: [GoldenCase; 14] = [
    ("ps1_320-generic_15", "generic_15", None, "ps1_320", &[]),
    ("ps1_512-generic_15", "generic_15", None, "ps1_512", &[]),
    ("ps1_480i-generic_15", "generic_15", None, "ps1_480i", &[]),
    (
        "ps1_480i-generic_15-pi5",
        "generic_15",
        Some("pi5_rp1_dpi"),
        "ps1_480i",
        &[],
    ),
    ("ps1_320-arcade_15", "arcade_15", None, "ps1_320", &[]),
    ("ps1_480i-arcade_15", "arcade_15", None, "ps1_480i", &[]),
    ("ps1_320-pc_31_120", "pc_31_120", None, "ps1_320", &[]),
    ("ps1_480i-pc_31_120", "pc_31_120", None, "ps1_480i", &[]),
    ("vga_480p-pc_31_120", "pc_31_120", None, "vga_480p", &[]),
    ("ps1_320-ntsc", "ntsc", None, "ps1_320", &[]),
    ("ps1_480i-ntsc", "ntsc", None, "ps1_480i", &[]),
    ("ps1_320-pal", "pal", None, "ps1_320", &[]),
    ("ps1_480i-pal", "pal", None, "ps1_480i", &[]),
    ("ps1_480i-panel_1080", "1920x1080@60", None, "ps1_480i", &[]),
];

/// The reference cases of a width change in play with a golden glass
/// (`docs/spec/interlaced-systems.md`, section 3): the SNES at 256 and 512
/// and the Mega Drive at 320 and 256 pixels, with the widths of their
/// system data, on two 15 kHz tubes with the bench PC and on an NTSC and a
/// PAL TV. Name, display, GPU entry, source and settings.
pub const WIDTH_CASES: [GoldenCase; 16] = [
    ("snes_256-generic_15", "generic_15", None, "snes_256", &[]),
    ("snes_512-generic_15", "generic_15", None, "snes_512", &[]),
    ("md_320-generic_15", "generic_15", None, "md_320", &[]),
    ("md_256-generic_15", "generic_15", None, "md_256", &[]),
    ("snes_256-arcade_15", "arcade_15", None, "snes_256", &[]),
    ("snes_512-arcade_15", "arcade_15", None, "snes_512", &[]),
    ("md_320-arcade_15", "arcade_15", None, "md_320", &[]),
    ("md_256-arcade_15", "arcade_15", None, "md_256", &[]),
    ("snes_256-ntsc", "ntsc", None, "snes_256", &[]),
    ("snes_512-ntsc", "ntsc", None, "snes_512", &[]),
    ("md_320-ntsc", "ntsc", None, "md_320", &[]),
    ("md_256-ntsc", "ntsc", None, "md_256", &[]),
    ("snes_256-pal", "pal", None, "snes_256", &[]),
    ("snes_512-pal", "pal", None, "snes_512", &[]),
    ("md_320-pal", "pal", None, "md_320", &[]),
    ("md_256-pal", "pal", None, "md_256", &[]),
];

/// One golden case: name, display, GPU entry (`None` for the default of
/// the display), source and settings.
pub type GoldenCase = (
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static str,
    &'static [&'static str],
);

/// The case of golden `index`: [`GOLDEN_CASES`], then [`TATE_CASES`],
/// then [`INTERLACED_CASES`], then [`WIDTH_CASES`].
fn golden_case(index: usize) -> GoldenCase {
    let mut i = index % GOLDEN_COUNT;
    for list in [&GOLDEN_CASES[..], &TATE_CASES[..]] {
        if let Some(&(name, display, source, sets)) = list.get(i) {
            return (name, display, None, source, sets);
        }
        i -= list.len();
    }
    match INTERLACED_CASES.get(i) {
        Some(case) => *case,
        None => WIDTH_CASES[i - INTERLACED_CASES.len()],
    }
}

/// The number of goldens: [`GOLDEN_CASES`], [`TATE_CASES`],
/// [`INTERLACED_CASES`] and [`WIDTH_CASES`].
pub const GOLDEN_COUNT: usize =
    GOLDEN_CASES.len() + TATE_CASES.len() + INTERLACED_CASES.len() + WIDTH_CASES.len();

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
    for i in 0..GOLDEN_COUNT {
        let (name, ..) = golden_case(i);
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

/// The glass of golden case `index` (see [`golden_case`]), at
/// [`GOLDEN_SIZE`] turned as the player sees it, and its measures.
///
/// # Errors
///
/// [`RunError`] when the case does not run.
pub fn golden_glass(tables: &Tables, index: usize) -> Result<(Rgb, Measures), RunError> {
    let (_, display, gpu, source, sets) = golden_case(index);
    let case = tables.named_case_on(display, gpu, source, sets)?;
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
    const EXPECTED: [Expected; 10] = [
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
        // Fit: the SNES fills 240 lines at the console shape, 690 samples.
        e(
            (720, 240),
            (15, 0, 690, 240),
            (0, 0, 0, 0),
            (1000, 1000),
            0,
            true,
        ),
        // Fit: DoDonPachi fills the 512 lines at 1.6, 3:4 on the face.
        e(
            (854, 512),
            (187, 0, 480, 512),
            (0, 0, 0, 0),
            (562, 970),
            -1,
            true,
        ),
        // Fit: R-Type shows its 256 lines in 240 at 704 samples.
        e(
            (720, 240),
            (8, 0, 704, 240),
            (0, 0, 0, 0),
            (1000, 1000),
            0,
            true,
        ),
        // Fit: R-Type fills 288 lines on PAL at 704 samples.
        e(
            (720, 288),
            (8, 0, 704, 288),
            (0, 0, 0, 0),
            (1000, 1000),
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

    /// The Fit cases of the golden list: the whole picture, every line on
    /// the screen, the shape within 1 %, smoothed.
    #[test]
    fn fit_shows_every_line_at_its_shape() {
        let tables = Tables::load(&workspace_root()).unwrap();
        for (i, (name, _, _, sets)) in GOLDEN_CASES.iter().enumerate() {
            if !sets.contains(&"style=fit") {
                continue;
            }
            let (_, m) = golden_glass(&tables, i).unwrap();
            let p = m.picture.clone().unwrap();
            assert_eq!(p.cropped, Edges::default(), "{name}");
            assert_eq!(p.dropped_lines, 0, "{name}");
            assert!(
                p.aspect_error_permille.abs() < 10,
                "{name}: {}",
                p.aspect_error_permille
            );
            assert!(!p.mode.interlace, "{name}");
            assert_eq!(p.filter, "sharpbilinear", "{name}");
            assert_eq!(
                p.dest.h,
                u32::from(p.mode.vactive),
                "{name}: the whole height"
            );
            assert!(
                survey::own_flags("fit", &m)
                    .iter()
                    .all(|f| f.severity() == survey::Severity::Note),
                "{name}"
            );
        }
    }

    #[test]
    fn rtype_on_ntsc_crops_16_lines_in_auto_and_fit_shows_them_all() {
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
        let fit = out.measures.picture.unwrap();
        assert_eq!(fit.crop.h, 256);
        assert_eq!((fit.dest.h, fit.scale_y_milli), (240, 938));
        assert_eq!(fit.notices, ["fitted", "speed_change", "soft=FillMode"]);
    }

    #[test]
    fn a_fitted_272_line_board_on_ntsc_shows_every_line_smoothed() {
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
        assert_eq!(p.filter, "sharpbilinear");
        assert_eq!(p.dropped_lines, 0);
        assert_eq!(p.cropped, Edges::default());
        assert!(
            p.aspect_error_permille.abs() < 10,
            "{}",
            p.aspect_error_permille
        );
        let flags = survey::own_flags("fit", &out.measures);
        assert_eq!(flags, [Flag::UnevenLines { fit: true }, Flag::SpeedAlert]);
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
        assert!(!fit.flags.contains(&Flag::SameAs("native".into())));
        assert!(!fit.flags.contains(&Flag::FillNotLarger));
        assert!(!fit.look(), "{:?}", fit.flags);
    }

    /// The measures of a TATE case: the mode, `dest` in the mode, and the
    /// part of the width and the height of the screen that the picture
    /// fills for the player.
    type Tate = ((u16, u16), (i32, i32, u32, u32), (u32, u32));

    /// The measures of [`TATE_CASES`], in its order. A vertical board on a
    /// screen at 90 gets the mode of the horizontal board at 0, and the
    /// other way round: the engine plans the lines that the display scans.
    const TATE_EXPECTED: [Tate; 16] = [
        // DoDonPachi at 0: 320 lines in 256, 3:4 on the face.
        ((1440, 256), (315, 0, 810, 256), (563, 1000)),
        // At 90: its own 240 lines, the full height of the turned tube.
        ((1280, 240), (0, 0, 1280, 240), (975, 999)),
        ((1280, 240), (0, 0, 1280, 240), (999, 975)),
        ((1440, 256), (315, 0, 810, 256), (1000, 563)),
        // The PC monitor at 0: DoDonPachi at 1x, 240x320 (Auto).
        ((683, 512), (221, 96, 240, 320), (352, 606)),
        ((640, 480), (0, 0, 640, 480), (909, 1000)),
        ((640, 480), (0, 0, 640, 480), (1000, 909)),
        ((683, 512), (221, 96, 240, 320), (606, 352)),
        // NTSC: 320 lines in 480i at 0; 240p at 90, 704 samples.
        ((720, 480), (228, 80, 264, 320), (417, 741)),
        ((720, 240), (8, 0, 704, 240), (1000, 1000)),
        ((720, 240), (8, 0, 704, 240), (1000, 1000)),
        ((720, 480), (228, 80, 264, 320), (741, 417)),
        // The panel at 90: 1440x1080 on the wire, 1080x1440 for the
        // player, the full width of the turned panel.
        ((1920, 1080), (600, 60, 720, 960), (375, 889)),
        ((1920, 1080), (240, 0, 1440, 1080), (1000, 750)),
        ((1920, 1080), (240, 0, 1440, 1080), (750, 1000)),
        ((1920, 1080), (600, 60, 720, 960), (889, 375)),
    ];

    /// The largest error of the shape of the game as the player sees it,
    /// in per mille.
    const PLAYER_SHAPE_MAX_PERMILLE: i64 = 10;

    /// A game at the orientation of the screen fills one side of it.
    const ONE_SIDE_MIN_PERMILLE: u32 = 990;

    #[test]
    fn the_tate_cases_match_their_goldens_and_measures() {
        let root = workspace_root();
        let tables = Tables::load(&root).unwrap();
        let golden = Golden::new(
            root.join(GOLDEN_DIR),
            root.join(OUT_DIR).join("golden-actual"),
        );
        for (i, ((mode, dest, fill), case)) in TATE_EXPECTED.iter().zip(TATE_CASES).enumerate() {
            let name = case.0;
            let (image, m) = golden_glass(&tables, GOLDEN_CASES.len() + i).unwrap();
            golden
                .check(name, &to_rgb_image(&image).unwrap())
                .unwrap_or_else(|e| panic!("{e}"));
            let p = m.picture.as_ref().unwrap();
            let q = p.player;
            assert_eq!((p.mode.hactive, p.mode.vactive), *mode, "{name}");
            let (x, y, w, h) = *dest;
            assert_eq!(p.dest, Rect { x, y, w, h }, "{name}");
            assert_eq!((q.fill_w_permille, q.fill_h_permille), *fill, "{name}");
            assert!(
                q.aspect_error_permille.abs() <= PLAYER_SHAPE_MAX_PERMILLE,
                "{name}: {}",
                q.aspect_error_permille
            );
            assert_eq!(q.upright, Some(true), "{name}");
            let turned = q.screen_degrees == 90;
            assert_eq!(turned, name.ends_with("-o90"), "{name}");
            let vertical = case.2 == "ddonpach";
            if vertical == turned {
                assert!(
                    q.fill_w_permille.max(q.fill_h_permille) >= ONE_SIDE_MIN_PERMILLE,
                    "{name}: {fill:?}"
                );
            }
            assert_eq!(
                image.height > image.width,
                turned,
                "{name}: the glass stands as the player sees it"
            );
        }
    }

    // These values record the picture of a TV as it is on `main`: a console
    // picture keeps its width in the field, so its shape on the glass is
    // not the shape of the game. They are not a claim that the shape is
    // right; the question is with the owner. The policy measures the shape
    // on a TV against its own width rule (`aspect_error_permille`), so only
    // the glass gives a number that a change of that rule moves.
    const TV_GLASS_ASPECT_PERMILLE: [(&str, i64); 12] = [
        ("ps1_320-ntsc", -91),
        ("ps1_480i-ntsc", -91),
        ("ps1_320-pal", 91),
        ("ps1_480i-pal", 91),
        ("snes_256-ntsc", -26),
        ("snes_512-ntsc", -20),
        ("md_320-ntsc", -26),
        ("md_256-ntsc", -26),
        ("snes_256-pal", 169),
        ("snes_512-pal", 187),
        ("md_320-pal", 169),
        ("md_256-pal", 169),
    ];

    /// The glass shape error of the TV case `name`, from
    /// [`TV_GLASS_ASPECT_PERMILLE`], for a case on `ntsc` or `pal`.
    fn assert_tv_glass(name: &str, display: &str, m: &Measures) {
        if !matches!(display, "ntsc" | "pal") {
            return;
        }
        let expected = TV_GLASS_ASPECT_PERMILLE
            .iter()
            .find(|(case, _)| *case == name)
            .unwrap_or_else(|| panic!("{name}: a TV case needs its glass value"));
        let p = m.picture.as_ref().unwrap();
        assert_eq!(p.glass_aspect_error_permille, expected.1, "{name}");
    }

    /// The measures of a case of [`INTERLACED_CASES`].
    struct Interlaced {
        mode: (u16, u16, bool),
        field_rate_mhz: u64,
        dest: (i32, i32, u32, u32),
        scale_milli: (u32, u32),
        fill: (u32, u32),
        aspect_error_permille: i64,
        scanlines: &'static str,
        notices: &'static [&'static str],
    }

    /// The measures of [`INTERLACED_CASES`], in its order.
    const INTERLACED_EXPECTED: [Interlaced; 14] = [
        // The wide mode: 320 at 4 in 1280, 240 lines 1:1.
        Interlaced {
            mode: (1_280, 240, false),
            field_rate_mhz: 59_826,
            dest: (0, 0, 1_280, 240),
            scale_milli: (4_000, 1_000),
            fill: (1_000, 995),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &[],
        },
        // The same mode for 512: 2.5, sharp bilinear.
        Interlaced {
            mode: (1_280, 240, false),
            field_rate_mhz: 59_826,
            dest: (0, 0, 1_280, 240),
            scale_milli: (2_500, 1_000),
            fill: (1_000, 995),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &[],
        },
        // No interlace on the PC: 480 lines pressed into 240, with the
        // reason of the owner's notice.
        Interlaced {
            mode: (1_280, 240, false),
            field_rate_mhz: 59_940,
            dest: (0, 0, 1_280, 240),
            scale_milli: (2_000, 500),
            fill: (1_000, 997),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &["fitted", "soft=InterlaceNotSent { lines: 480 }"],
        },
        // The Pi 5 sends interlace: 480i, 1:1.
        Interlaced {
            mode: (1_280, 480, true),
            field_rate_mhz: 59_940,
            dest: (0, 0, 1_280, 480),
            scale_milli: (2_000, 1_000),
            fill: (1_000, 995),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &[],
        },
        Interlaced {
            mode: (1_280, 240, false),
            field_rate_mhz: 59_826,
            dest: (0, 0, 1_280, 240),
            scale_milli: (4_000, 1_000),
            fill: (1_000, 995),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &[],
        },
        // 480 lines in the 248 progressive lines of an arcade tube.
        Interlaced {
            mode: (1_280, 248, false),
            field_rate_mhz: 59_940,
            dest: (0, 0, 1_280, 248),
            scale_milli: (2_000, 517),
            fill: (1_000, 998),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &["fitted", "soft=InterlaceNotSent { lines: 480 }"],
        },
        // 31 kHz: 240p as 480p, each line twice, with scanlines.
        Interlaced {
            mode: (1_280, 480, false),
            field_rate_mhz: 59_826,
            dest: (0, 0, 1_280, 480),
            scale_milli: (4_000, 2_000),
            fill: (1_000, 947),
            aspect_error_permille: 0,
            scanlines: "black 75%",
            notices: &[],
        },
        // 31 kHz: 480i as 480p full frames, 1:1.
        Interlaced {
            mode: (1_280, 480, false),
            field_rate_mhz: 59_940,
            dest: (0, 0, 1_280, 480),
            scale_milli: (2_000, 1_000),
            fill: (1_000, 948),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &[],
        },
        // A VGA frame names no system: its own width, 1:1.
        Interlaced {
            mode: (640, 480, false),
            field_rate_mhz: 60_000,
            dest: (0, 0, 640, 480),
            scale_milli: (1_000, 1_000),
            fill: (999, 949),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &[],
        },
        // A TV: 640 samples against the console width of 644 (650 on
        // PAL), as for every console picture on a TV.
        Interlaced {
            mode: (720, 240, false),
            field_rate_mhz: 60_054,
            dest: (40, 0, 640, 240),
            scale_milli: (2_000, 1_000),
            fill: (1_000, 1_000),
            aspect_error_permille: -6,
            scanlines: "none",
            notices: &[],
        },
        Interlaced {
            mode: (720, 480, true),
            field_rate_mhz: 59_940,
            dest: (40, 0, 640, 480),
            scale_milli: (1_000, 1_000),
            fill: (1_000, 1_000),
            aspect_error_permille: -6,
            scanlines: "none",
            notices: &[],
        },
        // A PAL TV runs the 60 Hz game at 50 Hz.
        Interlaced {
            mode: (720, 288, false),
            field_rate_mhz: 50_080,
            dest: (40, 24, 640, 240),
            scale_milli: (2_000, 1_000),
            fill: (1_000, 926),
            aspect_error_permille: -15,
            scanlines: "none",
            notices: &["speed_change"],
        },
        Interlaced {
            mode: (720, 576, true),
            field_rate_mhz: 50_000,
            dest: (40, 48, 640, 480),
            scale_milli: (1_000, 1_000),
            fill: (992, 926),
            aspect_error_permille: -15,
            scanlines: "none",
            notices: &["speed_change"],
        },
        // A panel shows the frames of the core, 2x, nearest.
        Interlaced {
            mode: (1_920, 1_080, false),
            field_rate_mhz: 60_000,
            dest: (320, 60, 1_280, 960),
            scale_milli: (2_000, 2_000),
            fill: (667, 889),
            aspect_error_permille: 0,
            scanlines: "none",
            notices: &[],
        },
    ];

    #[test]
    fn the_interlaced_cases_match_their_goldens_and_measures() {
        let root = workspace_root();
        let tables = Tables::load(&root).unwrap();
        let golden = Golden::new(
            root.join(GOLDEN_DIR),
            root.join(OUT_DIR).join("golden-actual"),
        );
        let first = GOLDEN_CASES.len() + TATE_CASES.len();
        for (i, expected) in INTERLACED_EXPECTED.iter().enumerate() {
            let (name, display, ..) = INTERLACED_CASES[i];
            let (image, m) = golden_glass(&tables, first + i).unwrap();
            assert_tv_glass(name, display, &m);
            golden
                .check(name, &to_rgb_image(&image).unwrap())
                .unwrap_or_else(|e| panic!("{e}"));
            let p = m
                .picture
                .as_ref()
                .unwrap_or_else(|| panic!("{name}: no picture"));
            let mode = (p.mode.hactive, p.mode.vactive, p.mode.interlace);
            assert_eq!(mode, expected.mode, "{name}");
            assert_eq!(p.mode.field_rate_mhz, expected.field_rate_mhz, "{name}");
            let (x, y, w, h) = expected.dest;
            assert_eq!(p.dest, Rect { x, y, w, h }, "{name}");
            assert_eq!(
                (p.scale_x_milli, p.scale_y_milli),
                expected.scale_milli,
                "{name}"
            );
            assert_eq!(
                (p.fill_w_permille, p.fill_h_permille),
                expected.fill,
                "{name}"
            );
            assert_eq!(p.scanlines, expected.scanlines, "{name}");
            assert_eq!(p.notices, expected.notices, "{name}");
            assert_eq!(p.cropped, Edges::default(), "{name}: every line");
            assert_eq!(p.dropped_lines, 0, "{name}");
            assert_eq!(
                p.aspect_error_permille, expected.aspect_error_permille,
                "{name}"
            );
            assert_eq!(p.player.upright, Some(true), "{name}");
        }
    }

    /// The measures of [`WIDTH_CASES`], in its order: the mode, `dest`,
    /// the scale in thousandths, the fill and the shape error. On a CRT
    /// both widths of a system share the mode at whole factors; the fill
    /// and the shape are those of the mode of each width before the wide
    /// mode. A TV has one mode width for every game already.
    #[allow(clippy::type_complexity)]
    const WIDTH_EXPECTED: [(
        (u16, u16),
        (i32, i32, u32, u32),
        (u32, u32),
        (u32, u32),
        i64,
    ); 16] = [
        ((1536, 224), (0, 0, 1536, 224), (6000, 1000), (1000, 932), 0),
        ((1536, 224), (0, 0, 1536, 224), (3000, 1000), (1000, 932), 0),
        ((1280, 224), (0, 0, 1280, 224), (4000, 1000), (1000, 931), 0),
        ((1280, 224), (0, 0, 1280, 224), (5000, 1000), (1000, 931), 0),
        ((1536, 224), (0, 0, 1536, 224), (6000, 1000), (1000, 932), 0),
        ((1536, 224), (0, 0, 1536, 224), (3000, 1000), (1000, 932), 0),
        ((1280, 224), (0, 0, 1280, 224), (4000, 1000), (1000, 931), 0),
        ((1280, 224), (0, 0, 1280, 224), (5000, 1000), (1000, 931), 0),
        (
            (720, 240),
            (40, 8, 640, 224),
            (2500, 1000),
            (1000, 1000),
            -6,
        ),
        ((720, 240), (38, 8, 644, 224), (1258, 1000), (1000, 1000), 0),
        (
            (720, 240),
            (40, 8, 640, 224),
            (2000, 1000),
            (1000, 1000),
            -6,
        ),
        (
            (720, 240),
            (40, 8, 640, 224),
            (2500, 1000),
            (1000, 1000),
            -6,
        ),
        (
            (720, 288),
            (40, 32, 640, 224),
            (2500, 1000),
            (1000, 864),
            -15,
        ),
        ((720, 288), (35, 32, 650, 224), (1270, 1000), (1000, 864), 0),
        (
            (720, 288),
            (40, 32, 640, 224),
            (2000, 1000),
            (1000, 864),
            -15,
        ),
        (
            (720, 288),
            (40, 32, 640, 224),
            (2500, 1000),
            (1000, 864),
            -15,
        ),
    ];

    #[test]
    fn the_width_cases_match_their_goldens_and_measures() {
        let root = workspace_root();
        let tables = Tables::load(&root).unwrap();
        let golden = Golden::new(
            root.join(GOLDEN_DIR),
            root.join(OUT_DIR).join("golden-actual"),
        );
        let first = GOLDEN_CASES.len() + TATE_CASES.len() + INTERLACED_CASES.len();
        for (i, (mode, dest, scale, fill, aspect)) in WIDTH_EXPECTED.iter().enumerate() {
            let (name, display, ..) = WIDTH_CASES[i];
            let (image, m) = golden_glass(&tables, first + i).unwrap();
            assert_tv_glass(name, display, &m);
            golden
                .check(name, &to_rgb_image(&image).unwrap())
                .unwrap_or_else(|e| panic!("{e}"));
            let p = m.picture.as_ref().unwrap();
            assert_eq!((p.mode.hactive, p.mode.vactive), *mode, "{name}");
            assert!(!p.mode.interlace, "{name}");
            let (x, y, w, h) = *dest;
            assert_eq!(p.dest, Rect { x, y, w, h }, "{name}");
            assert_eq!((p.scale_x_milli, p.scale_y_milli), *scale, "{name}");
            assert_eq!((p.fill_w_permille, p.fill_h_permille), *fill, "{name}");
            assert_eq!(p.aspect_error_permille, *aspect, "{name}");
            assert_eq!(p.cropped, Edges::default(), "{name}");
            assert_eq!(p.dropped_lines, 0, "{name}");
            assert_eq!(p.player.upright, Some(true), "{name}");
            if p.mode.hactive != 720 {
                assert!(p.scale_x_whole, "{name}: a whole factor on a CRT");
            }
        }
    }

    /// The widths of a PlayStation, a SNES and a Mega Drive game share one
    /// mode on a CRT, and each fills the same picture.
    #[test]
    fn the_widths_of_a_system_share_one_mode_on_a_crt() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let systems: [&[&str]; 3] = [
            &["ps1_320", "ps1_256", "ps1_368", "ps1_512", "ps1_640"],
            &["snes_256", "snes_512"],
            &["md_320", "md_256"],
        ];
        for (display, sources) in ["generic_15", "arcade_15", "pc_31_120"]
            .iter()
            .flat_map(|d| systems.iter().map(move |s| (*d, *s)))
        {
            let pictures: Vec<_> = sources
                .iter()
                .map(|source| {
                    let case = tables.named_case(display, source, &[]).unwrap();
                    let options = GlassOptions {
                        overscan_permille: None,
                        size: GOLDEN_SIZE,
                        turn: Turn::R0,
                    };
                    let outcome = simulate(&case, &pattern_of(&case.source), &options).unwrap();
                    outcome.measures.picture.unwrap()
                })
                .collect();
            for p in &pictures {
                assert_eq!(p.mode, pictures[0].mode, "{display}");
                assert_eq!(p.dest, pictures[0].dest, "{display}");
                assert_eq!(p.dropped_lines, 0, "{display}");
            }
        }
    }

    /// Every display class at every screen orientation, a vertical and a
    /// horizontal board: the game has its own shape for the player and
    /// stands upright.
    #[test]
    fn every_class_shows_the_game_upright_at_its_shape_at_every_orientation() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let options = GlassOptions {
            overscan_permille: None,
            size: (64, 48),
            turn: Turn::R0,
        };
        let displays = [
            "generic_15",
            "arcade_15",
            "arcade_31",
            "pc_31_120",
            "ntsc",
            "pal",
            "1920x1080@60",
        ];
        for display in displays {
            for source in ["ddonpach", "pwrinst2"] {
                for degrees in [0, 90, 180, 270] {
                    let set = format!("orientation={degrees}");
                    let case = tables.named_case(display, source, &[&set]).unwrap();
                    let out = simulate(&case, &pattern_of(&case.source), &options).unwrap();
                    let what = format!("{source} on {display} at {degrees}");
                    let q = out
                        .measures
                        .picture
                        .unwrap_or_else(|| panic!("{what}"))
                        .player;
                    assert_eq!(q.screen_degrees, degrees, "{what}");
                    assert!(
                        q.aspect_error_permille.abs() <= PLAYER_SHAPE_MAX_PERMILLE,
                        "{what}: {}",
                        q.aspect_error_permille
                    );
                    let want = if source == "ddonpach" { 750 } else { 1_333 };
                    assert_eq!(q.aspect_asked_milli, want, "{what}");
                    assert_eq!(q.upright, Some(true), "{what}");
                }
            }
        }
    }

    #[test]
    fn the_glass_turns_with_the_screen_and_then_with_turn() {
        use super::super::run::player_turn;
        use neptron_types::Rotation;
        assert_eq!(player_turn(Rotation::R0, Turn::R0), Turn::R0);
        assert_eq!(player_turn(Rotation::R90, Turn::R0), Turn::R270);
        assert_eq!(player_turn(Rotation::R270, Turn::R0), Turn::R90);
        assert_eq!(player_turn(Rotation::R90, Turn::R90), Turn::R0);
        assert_eq!(player_turn(Rotation::R180, Turn::R270), Turn::R90);
    }

    /// A game override that turns the picture the wrong way is not
    /// upright, and a frame of its own has no corner marks to read.
    #[test]
    fn a_wrong_game_turn_shows_as_not_upright() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let options = GlassOptions {
            overscan_permille: None,
            size: (64, 48),
            turn: Turn::R0,
        };
        let case = tables
            .named_case(
                "1920x1080@60",
                "ddonpach",
                &["orientation=90", "rotation=270"],
            )
            .unwrap();
        let frame = pattern_of(&case.source);
        let out = simulate(&case, &frame, &options).unwrap();
        let q = out.measures.picture.unwrap().player;
        assert_eq!(q.upright, Some(false));
        let mut own = frame;
        own.put(0, 0, [1, 2, 3]);
        let out = simulate(&case, &own, &options).unwrap();
        assert_eq!(out.measures.picture.unwrap().player.upright, None);
    }
}
