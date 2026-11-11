//! GPU and board detection from sysfs, the capability table, and a monitor
//! profile built from the range limits in an EDID.

#![forbid(unsafe_code)]

mod edid;
mod gtf;
mod profile;

pub use edid::{EDID_BLOCK_LEN, Edid, EdidError, RangeLimits, TimingFormula, parse_edid};
pub use profile::{EdidProfile, ProfileError, profile_from_edid};
