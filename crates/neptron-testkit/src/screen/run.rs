//! One case end to end: the wire, the glass and the measures; and the
//! cases of a survey.

use std::path::{Path, PathBuf};
use std::{fs, io};

use neptron_platform::Matrix;
use neptron_settings::Schema;
use neptron_types::{MonitorKind, MonitorProfile, Orientation, Rotation};
use neptron_video::{PolicySettings, wide_strategy};

use super::card;
use super::case::{self, Board, CaseError, DisplaySpec, Screen, Settings, Source};
use super::consoles;
use super::glass::Glass;
use super::image::{Rgb, Turn};
use super::measure::{self, Measures, Names, View};
use super::pattern::{test_pattern, test_pattern_for};
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
    /// A turn of the output, clockwise, after the turn of the screen
    /// orientation. It changes no decision: it is not TATE mode.
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
    /// The test card is drawn with this pixel shape, the width of a pixel
    /// over its height: the shape of the real console. `None` for the
    /// shape of the display aspect of the core.
    pub card_pixel_aspect: Option<f64>,
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
        self.named_case_on(display, None, source, sets)
    }

    /// [`Self::named_case`] with the GPU entry `gpu`, or the default of the
    /// display for `None`.
    ///
    /// # Errors
    ///
    /// As [`Self::named_case`].
    pub fn named_case_on(
        &self,
        display: &str,
        gpu: Option<&str>,
        source: &str,
        sets: &[&str],
    ) -> Result<Case, RunError> {
        let screen = self.screen(&DisplaySpec::parse(display)?, gpu)?;
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
            card_pixel_aspect: None,
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
    let settings = PolicySettings {
        horizontal: wide_strategy(case.source.widths),
        ..case.settings.policy(case.source.arcade)?
    };
    let scanlines = case.settings.scanlines()?;
    let orientation = case.settings.orientation()?;
    let wire = wire::run(
        &case.screen,
        &geometry,
        settings,
        scanlines,
        orientation,
        frame,
    )?;
    Ok(finish(case, frame, options, orientation, wire))
}

/// The glass and the measures of `case` at `orientation` for the wire
/// `wire` of `frame`: the second half of [`simulate`]. A test gives it a
/// wire that the engine did not make, to prove that a measure sees a
/// wrong placement.
pub fn finish(
    case: &Case,
    frame: &Rgb,
    options: &GlassOptions,
    orientation: Orientation,
    wire: Wire,
) -> Outcome {
    let geometry = case.source.geometry;
    let turn = player_turn(orientation.screen, options.turn);
    let glass = wire
        .shown
        .as_ref()
        .map(|s| Glass::of(&case.screen.profile, &s.mode, options.overscan_permille));
    let bare = glass.as_ref().map(Glass::without_overscan);
    let glass_image = wire.shown.as_ref().zip(glass.as_ref()).map(|(shown, g)| {
        let size = g.picture_size(options.size);
        g.render(&shown.mode, &shown.frame_buffer, size)
            .turned(turn)
    });
    let names = Names {
        display: &case.screen.name,
        class: class_of(&case.screen.profile),
        gpu: &case.screen.gpu,
        source: &case.source.name,
        settings: &case.label,
        arcade: case.source.arcade,
    };
    let pattern = *frame == card_of(case);
    let view = View {
        orientation,
        core_source: &geometry,
        pattern,
    };
    let mut measures = measure::measure(names, view, &wire, glass.as_ref().zip(bare.as_ref()));
    if let (Some(p), Some(shown), true) = (&mut measures.picture, &wire.shown, pattern) {
        p.border_on_wire = Some(card::border_seen(&shown.frame_buffer));
    }
    Outcome {
        wire,
        glass_image,
        measures,
    }
}

/// The glass size on which the test card is measured: large, so one
/// glass pixel is about 0.1 % of the circle.
pub const CARD_MEASURE_SIZE: (u32, u32) = (1440, 1080);

/// Measures the test card `frame` of `case` on the glass of `outcome`, in
/// the scan-out frame of the mode at [`CARD_MEASURE_SIZE`]: the glass
/// shape against both references and the sides of the border that show.
/// Nothing changes when the frame is not the test card or nothing shows.
pub fn measure_card(
    case: &Case,
    frame: &Rgb,
    overscan_permille: Option<u32>,
    outcome: &mut Outcome,
) {
    let (Some(shown), Some(p)) = (&outcome.wire.shown, &mut outcome.measures.picture) else {
        return;
    };
    if *frame != card_of(case) {
        return;
    }
    let glass = Glass::of(&case.screen.profile, &shown.mode, overscan_permille);
    let size = glass.picture_size(CARD_MEASURE_SIZE);
    let image = glass.render(&shown.mode, &shown.frame_buffer, size);
    p.border_on_glass = Some(card::border_seen(&image));
    p.glass_centre_permille = card::lit_centre(&image);
    let (aw, ah) = glass.aspect();
    let unit = (
        f64::from(aw) / f64::from(image.width),
        f64::from(ah) / f64::from(image.height),
    );
    let g = case.source.geometry;
    let (Some(on_glass), Some(in_frame)) = (
        card::card_size(&image, unit),
        card::card_size(frame, (1.0, 1.0)),
    ) else {
        return;
    };
    let core_pixel_aspect = match g.aspect_milli {
        0 => 1.0,
        a => f64::from(a) / 1000.0 * f64::from(g.height) / f64::from(g.width),
    };
    let console = consoles::console_of(&case.source.name).and_then(|c| {
        card::console_pixel_aspect(c, g.width, g.interlaced, &glass)
            .map(|(clock, par)| (c.name, clock, par))
    });
    let expected = card::Expected {
        frame: in_frame,
        frame_size: (g.width, g.height),
        across: shown.scene.rotation.across(),
        core_pixel_aspect,
        console,
    };
    p.glass_shape = Some(card::glass_shape(&on_glass, &expected));
}

/// The clockwise turn of the glass picture: the screen at `screen`
/// degrees counter-clockwise, as the player sees it, then `extra`.
pub fn player_turn(screen: Rotation, extra: Turn) -> Turn {
    let extra_degrees: u16 = match extra {
        Turn::R0 => 0,
        Turn::R90 => 90,
        Turn::R180 => 180,
        Turn::R270 => 270,
    };
    let degrees = (360 - screen.degrees() + extra_degrees) % 360;
    // The sum of two quarter turns is a quarter turn.
    Turn::from_degrees(u32::from(degrees)).unwrap_or(Turn::R0)
}

/// The test card of `case`: drawn with the pixel shape of the console
/// when the case names one, else with the display aspect of the core.
pub fn card_of(case: &Case) -> Rgb {
    let g = case.source.geometry;
    match case.card_pixel_aspect {
        Some(par) => test_pattern_for(g.width, g.height, par),
        None => pattern_of(&case.source),
    }
}

/// The pixel shape of the real console of `case` on the glass of the
/// mode of `outcome`, or `None` when the table has no clock for it or
/// nothing shows.
pub fn console_pixel_shape(case: &Case, outcome: &Outcome) -> Option<f64> {
    let shown = outcome.wire.shown.as_ref()?;
    let console = consoles::console_of(&case.source.name)?;
    let glass = Glass::of(&case.screen.profile, &shown.mode, None);
    let g = case.source.geometry;
    card::console_pixel_aspect(console, g.width, g.interlaced, &glass).map(|(_, par)| par)
}

/// Runs `case` with the card of the reference that judges it: the card of
/// the console when the table has a clock for it (the case then keeps
/// that card in `card_pixel_aspect`), else the card of the core. Returns
/// the outcome and the card.
///
/// # Errors
///
/// As [`simulate`].
pub fn simulate_with_reference_card(
    case: &mut Case,
    options: &GlassOptions,
) -> Result<(Outcome, Rgb), RunError> {
    case.card_pixel_aspect = None;
    let core = pattern_of(&case.source);
    let outcome = simulate(case, &core, options)?;
    let Some(par) = console_pixel_shape(case, &outcome) else {
        return Ok((outcome, core));
    };
    case.card_pixel_aspect = Some(par);
    let card = card_of(case);
    Ok((simulate(case, &card, options)?, card))
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
                    card_pixel_aspect: None,
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
