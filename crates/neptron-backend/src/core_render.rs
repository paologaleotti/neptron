use std::ffi::CStr;

use neptron_types::{FrameOrigin, GpuImageId};

/// A display that can host the GL context of a libretro core that draws
/// with the GPU (`docs/spec/hw-render.md`).
///
/// Every call runs on the thread that runs the core and the display. The
/// context of the core is current on that thread only between
/// [`CoreRender::begin`] and [`CoreRender::end`].
pub trait CoreRender {
    /// The context types and the highest versions that this display can
    /// make. It does not change while the process runs.
    fn offer(&mut self) -> ContextOffer;

    /// Makes the context of `request`, a colour image of `max_width` by
    /// `max_height` texels, the depth and stencil buffer that `request`
    /// asks for, and the framebuffer that the core draws into. A second
    /// call destroys what the first one made.
    ///
    /// # Errors
    ///
    /// [`CoreRenderError::NotOffered`] when [`CoreRender::offer`] does not
    /// hold `request`, [`CoreRenderError::Size`] for a side of 0 or above
    /// the limit of the display, [`CoreRenderError::Driver`] when the
    /// driver fails. Nothing stays made after an error.
    fn create(
        &mut self,
        request: &ContextRequest,
        max_width: u16,
        max_height: u16,
    ) -> Result<CoreTarget, CoreRenderError>;

    /// Gives the colour image and the depth and stencil buffer a new size,
    /// between two core frames. The framebuffer name and the image id stay.
    ///
    /// # Errors
    ///
    /// [`CoreRenderError::NoContext`] before `create`,
    /// [`CoreRenderError::Size`] and [`CoreRenderError::Driver`] as for
    /// `create`. After an error the old attachments stay.
    fn resize(&mut self, max_width: u16, max_height: u16) -> Result<(), CoreRenderError>;

    /// Makes the context of the core current and orders the draws of the
    /// core after the last present that read the image. Call it before
    /// each call into the core that can draw.
    ///
    /// # Errors
    ///
    /// [`CoreRenderError::NoContext`] before `create`,
    /// [`CoreRenderError::Driver`] when the context cannot be made current.
    fn begin(&mut self) -> Result<(), CoreRenderError>;

    /// Orders the next present after the draws of the core, and reports
    /// the GL errors and a lost context. Call it after each call into the
    /// core that `begin` started.
    ///
    /// # Errors
    ///
    /// [`CoreRenderError::NoContext`] before `create` or without a
    /// `begin`, [`CoreRenderError::Driver`] when the driver fails.
    fn end(&mut self) -> Result<CoreFrameStatus, CoreRenderError>;

    /// Copies the picture of `width` by `height` pixels at texel `(0, 0)`
    /// into `out`, upright by the rule of `origin`: XRGB8888 pixels, one
    /// native-endian `u32` each, rows of `width * 4` bytes with no
    /// padding. It stalls the GPU, so it runs only on a request, never in
    /// a steady frame.
    ///
    /// # Errors
    ///
    /// [`CoreRenderError::NoContext`] before `create`,
    /// [`CoreRenderError::Size`] for a side of 0 or above the storage,
    /// [`CoreRenderError::BufferTooSmall`] when `out` is shorter than
    /// `width * height * 4` bytes, [`CoreRenderError::Driver`] when the
    /// driver fails. `out` is unchanged after an error.
    fn read_back(
        &mut self,
        out: &mut [u8],
        width: u16,
        height: u16,
        origin: FrameOrigin,
    ) -> Result<(), CoreRenderError>;

    /// Destroys the context, the image and the buffers. The image id is
    /// then no longer valid. A display with nothing made does nothing.
    fn destroy(&mut self);
}

/// The GL API of a context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlApi {
    /// OpenGL ES.
    Gles,
    /// Desktop OpenGL with the core profile, version 3.2 or later.
    GlCore,
    /// Desktop OpenGL with the compatibility profile, as version 2.x has.
    GlCompatibility,
}

/// A GL version, `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlVersion {
    /// The major version.
    pub major: u8,
    /// The minor version.
    pub minor: u8,
}

/// What a core asks for: the context and its buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContextRequest {
    /// The API of the context.
    pub api: GlApi,
    /// The lowest version that the core can use.
    pub version: GlVersion,
    /// The core needs a depth buffer.
    pub depth: bool,
    /// The core needs a stencil buffer. It counts only with `depth`, as
    /// libretro.h says.
    pub stencil: bool,
    /// Where the core puts line 0 of its picture.
    pub origin: FrameOrigin,
}

/// The highest version of each API that a display can make. `None` for an
/// API it cannot make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ContextOffer {
    /// OpenGL ES.
    pub gles: Option<GlVersion>,
    /// Desktop OpenGL, core profile.
    pub gl_core: Option<GlVersion>,
    /// Desktop OpenGL, compatibility profile.
    pub gl_compatibility: Option<GlVersion>,
}

/// What [`CoreRender::create`] made.
#[derive(Debug, Clone, Copy)]
pub struct CoreTarget {
    /// The name of the framebuffer object that the core draws into.
    pub framebuffer: u32,
    /// The image of the picture, for [`GpuFrame`](neptron_types::GpuFrame).
    pub image: GpuImageId,
    /// The address of the GL function of a name, or 0 when the context
    /// has none. It reads no state of the display, so a core can call it
    /// on any thread.
    pub lookup: fn(&CStr) -> usize,
}

/// What [`CoreRender::end`] found after a core call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoreFrameStatus {
    /// The GL errors read in this call. A display reads them only now and
    /// then, so 0 does not prove that the core raised none.
    pub gl_errors: u32,
    /// The GPU lost the context: every object of the core is gone.
    pub context_lost: bool,
}

/// An error of a [`CoreRender`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CoreRenderError {
    /// The display cannot make the context that the core asks for.
    #[error("the display offers no {request:?}")]
    NotOffered {
        /// The request.
        request: ContextRequest,
    },
    /// Nothing is made yet, or a frame was not begun.
    #[error("{operation} needs a core context, and none is ready")]
    NoContext {
        /// The call.
        operation: &'static str,
    },
    /// A size is 0 or above the limit.
    #[error("{operation} cannot take {width}x{height}: the limit is {max_width}x{max_height}")]
    Size {
        /// The call.
        operation: &'static str,
        /// The width given.
        width: u16,
        /// The height given.
        height: u16,
        /// The largest width.
        max_width: u16,
        /// The largest height.
        max_height: u16,
    },
    /// The output buffer of a read back is too short.
    #[error("the read back needs {min} bytes, and the buffer holds {len}")]
    BufferTooSmall {
        /// Bytes in the buffer.
        len: usize,
        /// Bytes needed.
        min: usize,
    },
    /// The driver failed.
    #[error("the graphics driver failed in {operation}: {reason}")]
    Driver {
        /// The call.
        operation: &'static str,
        /// What the driver reported.
        reason: String,
    },
}

impl GlVersion {
    /// The version `major.minor`.
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }
}

impl ContextOffer {
    /// An offer of nothing: the display hosts no core context.
    pub const NONE: Self = Self {
        gles: None,
        gl_core: None,
        gl_compatibility: None,
    };

    /// The highest version of `api` on offer, or `None`.
    pub fn highest(&self, api: GlApi) -> Option<GlVersion> {
        match api {
            GlApi::Gles => self.gles,
            GlApi::GlCore => self.gl_core,
            GlApi::GlCompatibility => self.gl_compatibility,
        }
    }

    /// True when the display can make the API of `request` at its version
    /// or later.
    pub fn holds(&self, request: &ContextRequest) -> bool {
        self.highest(request.api)
            .is_some_and(|highest| request.version <= highest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(api: GlApi, major: u8, minor: u8) -> ContextRequest {
        ContextRequest {
            api,
            version: GlVersion::new(major, minor),
            depth: false,
            stencil: false,
            origin: FrameOrigin::TopLeft,
        }
    }

    #[test]
    fn an_offer_holds_versions_up_to_its_highest() {
        let offer = ContextOffer {
            gles: Some(GlVersion::new(3, 1)),
            gl_core: Some(GlVersion::new(4, 6)),
            gl_compatibility: None,
        };
        assert!(offer.holds(&request(GlApi::Gles, 2, 0)));
        assert!(offer.holds(&request(GlApi::Gles, 3, 1)));
        assert!(!offer.holds(&request(GlApi::Gles, 3, 2)));
        assert!(offer.holds(&request(GlApi::GlCore, 3, 3)));
        assert!(!offer.holds(&request(GlApi::GlCore, 5, 0)));
        assert!(!offer.holds(&request(GlApi::GlCompatibility, 2, 0)));
        assert!(!ContextOffer::NONE.holds(&request(GlApi::Gles, 2, 0)));
        assert_eq!(ContextOffer::default(), ContextOffer::NONE);
    }

    #[test]
    fn versions_order_by_major_then_minor() {
        assert!(GlVersion::new(3, 10) < GlVersion::new(4, 0));
        assert!(GlVersion::new(3, 0) < GlVersion::new(3, 1));
    }
}
