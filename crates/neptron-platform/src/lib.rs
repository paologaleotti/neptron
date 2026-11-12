//! GPU and board detection from sysfs, the capability table, and a monitor
//! profile built from an EDID: a CRT profile from its range limits, or a
//! flat-panel profile from its listed modes.
//!
//! The parsers and the table lookup are pure functions over bytes and
//! strings. Only [`detect`] reads files, from a root path that tests point
//! at a fixture tree.

#![forbid(unsafe_code)]

mod edid;
mod gpu;
mod gtf;
mod matrix;
mod modes;
mod panel;
mod profile;
mod sysfs;

pub use edid::{EDID_BLOCK_LEN, Edid, EdidError, RangeLimits, TimingFormula, parse_edid};
pub use gpu::{
    Connector, ConnectorName, ConnectorStatus, Gpu, ParseError, PciId, Platform, Uevent,
    parse_card_name, parse_connector_name, parse_status, parse_uevent,
};
pub use matrix::{
    DigitalLimits, GpuEntry, MATRIX_VERSION, Matrix, MatrixError, Quirk, UNKNOWN_GPU_CAPS,
    UNKNOWN_GPU_PCLOCK_MIN_HZ,
};
pub use modes::{CTA_MODES, DMT_MODES, ListedModes, listed_modes};
pub use panel::{
    ANALOG_CONNECTOR_KINDS, ANALOG_SUBCONNECTORS, DisplayClass, PanelFit, display_class,
    panel_profile_from_edid, profile_for_display,
};
pub use profile::{EdidProfile, ProfileError, profile_from_edid};
pub use sysfs::{SysfsError, detect};
