//! The backend on DRM/KMS: Linux only, behind the `kms` feature.

#[forbid(unsafe_code)]
mod capture;
#[forbid(unsafe_code)]
mod device;
#[forbid(unsafe_code)]
mod display;
#[forbid(unsafe_code)]
mod error;
#[forbid(unsafe_code)]
mod events;
mod ffi;
#[forbid(unsafe_code)]
mod mode;
#[forbid(unsafe_code)]
mod request;
#[forbid(unsafe_code)]
mod restore;

pub use capture::{Capture, WRITEBACK_WAIT_LIMIT_MS};
pub use display::{DRM_FD_ENV, FLIP_WAIT_LIMIT_MS, KmsConfig, KmsDisplay};
pub use error::KmsError;
