use std::fmt;

use super::units::Hz;

/// Why the policy drops a candidate before the validator sees it. Each
/// variant names its rule and carries the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dropped {
    /// Rung 3 is only for a source whose lines fit no progressive range.
    FitsProgressive {
        /// Lines of the source.
        lines: u16,
        /// Most progressive lines of any range of the profile.
        progressive_max: u16,
    },
    /// Rung 3 of an interlaced source asks for the same mode as rung 1.
    SameAsNative,
    /// The GPU cannot send an interlaced mode.
    NoInterlace,
    /// The game has fewer lines than the ranges at the rate allow, and no
    /// border height holds it.
    NoBorderMode {
        /// Lines of the game.
        lines: u16,
        /// Fewest lines the ranges allow at the rate.
        line_min: u16,
    },
    /// The mode needs more lines than the GPU sends.
    TallerThanGpu {
        /// Lines of the mode.
        lines: u32,
        /// Most lines the GPU sends.
        max_lines: u16,
    },
    /// No whole multiple of the game width reaches the GPU's lowest pixel
    /// clock within the widest mode the GPU sends.
    PixelClockFloor {
        /// Widest mode tried, in pixels.
        width: u32,
        /// Its pixel clock, in hertz.
        pclock_hz: u32,
        /// The lowest pixel clock of the GPU, in hertz.
        floor_hz: u32,
        /// Widest mode the GPU sends, in pixels.
        max_width: u16,
    },
    /// Switchres has no mode for the request on any range.
    NoMode,
    /// Switchres changed the line count: the request does not fit a range
    /// at its own line count.
    LinesChanged {
        /// Lines asked for.
        asked: u16,
        /// Lines Switchres gave.
        got: u16,
    },
    /// Switchres changed the scan: the lines fit a range only with the
    /// other scan.
    ScanChanged {
        /// The request was interlaced.
        asked_interlace: bool,
    },
    /// Switchres changed the width.
    WidthChanged {
        /// Width asked for.
        asked: u16,
        /// Width Switchres gave.
        got: u16,
    },
    /// The field rate of the mode does not match its target: the range
    /// cannot reach the rate with this line count.
    RefreshMissed {
        /// The target field rate, in millihertz.
        target_mhz: u32,
        /// The field rate of the mode, in millihertz.
        got_mhz: u64,
    },
}

impl Dropped {
    /// The name of the rule that drops the candidate.
    pub fn rule(&self) -> &'static str {
        match self {
            Self::FitsProgressive { .. } => "fits_progressive",
            Self::SameAsNative => "same_as_native",
            Self::NoInterlace => "no_interlace",
            Self::NoBorderMode { .. } => "no_border_mode",
            Self::TallerThanGpu { .. } => "taller_than_gpu",
            Self::PixelClockFloor { .. } => "pixel_clock_floor",
            Self::NoMode => "no_mode",
            Self::LinesChanged { .. } => "lines_changed",
            Self::ScanChanged { .. } => "scan_changed",
            Self::WidthChanged { .. } => "width_changed",
            Self::RefreshMissed { .. } => "refresh_missed",
        }
    }
}

impl fmt::Display for Dropped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.rule())?;
        match *self {
            Self::FitsProgressive {
                lines,
                progressive_max,
            } => write!(
                f,
                "{lines} lines fit the progressive limit of {progressive_max}"
            ),
            Self::SameAsNative => {
                f.write_str("the source is interlaced; rung 1 asks for this mode")
            }
            Self::NoInterlace => f.write_str("the GPU sends no interlaced mode"),
            Self::NoBorderMode { lines, line_min } => write!(
                f,
                "{lines} lines, below the minimum of {line_min}, and no border mode of \
                 224 or 240 lines reaches it"
            ),
            Self::TallerThanGpu { lines, max_lines } => {
                write!(f, "{lines} lines, the GPU sends at most {max_lines}")
            }
            Self::PixelClockFloor {
                width,
                pclock_hz,
                floor_hz,
                max_width,
            } => write!(
                f,
                "{width} pixels give {pclock_hz} Hz, below the floor of {floor_hz} Hz, \
                 and the GPU sends at most {max_width} pixels"
            ),
            Self::NoMode => f.write_str("no range of the profile has a mode"),
            Self::LinesChanged { asked, got } => {
                write!(f, "asked for {asked} lines, the range gives {got}")
            }
            Self::ScanChanged { asked_interlace } => {
                let (asked, got) = if asked_interlace {
                    ("interlaced", "progressive")
                } else {
                    ("progressive", "interlaced")
                };
                write!(f, "asked for {asked}, the range gives {got}")
            }
            Self::WidthChanged { asked, got } => {
                write!(f, "asked for {asked} pixels, the range gives {got}")
            }
            Self::RefreshMissed {
                target_mhz,
                got_mhz,
            } => write!(
                f,
                "aimed for {}, the range reaches {}",
                Hz(u64::from(target_mhz)),
                Hz(got_mhz)
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reason_names_its_rule_and_its_numbers() {
        let missed = Dropped::RefreshMissed {
            target_mhz: 60_099,
            got_mhz: 54_321,
        };
        assert_eq!(
            missed.to_string(),
            "refresh_missed: aimed for 60.099 Hz, the range reaches 54.321 Hz"
        );
        let lines = Dropped::LinesChanged {
            asked: 224,
            got: 448,
        };
        assert_eq!(
            lines.to_string(),
            "lines_changed: asked for 224 lines, the range gives 448"
        );
    }
}
