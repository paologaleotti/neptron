/// The settings of the Switchres modeline generator that NEPTRON exposes.
///
/// `Settings::default()` holds the defaults of Switchres, except
/// `doublescan`, which is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Settings {
    /// Allow doublescan modes (each line sent twice).
    pub doublescan: bool,
    /// Largest difference between the refresh of a mode and the source that
    /// Switchres does not flag as off, in millihertz.
    pub refresh_tolerance_mhz: u32,
    /// Width from which Switchres treats a mode as a super width, in pixels.
    pub super_width: u16,
    /// The shape of the screen.
    pub monitor_aspect: Aspect,
}

/// A screen shape, as a ratio of two whole numbers: 4:3 or 16:9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aspect {
    /// Width part of the ratio.
    pub width: u16,
    /// Height part of the ratio.
    pub height: u16,
}

impl Default for Settings {
    /// Doublescan is off by default: not every DRM driver accepts a
    /// doublescan mode (i915 refuses it), and the safe default is a mode that
    /// every output can send. The other values are the defaults of
    /// `switchres_manager`.
    fn default() -> Self {
        Self {
            doublescan: false,
            refresh_tolerance_mhz: 2_000,
            super_width: 2_560,
            monitor_aspect: Aspect::STANDARD_CRT,
        }
    }
}

impl Aspect {
    /// 4:3, the shape of a standard CRT.
    pub const STANDARD_CRT: Self = Self {
        width: 4,
        height: 3,
    };
}
