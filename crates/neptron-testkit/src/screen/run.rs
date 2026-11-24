//! One case end to end: the wire, the glass and the measures; and the
//! cases of a survey.

use std::path::{Path, PathBuf};
use std::{fs, io};

use neptron_platform::Matrix;
use neptron_settings::Schema;
use neptron_types::{MonitorKind, MonitorProfile};

use super::case::{self, Board, CaseError, DisplaySpec, Screen, Settings, Source};
use super::glass::Glass;
use super::image::{Rgb, Turn};
use super::measure::{self, Measures, Names};
use super::pattern::test_pattern;
use super::survey::{self, Row};
use super::wire::{self, Wire, WireError};
use crate::preset::{PresetError, monitor_preset};

/// The arcade table of the policy, relative to the workspace root.
pub const ARCADE_TABLE: &str = "docs/spec/arcade-table.md";

/// Why a case cannot run.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// The case does not parse.
    #[error(transparent)]
    Case(#[from] CaseError),
    /// A preset does not load.
    #[error(transparent)]
    Preset(#[from] PresetError),
    /// The engine or the renderer failed.
    #[error(transparent)]
    Wire(#[from] WireError),
    /// A table cannot be read.
    #[error("{path}: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// The error of the operating system.
        source: io::Error,
    },
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
#[derive(Debug)]
pub struct Outcome {
    /// Stage A.
    pub wire: Wire,
    /// The glass picture, or `None` with no picture.
    pub glass_image: Option<Rgb>,
    /// The measures.
    pub measures: Measures,
}

/// The tables a run reads once.
#[derive(Debug)]
pub struct Tables {
    /// The GPU table.
    pub matrix: Matrix,
    /// The settings schema.
    pub schema: Schema,
    /// The boards of the arcade table.
    pub boards: Vec<Board>,
}

/// The rows of a survey in run order, the glass tile of each row, and the
/// display names.
pub type SurveyRun = (Vec<Row>, Vec<Option<Rgb>>, Vec<String>);

impl Tables {
    /// Reads the tables of the workspace at `root`.
    ///
    /// # Errors
    ///
    /// [`RunError`] when a table does not load.
    pub fn load(root: &Path) -> Result<Self, RunError> {
        let matrix = Matrix::builtin().map_err(|e| CaseError::Matrix(e.to_string()))?;
        let path = root.join(ARCADE_TABLE);
        let text = fs::read_to_string(&path).map_err(|source| RunError::Read { path, source })?;
        Ok(Self {
            matrix,
            schema: Schema::builtin(),
            boards: case::parse_arcade_table(&text),
        })
    }

    /// The screen of `spec` on `gpu`, or on its default GPU.
    ///
    /// # Errors
    ///
    /// [`RunError`] for an unknown GPU or a preset that does not load.
    pub fn screen(&self, spec: &DisplaySpec, gpu: Option<&str>) -> Result<Screen, RunError> {
        let gpu = gpu.unwrap_or_else(|| spec.default_gpu());
        case::screen(
            spec,
            gpu,
            &self.matrix,
            |id| -> Result<MonitorProfile, RunError> { Ok(monitor_preset(id)?) },
        )
    }

    /// The case of the display, source and settings that `display`,
    /// `source` and `sets` name.
    ///
    /// # Errors
    ///
    /// [`RunError`] for a name or a setting that does not parse.
    pub fn named_case(&self, display: &str, source: &str, sets: &[&str]) -> Result<Case, RunError> {
        let screen = self.screen(&DisplaySpec::parse(display)?, None)?;
        let source = Source::named(source, &self.boards)?;
        let defaults = Settings::defaults(&self.schema);
        let mut settings = defaults.clone();
        for set in sets {
            settings.set(set)?;
        }
        Ok(Case {
            screen,
            source,
            label: settings.label(&defaults),
            settings,
        })
    }
}

/// The class name of a screen: `crt`, `tv` or `panel`.
pub fn class_of(profile: &MonitorProfile) -> &'static str {
    match profile.kind {
        MonitorKind::Crt { .. } => "crt",
        MonitorKind::Tv { .. } => "tv",
        MonitorKind::FlatPanel { .. } => "panel",
    }
}

/// Runs `case` with the picture `frame` and draws its glass.
///
/// # Errors
///
/// [`RunError`] for a setting value the policy does not know, or an
/// engine that fails after it set a mode.
pub fn simulate(case: &Case, frame: &Rgb, options: &GlassOptions) -> Result<Outcome, RunError> {
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
        class: class_of(&case.screen.profile),
        gpu: &case.screen.gpu,
        source: &case.source.name,
        settings: &case.label,
        arcade: case.source.arcade,
    };
    let measures = measure::measure(names, &geometry, &wire, glass.as_ref().zip(bare.as_ref()));
    Ok(Outcome {
        wire,
        glass_image,
        measures,
    })
}

/// The test picture of `source`.
pub fn pattern_of(source: &Source) -> Rgb {
    let g = source.geometry;
    test_pattern(g.width, g.height, g.aspect_milli)
}

/// Runs every case of the survey matrix for `displays` and `sources`,
/// with glass tiles of `tile` pixels, and flags the rows.
///
/// # Errors
///
/// [`RunError`] for a name that does not parse or a case that fails.
pub fn survey_rows(
    tables: &Tables,
    displays: &[String],
    sources: &[String],
    tile: (u32, u32),
    mut progress: impl FnMut(&str),
) -> Result<SurveyRun, RunError> {
    let defaults = Settings::defaults(&tables.schema);
    let options = GlassOptions {
        overscan_permille: None,
        size: tile,
        turn: Turn::R0,
    };
    let mut rows = Vec::new();
    let mut tiles = Vec::new();
    let mut names = Vec::new();
    for display in displays {
        let screen = tables.screen(&DisplaySpec::parse(display)?, None)?;
        names.push(screen.name.clone());
        for name in sources {
            let source = Source::named(name, &tables.boards)?;
            let frame = pattern_of(&source);
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
                rows.push(Row {
                    style: style.to_owned(),
                    refresh: refresh.to_owned(),
                    flags: survey::own_flags(style, &outcome.measures),
                    measures: outcome.measures,
                });
                tiles.push(outcome.glass_image);
            }
        }
        progress(&screen.name);
    }
    survey::compare_rows(&mut rows);
    Ok((rows, tiles, names))
}
