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
mod mode_record;

pub use connector::{ChoiceError, ConnectorClass, ConnectorFacts, choose};
pub use mode_record::{ModeRecord, ModeRecordError};
