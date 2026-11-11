//! `Display` on DRM/KMS: connector choice, atomic modeset, GBM buffers, flip
//! events. It accepts a `ValidatedMode` only.
//!
//! The pure parts (the connector rule, the kernel mode record, the plane
//! rectangles, the vblank counter and the order rules of the calls) build
//! and test on every host. The backend itself needs Linux and the `kms`
//! feature; `docs/spec/kms.md` defines it.

#[forbid(unsafe_code)]
mod connector;
#[forbid(unsafe_code)]
mod flip_state;
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

pub use connector::{ChoiceError, ConnectorClass, ConnectorFacts, choose};
pub use flip_state::FlipState;
pub use layout::{PlaneRect, TooLarge};
pub use mode_record::{ModeRecord, ModeRecordError};
pub use sequence::VblankCounter;
