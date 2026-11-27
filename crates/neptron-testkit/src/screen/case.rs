//! One case of the screen simulator: a display, a source and the display
//! settings, parsed from the command line. Pure: the callers read the
//! files.

use std::collections::BTreeMap;
use std::fmt;

use neptron_platform::Matrix;
use neptron_settings::Schema;
use neptron_types::{
    GpuCaps, Modeline, MonitorKind, MonitorProfile, Orientation, Rotation, SourceGeometry,
    TvStandard,
};
use neptron_video::{
    PolicySettings, REFERENCE_SOURCES, ScanlineSettings, SettingsError, policy_settings,
    scanline_settings, with_display_choices,
};

/// The GPU entry of `data/hardware/matrix.toml` for a CRT and a panel:
/// the bench PC.
pub const DEFAULT_GPU: &str = "intel_gen9";
/// The GPU entry for a TV: the composite output of the Raspberry Pi.
pub const DEFAULT_TV_GPU: &str = "pi_vc4_composite";

/// The names of [`REFERENCE_SOURCES`], in its order.
pub const REFERENCE_NAMES: [&str; 10] = [
    "gb",
    "snes",
    "psx",
    "nes_pal",
    "amiga_pal",
    "psx_480i",
    "neogeo",
    "mk",
    "ddonpach",
    "dos_vga",
];

/// The widths of the SNES in play: the key `widths` of
/// `data/systems/snes.toml`.
pub const SNES_WIDTHS: [u16; 2] = [256, 512];
/// The widths of the Mega Drive in play: the key `widths` of
/// `data/systems/megadrive.toml`.
pub const MEGA_DRIVE_WIDTHS: [u16; 2] = [320, 256];
/// The widths of a PlayStation game, the most used first. Its system file
/// comes with its core (task S10); until then the list is a fixture here.
pub const PLAYSTATION_WIDTHS: [u16; 5] = [320, 256, 640, 512, 368];

/// The sources of a system with the widths that its system data names, as
/// the app runs a game of that system: the SNES and the Mega Drive at each
/// width they draw in play, the PlayStation at its five widths in 240p at
/// 59.826 Hz and in 480i at 59.940 Hz, and a 640x480 progressive VGA frame
/// at 60 Hz with no widths (DOS, the Dreamcast VGA cable).
pub const SYSTEM_SOURCES: [(&str, &[u16], SourceGeometry); 11] = [
    (
        "snes_256",
        &SNES_WIDTHS,
        system_source(256, 224, 60_099, false),
    ),
    (
        "snes_512",
        &SNES_WIDTHS,
        system_source(512, 224, 60_099, false),
    ),
    (
        "md_320",
        &MEGA_DRIVE_WIDTHS,
        system_source(320, 224, 59_922, false),
    ),
    (
        "md_256",
        &MEGA_DRIVE_WIDTHS,
        system_source(256, 224, 59_922, false),
    ),
    (
        "ps1_320",
        &PLAYSTATION_WIDTHS,
        system_source(320, 240, 59_826, false),
    ),
    (
        "ps1_256",
        &PLAYSTATION_WIDTHS,
        system_source(256, 240, 59_826, false),
    ),
    (
        "ps1_368",
        &PLAYSTATION_WIDTHS,
        system_source(368, 240, 59_826, false),
    ),
    (
        "ps1_512",
        &PLAYSTATION_WIDTHS,
        system_source(512, 240, 59_826, false),
    ),
    (
        "ps1_640",
        &PLAYSTATION_WIDTHS,
        system_source(640, 240, 59_826, false),
    ),
    (
        "ps1_480i",
        &PLAYSTATION_WIDTHS,
        system_source(640, 480, 59_940, true),
    ),
    ("vga_480p", &[], system_source(640, 480, 60_000, false)),
];

const fn system_source(
    width: u16,
    height: u16,
    refresh_mhz: u32,
    interlaced: bool,
) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz,
        aspect_milli: 1_333,
        interlaced,
        rotation: Rotation::R0,
    }
}

/// The reference sources that are arcade boards: the policy gives them
/// the arcade rules, as the app does for the systems `arcade` and
/// `neogeo`.
const REFERENCE_BOARDS: [&str; 3] = ["neogeo", "mk", "ddonpach"];

/// The display settings the tool reads, by their key in the settings
/// schema: the settings of the policy, then the screen orientation and the
/// game orientation, which the engine reads.
pub const SETTING_KEYS: [&str; 9] = [
    "display.style",
    "display.refresh_policy",
    "display.tv_width",
    "display.vertical",
    "display.panel_scale",
    "display.scanlines",
    "display.scanline_strength",
    "display.orientation",
    "display.rotation",
];

/// The horizontal blank of a panel mode, in pixels: the 80 pixels of CVT
/// reduced blanking version 2 (front porch 8, sync 32).
const PANEL_HBLANK_PX: u16 = 80;
/// The shortest vertical blank of a panel mode, in microseconds: the 460
/// us of CVT reduced blanking.
const PANEL_VBLANK_MIN_US: u64 = 460;

/// What the screen is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplaySpec {
    /// A preset of `data/monitors`.
    Preset(String),
    /// A TV on a composite encoder with this standard.
    Tv(TvStandard),
    /// A flat panel with one mode at its native size and rate.
    Panel {
        /// Width, in pixels.
        width: u16,
        /// Height, in lines.
        height: u16,
        /// Refresh rate, in millihertz.
        rate_mhz: u32,
    },
}

/// A display ready for the engine: its profile, its GPU caps and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// The name in the output: the preset id, the standard or `WxH@R`.
    pub name: String,
    /// What it is.
    pub spec: DisplaySpec,
    /// The monitor profile the engine drives.
    pub profile: MonitorProfile,
    /// The GPU entry id.
    pub gpu: String,
    /// The caps of that entry for this output.
    pub caps: GpuCaps,
}

/// A source with its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The name in the output.
    pub name: String,
    /// What the core draws.
    pub geometry: SourceGeometry,
    /// An arcade board: the policy takes the arcade rules.
    pub arcade: bool,
    /// The widths that the system of the source draws in play, from which
    /// the policy takes its horizontal strategy
    /// (`neptron_video::wide_strategy`). Empty for no system.
    pub widths: &'static [u16],
}

/// An arcade board of the arcade table of the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// The set name, for example `rtype`.
    pub name: String,
    /// The frame as drawn, and its rate and turn.
    pub geometry: SourceGeometry,
}

/// The display settings of a case, by key, with the schema defaults for
/// the keys that are not given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    values: BTreeMap<&'static str, String>,
}

/// Why a case cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaseError {
    /// The size and rate text is not `WxH@RATE`.
    #[error("{text:?} is not WIDTHxHEIGHT@RATE, for example 1920x1080@60")]
    BadSize {
        /// The text that was given.
        text: String,
    },
    /// No GPU entry has this id.
    #[error("no GPU entry {id:?} in data/hardware/matrix.toml; the entries are: {known}")]
    UnknownGpu {
        /// The id that was given.
        id: String,
        /// Every id.
        known: String,
    },
    /// The capability table does not load.
    #[error("data/hardware/matrix.toml: {0}")]
    Matrix(String),
    /// The source name is no reference source and no arcade set.
    #[error(
        "no source {name:?}: not a reference source ({references}) and not a set of docs/spec/arcade-table.md ({boards})"
    )]
    UnknownSource {
        /// The name that was given.
        name: String,
        /// The reference names.
        references: String,
        /// The arcade set names.
        boards: String,
    },
    /// A setting key is not one the policy reads.
    #[error("no display setting {key:?}; the settings are: {known}")]
    UnknownSetting {
        /// The key that was given.
        key: String,
        /// Every key.
        known: String,
    },
    /// A setting is not `key=value`.
    #[error("{text:?} is not key=value")]
    BadSetting {
        /// The text that was given.
        text: String,
    },
    /// A setting value is not a choice of the schema.
    #[error(transparent)]
    Value(#[from] SettingsError),
    /// The aspect text is not `W:H`, a decimal or `par:W:H`.
    #[error("{text:?} is not an aspect: W:H, a decimal such as 1.333, or par:W:H")]
    BadAspect {
        /// The text that was given.
        text: String,
    },
    /// The turn is not 0, 90, 180 or 270.
    #[error("{text:?} is not a turn: 0, 90, 180 or 270")]
    BadRotation {
        /// The text that was given.
        text: String,
    },
}

impl DisplaySpec {
    /// The display that `text` names: `WxH@RATE` is a flat panel; the key
    /// of a TV standard (`ntsc`, `pal`, ...) is a TV on the composite
    /// output; `preset:ID` or any other id is a preset of `data/monitors`.
    /// `ntsc` and `pal` are also presets of `data/monitors`: write
    /// `preset:ntsc` for the CRT preset.
    ///
    /// # Errors
    ///
    /// [`CaseError::BadSize`] for a panel size that does not parse.
    pub fn parse(text: &str) -> Result<Self, CaseError> {
        if let Some(id) = text.strip_prefix("preset:") {
            return Ok(Self::Preset(id.to_owned()));
        }
        if let Some(standard) = TvStandard::from_key(text) {
            return Ok(Self::Tv(standard));
        }
        if text.contains('x') && text.contains('@') {
            let (width, height, rate_mhz) = size_at_rate(text)?;
            return Ok(Self::Panel {
                width,
                height,
                rate_mhz,
            });
        }
        Ok(Self::Preset(text.to_owned()))
    }

    /// The name in the output.
    pub fn name(&self) -> String {
        match self {
            Self::Preset(id) => id.clone(),
            Self::Tv(standard) => standard.key().to_owned(),
            Self::Panel {
                width,
                height,
                rate_mhz,
            } => format!("{width}x{height}@{}", rate_text(*rate_mhz)),
        }
    }

    /// The GPU entry of this display when none is given.
    pub fn default_gpu(&self) -> &'static str {
        match self {
            Self::Tv(_) => DEFAULT_TV_GPU,
            Self::Preset(_) | Self::Panel { .. } => DEFAULT_GPU,
        }
    }
}

/// The screen for `spec` with the GPU entry `gpu` of `matrix`. A preset
/// profile comes from `load_preset`. A panel gets the widest digital
/// limits of the entry, the limits of its DisplayPort link on the bench
/// PC; a CRT and a TV get the analog caps.
///
/// # Errors
///
/// [`CaseError::UnknownGpu`] for an id that is not in the table, and the
/// error of `load_preset`.
pub fn screen<E>(
    spec: &DisplaySpec,
    gpu: &str,
    matrix: &Matrix,
    load_preset: impl FnOnce(&str) -> Result<MonitorProfile, E>,
) -> Result<Screen, E>
where
    E: From<CaseError>,
{
    let entry = matrix
        .entries()
        .iter()
        .find(|e| e.id == gpu)
        .ok_or_else(|| CaseError::UnknownGpu {
            id: gpu.to_owned(),
            known: matrix
                .entries()
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        })?;
    let (profile, digital) = match spec {
        DisplaySpec::Preset(id) => (load_preset(id)?, false),
        DisplaySpec::Tv(standard) => (standard.profile(), false),
        DisplaySpec::Panel {
            width,
            height,
            rate_mhz,
        } => (
            MonitorProfile {
                id: format!("panel_{width}x{height}"),
                kind: MonitorKind::FlatPanel {
                    modes: vec![panel_mode(*width, *height, *rate_mhz)],
                },
            },
            true,
        ),
    };
    let mut caps = entry.caps;
    if digital && let Some(link) = entry.digital.iter().max_by_key(|d| d.pclock_max_hz) {
        caps.pclock_max_hz = link.pclock_max_hz;
        caps.max_width = link.max_width;
        caps.max_height = link.max_height;
    }
    Ok(Screen {
        name: spec.name(),
        spec: spec.clone(),
        profile,
        gpu: gpu.to_owned(),
        caps,
    })
}

/// The one mode of a panel: CVT reduced blanking v2, 80 pixels of horizontal
/// blank and at least 460 us of vertical blank, the pixel clock rounded
/// to whole kHz as a driver keeps it.
pub fn panel_mode(width: u16, height: u16, rate_mhz: u32) -> Modeline {
    let rate = u64::from(rate_mhz.max(1));
    // Lines of the whole frame: active / (1 - blank time x rate).
    let blank_parts = PANEL_VBLANK_MIN_US * rate;
    let whole = 1_000_000_000_u64.saturating_sub(blank_parts).max(1);
    let vtotal = (u64::from(height) * 1_000_000_000).div_ceil(whole);
    let vtotal = u16::try_from(vtotal.max(u64::from(height) + 6)).unwrap_or(u16::MAX);
    let htotal = width.saturating_add(PANEL_HBLANK_PX);
    let pclock_hz = u64::from(htotal) * u64::from(vtotal) * rate / 1_000;
    let pclock_hz = u32::try_from(pclock_hz.div_ceil(1_000) * 1_000).unwrap_or(u32::MAX);
    Modeline {
        pclock_hz,
        hactive: width,
        hsync_start: width.saturating_add(8),
        hsync_end: width.saturating_add(40),
        htotal,
        vactive: height,
        vsync_start: height.saturating_add(3),
        vsync_end: height.saturating_add(9),
        vtotal,
        interlace: false,
        doublescan: false,
        hsync_positive: true,
        vsync_positive: false,
    }
}

impl Source {
    /// The source `name`: a reference source, or an arcade set of
    /// `boards`.
    ///
    /// # Errors
    ///
    /// [`CaseError::UnknownSource`] when neither has the name.
    pub fn named(name: &str, boards: &[Board]) -> Result<Self, CaseError> {
        if let Some(i) = REFERENCE_NAMES.iter().position(|n| *n == name) {
            return Ok(Self {
                name: REFERENCE_NAMES[i].to_owned(),
                geometry: REFERENCE_SOURCES[i],
                arcade: REFERENCE_BOARDS.contains(&name),
                widths: &[],
            });
        }
        if let Some((name, widths, geometry)) = SYSTEM_SOURCES.iter().find(|s| s.0 == name) {
            return Ok(Self {
                name: (*name).to_owned(),
                geometry: *geometry,
                arcade: false,
                widths,
            });
        }
        if let Some(board) = boards.iter().find(|b| b.name == name) {
            return Ok(Self {
                name: board.name.clone(),
                geometry: board.geometry,
                arcade: true,
                widths: &[],
            });
        }
        Err(CaseError::UnknownSource {
            name: name.to_owned(),
            references: REFERENCE_NAMES
                .iter()
                .chain(SYSTEM_SOURCES.iter().map(|s| &s.0))
                .copied()
                .collect::<Vec<_>>()
                .join(", "),
            boards: boards
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        })
    }

    /// A source given by hand: `WxH@RATE`, a display aspect (`W:H`, a
    /// decimal or the pixel aspect `par:W:H`), a turn and the interlace
    /// flag.
    ///
    /// # Errors
    ///
    /// [`CaseError`] for a size, an aspect or a turn that does not parse.
    pub fn by_hand(
        size: &str,
        aspect: Option<&str>,
        rotation: Option<&str>,
        interlaced: bool,
        arcade: bool,
    ) -> Result<Self, CaseError> {
        let (width, height, refresh_mhz) = size_at_rate(size)?;
        let aspect_milli = match aspect {
            None => 1_333,
            Some(text) => aspect_milli(text, width, height)?,
        };
        let rotation = match rotation {
            None => Rotation::R0,
            Some(text) => rotation_of(text)?,
        };
        Ok(Self {
            name: size.to_owned(),
            geometry: SourceGeometry {
                width,
                height,
                refresh_mhz,
                aspect_milli,
                interlaced,
                rotation,
            },
            arcade,
            widths: &[],
        })
    }
}

/// The boards of the arcade table: every heading
/// `` ### `name`: WxH frame, Rnn, R.RRR Hz `` of `docs/spec/arcade-table.md`.
/// FinalBurn Neo reports the 4:3 aspect of the frame as drawn for every
/// board, and no board is interlaced.
pub fn parse_arcade_table(text: &str) -> Vec<Board> {
    text.lines().filter_map(board_heading).collect()
}

fn board_heading(line: &str) -> Option<Board> {
    let rest = line.strip_prefix("### `")?;
    let (name, rest) = rest.split_once("`: ")?;
    let mut parts = rest.split(", ");
    let size = parts.next()?.strip_suffix(" frame")?;
    let rotation = rotation_of(parts.next()?.strip_prefix('R')?).ok()?;
    let rate = parts.next()?.strip_suffix(" Hz")?;
    let (w, h) = size.split_once('x')?;
    Some(Board {
        name: name.to_owned(),
        geometry: SourceGeometry {
            width: w.parse().ok()?,
            height: h.parse().ok()?,
            refresh_mhz: millis(rate)?,
            aspect_milli: 1_333,
            interlaced: false,
            rotation,
        },
    })
}

impl Settings {
    /// The schema defaults.
    pub fn defaults(schema: &Schema) -> Self {
        let values = SETTING_KEYS
            .iter()
            .map(|&key| {
                let value = schema
                    .get(key)
                    .map(|e| e.kind.default_value().to_string())
                    .unwrap_or_default();
                (key, value)
            })
            .collect();
        Self { values }
    }

    /// Sets `key=value`; the key can leave out `display.`.
    ///
    /// # Errors
    ///
    /// [`CaseError::BadSetting`] or [`CaseError::UnknownSetting`].
    pub fn set(&mut self, text: &str) -> Result<(), CaseError> {
        let (key, value) = text.split_once('=').ok_or_else(|| CaseError::BadSetting {
            text: text.to_owned(),
        })?;
        let full = if key.starts_with("display.") {
            key.to_owned()
        } else {
            format!("display.{key}")
        };
        let key =
            SETTING_KEYS
                .iter()
                .find(|k| **k == full)
                .ok_or_else(|| CaseError::UnknownSetting {
                    key: key.to_owned(),
                    known: SETTING_KEYS.join(", "),
                })?;
        self.values.insert(key, value.to_owned());
        Ok(())
    }

    /// The value of `key`.
    pub fn get(&self, key: &str) -> &str {
        self.values.get(key).map_or("", String::as_str)
    }

    /// The policy settings, by the functions the app uses.
    ///
    /// # Errors
    ///
    /// [`CaseError::Value`] for a value that is not a choice.
    pub fn policy(&self, arcade: bool) -> Result<PolicySettings, CaseError> {
        let settings = policy_settings(
            self.get("display.style"),
            self.get("display.refresh_policy"),
            None,
            self.get("display.tv_width"),
        )?;
        let settings = with_display_choices(
            settings,
            self.get("display.vertical"),
            self.get("display.panel_scale"),
        )?;
        Ok(PolicySettings { arcade, ..settings })
    }

    /// The scanline settings.
    ///
    /// # Errors
    ///
    /// [`CaseError::Value`] for a mode or a strength that the schema
    /// refuses.
    pub fn scanlines(&self) -> Result<ScanlineSettings, CaseError> {
        let strength = self
            .get("display.scanline_strength")
            .parse()
            .unwrap_or(i64::MIN);
        Ok(scanline_settings(self.get("display.scanlines"), strength)?)
    }

    /// The screen orientation and the game orientation, as the app makes
    /// them from the same two settings: the screen at `display.orientation`
    /// degrees counter-clockwise, the game at Auto for
    /// `display.rotation = auto`, else at that turn.
    ///
    /// # Errors
    ///
    /// [`CaseError::BadRotation`] for a value that is not a turn.
    pub fn orientation(&self) -> Result<Orientation, CaseError> {
        let screen = rotation_of(self.get("display.orientation"))?;
        let game_override = match self.get("display.rotation") {
            "auto" => None,
            text => Some(rotation_of(text)?),
        };
        Ok(Orientation {
            screen,
            game_override,
        })
    }

    /// The settings that differ from `defaults`, as `key=value` without
    /// `display.`, joined by spaces; the style and refresh rate always.
    pub fn label(&self, defaults: &Self) -> String {
        let mut parts = vec![format!(
            "{}/{}",
            self.get("display.style"),
            self.get("display.refresh_policy")
        )];
        for key in SETTING_KEYS.iter().skip(2) {
            if self.get(key) != defaults.get(key) {
                let short = key.trim_start_matches("display.");
                parts.push(format!("{short}={}", self.get(key)));
            }
        }
        parts.join(" ")
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let g = &self.geometry;
        write!(
            f,
            "{} {}x{} {:?} {} Hz",
            self.name,
            g.width,
            g.height,
            g.rotation,
            rate_text(g.refresh_mhz)
        )
    }
}

/// `WxH@RATE`, the rate in Hz with up to three decimals.
fn size_at_rate(text: &str) -> Result<(u16, u16, u32), CaseError> {
    let bad = || CaseError::BadSize {
        text: text.to_owned(),
    };
    let (size, rate) = text.split_once('@').ok_or_else(bad)?;
    let (w, h) = size.split_once('x').ok_or_else(bad)?;
    let width: u16 = w.parse().map_err(|_| bad())?;
    let height: u16 = h.parse().map_err(|_| bad())?;
    let rate = millis(rate).ok_or_else(bad)?;
    if width == 0 || height == 0 || rate == 0 {
        return Err(bad());
    }
    Ok((width, height, rate))
}

/// A decimal with up to three decimals, in thousandths.
fn millis(text: &str) -> Option<u32> {
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    if frac.len() > 3 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: u32 = whole.parse().ok()?;
    let frac: u32 = format!("{frac:0<3}").parse().ok()?;
    whole.checked_mul(1_000)?.checked_add(frac)
}

/// The display aspect in thousandths: `W:H`, a decimal, or the pixel
/// aspect `par:W:H` of a `width` by `height` frame.
fn aspect_milli(text: &str, width: u16, height: u16) -> Result<u32, CaseError> {
    let bad = || CaseError::BadAspect {
        text: text.to_owned(),
    };
    let ratio = |t: &str| -> Option<(u64, u64)> {
        let (a, b) = t.split_once(':')?;
        let (a, b): (u64, u64) = (a.parse().ok()?, b.parse().ok()?);
        (a > 0 && b > 0).then_some((a, b))
    };
    let milli = if let Some(par) = text.strip_prefix("par:") {
        let (a, b) = ratio(par).ok_or_else(bad)?;
        (u64::from(width) * a * 1_000 + u64::from(height) * b / 2) / (u64::from(height) * b)
    } else if text.contains(':') {
        let (a, b) = ratio(text).ok_or_else(bad)?;
        a * 1_000 / b
    } else {
        u64::from(millis(text).ok_or_else(bad)?)
    };
    u32::try_from(milli).ok().filter(|m| *m > 0).ok_or_else(bad)
}

fn rotation_of(text: &str) -> Result<Rotation, CaseError> {
    match text {
        "0" => Ok(Rotation::R0),
        "90" => Ok(Rotation::R90),
        "180" => Ok(Rotation::R180),
        "270" => Ok(Rotation::R270),
        _ => Err(CaseError::BadRotation {
            text: text.to_owned(),
        }),
    }
}

/// A rate in millihertz as Hz, with the decimals it has.
pub fn rate_text(mhz: u32) -> String {
    let (whole, frac) = (mhz / 1_000, mhz % 1_000);
    if frac == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{frac:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The widths of the simulator are the widths of the system data.
    #[test]
    fn the_system_widths_are_those_of_the_system_files() {
        let root = crate::golden::workspace_root();
        for (system, widths) in [
            ("snes", &SNES_WIDTHS[..]),
            ("megadrive", &MEGA_DRIVE_WIDTHS),
        ] {
            let path = root.join("data/systems").join(format!("{system}.toml"));
            let text = std::fs::read_to_string(&path).unwrap();
            let file: toml::Table = toml::from_str(&text).unwrap();
            let listed: Vec<u16> = file["widths"]
                .as_array()
                .unwrap()
                .iter()
                .map(|w| u16::try_from(w.as_integer().unwrap()).unwrap())
                .collect();
            assert_eq!(listed, widths, "{system}");
        }
    }

    #[test]
    fn the_names_follow_the_reference_sources() {
        let snes = Source::named("snes", &[]).unwrap();
        assert_eq!(
            (snes.geometry.width, snes.geometry.height, snes.arcade),
            (256, 224, false)
        );
        let ddp = Source::named("ddonpach", &[]).unwrap();
        assert_eq!(ddp.geometry.rotation, Rotation::R90);
        assert!(ddp.arcade);
        assert!(Source::named("psx_480i", &[]).unwrap().geometry.interlaced);
    }

    #[test]
    fn the_arcade_table_gives_rtype() {
        let text = "x\n### `rtype`: 384x256 frame, R0, 55.000 Hz\n\n\
                    ### `pacman`: 288x224 frame, R270, 60.000 Hz\n### bad";
        let boards = parse_arcade_table(text);
        assert_eq!(boards.len(), 2);
        let rtype = Source::named("rtype", &boards).unwrap();
        assert_eq!(
            rtype.geometry,
            SourceGeometry {
                width: 384,
                height: 256,
                refresh_mhz: 55_000,
                aspect_milli: 1_333,
                interlaced: false,
                rotation: Rotation::R0,
            }
        );
        assert!(rtype.arcade);
        assert_eq!(boards[1].geometry.rotation, Rotation::R270);
        let err = Source::named("xmen", &boards).unwrap_err().to_string();
        assert!(err.contains("rtype") && err.contains("snes"), "{err}");
    }

    #[test]
    fn a_display_is_a_panel_a_tv_or_a_preset() {
        assert_eq!(
            DisplaySpec::parse("2560x1440@120").unwrap(),
            DisplaySpec::Panel {
                width: 2560,
                height: 1440,
                rate_mhz: 120_000
            }
        );
        assert_eq!(
            DisplaySpec::parse("ntsc").unwrap(),
            DisplaySpec::Tv(TvStandard::Ntsc)
        );
        assert_eq!(
            DisplaySpec::parse("preset:ntsc").unwrap(),
            DisplaySpec::Preset("ntsc".into())
        );
        assert_eq!(
            DisplaySpec::parse("arcade_15").unwrap(),
            DisplaySpec::Preset("arcade_15".into())
        );
        assert!(DisplaySpec::parse("1920x1080@x").is_err());
        assert_eq!(
            DisplaySpec::parse("1920x1080@59.94").unwrap().name(),
            "1920x1080@59.94"
        );
    }

    #[test]
    fn a_panel_mode_runs_at_its_rate_with_a_whole_khz_clock() {
        for (w, h, rate) in [(1920, 1080, 60_000), (2560, 1440, 120_000)] {
            let mode = panel_mode(w, h, rate);
            assert!(mode.pclock_is_whole_khz());
            let off = mode.field_rate_mhz().abs_diff(u64::from(rate));
            assert!(off <= 100, "{w}x{h}: {} mHz", mode.field_rate_mhz());
            let blank_us =
                u64::from(mode.vtotal - mode.vactive) * 1_000_000_000 / mode.line_rate_mhz();
            assert!(blank_us >= PANEL_VBLANK_MIN_US, "{blank_us}");
        }
    }

    #[test]
    fn the_screen_takes_the_caps_of_its_output() {
        let matrix = Matrix::builtin().unwrap();
        let none = |_: &str| -> Result<MonitorProfile, CaseError> { unreachable!() };
        let tv = DisplaySpec::Tv(TvStandard::Ntsc);
        let s = screen(&tv, tv.default_gpu(), &matrix, none).unwrap();
        assert_eq!(s.caps.pclock_min_hz, 13_500_000);
        assert!(s.caps.interlace);
        let panel = DisplaySpec::parse("2560x1440@120").unwrap();
        let s = screen(&panel, panel.default_gpu(), &matrix, none).unwrap();
        assert_eq!(s.caps.pclock_max_hz, 533_250_000, "DisplayPort");
        let err = screen(&panel, "voodoo", &matrix, none).unwrap_err();
        assert!(matches!(err, CaseError::UnknownGpu { .. }));
    }

    #[test]
    fn settings_start_at_the_schema_defaults_and_take_short_keys() {
        let schema = Schema::builtin();
        let defaults = Settings::defaults(&schema);
        assert_eq!(defaults.get("display.style"), "auto");
        assert_eq!(defaults.get("display.scanline_strength"), "75");
        let mut s = defaults.clone();
        s.set("style=fit").unwrap();
        s.set("display.tv_width=fill").unwrap();
        assert_eq!(s.label(&defaults), "fit/exact tv_width=fill");
        assert!(s.policy(true).unwrap().arcade);
        assert!(matches!(s.set("style"), Err(CaseError::BadSetting { .. })));
        assert!(matches!(
            s.set("volume=3"),
            Err(CaseError::UnknownSetting { .. })
        ));
        s.set("style=5x").unwrap();
        assert!(matches!(s.policy(false), Err(CaseError::Value(_))));
        s.set("scanline_strength=x").unwrap();
        assert!(s.scanlines().is_err());
    }

    #[test]
    fn the_orientation_settings_make_an_orientation() {
        let schema = Schema::builtin();
        let defaults = Settings::defaults(&schema);
        assert_eq!(defaults.orientation(), Ok(Orientation::NORMAL));
        let mut s = defaults.clone();
        s.set("orientation=90").unwrap();
        s.set("display.rotation=270").unwrap();
        assert_eq!(
            s.orientation(),
            Ok(Orientation {
                screen: Rotation::R90,
                game_override: Some(Rotation::R270),
            })
        );
        assert_eq!(s.label(&defaults), "auto/exact orientation=90 rotation=270");
        s.set("orientation=45").unwrap();
        assert!(matches!(
            s.orientation(),
            Err(CaseError::BadRotation { .. })
        ));
        let mut s = defaults;
        s.set("rotation=up").unwrap();
        assert!(matches!(
            s.orientation(),
            Err(CaseError::BadRotation { .. })
        ));
    }

    #[test]
    fn a_source_by_hand_takes_its_aspect() {
        let s = Source::by_hand("256x224@60.099", Some("par:8:7"), None, false, false).unwrap();
        assert_eq!(s.geometry.aspect_milli, 1_306, "256 x 8 / (224 x 7)");
        assert_eq!(s.geometry.refresh_mhz, 60_099);
        let s = Source::by_hand("320x240@60", Some("4:3"), Some("90"), false, true).unwrap();
        assert_eq!(
            (s.geometry.aspect_milli, s.geometry.rotation),
            (1_333, Rotation::R90)
        );
        assert!(Source::by_hand("320x240@60", Some("0:3"), None, false, false).is_err());
        assert!(Source::by_hand("320x240@60", None, Some("45"), false, false).is_err());
        assert_eq!(rate_text(55_000), "55");
        assert_eq!(rate_text(59_940), "59.94");
    }
}
