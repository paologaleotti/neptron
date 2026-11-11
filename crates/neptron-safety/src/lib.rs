//! The CRT safety validator. `validate` is the only way to make a
//! `ValidatedMode`. Range checks and hard rails, integer maths only.

#![forbid(unsafe_code)]

mod rejection;
mod validated;

pub use rejection::{Axis, InterlaceLimit, PorchEdge, Rejection};
pub use validated::{ValidatedMode, validate};
