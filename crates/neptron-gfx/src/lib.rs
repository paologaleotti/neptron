//! Renderers for a `Scene`. `ReferenceRenderer` (CPU, pure Rust) defines the correct pixels. `GlesRenderer` (EGL, OpenGL ES 2) must match it.

// The crate is on the `unsafe` allow-list for the GLES code only.
#[forbid(unsafe_code)]
mod reference;

pub use reference::{
    Image, MAX_RECT_SIDE_PX, OutputBuffer, RectKind, ReferenceRenderer, RenderError,
};
