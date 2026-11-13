use std::fmt;

use neptron_types::{Modeline, TvStandard};

/// Why `validate` refuses a mode. Each variant carries the value it found
/// and the limit it checked the value against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// A timing value is out of order or zero: `value` must not be less
    /// than `limit`. For example `hsync_start` below `hactive`, or `htotal`
    /// 0 against a limit of 1.
    Malformed {
        /// The name of the `Modeline` field.
        field: &'static str,
        /// The value of the field.
        value: u16,
        /// The smallest value the field can have.
        limit: u16,
    },
    /// The horizontal line rate is outside the range.
    LineRate {
        /// The line rate of the mode, in millihertz.
        value_mhz: u64,
        /// Lowest line rate allowed, in millihertz.
        min_mhz: u64,
        /// Highest line rate allowed, in millihertz.
        max_mhz: u64,
    },
    /// The vertical field rate is outside the range.
    FieldRate {
        /// The field rate of the mode, in millihertz.
        value_mhz: u64,
        /// Lowest field rate allowed, in millihertz.
        min_mhz: u64,
        /// Highest field rate allowed, in millihertz.
        max_mhz: u64,
    },
    /// The count of active lines is outside the range.
    LineCount {
        /// The active lines of the mode, both fields counted.
        value: u16,
        /// Fewest lines allowed.
        min: u16,
        /// Most lines allowed.
        max: u16,
        /// The limits are the interlaced ones.
        interlaced: bool,
    },
    /// A sync pulse is too short or too long.
    SyncWidth {
        /// The axis of the pulse.
        axis: Axis,
        /// The pulse width of the mode, in nanoseconds.
        value_ns: u64,
        /// Shortest pulse allowed, in nanoseconds.
        min_ns: u64,
        /// Longest pulse allowed, in nanoseconds.
        max_ns: u64,
    },
    /// A porch is too short.
    Porch {
        /// The axis of the porch.
        axis: Axis,
        /// Front or back porch.
        edge: PorchEdge,
        /// The porch of the mode, in nanoseconds.
        value_ns: u64,
        /// Shortest porch allowed, in nanoseconds.
        min_ns: u64,
    },
    /// The pixel clock is outside what the GPU can make.
    PixelClock {
        /// The pixel clock of the mode, in hertz.
        value_hz: u32,
        /// Lowest pixel clock allowed, in hertz.
        min_hz: u32,
        /// Highest pixel clock allowed, in hertz.
        max_hz: u32,
    },
    /// The mode is interlaced and interlace is not allowed.
    Interlace {
        /// What does not allow interlace.
        forbidden_by: InterlaceLimit,
    },
    /// A compiled-in hard rail refuses the mode.
    HardRail {
        /// The name of the rail.
        rail: &'static str,
        /// The value the rail checked, in the unit the name of the rail
        /// gives.
        value: u64,
        /// The limit of the rail, in the same unit.
        limit: u64,
    },
    /// The profile is a flat panel and the mode is not one of its modes.
    NotAPanelMode {
        /// The mode that was asked for.
        mode: Modeline,
        /// How many modes the panel profile lists.
        panel_modes: usize,
    },
    /// The profile is a TV and the mode is not one of the two modes of
    /// its standard.
    NotATvMode {
        /// The mode that was asked for.
        mode: Modeline,
        /// The standard of the profile.
        standard: TvStandard,
    },
}

/// The axis of a timing value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Along a line.
    Horizontal,
    /// Along a frame.
    Vertical,
}

/// Which porch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PorchEdge {
    /// Between the active area and the sync pulse.
    Front,
    /// Between the sync pulse and the next active area.
    Back,
}

/// What forbids an interlaced mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterlaceLimit {
    /// The GPU or its output cannot send interlace.
    Gpu,
    /// The monitor profile has no interlaced line range.
    Profile,
}

// The crate may depend on neptron-types only, so these impls are written by
// hand and not derived with thiserror.
impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::Malformed {
                field,
                value,
                limit,
            } => write!(f, "{field} is {value}, the minimum is {limit}"),
            Rejection::LineRate {
                value_mhz,
                min_mhz,
                max_mhz,
            } => write!(
                f,
                "line rate {value_mhz} mHz is outside {min_mhz} to {max_mhz} mHz"
            ),
            Rejection::FieldRate {
                value_mhz,
                min_mhz,
                max_mhz,
            } => write!(
                f,
                "field rate {value_mhz} mHz is outside {min_mhz} to {max_mhz} mHz"
            ),
            Rejection::LineCount {
                value,
                min,
                max,
                interlaced,
            } => {
                let scan = if *interlaced {
                    "interlaced"
                } else {
                    "progressive"
                };
                write!(f, "{value} {scan} lines is outside {min} to {max}")
            }
            Rejection::SyncWidth {
                axis,
                value_ns,
                min_ns,
                max_ns,
            } => write!(
                f,
                "{axis} sync {value_ns} ns is outside {min_ns} to {max_ns} ns"
            ),
            Rejection::Porch {
                axis,
                edge,
                value_ns,
                min_ns,
            } => write!(
                f,
                "{axis} {edge} porch {value_ns} ns is below the minimum {min_ns} ns"
            ),
            Rejection::PixelClock {
                value_hz,
                min_hz,
                max_hz,
            } => write!(
                f,
                "pixel clock {value_hz} Hz is outside {min_hz} to {max_hz} Hz"
            ),
            Rejection::Interlace { forbidden_by } => {
                write!(f, "interlace is not allowed by the {forbidden_by}")
            }
            Rejection::HardRail { rail, value, limit } => {
                write!(f, "hard rail {rail}: {value} is beyond the limit {limit}")
            }
            Rejection::NotAPanelMode { mode, panel_modes } => write!(
                f,
                "{}x{} at {} Hz pixel clock is not one of the {panel_modes} panel modes",
                mode.hactive, mode.vactive, mode.pclock_hz
            ),
            Rejection::NotATvMode { mode, standard } => write!(
                f,
                "{}x{} at {} Hz pixel clock is not one of the two {} modes",
                mode.hactive,
                mode.vactive,
                mode.pclock_hz,
                standard.key()
            ),
        }
    }
}

impl std::error::Error for Rejection {}

impl fmt::Display for Axis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Axis::Horizontal => "horizontal",
            Axis::Vertical => "vertical",
        })
    }
}

impl fmt::Display for PorchEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PorchEdge::Front => "front",
            PorchEdge::Back => "back",
        })
    }
}

impl fmt::Display for InterlaceLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InterlaceLimit::Gpu => "GPU",
            InterlaceLimit::Profile => "monitor profile",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_carry_value_and_limit() {
        let cases = [
            (
                Rejection::Malformed {
                    field: "htotal",
                    value: 0,
                    limit: 1,
                },
                "htotal is 0, the minimum is 1",
            ),
            (
                Rejection::LineRate {
                    value_mhz: 31_468_750,
                    min_mhz: 15_625_000,
                    max_mhz: 15_750_000,
                },
                "line rate 31468750 mHz is outside 15625000 to 15750000 mHz",
            ),
            (
                Rejection::FieldRate {
                    value_mhz: 70_000,
                    min_mhz: 49_500,
                    max_mhz: 65_000,
                },
                "field rate 70000 mHz is outside 49500 to 65000 mHz",
            ),
            (
                Rejection::LineCount {
                    value: 480,
                    min: 192,
                    max: 288,
                    interlaced: false,
                },
                "480 progressive lines is outside 192 to 288",
            ),
            (
                Rejection::SyncWidth {
                    axis: Axis::Horizontal,
                    value_ns: 1_000,
                    min_ns: 4_000,
                    max_ns: 5_000,
                },
                "horizontal sync 1000 ns is outside 4000 to 5000 ns",
            ),
            (
                Rejection::Porch {
                    axis: Axis::Vertical,
                    edge: PorchEdge::Back,
                    value_ns: 10,
                    min_ns: 1_024_000,
                },
                "vertical back porch 10 ns is below the minimum 1024000 ns",
            ),
            (
                Rejection::PixelClock {
                    value_hz: 6_000_000,
                    min_hz: 25_000_000,
                    max_hz: 400_000_000,
                },
                "pixel clock 6000000 Hz is outside 25000000 to 400000000 Hz",
            ),
            (
                Rejection::Interlace {
                    forbidden_by: InterlaceLimit::Gpu,
                },
                "interlace is not allowed by the GPU",
            ),
            (
                Rejection::HardRail {
                    rail: "15 kHz class field rate max mHz",
                    value: 130_000,
                    limit: 80_000,
                },
                "hard rail 15 kHz class field rate max mHz: 130000 is beyond the limit 80000",
            ),
            (
                Rejection::NotAPanelMode {
                    mode: neptron_types::Modeline {
                        pclock_hz: 13_500_000,
                        hactive: 720,
                        hsync_start: 739,
                        hsync_end: 801,
                        htotal: 858,
                        vactive: 480,
                        vsync_start: 488,
                        vsync_end: 494,
                        vtotal: 525,
                        interlace: true,
                        doublescan: false,
                        hsync_positive: false,
                        vsync_positive: false,
                    },
                    panel_modes: 3,
                },
                "720x480 at 13500000 Hz pixel clock is not one of the 3 panel modes",
            ),
            (
                Rejection::NotATvMode {
                    mode: neptron_types::TV_625_288P,
                    standard: TvStandard::NtscJ,
                },
                "720x288 at 13500000 Hz pixel clock is not one of the two ntsc_j modes",
            ),
        ];
        for (rejection, message) in cases {
            assert_eq!(rejection.to_string(), message);
        }
    }
}
