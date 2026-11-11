//! The GPU capabilities the spike validates against.
//!
//! The spike does not detect the GPU. It starts from the values of an
//! unknown GPU and widens only the two values the user names on the command
//! line, because measuring them is the purpose of the spike.

use std::fmt;

use neptron_types::{GpuCaps, Modeline};

/// The values of an unknown GPU: 25 MHz lowest pixel clock, no interlace,
/// 165 MHz highest pixel clock, 1920 by 1200.
pub const CONSERVATIVE_CAPS: GpuCaps = GpuCaps {
    pclock_min_hz: 25_000_000,
    pclock_max_hz: 165_000_000,
    interlace: false,
    flip_per_field: false,
    max_width: 1920,
    max_height: 1200,
};

/// What the user allows beyond the conservative values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Allowances {
    /// A lower minimum pixel clock, in hertz (`--allow-low-pclock`).
    pub pclock_min_hz: Option<u32>,
    /// Interlaced modes (`--allow-interlace`).
    pub interlace: bool,
}

/// A way a mode is outside the conservative values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beyond {
    /// The pixel clock is below the conservative minimum.
    LowPixelClock {
        /// The pixel clock of the mode, in hertz.
        pclock_hz: u32,
    },
    /// The mode is interlaced.
    Interlace,
}

impl Allowances {
    /// The capabilities that `validate` gets: the conservative values with
    /// the allowances applied.
    pub fn caps(&self) -> GpuCaps {
        GpuCaps {
            pclock_min_hz: self
                .pclock_min_hz
                .unwrap_or(CONSERVATIVE_CAPS.pclock_min_hz),
            interlace: self.interlace,
            ..CONSERVATIVE_CAPS
        }
    }
}

/// Every way `mode` is outside the conservative values.
pub fn beyond_conservative(mode: &Modeline) -> Vec<Beyond> {
    let mut out = Vec::new();
    if mode.pclock_hz < CONSERVATIVE_CAPS.pclock_min_hz {
        out.push(Beyond::LowPixelClock {
            pclock_hz: mode.pclock_hz,
        });
    }
    if mode.interlace && !CONSERVATIVE_CAPS.interlace {
        out.push(Beyond::Interlace);
    }
    out
}

impl Beyond {
    /// The command-line flag that allows it.
    pub fn flag(&self) -> &'static str {
        match self {
            Beyond::LowPixelClock { .. } => "--allow-low-pclock <HZ>",
            Beyond::Interlace => "--allow-interlace",
        }
    }
}

impl fmt::Display for Beyond {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Beyond::LowPixelClock { pclock_hz } => write!(
                f,
                "pixel clock {} Hz is below the conservative minimum of {} Hz",
                pclock_hz, CONSERVATIVE_CAPS.pclock_min_hz
            ),
            Beyond::Interlace => f.write_str("interlace is outside the conservative capabilities"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(pclock_hz: u32, interlace: bool) -> Modeline {
        Modeline {
            pclock_hz,
            hactive: 640,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 525,
            interlace,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn no_allowance_gives_the_conservative_caps() {
        assert_eq!(Allowances::default().caps(), CONSERVATIVE_CAPS);
    }

    #[test]
    fn allowances_change_only_their_own_value() {
        let caps = Allowances {
            pclock_min_hz: Some(6_000_000),
            interlace: true,
        }
        .caps();
        assert_eq!(caps.pclock_min_hz, 6_000_000);
        assert!(caps.interlace);
        assert_eq!(caps.pclock_max_hz, CONSERVATIVE_CAPS.pclock_max_hz);
        assert_eq!(caps.max_width, CONSERVATIVE_CAPS.max_width);
        assert!(!caps.flip_per_field);
    }

    #[test]
    fn beyond_lists_each_reason() {
        assert!(beyond_conservative(&mode(25_000_000, false)).is_empty());
        assert_eq!(
            beyond_conservative(&mode(24_999_999, true)),
            [
                Beyond::LowPixelClock {
                    pclock_hz: 24_999_999
                },
                Beyond::Interlace
            ]
        );
    }

    #[test]
    fn each_reason_names_its_flag() {
        assert_eq!(Beyond::Interlace.flag(), "--allow-interlace");
        let low = Beyond::LowPixelClock { pclock_hz: 6 };
        assert_eq!(low.flag(), "--allow-low-pclock <HZ>");
        assert!(low.to_string().contains("25000000 Hz"));
    }
}
