use std::fmt;

use neptron_types::GpuCaps;

use crate::Settings;

/// The picture style: the rung of the pixel-perfect ladder that a game
/// starts at. In the settings files: `auto`, `native`, `2x`, `3x`, `4x`.
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
}

/// The refresh policy. In the settings files: `exact`, `exact_x2`,
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
}

/// The effective settings that the policy reads, after the game, system
/// and global levels are resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PolicySettings {
    /// The picture style.
    pub style: PictureStyle,
    /// The refresh policy.
    pub refresh: RefreshPolicy,
    /// The horizontal strategy that the system names, or `None` for the
    /// default of the GPU ([`HorizontalStrategy::default_for`]).
    pub horizontal: Option<HorizontalStrategy>,
    /// The settings of the Switchres generator.
    pub solver: Settings,
    /// How wide a game is on a TV profile. Only a TV profile reads it.
    pub tv_width: TvWidth,
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

/// The widths that a fixed super width uses, widest first.
pub const FIXED_SUPER_WIDTHS: [u16; 3] = [3_840, 2_560, 1_920];

impl PictureStyle {
    /// Every style, in the order of the Display panel.
    pub const ALL: [Self; 5] = [Self::Auto, Self::Native, Self::X2, Self::X3, Self::X4];

    /// The value in the settings files.
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::X2 => "2x",
            Self::X3 => "3x",
            Self::X4 => "4x",
        }
    }
}

impl RefreshPolicy {
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
}

impl Default for PolicySettings {
    /// The defaults of the settings schema: `auto` and `exact`, the GPU's
    /// horizontal strategy, the default generator settings, and `aspect`
    /// for a TV.
    fn default() -> Self {
        Self {
            style: PictureStyle::Auto,
            refresh: RefreshPolicy::Exact,
            horizontal: None,
            solver: Settings::default(),
            tv_width: TvWidth::Aspect,
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
}

/// Step 2: read the effective settings. No strategy gives the GPU default.
/// A fixed width of 0 or wider than the GPU becomes the fixed width of the
/// GPU, because the output cannot send it.
pub(crate) fn read_settings(settings: &PolicySettings, caps: &GpuCaps) -> Effective {
    let horizontal = match settings.horizontal {
        None => HorizontalStrategy::default_for(caps),
        Some(HorizontalStrategy::FixedSuperWidth { width })
            if width == 0 || width > caps.max_width =>
        {
            HorizontalStrategy::fixed_for(caps)
        }
        Some(strategy) => strategy,
    };
    Effective {
        style: settings.style,
        refresh: settings.refresh,
        horizontal,
        solver: settings.solver,
        tv_width: settings.tv_width,
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
    fn the_keys_are_the_values_of_the_settings_schema() {
        let styles: Vec<&str> = PictureStyle::ALL.iter().map(|s| s.key()).collect();
        assert_eq!(styles, ["auto", "native", "2x", "3x", "4x"]);
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
