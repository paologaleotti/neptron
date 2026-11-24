//! `cargo xtask screen`: the screen simulator. It runs the decision and
//! the scene of the app for a source on a display, renders the frame
//! buffer of the mode (the wire), then models what the screen shows (the
//! glass), and writes both pictures and their measures.
//!
//! The modules under `screen/` decide and draw in memory; this file reads
//! the presets and the tables and writes the files.

mod case;
mod glass;
mod image;
mod measure;
mod pattern;
mod survey;
mod wire;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, io};

use neptron_platform::Matrix;
use neptron_settings::Schema;
use neptron_testkit::golden::decode_ppm;
use neptron_testkit::preset::monitor_preset;
use neptron_types::MonitorProfile;

use case::{Board, CaseError, DisplaySpec, Screen, Settings, Source};
use glass::{GLASS_SIZE, Glass};
use image::{Rgb, Turn};
use measure::{Measures, Names};
use wire::{Wire, WireError};

use crate::repo::Repo;
use crate::ui::png;

/// The output directory, relative to the repository root.
const OUT_DIR: &str = "target/screen";
/// The arcade table of the policy, relative to the repository root.
const ARCADE_TABLE: &str = "docs/spec/arcade-table.md";
/// The size of each 1:1 crop of `--exact` on a panel, in pixels.
const EXACT_CROP: (u32, u32) = (480, 270);
/// The box of each glass picture on a contact sheet of the survey, in
/// pixels: 36 characters of the label font across.
const SURVEY_TILE: (u32, u32) = (216, 162);
/// The golden images of the reference cases, relative to the repository
/// root.
const GOLDEN_DIR: &str = "xtask/tests/golden/screen";
/// The glass size of a golden: small, to keep the repository small.
const GOLDEN_SIZE: (u32, u32) = (360, 270);

/// The reference cases with a golden glass: name, display, source and
/// settings.
const GOLDEN_CASES: [(&str, &str, &str, &[&str]); 8] = [
    ("snes-generic_15-auto", "generic_15", "snes", &[]),
    ("rtype-ntsc-auto", "ntsc", "rtype", &[]),
    ("rtype-ntsc-fill", "ntsc", "rtype", &["style=fit"]),
    ("psx-pc_31_120-auto", "pc_31_120", "psx", &[]),
    ("psx_480i-ntsc-auto", "ntsc", "psx_480i", &[]),
    ("ddonpach-generic_15-auto", "generic_15", "ddonpach", &[]),
    ("snes-panel_1080-auto", "1920x1080@60", "snes", &[]),
    ("amiga_pal-pal-auto", "pal", "amiga_pal", &[]),
];

const USAGE: &str = "cargo xtask screen survey [--display D]... [--source S]... | \
cargo xtask screen --display D --source S [--gpu ID] [--set KEY=VALUE]... \
[--aspect A] [--rotate 0|90|180|270] [--interlaced] [--arcade] [--frame FILE.ppm] \
[--overscan PERCENT] [--size WxH] [--turn 0|90|180|270] [--exact] [--out NAME]";

/// An error of `cargo xtask screen`.
#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    /// The arguments are wrong.
    #[error("{0}\nusage: {USAGE}")]
    Usage(String),
    /// The case does not parse.
    #[error(transparent)]
    Case(#[from] CaseError),
    /// A preset does not load.
    #[error(transparent)]
    Preset(#[from] neptron_testkit::preset::PresetError),
    /// The engine or the renderer failed.
    #[error(transparent)]
    Wire(#[from] WireError),
    /// The frame file is not a PPM of the source size.
    #[error("{path}: {reason}")]
    Frame {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// A file cannot be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error of the operating system.
        source: io::Error,
    },
    /// The repository cannot be found.
    #[error(transparent)]
    Repo(#[from] crate::repo::RepoError),
}

/// How the glass is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlassOptions {
    /// The overscan at each side, in per mille; `None` for the class
    /// default.
    pub overscan_permille: Option<u32>,
    /// The size of the glass picture box, in pixels.
    pub size: (u32, u32),
    /// The turn of the monitor.
    pub turn: Turn,
}

/// One case, ready to run.
#[derive(Debug, Clone)]
pub struct Case {
    /// The display.
    pub screen: Screen,
    /// The source.
    pub source: Source,
    /// The settings.
    pub settings: Settings,
    /// The settings label.
    pub label: String,
}

/// What a case gives.
pub struct Outcome {
    /// Stage A.
    pub wire: Wire,
    /// The glass picture, or `None` with no picture.
    pub glass_image: Option<Rgb>,
    /// The measures.
    pub measures: Measures,
}

/// What the files of a run come from: the tables, read once.
pub struct Tables {
    /// The GPU table.
    pub matrix: Matrix,
    /// The settings schema.
    pub schema: Schema,
    /// The boards of the arcade table.
    pub boards: Vec<Board>,
}

impl Tables {
    /// Reads the tables of the repository at `root`.
    ///
    /// # Errors
    ///
    /// [`ScreenError`] when a table does not load.
    pub fn load(root: &Path) -> Result<Self, ScreenError> {
        let matrix = Matrix::builtin().map_err(|e| CaseError::Matrix(e.to_string()))?;
        let path = root.join(ARCADE_TABLE);
        let text = fs::read_to_string(&path).map_err(|source| ScreenError::Io { path, source })?;
        Ok(Self {
            matrix,
            schema: Schema::builtin(),
            boards: case::parse_arcade_table(&text),
        })
    }

    /// The screen of `spec` on `gpu` (or its default GPU).
    ///
    /// # Errors
    ///
    /// [`ScreenError`] for an unknown GPU or a preset that does not load.
    pub fn screen(&self, spec: &DisplaySpec, gpu: Option<&str>) -> Result<Screen, ScreenError> {
        let gpu = gpu.unwrap_or_else(|| spec.default_gpu());
        case::screen(
            spec,
            gpu,
            &self.matrix,
            |id| -> Result<MonitorProfile, ScreenError> { Ok(monitor_preset(id)?) },
        )
    }
}

/// Runs `case` with the picture `frame` and draws its glass.
///
/// # Errors
///
/// [`ScreenError`] for a setting value the policy does not know, or an
/// engine that fails after it set a mode.
pub fn simulate(case: &Case, frame: &Rgb, options: &GlassOptions) -> Result<Outcome, ScreenError> {
    let geometry = case.source.geometry;
    let settings = case.settings.policy(case.source.arcade)?;
    let scanlines = case.settings.scanlines()?;
    let wire = wire::run(&case.screen, &geometry, settings, scanlines, frame)?;
    let glass = wire
        .shown
        .as_ref()
        .map(|s| Glass::of(&case.screen.profile, &s.mode, options.overscan_permille));
    let bare = glass.as_ref().map(Glass::without_overscan);
    let glass_image = wire.shown.as_ref().zip(glass.as_ref()).map(|(shown, g)| {
        let size = g.picture_size(options.size);
        g.render(&shown.mode, &shown.frame_buffer, size)
            .turned(options.turn)
    });
    let names = Names {
        display: &case.screen.name,
        gpu: &case.screen.gpu,
        source: &case.source.name,
        settings: &case.label,
    };
    let measures = measure::measure(names, &geometry, &wire, glass.as_ref().zip(bare.as_ref()));
    Ok(Outcome {
        wire,
        glass_image,
        measures,
    })
}

/// Runs `cargo xtask screen ...`.
///
/// # Errors
///
/// [`ScreenError`] for bad arguments, a case that does not load, or a
/// file that cannot be written.
pub fn run(args: &[&str]) -> Result<ExitCode, ScreenError> {
    let root = Repo::discover()?.root;
    let tables = Tables::load(&root)?;
    match args.split_first() {
        Some((&"survey", rest)) => return run_survey(&root, &tables, rest),
        Some((&"golden", rest)) => return run_golden(&root, &tables, rest),
        _ => {}
    }
    let parsed = Args::parse(args)?;
    let case = parsed.case(&tables)?;
    let frame = match &parsed.frame {
        Some(path) => read_frame(path)?,
        None => {
            let g = case.source.geometry;
            pattern::test_pattern(g.width, g.height, g.aspect_milli)
        }
    };
    let outcome = simulate(&case, &frame, &parsed.glass)?;
    let name = parsed.out.clone().unwrap_or_else(|| case_dir_name(&case));
    let dir = root.join(OUT_DIR).join(name);
    write_png(&dir.join("frame.png"), &frame)?;
    if let Some(shown) = &outcome.wire.shown {
        write_png(&dir.join("wire.png"), &shown.frame_buffer)?;
    }
    if let Some(image) = &outcome.glass_image {
        write_png(&dir.join("glass.png"), image)?;
    }
    if parsed.exact {
        write_exact(&dir, &outcome)?;
    }
    let json = serde_json::to_string_pretty(&outcome.measures.to_json()).unwrap_or_default();
    write(&dir.join("measures.json"), json.as_bytes())?;
    println!("{}", Measures::HEADER);
    println!("{}", outcome.measures.row());
    println!("{}", dir.display());
    Ok(ExitCode::SUCCESS)
}

/// One golden case, ready to run.
fn golden_case(tables: &Tables, index: usize) -> Result<Case, ScreenError> {
    let (_, display, source, sets) = GOLDEN_CASES[index];
    let mut args = vec!["--display", display, "--source", source];
    for set in sets {
        args.extend(["--set", set]);
    }
    Args::parse(&args)?.case(tables)
}

/// The glass of golden case `index`, at [`GOLDEN_SIZE`], and its measures.
fn golden_glass(tables: &Tables, index: usize) -> Result<(Rgb, Measures), ScreenError> {
    let case = golden_case(tables, index)?;
    let g = case.source.geometry;
    let frame = pattern::test_pattern(g.width, g.height, g.aspect_milli);
    let options = GlassOptions {
        overscan_permille: None,
        size: GOLDEN_SIZE,
        turn: Turn::R0,
    };
    let outcome = simulate(&case, &frame, &options)?;
    let image = outcome
        .glass_image
        .unwrap_or_else(|| Rgb::new(GOLDEN_SIZE.0, GOLDEN_SIZE.1, [0; 3]));
    Ok((image, outcome.measures))
}

/// Runs `cargo xtask screen golden [--bless]`: compares the glass of each
/// reference case with its golden, or with `--bless` writes it. Look at
/// each picture in `target/screen/golden/` before a bless.
fn run_golden(root: &Path, tables: &Tables, args: &[&str]) -> Result<ExitCode, ScreenError> {
    let bless = match args {
        [] => false,
        ["--bless"] => true,
        _ => {
            return Err(ScreenError::Usage(
                "cargo xtask screen golden [--bless]".into(),
            ));
        }
    };
    let mut failed = 0;
    for (i, (name, ..)) in GOLDEN_CASES.iter().enumerate() {
        let (image, _) = golden_glass(tables, i)?;
        write_png(
            &root
                .join(OUT_DIR)
                .join("golden")
                .join(format!("{name}.png")),
            &image,
        )?;
        let path = root.join(GOLDEN_DIR).join(format!("{name}.ppm"));
        let ppm = to_ppm(&image);
        let same = fs::read(&path).is_ok_and(|old| old == ppm);
        if same {
            println!("same     {name}");
        } else if bless {
            write(&path, &ppm)?;
            println!("blessed  {name}");
        } else {
            failed += 1;
            println!("FAILED   {name}: differs from {}", path.display());
        }
    }
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn to_ppm(image: &Rgb) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
    out.extend_from_slice(&image.rgb);
    out
}

/// Runs `cargo xtask screen survey`: every display, source and setting of
/// the matrix; writes `survey.md`, `survey.json` and one contact sheet per
/// display under `target/screen/survey/`.
fn run_survey(root: &Path, tables: &Tables, args: &[&str]) -> Result<ExitCode, ScreenError> {
    let (displays, sources) = survey_lists(args)?;
    let dir = root.join(OUT_DIR).join("survey");
    let matrix = survey::settings_matrix();
    let (mut rows, tiles, display_names) = survey_rows(tables, &displays, &sources)?;
    for name in &display_names {
        let sheet_tiles: Vec<survey::Tile<'_>> = rows
            .iter()
            .zip(&tiles)
            .filter(|(r, _)| &r.measures.display == name)
            .map(|(row, glass)| survey::Tile {
                row,
                glass: glass.as_ref(),
            })
            .collect();
        let sheet = survey::contact_sheet(&sheet_tiles, matrix.len(), SURVEY_TILE);
        let path = dir.join(format!("{}.png", file_stem(name)));
        write_png(&path, &sheet)?;
        println!("{}", path.display());
    }
    survey::sort_rows(&mut rows);
    let md = survey::markdown(&rows, &display_names);
    write(&dir.join("survey.md"), md.as_bytes())?;
    let json = serde_json::to_string_pretty(&survey::to_json(&rows)).unwrap_or_default();
    write(&dir.join("survey.json"), json.as_bytes())?;
    let flagged = rows.iter().filter(|r| !r.flags.is_empty()).count();
    println!(
        "{} ({} rows, {flagged} flagged)",
        dir.join("survey.md").display(),
        rows.len()
    );
    Ok(ExitCode::SUCCESS)
}

/// Rows of the survey and their glass tiles, in run order, with the flags
/// of each row and of the comparisons; and the display names.
type SurveyRun = (Vec<survey::Row>, Vec<Option<Rgb>>, Vec<String>);

/// Runs every case of the survey matrix.
fn survey_rows(
    tables: &Tables,
    displays: &[String],
    sources: &[String],
) -> Result<SurveyRun, ScreenError> {
    let defaults = Settings::defaults(&tables.schema);
    let options = GlassOptions {
        overscan_permille: None,
        size: SURVEY_TILE,
        turn: Turn::R0,
    };
    let mut rows = Vec::new();
    let mut tiles = Vec::new();
    let mut display_names = Vec::new();
    for display in displays {
        let screen = tables.screen(&DisplaySpec::parse(display)?, None)?;
        display_names.push(screen.name.clone());
        for name in sources {
            let source = Source::named(name, &tables.boards)?;
            let g = source.geometry;
            let frame = pattern::test_pattern(g.width, g.height, g.aspect_milli);
            for (style, refresh) in survey::settings_matrix() {
                let mut settings = defaults.clone();
                settings.set(&format!("display.style={style}"))?;
                settings.set(&format!("display.refresh_policy={refresh}"))?;
                let case = Case {
                    screen: screen.clone(),
                    source: source.clone(),
                    label: settings.label(&defaults),
                    settings,
                };
                let outcome = simulate(&case, &frame, &options)?;
                rows.push(survey::Row {
                    style: style.to_owned(),
                    refresh: refresh.to_owned(),
                    flags: survey::own_flags(style, &outcome.measures),
                    measures: outcome.measures,
                });
                tiles.push(outcome.glass_image);
            }
        }
        eprintln!("xtask: survey {} done", screen.name);
    }
    survey::compare_rows(&mut rows);
    Ok((rows, tiles, display_names))
}

/// The displays and the sources of a survey: the defaults, or the ones
/// given with `--display` and `--source`.
fn survey_lists(args: &[&str]) -> Result<(Vec<String>, Vec<String>), ScreenError> {
    let mut displays = Vec::new();
    let mut sources = Vec::new();
    let mut it = args.iter();
    while let Some(&arg) = it.next() {
        let list = match arg {
            "--display" => &mut displays,
            "--source" => &mut sources,
            other => return Err(ScreenError::Usage(format!("unknown argument {other:?}"))),
        };
        let value = it
            .next()
            .ok_or_else(|| ScreenError::Usage(format!("{arg} needs a value")))?;
        list.push((*value).to_owned());
    }
    if displays.is_empty() {
        displays = survey::DISPLAYS.iter().map(|d| (*d).to_owned()).collect();
    }
    if sources.is_empty() {
        sources = case::REFERENCE_NAMES
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

/// The 1:1 pictures of a panel: the whole panel, and two crops at the
/// top-left corner of the picture and at the centre.
fn write_exact(dir: &Path, outcome: &Outcome) -> Result<(), ScreenError> {
    let Some(shown) = &outcome.wire.shown else {
        return Ok(());
    };
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
    write_png(&dir.join("exact-corner.png"), &corner)?;
    write_png(&dir.join("exact-centre.png"), &centre)
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

fn read_frame(path: &Path) -> Result<Rgb, ScreenError> {
    let bytes = fs::read(path).map_err(|source| ScreenError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let image = decode_ppm(&bytes).map_err(|e| ScreenError::Frame {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    Rgb::from_rgb(image.width(), image.height(), image.bytes().to_vec()).ok_or_else(|| {
        ScreenError::Frame {
            path: path.to_path_buf(),
            reason: "the pixel data does not match the size".into(),
        }
    })
}

fn write_png(path: &Path, image: &Rgb) -> Result<(), ScreenError> {
    write(path, &png::encode(image.width, image.height, &image.rgb))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), ScreenError> {
    let io = |source| ScreenError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io)?;
    }
    fs::write(path, bytes).map_err(io)
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
    fn parse(args: &[&str]) -> Result<Self, ScreenError> {
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
                    .ok_or_else(|| ScreenError::Usage(format!("{arg} needs a value")))
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
                        .ok_or_else(|| ScreenError::Usage(format!("--overscan {v}: 0 to 49.9")))?;
                    out.glass.overscan_permille = Some((percent * 10.0).round() as u32);
                }
                "--size" => out.glass.size = parse_size(&value()?)?,
                "--turn" => {
                    let v = value()?;
                    out.glass.turn =
                        v.parse().ok().and_then(Turn::from_degrees).ok_or_else(|| {
                            ScreenError::Usage(format!("--turn {v}: 0, 90, 180 or 270"))
                        })?;
                }
                "--exact" => out.exact = true,
                "--out" => out.out = Some(value()?),
                other => return Err(ScreenError::Usage(format!("unknown argument {other:?}"))),
            }
        }
        Ok(out)
    }

    fn case(&self, tables: &Tables) -> Result<Case, ScreenError> {
        let display = self
            .display
            .as_deref()
            .ok_or_else(|| ScreenError::Usage("--display is missing".into()))?;
        let source = self
            .source
            .as_deref()
            .ok_or_else(|| ScreenError::Usage("--source is missing".into()))?;
        let spec = DisplaySpec::parse(display)?;
        let screen = tables.screen(&spec, self.gpu.as_deref())?;
        let source = if source.contains('@') {
            Source::by_hand(
                source,
                self.aspect.as_deref(),
                self.rotate.as_deref(),
                self.interlaced,
                self.arcade,
            )?
        } else {
            let mut named = Source::named(source, &tables.boards)?;
            named.arcade |= self.arcade;
            named
        };
        let defaults = Settings::defaults(&tables.schema);
        let mut settings = defaults.clone();
        for set in &self.sets {
            settings.set(set)?;
        }
        let label = settings.label(&defaults);
        Ok(Case {
            screen,
            source,
            settings,
            label,
        })
    }
}

fn parse_size(text: &str) -> Result<(u32, u32), ScreenError> {
    text.split_once('x')
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .filter(|(w, h): &(u32, u32)| *w > 0 && *h > 0 && *w <= 8_192 && *h <= 8_192)
        .ok_or_else(|| ScreenError::Usage(format!("--size {text}: WIDTHxHEIGHT")))
}

#[cfg(test)]
mod tests {
    use neptron_testkit::{Golden, RgbImage};
    use neptron_types::Rect;

    use super::measure::Edges;
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits in the repository root")
            .to_path_buf()
    }

    /// The expected measures of a golden case: mode size, dest, crop at
    /// each edge, fill across and down, aspect error, whole scale down.
    struct Expected {
        mode: (u16, u16),
        dest: (i32, i32, u32, u32),
        cropped: (u32, u32, u32, u32),
        fill: (u32, u32),
        aspect_error_permille: i64,
        whole_y: bool,
    }

    const fn e(
        mode: (u16, u16),
        dest: (i32, i32, u32, u32),
        cropped: (u32, u32, u32, u32),
        fill: (u32, u32),
        aspect_error_permille: i64,
        whole_y: bool,
    ) -> Expected {
        Expected {
            mode,
            dest,
            cropped,
            fill,
            aspect_error_permille,
            whole_y,
        }
    }

    /// The measures of [`GOLDEN_CASES`], in its order, computed by hand
    /// from the policy's placement and the glass model, then read back.
    const EXPECTED: [Expected; 8] = [
        // 256 wide at 5x fills the 15 kHz line; 224 of the 240 lines of
        // the window: 93.2 % high, so 7.3 % wider than 4:3.
        e(
            (1280, 224),
            (0, 0, 1280, 224),
            (0, 0, 0, 0),
            (1000, 932),
            73,
            true,
        ),
        // 256 lines in 240p: 8 cut at each edge; 704 samples for a frame
        // that asks 750: 6.2 % narrow.
        e(
            (720, 240),
            (8, 0, 704, 240),
            (8, 8, 0, 0),
            (1000, 1000),
            -62,
            true,
        ),
        // Fill on the TV is the same picture as Auto.
        e(
            (720, 240),
            (8, 0, 704, 240),
            (8, 8, 0, 0),
            (1000, 1000),
            -62,
            true,
        ),
        // 2x at 31 kHz, 480 lines of the 507 of the window.
        e(
            (640, 480),
            (0, 0, 640, 480),
            (0, 0, 0, 0),
            (1000, 947),
            58,
            true,
        ),
        // 480i 1:1 at the console width of 640 samples.
        e(
            (720, 480),
            (40, 0, 640, 480),
            (0, 0, 0, 0),
            (1000, 1000),
            -91,
            true,
        ),
        // A vertical board with no interlace on the PC: fitted into 248p.
        e(
            (1440, 248),
            (315, 0, 810, 248),
            (0, 0, 0, 0),
            (562, 992),
            -29,
            false,
        ),
        // The panel fills its height at 4:3.
        e(
            (1920, 1080),
            (240, 0, 1440, 1080),
            (0, 0, 0, 0),
            (750, 1000),
            0,
            false,
        ),
        // 256 lines centred in 288p, 2x across.
        e(
            (720, 288),
            (40, 16, 640, 256),
            (0, 0, 0, 0),
            (1000, 988),
            23,
            true,
        ),
    ];

    #[test]
    fn the_reference_cases_match_their_goldens_and_measures() {
        let root = root();
        let tables = Tables::load(&root).unwrap();
        let golden = Golden::new(
            root.join(GOLDEN_DIR),
            root.join(OUT_DIR).join("golden-actual"),
        );
        for (i, expected) in EXPECTED.iter().enumerate() {
            let name = GOLDEN_CASES[i].0;
            let (image, m) = golden_glass(&tables, i).unwrap();
            let rgb = RgbImage::from_rgb(image.width, image.height, image.rgb).unwrap();
            golden.check(name, &rgb).unwrap_or_else(|e| panic!("{e}"));
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
            assert_eq!(p.scale_y_whole, expected.whole_y, "{name}");
        }
    }

    #[test]
    fn the_tv_overscan_hides_12_lines_of_rtype_at_each_edge() {
        let tables = Tables::load(&root()).unwrap();
        let (_, m) = golden_glass(&tables, 1).unwrap();
        let p = m.picture.unwrap();
        let hidden = p.overscan_hidden.unwrap();
        assert_eq!((hidden.top, hidden.bottom), (12, 12));
        assert_eq!(
            (hidden.left, hidden.right),
            (19, 19),
            "35.2 of 704 samples, 384 wide"
        );
        assert_eq!(p.notices, ["speed_change", "cropped_lines=16"]);
        assert!(p.speed_alert);
    }

    #[test]
    fn the_survey_runs_and_flags_the_crop_of_rtype_on_a_tv() {
        let tables = Tables::load(&root()).unwrap();
        let (rows, tiles, names) =
            survey_rows(&tables, &["ntsc".into()], &["rtype".into()]).unwrap();
        assert_eq!(names, ["ntsc"]);
        assert_eq!(rows.len(), survey::settings_matrix().len());
        assert_eq!(tiles.len(), rows.len());
        let auto = &rows[0];
        assert_eq!(
            (auto.style.as_str(), auto.refresh.as_str()),
            ("auto", "exact")
        );
        assert!(
            auto.flags.contains(&survey::Flag::Cropped),
            "{:?}",
            auto.flags
        );
        assert!(
            auto.flags.contains(&survey::Flag::Aspect),
            "{:?}",
            auto.flags
        );
        let fit = rows.iter().find(|r| r.style == "fit").unwrap();
        assert!(
            fit.flags.contains(&survey::Flag::SameAs("native".into())),
            "{:?}",
            fit.flags
        );
    }

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
                matches!(Args::parse(bad), Err(ScreenError::Usage(_))),
                "{bad:?}"
            );
        }
    }
}
