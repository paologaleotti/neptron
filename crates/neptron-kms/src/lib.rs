//! `Display` on DRM/KMS: connector choice, atomic modeset, GBM buffers, flip
//! events. It accepts a `ValidatedMode` only.
//!
//! The pure parts (the connector rule, the kernel mode record, the plane
//! rectangles, the vblank counter and the order rules of the calls) build
//! and test on every host. The backend itself needs Linux and the `kms`
//! feature; `docs/spec/kms.md` defines it.

#[forbid(unsafe_code)]
mod caps;
#[forbid(unsafe_code)]
mod connector;
#[forbid(unsafe_code)]
mod device_node;
#[forbid(unsafe_code)]
mod flip_state;
#[cfg(all(feature = "kms", target_os = "linux"))]
mod kms;
#[forbid(unsafe_code)]
mod layout;
#[forbid(unsafe_code)]
mod mode_record;
// Only the Linux backend sends a request; the tests run on every host.
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
#[forbid(unsafe_code)]
mod request;
#[forbid(unsafe_code)]
mod sequence;
#[forbid(unsafe_code)]
mod wire_mode;

pub use caps::lowered_caps;
pub use connector::{ChoiceError, ConnectorClass, ConnectorFacts, choose};
pub use device_node::{DRM_MAJOR, STANDARD_FD_MAX, is_drm_node, major};
pub use flip_state::FlipState;
#[cfg(all(feature = "kms", target_os = "linux"))]
pub use kms::{
    Capture, DRM_FD_ENV, FLIP_WAIT_LIMIT_MS, KmsConfig, KmsDisplay, KmsError,
    WRITEBACK_WAIT_LIMIT_MS,
};
pub use layout::{PlaneRect, TooLarge};
pub use mode_record::{ModeRecord, ModeRecordError};
pub use sequence::VblankCounter;
pub use wire_mode::{NextStep, WireMode};
