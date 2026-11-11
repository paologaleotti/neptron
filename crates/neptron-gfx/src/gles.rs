//! The OpenGL ES 2 renderer, behind the `gles` feature. It must give the
//! pixels of the reference renderer.

#[cfg(target_endian = "big")]
compile_error!("the GLES shader reads source pixels as little-endian integers");

#[forbid(unsafe_code)]
mod error;
mod ffi;
#[forbid(unsafe_code)]
mod quad;
#[forbid(unsafe_code)]
mod renderer;
#[forbid(unsafe_code)]
mod shader;
#[forbid(unsafe_code)]
mod version;

pub use error::GlesError;
pub use ffi::GbmTarget;
pub use quad::MAX_AXIS_PRODUCT;
pub use renderer::GlesRenderer;
