//! The OpenGL ES 2 renderer, behind the `gles` feature. It must give the
//! pixels of the reference renderer.
//!
//! The modules that call GL (`ffi`, `renderer`) need the feature. The pure
//! modules (texels, quads, shaders, the UI row copy, the cover of the game)
//! also build in the tests of the default build, so the tier gates run
//! their unit tests.

// Without the feature only the unit tests use the pure modules.
#![cfg_attr(not(feature = "gles"), allow(dead_code))]

#[cfg(target_endian = "big")]
compile_error!("the GLES renderer uploads XRGB8888 pixels as little-endian bytes");

#[forbid(unsafe_code)]
mod changed;
#[forbid(unsafe_code)]
mod core_spec;
#[forbid(unsafe_code)]
mod cover;
#[forbid(unsafe_code)]
mod error;
#[cfg(feature = "gles")]
mod ffi;
#[forbid(unsafe_code)]
mod quad;
#[cfg(feature = "gles")]
#[forbid(unsafe_code)]
mod renderer;
#[forbid(unsafe_code)]
mod shader;
#[forbid(unsafe_code)]
mod texels;
#[forbid(unsafe_code)]
mod version;

pub use core_spec::{
    CoreApi, CoreOffer, CoreSpec, CoreStatus, CoreSurface, GL_ERROR_PERIOD_FRAMES,
    MAX_GL_ERRORS_PER_CHECK,
};
pub use error::GlesError;
#[cfg(feature = "gles")]
pub use ffi::GbmTarget;
pub use quad::MAX_AXIS_PRODUCT;
#[cfg(feature = "gles")]
pub use renderer::{EglStrings, GlesRenderer};
