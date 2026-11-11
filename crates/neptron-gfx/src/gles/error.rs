use crate::{Image, RectKind, RenderError};

/// Why [`GlesRenderer`](super::GlesRenderer) could not start or draw.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GlesError {
    /// `libEGL.so.1` is missing or does not provide EGL 1.5.
    #[error("cannot load EGL 1.5 from libEGL.so.1: {reason}")]
    LoadEgl {
        /// The loader error.
        reason: String,
    },
    /// An EGL call failed.
    #[error("{call} failed with EGL error {code:#06x}: {message}")]
    Egl {
        /// The EGL call.
        call: &'static str,
        /// The EGL error code.
        code: i32,
        /// The text of the error.
        message: String,
    },
    /// The display has no config for OpenGL ES 2 of the surface kind the
    /// renderer needs: pbuffer with no surface, or a window of the GBM
    /// format.
    #[error("eglChooseConfig found no config for OpenGL ES 2")]
    NoConfig,
    /// A draw into a window surface on a renderer made with no surface.
    #[error("the renderer has no window surface")]
    NoWindow,
    /// `GL_VERSION` is missing or does not name OpenGL ES 2 or later in a
    /// form the GL loader parses.
    #[error("the GL version string {text:?} is not OpenGL ES 2 or later")]
    GlVersion {
        /// The string, or what was wrong instead of it.
        text: String,
    },
    /// A highp float of the fragment shader has fewer mantissa bits than
    /// the exact integer maths of the shader needs.
    #[error(
        "highp floats of the fragment shader have {bits:?} bits of precision; the renderer needs {needed}"
    )]
    LowPrecision {
        /// The precision that `glGetShaderPrecisionFormat` gives, or `None`
        /// when the driver gives none.
        bits: Option<i32>,
        /// The precision the renderer needs.
        needed: i32,
    },
    /// GL could not make an object.
    #[error("{call} failed: {message}")]
    GlObject {
        /// The GL call.
        call: &'static str,
        /// The text that the GL wrapper gave.
        message: String,
    },
    /// A shader did not compile.
    #[error("the {stage} shader does not compile:\n{log}")]
    Compile {
        /// `vertex` or `fragment`.
        stage: &'static str,
        /// The info log of the shader.
        log: String,
    },
    /// The program did not link.
    #[error("the shader program does not link:\n{log}")]
    Link {
        /// The info log of the program.
        log: String,
    },
    /// The framebuffer object is not complete.
    #[error("the framebuffer is not complete: status {status:#06x}")]
    Framebuffer {
        /// The value of `glCheckFramebufferStatus`.
        status: u32,
    },
    /// GL raised an error.
    #[error("{call} raised GL error {code:#06x}")]
    Gl {
        /// The step that raised it.
        call: &'static str,
        /// The value of `glGetError`.
        code: u32,
    },
    /// The scene or the output buffer is not valid. The reference renderer
    /// refuses it too.
    #[error(transparent)]
    Scene(#[from] RenderError),
    /// An image needs a texture above the GL limit.
    #[error(
        "the {image} needs a texture of {width}x{height} texels, above the GL limit of {limit}"
    )]
    TextureTooLarge {
        /// The image.
        image: Image,
        /// Texture width: one texel per byte of a row.
        width: u64,
        /// Texture height, in rows.
        height: u32,
        /// `GL_MAX_TEXTURE_SIZE`.
        limit: u32,
    },
    /// The output is larger than GL can draw.
    #[error("the output of {width}x{height} pixels is above the GL limit of {limit} per side")]
    OutputTooLarge {
        /// Output width.
        width: u32,
        /// Output height.
        height: u32,
        /// The smaller of `GL_MAX_TEXTURE_SIZE` and `GL_MAX_VIEWPORT_DIMS`.
        limit: u32,
    },
    /// On one axis, the source side times the destination side is above
    /// [`MAX_AXIS_PRODUCT`](super::MAX_AXIS_PRODUCT), so the shader could
    /// not compute the sample positions exactly.
    #[error(
        "the {rect} maps {source_len} source pixels to {dest_len} pixels; their product is \
         above the limit of {limit}"
    )]
    ScaleTooLarge {
        /// The rectangle of the destination.
        rect: RectKind,
        /// The source side: the crop side, or the canvas side.
        source_len: u32,
        /// The destination side.
        dest_len: u32,
        /// [`MAX_AXIS_PRODUCT`](super::MAX_AXIS_PRODUCT).
        limit: u64,
    },
    /// A buffer is smaller than a GL transfer needs. The renderer checks
    /// every image first, so this means a fault in the renderer.
    #[error(
        "a GL transfer of {rows} rows of {width} bytes with a pitch of {pitch} bytes \
         needs more than the {len} bytes of its buffer"
    )]
    UploadOutOfBounds {
        /// Bytes in the buffer.
        len: usize,
        /// Bytes per row.
        width: u32,
        /// Rows.
        rows: u32,
        /// Bytes from one row to the next.
        pitch: usize,
    },
}
