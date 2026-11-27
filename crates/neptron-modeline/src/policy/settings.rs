use std::fmt;

use neptron_types::GpuCaps;

use super::OrientedSource;
use crate::Settings;

/// The picture mode: the rung of the pixel-perfect ladder that a game
/// starts at, or the trade that the player picks. In the settings files:
/// `auto`, `native`, `2x`, `3x`, `4x`, `sharp`, `speed`, `fit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PictureStyle {
    /// Every rung in order: native on a 15 kHz monitor, line doubled on a
    /// 31 kHz monitor.
    Auto,
    /// The game's own line count.
    Native,
    /// Twice the game's line count.
    X2,
    /// Three times the game's line count.
    X3,
    /// Four times the game's line count.
    X4,
    /// Every line exact: the walk of auto, with every exact mode before the
    /// fit at any speed change. `display.vertical` reads as exact, and a
    /// flat panel takes the largest whole multiple at every scale.
    Sharp,
    /// The game's own speed first: the walk of auto, where an exact mode at
    /// a slower window rate wins over the fit only when it changes the
    /// speed no more than the fit. It takes no doubled rate of its own. A
    /// flat panel takes the mode nearest the game's rate first, after the
    /// modes that must shrink the game.
    Speed,
    /// The picture fills the height with no flicker: for a progressive
    /// source, every interlaced 1:1 mode and every small exact picture
    /// drop, as `display.vertical = fit` does for a turned game. A flat
    /// panel fills at every scale.
    Fit,
}

/// The refresh choice (the Refresh rate row). In the settings files: `exact`, `exact_x2`,
/// `force_50`, `force_60`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefreshPolicy {
    /// The game's own rate.
    Exact,
    /// Twice the game's rate, each game frame shown twice.
    ExactX2,
    /// 50 Hz; the game speed changes by the ratio.
    Force50,
    /// 60 Hz; the game speed changes by the ratio.
    Force60,
}

/// How the policy chooses the width of a CRT mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HorizontalStrategy {
    /// The mode width is the game width.
    Native,
    /// The smallest whole multiple of the game width whose pixel clock is
    /// at least the GPU's lowest pixel clock: an exact integer scale.
    DynamicSuperWidth,
    /// One wide mode for every game; the game is scaled horizontally with a
    /// sharp filter, so a width change during play needs no modeset.
    FixedSuperWidth {
        /// Width of the mode, in pixels.
        width: u16,
    },
    /// One mode for every width of a game whose system changes its width
    /// in play at one line timing, as the console does with no new sync: a
    /// whole multiple of the widths that the GPU can send
    /// ([`HorizontalStrategy::wide_base`]), so a width change needs no
    /// modeset. Each candidate keeps the mode of the GPU's default strategy
    /// right after it, for the validator.
    Wide {
        /// The widths of the system, from its data.
        widths: GameWidths,
    },
}

/// The most widths that a system names.
pub const GAME_WIDTHS_MAX: usize = 8;

/// The widths in pixels that the games of a system draw in play, at one
/// line timing, the most used first: the key `widths` of the system data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GameWidths {
    widths: [u16; GAME_WIDTHS_MAX],
    len: u8,
}

/// Why a list of widths is not a [`GameWidths`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidthsError {
    /// The list has more widths than [`GAME_WIDTHS_MAX`].
    TooMany {
        /// Widths in the list.
        count: usize,
        /// The most widths.
        max: usize,
    },
    /// A width is 0.
    Zero {
        /// The place of the width in the list, from 0.
        index: usize,
    },
}

/// The effective settings that the policy reads, after the game, system
/// and global levels are resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PolicySettings {
    /// The picture mode.
    pub style: PictureStyle,
    /// The refresh choice.
    pub refresh: RefreshPolicy,
    /// The horizontal strategy that the system names, or `None` for the
    /// default of the GPU ([`HorizontalStrategy::default_for`]).
    pub horizontal: Option<HorizontalStrategy>,
    /// The settings of the Switchres generator.
    pub solver: Settings,
    /// How wide a game is on a TV profile. Only a TV profile reads it.
    pub tv_width: TvWidth,
    /// How a flat panel scales a game. Only a flat panel reads it.
    pub panel_scale: PanelScale,
    /// How a turned game shows on a horizontal display.
    pub vertical: VerticalMode,
    /// Scanlines show on a flat panel (`display.scanlines = on` with a
    /// strength above 0). The renderer draws them only at a whole vertical
    /// scale of 2 or more, so the panel then takes the largest whole
    /// multiple at every scale and never the fill. Only a flat panel reads
    /// it; the video engine sets it from its scanline settings.
    pub panel_scanlines: bool,
    /// The source is an arcade board (the systems `arcade` and `neogeo`).
    /// A board fills its monitor, so on a TV it gets the width of its
    /// aspect.
    pub arcade: bool,
}

/// How wide the game is on a TV profile, in samples of the 720-sample
/// line. In the settings files: `aspect`, `integer`, `fill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TvWidth {
    /// The width of the console picture at the game's display aspect,
    /// snapped to a whole factor of the game width when one is near.
    #[default]
    Aspect,
    /// The largest whole factor of the game width that fits the safe
    /// width: every pixel has the same width.
    Integer,
    /// The full safe width.
    Fill,
}

/// How a flat panel scales a game. In the settings files: `auto`,
/// `integer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PanelScale {
    /// The game fills the height when that scale is at least
    /// `PANEL_FILL_MIN_SCALE_MILLI`, with a sharp filter and its shape
    /// kept; under it, the largest whole multiple that fits.
    #[default]
    Auto,
    /// The largest whole multiple that fits, at every scale.
    Integer,
}

/// How a turned game (a vertical game on a horizontal display) shows. In
/// the settings files: `exact`, `fit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VerticalMode {
    /// Every pixel exact: the rule of every other game, with black borders,
    /// and on a 15 kHz screen or a TV interlaced.
    #[default]
    Exact,
    /// The picture fills the height of the screen with its shape kept,
    /// scaled with a filter; on a 15 kHz screen or a TV progressive.
    Fit,
}

/// The widths that a fixed super width uses, widest first.
pub const FIXED_SUPER_WIDTHS: [u16; 3] = [3_840, 2_560, 1_920];

impl GameWidths {
    /// The widths of `list`, in its order.
    ///
    /// # Errors
    ///
    /// [`WidthsError`] for more than [`GAME_WIDTHS_MAX`] widths or a width
    /// of 0.
    pub fn from_slice(list: &[u16]) -> Result<Self, WidthsError> {
        if list.len() > GAME_WIDTHS_MAX {
            return Err(WidthsError::TooMany {
                count: list.len(),
                max: GAME_WIDTHS_MAX,
            });
        }
        if let Some(index) = list.iter().position(|&w| w == 0) {
            return Err(WidthsError::Zero { index });
        }
        let mut widths = [0; GAME_WIDTHS_MAX];
        widths[..list.len()].copy_from_slice(list);
        // The length is at most GAME_WIDTHS_MAX, checked above.
        let len = u8::try_from(list.len()).unwrap_or(u8::MAX);
        Ok(Self { widths, len })
    }

    /// The widths, in the order of the data.
    pub fn as_slice(&self) -> &[u16] {
        &self.widths[..usize::from(self.len)]
    }
}

impl fmt::Display for WidthsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany { count, max } => write!(f, "{count} widths, at most {max}"),
            Self::Zero { index } => write!(f, "width {index} is 0"),
        }
    }
}

impl std::error::Error for WidthsError {}

impl PictureStyle {
    /// Every style, in the order of the settings schema and of the menus.
    pub const ALL: [Self; 8] = [
        Self::Auto,
        Self::Native,
        Self::X2,
        Self::X3,
        Self::X4,
        Self::Sharp,
        Self::Speed,
        Self::Fit,
    ];

    /// The value in the settings files.
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::X2 => "2x",
            Self::X3 => "3x",
            Self::X4 => "4x",
            Self::Sharp => "sharp",
            Self::Speed => "speed",
            Self::Fit => "fit",
        }
    }

    /// The whole factor that the style asks for: 1 for `native`, 2 to 4
    /// for a multiple, `None` for the other styles.
    pub fn factor(self) -> Option<u16> {
        match self {
            Self::Native => Some(1),
            Self::X2 => Some(2),
            Self::X3 => Some(3),
            Self::X4 => Some(4),
            Self::Auto | Self::Sharp | Self::Speed | Self::Fit => None,
        }
    }
}

impl RefreshPolicy {
    /// Every refresh choice, in the order of the settings schema.
    pub const ALL: [Self; 4] = [Self::Exact, Self::ExactX2, Self::Force50, Self::Force60];

    /// The value in the settings files.
    pub fn key(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::ExactX2 => "exact_x2",
            Self::Force50 => "force_50",
            Self::Force60 => "force_60",
        }
    }
}

impl TvWidth {
    /// Every value, in the order of the Display panel.
    pub const ALL: [Self; 3] = [Self::Aspect, Self::Integer, Self::Fill];

    /// The value in the settings files.
    pub fn key(self) -> &'static str {
        match self {
            Self::Aspect => "aspect",
            Self::Integer => "integer",
            Self::Fill => "fill",
        }
    }
}

impl PanelScale {
    /// Every value, in the order of the Display panel.
    pub const ALL: [Self; 2] = [Self::Auto, Self::Integer];

    /// The value in the settings files.
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Integer => "integer",
        }
    }
}

impl VerticalMode {
    /// Every value, in the order of the Display panel.
    pub const ALL: [Self; 2] = [Self::Exact, Self::Fit];

    /// The value in the settings files.
    pub fn key(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Fit => "fit",
        }
    }
}

impl fmt::Display for PanelScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl fmt::Display for VerticalMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl HorizontalStrategy {
    /// The default for a GPU: a GPU with a pixel clock floor needs a super
    /// width, and a GPU without one sends the game width.
    pub fn default_for(caps: &GpuCaps) -> Self {
        if caps.pclock_min_hz > 0 {
            Self::DynamicSuperWidth
        } else {
            Self::Native
        }
    }

    /// The fixed super width for a GPU: the widest of
    /// [`FIXED_SUPER_WIDTHS`] that the GPU can send, or its widest mode.
    pub fn fixed_for(caps: &GpuCaps) -> Self {
        let width = FIXED_SUPER_WIDTHS
            .into_iter()
            .find(|&w| w <= caps.max_width)
            .unwrap_or(caps.max_width);
        Self::FixedSuperWidth { width }
    }

    /// The base of the wide mode for `widths` on a GPU of `max_width`
    /// pixels: the least common multiple of the longest run of `widths`,
    /// from the first, that is at most `max_width`. `None` when the first
    /// width is 0 or wider than the GPU.
    pub fn wide_base(widths: &[u16], max_width: u16) -> Option<u16> {
        let mut base: Option<u64> = None;
        for &width in widths {
            let width = u64::from(width);
            if width == 0 {
                break;
            }
            let next = match base {
                None => width,
                Some(b) => b / gcd(b, width) * width,
            };
            if next > u64::from(max_width) {
                break;
            }
            base = Some(next);
        }
        base.and_then(|b| u16::try_from(b).ok())
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl PolicySettings {
    /// The vertical mode the policy reads: the `sharp` mode reads the
    /// expert key `display.vertical` as exact, every other mode as it is.
    /// The app reads the same value for the notice of a vertical game.
    pub fn read_vertical(&self) -> VerticalMode {
        match self.style {
            PictureStyle::Sharp => VerticalMode::Exact,
            PictureStyle::Auto
            | PictureStyle::Native
            | PictureStyle::X2
            | PictureStyle::X3
            | PictureStyle::X4
            | PictureStyle::Speed
            | PictureStyle::Fit => self.vertical,
        }
    }

    /// The panel scale the policy reads: the `sharp` mode reads the expert
    /// key `display.panel_scale` as integer, every other mode as it is.
    pub fn read_panel_scale(&self) -> PanelScale {
        match self.style {
            PictureStyle::Sharp => PanelScale::Integer,
            PictureStyle::Auto
            | PictureStyle::Native
            | PictureStyle::X2
            | PictureStyle::X3
            | PictureStyle::X4
            | PictureStyle::Speed
            | PictureStyle::Fit => self.panel_scale,
        }
    }
}

impl Default for PolicySettings {
    /// The defaults of the settings schema: `auto` and `exact`, the GPU's
    /// horizontal strategy, the default generator settings, `aspect` for a
    /// TV, `auto` for a panel, `exact` for a turned game, and a source that
    /// is not an arcade board.
    fn default() -> Self {
        Self {
            style: PictureStyle::Auto,
            refresh: RefreshPolicy::Exact,
            horizontal: None,
            solver: Settings::default(),
            tv_width: TvWidth::Aspect,
            panel_scale: PanelScale::Auto,
            vertical: VerticalMode::Exact,
            panel_scanlines: false,
            arcade: false,
        }
    }
}

impl fmt::Display for PictureStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl fmt::Display for RefreshPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl fmt::Display for TvWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl fmt::Display for HorizontalStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native => f.write_str("native width"),
            Self::DynamicSuperWidth => f.write_str("dynamic super width"),
            Self::FixedSuperWidth { width } => write!(f, "fixed super width {width}"),
            Self::Wide { .. } => f.write_str("wide width"),
        }
    }
}

/// The settings after step 2: the horizontal strategy is resolved for the
/// GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Effective {
    pub(crate) style: PictureStyle,
    pub(crate) refresh: RefreshPolicy,
    pub(crate) horizontal: HorizontalStrategy,
    pub(crate) solver: Settings,
    pub(crate) tv_width: TvWidth,
    pub(crate) panel_scale: PanelScale,
    pub(crate) vertical: VerticalMode,
    pub(crate) panel_scanlines: bool,
    pub(crate) arcade: bool,
}

impl Effective {
    /// The fit is asked for `source`: `display.vertical = fit` for a turned
    /// game, or the `fit` style for a progressive game. An interlaced game
    /// keeps its interlaced mode under the `fit` style, because its lines
    /// need it.
    pub(crate) fn asks_fit(&self, source: &OrientedSource) -> bool {
        let vertical = source.turned && self.vertical == VerticalMode::Fit;
        let style = self.style == PictureStyle::Fit && !source.interlaced;
        vertical || style
    }
}

/// Step 2: read the effective settings. No strategy gives the GPU default.
/// A fixed width of 0 or wider than the GPU becomes the fixed width of the
/// GPU, because the output cannot send it. The `sharp` style reads
/// `display.vertical` as exact and `display.panel_scale` as integer: the
/// picture mode wins over the expert keys that it covers.
pub(crate) fn read_settings(settings: &PolicySettings, caps: &GpuCaps) -> Effective {
    let horizontal = match settings.horizontal {
        None => HorizontalStrategy::default_for(caps),
        Some(HorizontalStrategy::FixedSuperWidth { width })
            if width == 0 || width > caps.max_width =>
        {
            HorizontalStrategy::fixed_for(caps)
        }
        // One width has nothing to share a mode with.
        Some(HorizontalStrategy::Wide { widths }) if widths.as_slice().len() < 2 => {
            HorizontalStrategy::default_for(caps)
        }
        Some(strategy) => strategy,
    };
    Effective {
        style: settings.style,
        refresh: settings.refresh,
        horizontal,
        solver: settings.solver,
        tv_width: settings.tv_width,
        panel_scale: settings.read_panel_scale(),
        vertical: settings.read_vertical(),
        panel_scanlines: settings.panel_scanlines,
        arcade: settings.arcade,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(pclock_min_hz: u32, max_width: u16) -> GpuCaps {
        GpuCaps {
            pclock_min_hz,
            pclock_max_hz: 165_000_000,
            interlace: false,
            flip_per_field: false,
            max_width,
            max_height: 1_200,
        }
    }

    fn with(horizontal: Option<HorizontalStrategy>) -> PolicySettings {
        PolicySettings {
            horizontal,
            ..PolicySettings::default()
        }
    }

    #[test]
    fn a_gpu_with_a_pixel_clock_floor_defaults_to_a_dynamic_super_width() {
        let intel = caps(25_000_000, 1_920);
        assert_eq!(
            read_settings(&with(None), &intel).horizontal,
            HorizontalStrategy::DynamicSuperWidth
        );
    }

    #[test]
    fn a_gpu_without_a_floor_defaults_to_the_native_width() {
        let dpi = caps(0, 2_048);
        assert_eq!(
            read_settings(&with(None), &dpi).horizontal,
            HorizontalStrategy::Native
        );
    }

    #[test]
    fn a_named_strategy_wins_over_the_gpu_default() {
        let intel = caps(25_000_000, 1_920);
        assert_eq!(
            read_settings(&with(Some(HorizontalStrategy::Native)), &intel).horizontal,
            HorizontalStrategy::Native
        );
    }

    #[test]
    fn the_wide_base_is_the_common_multiple_of_the_widths_the_gpu_can_send() {
        // 320 and 256 give 1280; 640 keeps it; 512 needs 2560.
        let playstation = [320, 256, 640, 512, 368];
        assert_eq!(
            HorizontalStrategy::wide_base(&playstation, 1_920),
            Some(1_280)
        );
        assert_eq!(
            HorizontalStrategy::wide_base(&playstation, 2_560),
            Some(2_560)
        );
        assert_eq!(
            HorizontalStrategy::wide_base(&[320, 256], 1_920),
            Some(1_280)
        );
        assert_eq!(HorizontalStrategy::wide_base(&[256, 512], 1_920), Some(512));
        assert_eq!(HorizontalStrategy::wide_base(&[2_048], 1_920), None);
        assert_eq!(HorizontalStrategy::wide_base(&[0, 320], 1_920), None);
        assert_eq!(HorizontalStrategy::wide_base(&[], 1_920), None);
    }

    #[test]
    fn a_wide_strategy_with_fewer_than_two_widths_is_the_gpu_default() {
        let intel = caps(25_000_000, 1_920);
        for list in [&[][..], &[256]] {
            let widths = GameWidths::from_slice(list).unwrap();
            let one = Some(HorizontalStrategy::Wide { widths });
            assert_eq!(
                read_settings(&with(one), &intel).horizontal,
                HorizontalStrategy::DynamicSuperWidth
            );
        }
        let snes = HorizontalStrategy::Wide {
            widths: GameWidths::from_slice(&[256, 512]).unwrap(),
        };
        assert_eq!(read_settings(&with(Some(snes)), &intel).horizontal, snes);
    }

    #[test]
    fn game_widths_keep_their_order_and_refuse_a_bad_list() {
        let widths = GameWidths::from_slice(&[320, 256]).unwrap();
        assert_eq!(widths.as_slice(), [320, 256]);
        assert_eq!(
            GameWidths::from_slice(&[1; 9]),
            Err(WidthsError::TooMany { count: 9, max: 8 })
        );
        assert_eq!(
            GameWidths::from_slice(&[256, 0]),
            Err(WidthsError::Zero { index: 1 })
        );
    }

    #[test]
    fn a_fixed_width_wider_than_the_gpu_becomes_the_gpu_fixed_width() {
        let intel = caps(25_000_000, 1_920);
        let wide = Some(HorizontalStrategy::FixedSuperWidth { width: 2_560 });
        assert_eq!(
            read_settings(&with(wide), &intel).horizontal,
            HorizontalStrategy::FixedSuperWidth { width: 1_920 }
        );
        let zero = Some(HorizontalStrategy::FixedSuperWidth { width: 0 });
        assert_eq!(
            read_settings(&with(zero), &intel).horizontal,
            HorizontalStrategy::FixedSuperWidth { width: 1_920 }
        );
    }

    #[test]
    fn the_fixed_width_is_the_widest_standard_width_the_gpu_sends() {
        assert_eq!(
            HorizontalStrategy::fixed_for(&caps(0, 4_096)),
            HorizontalStrategy::FixedSuperWidth { width: 3_840 }
        );
        assert_eq!(
            HorizontalStrategy::fixed_for(&caps(0, 2_560)),
            HorizontalStrategy::FixedSuperWidth { width: 2_560 }
        );
        assert_eq!(
            HorizontalStrategy::fixed_for(&caps(0, 1_600)),
            HorizontalStrategy::FixedSuperWidth { width: 1_600 }
        );
    }

    #[test]
    fn sharp_reads_the_expert_keys_as_exact_and_integer() {
        let intel = caps(25_000_000, 1_920);
        let asked = PolicySettings {
            vertical: VerticalMode::Fit,
            panel_scale: PanelScale::Auto,
            ..PolicySettings::default()
        };
        for style in PictureStyle::ALL {
            let effective = read_settings(&PolicySettings { style, ..asked }, &intel);
            if style == PictureStyle::Sharp {
                assert_eq!(effective.vertical, VerticalMode::Exact);
                assert_eq!(effective.panel_scale, PanelScale::Integer);
            } else {
                assert_eq!(effective.vertical, VerticalMode::Fit, "{style}");
                assert_eq!(effective.panel_scale, PanelScale::Auto, "{style}");
            }
        }
    }

    #[test]
    fn fit_asks_the_fit_for_a_progressive_game_and_vertical_fit_for_a_turned_one() {
        let source = |turned, interlaced| OrientedSource {
            width: 320,
            height: 240,
            refresh_mhz: 60_000,
            interlaced,
            aspect_milli: 1_333,
            frame_width: 320,
            frame_height: 240,
            turned,
        };
        let gpu = caps(0, 2_048);
        let with = |style, vertical| {
            read_settings(
                &PolicySettings {
                    style,
                    vertical,
                    ..PolicySettings::default()
                },
                &gpu,
            )
        };
        let fit = with(PictureStyle::Fit, VerticalMode::Exact);
        assert!(fit.asks_fit(&source(false, false)));
        assert!(fit.asks_fit(&source(true, false)));
        assert!(!fit.asks_fit(&source(false, true)), "interlaced keeps 1:1");
        let auto = with(PictureStyle::Auto, VerticalMode::Fit);
        assert!(auto.asks_fit(&source(true, false)));
        assert!(!auto.asks_fit(&source(false, false)));
        let sharp = with(PictureStyle::Sharp, VerticalMode::Fit);
        assert!(!sharp.asks_fit(&source(true, false)));
    }

    #[test]
    fn only_native_and_the_multiples_name_a_factor() {
        let factors = PictureStyle::ALL.map(PictureStyle::factor);
        assert_eq!(
            factors,
            [None, Some(1), Some(2), Some(3), Some(4), None, None, None]
        );
    }

    #[test]
    fn the_keys_are_the_values_of_the_settings_schema() {
        let styles: Vec<&str> = PictureStyle::ALL.iter().map(|s| s.key()).collect();
        assert_eq!(
            styles,
            ["auto", "native", "2x", "3x", "4x", "sharp", "speed", "fit"]
        );
        let policies = [
            RefreshPolicy::Exact,
            RefreshPolicy::ExactX2,
            RefreshPolicy::Force50,
            RefreshPolicy::Force60,
        ]
        .map(RefreshPolicy::key);
        assert_eq!(policies, ["exact", "exact_x2", "force_50", "force_60"]);
        let widths: Vec<&str> = TvWidth::ALL.iter().map(|w| w.key()).collect();
        assert_eq!(widths, ["aspect", "integer", "fill"]);
        assert_eq!(PolicySettings::default().tv_width, TvWidth::Aspect);
    }
}
