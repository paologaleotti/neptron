use serde::{Deserialize, Serialize};

use crate::{Modeline, TvStandard};

/// One operating range of a CRT: the fields of one Switchres `crt_range`
/// line, in integer units.
///
/// Switchres writes the horizontal porches and sync in microseconds and the
/// vertical ones in milliseconds. This type holds all of them in
/// nanoseconds. Interlaced line limits of 0 and 0 mean that the range
/// accepts no interlaced mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrtRange {
    /// Lowest horizontal line rate, in millihertz.
    pub h_min_mhz: u64,
    /// Highest horizontal line rate, in millihertz.
    pub h_max_mhz: u64,
    /// Lowest vertical field rate, in millihertz.
    pub v_min_mhz: u64,
    /// Highest vertical field rate, in millihertz.
    pub v_max_mhz: u64,
    /// Horizontal front porch, in nanoseconds.
    pub h_front_porch_ns: u32,
    /// Horizontal sync pulse width, in nanoseconds.
    pub h_sync_ns: u32,
    /// Horizontal back porch, in nanoseconds.
    pub h_back_porch_ns: u32,
    /// Vertical front porch, in nanoseconds.
    pub v_front_porch_ns: u32,
    /// Vertical sync pulse width, in nanoseconds.
    pub v_sync_ns: u32,
    /// Vertical back porch, in nanoseconds.
    pub v_back_porch_ns: u32,
    /// The monitor expects a positive horizontal sync pulse.
    pub hsync_positive: bool,
    /// The monitor expects a positive vertical sync pulse.
    pub vsync_positive: bool,
    /// Fewest active lines of a progressive mode.
    pub progressive_lines_min: u16,
    /// Most active lines of a progressive mode.
    pub progressive_lines_max: u16,
    /// Fewest active lines of an interlaced mode, both fields counted.
    pub interlaced_lines_min: u16,
    /// Most active lines of an interlaced mode, both fields counted.
    pub interlaced_lines_max: u16,
}

/// What kind of display a profile describes.
///
/// In TOML and JSON the variant is the key of a table: `kind.crt.ranges`,
/// `kind.flat_panel.modes` or `kind.tv.standard`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorKind {
    /// A CRT that accepts any mode inside one of its ranges.
    Crt {
        /// The operating ranges, in the order of the source.
        ranges: Vec<CrtRange>,
    },
    /// A flat panel that accepts only the listed modes.
    FlatPanel {
        /// The modes the panel accepts.
        modes: Vec<Modeline>,
    },
    /// A TV on a composite encoder that accepts only the two modes of its
    /// standard ([`TvStandard::modes`]).
    Tv {
        /// The broadcast standard of the encoder.
        standard: TvStandard,
    },
}

/// A monitor: a stable id and what it accepts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorProfile {
    /// Stable id, for example the Switchres preset name `generic_15`.
    pub id: String,
    /// The modes the monitor accepts.
    pub kind: MonitorKind,
}
