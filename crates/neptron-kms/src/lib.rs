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
#[forbid(unsafe_code)]
mod sequence;

pub use connector::{ChoiceError, ConnectorClass, ConnectorFacts, choose};
pub use flip_state::FlipState;
pub use layout::{PlaneRect, TooLarge};
pub use mode_record::{ModeRecord, ModeRecordError};
pub use sequence::VblankCounter;
