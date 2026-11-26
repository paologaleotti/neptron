//! The context of a core that draws with GL, on the EGL display of the
//! renderer (`docs/spec/hw-render.md`).

use neptron_types::{FrameOrigin, GpuImageId};

use super::GlesRenderer;
use crate::gles::ffi::{CoreGl, core_lookup, probe_offer};
use crate::gles::{CoreOffer, CoreSpec, CoreStatus, CoreSurface, GlesError};

impl GlesRenderer {
    /// The context types and the highest versions that this renderer can
    /// make for a core. The first call makes a context of each API, reads
    /// its version and destroys it; later calls give the same answer. The
    /// offer is empty when EGL lacks `EGL_KHR_surfaceless_context`,
    /// `EGL_KHR_image_base` or `EGL_KHR_gl_texture_2D_image`, or GL lacks
    /// `GL_OES_EGL_image`.
    pub fn core_offer(&mut self) -> CoreOffer {
        if let Some(offer) = self.core_offer {
            return offer;
        }
        let offer = match self.context.current() {
            Ok(gl) => probe_offer(&self.context, &gl),
            Err(_) => CoreOffer::default(),
        };
        self.core_offer = Some(offer);
        offer
    }

    /// Makes the context of `spec` with no surface, a colour texture of
    /// `max_width` by `max_height` RGBA texels, the depth and stencil
    /// buffer of `spec`, and the framebuffer that the core draws into. A
    /// second call destroys what the first one made. No context is current
    /// for the core until [`core_begin`](Self::core_begin).
    ///
    /// # Errors
    ///
    /// [`GlesError::CoreNotOffered`] when [`core_offer`](Self::core_offer)
    /// does not hold `spec`, [`GlesError::CoreSize`] for a side of 0 or
    /// above the texture limit, and the EGL and GL errors of a failed
    /// setup. Nothing stays made after an error.
    pub fn core_create(
        &mut self,
        spec: &CoreSpec,
        max_width: u16,
        max_height: u16,
    ) -> Result<CoreSurface, GlesError> {
        self.core_destroy();
        if !self.core_offer().holds(spec) {
            return Err(GlesError::CoreNotOffered {
                api: spec.api,
                major: spec.major,
                minor: spec.minor,
            });
        }
        self.check_core_size("create", max_width, max_height)?;
        let id = GpuImageId(self.last_image.wrapping_add(1));
        let core = CoreGl::create(&self.context, spec, (max_width, max_height), id)?;
        self.last_image = id.0;
        let surface = CoreSurface {
            framebuffer: core.framebuffer(),
            image: id,
            lookup: core_lookup,
        };
        self.core = Some(core);
        Ok(surface)
    }

    /// Gives the colour texture and the depth and stencil buffer a new
    /// size, between two core frames. The framebuffer name and the image
    /// id stay.
    ///
    /// # Errors
    ///
    /// [`GlesError::NoCoreContext`] before [`core_create`](Self::core_create),
    /// [`GlesError::CoreSize`] as for `core_create`, and the EGL and GL
    /// errors of a failed resize. After an error the old attachments stay.
    pub fn core_resize(&mut self, max_width: u16, max_height: u16) -> Result<(), GlesError> {
        if self.core.is_none() {
            return Err(GlesError::NoCoreContext {
                operation: "resize",
            });
        }
        self.check_core_size("resize", max_width, max_height)?;
        let core = self.core.as_mut().ok_or(GlesError::NoCoreContext {
            operation: "resize",
        })?;
        core.resize(&self.context, (max_width, max_height))
    }

    /// Makes the core context current with no surface, and orders the
    /// draws of the core after the last present that read its image.
    ///
    /// # Errors
    ///
    /// [`GlesError::NoCoreContext`] before [`core_create`](Self::core_create),
    /// and [`GlesError::Egl`] when EGL cannot make the context current or
    /// wait. Then no context is current, and the core must not run.
    pub fn core_begin(&mut self) -> Result<(), GlesError> {
        let core = self
            .core
            .as_mut()
            .ok_or(GlesError::NoCoreContext { operation: "begin" })?;
        core.begin(&self.context)
    }

    /// Orders the next present after the draws of the core and releases
    /// the core context. Once every
    /// [`GL_ERROR_PERIOD_FRAMES`](crate::gles::GL_ERROR_PERIOD_FRAMES)
    /// calls it reads the GL errors of the core and, for a context with a
    /// reset notice, whether the GPU lost it.
    ///
    /// # Errors
    ///
    /// [`GlesError::NoCoreContext`] before [`core_create`](Self::core_create)
    /// or without a [`core_begin`](Self::core_begin), and [`GlesError::Egl`]
    /// for a failed fence or release.
    pub fn core_end(&mut self) -> Result<CoreStatus, GlesError> {
        let core = self
            .core
            .as_mut()
            .ok_or(GlesError::NoCoreContext { operation: "end" })?;
        core.end(&self.context)
    }

    /// Copies the picture of `width` by `height` pixels at texel `(0, 0)`
    /// of the core image into `out`, upright by the rule of `origin`:
    /// XRGB8888 pixels, one native-endian `u32` each with an X byte of 0,
    /// rows of `width * 4` bytes. It reads through a framebuffer of the
    /// renderer, so no GL state of the core changes. It waits for the GPU,
    /// so it is for a request, never for a steady frame.
    ///
    /// # Errors
    ///
    /// [`GlesError::NoCoreContext`] before [`core_create`](Self::core_create),
    /// [`GlesError::CoreSize`] for a side of 0 or above the image,
    /// [`GlesError::CoreBufferTooSmall`] for a short `out`, and the EGL
    /// and GL errors of the read. `out` is unchanged after an error.
    pub fn core_read_back(
        &mut self,
        out: &mut [u8],
        width: u16,
        height: u16,
        origin: FrameOrigin,
    ) -> Result<(), GlesError> {
        let (max_width, max_height) = self
            .core
            .as_ref()
            .ok_or(GlesError::NoCoreContext {
                operation: "read_back",
            })?
            .size();
        if width == 0 || height == 0 || width > max_width || height > max_height {
            return Err(GlesError::CoreSize {
                operation: "read_back",
                width,
                height,
                max_width,
                max_height,
            });
        }
        let min = usize::from(width) * usize::from(height) * 4;
        if out.len() < min {
            return Err(GlesError::CoreBufferTooSmall {
                len: out.len(),
                min,
            });
        }
        let gl = self.context.current()?;
        let core = self.core.as_mut().ok_or(GlesError::NoCoreContext {
            operation: "read_back",
        })?;
        let bottom_left = origin == FrameOrigin::BottomLeft;
        core.read_back(&self.context, &gl, (width, height), bottom_left, out)
    }

    /// Destroys the core context, its image and its buffers. The image id
    /// is then no longer valid. A renderer with no core context does
    /// nothing.
    pub fn core_destroy(&mut self) {
        if let Some(core) = self.core.take() {
            core.destroy(&self.context);
        }
    }

    fn check_core_size(
        &self,
        operation: &'static str,
        width: u16,
        height: u16,
    ) -> Result<(), GlesError> {
        let limit = u16::try_from(self.max_texture).unwrap_or(u16::MAX);
        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(GlesError::CoreSize {
                operation,
                width,
                height,
                max_width: limit,
                max_height: limit,
            });
        }
        Ok(())
    }
}

impl Drop for GlesRenderer {
    fn drop(&mut self) {
        // The core context lives on the display of the renderer, which the
        // context field ends.
        self.core_destroy();
    }
}
