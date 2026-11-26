//! The display as the host of the GL context of a core: the renderer makes
//! the context on its EGL display, and this maps the plain types of
//! `neptron-backend` to the ones of `neptron-gfx`.

use neptron_backend::{
    ContextOffer, ContextRequest, CoreFrameStatus, CoreRender, CoreRenderError, CoreTarget, GlApi,
    GlVersion,
};
use neptron_gfx::gles::{CoreApi, CoreOffer, CoreSpec, GlesError};
use neptron_types::FrameOrigin;

use super::KmsDisplay;

impl CoreRender for KmsDisplay {
    fn offer(&mut self) -> ContextOffer {
        offer_of(self.renderer.core_offer())
    }

    fn create(
        &mut self,
        request: &ContextRequest,
        max_width: u16,
        max_height: u16,
    ) -> Result<CoreTarget, CoreRenderError> {
        let surface = self
            .renderer
            .core_create(&spec_of(request), max_width, max_height)
            .map_err(|e| core_error(e, "create", Some(request)))?;
        Ok(CoreTarget {
            framebuffer: surface.framebuffer,
            image: surface.image,
            lookup: surface.lookup,
        })
    }

    fn resize(&mut self, max_width: u16, max_height: u16) -> Result<(), CoreRenderError> {
        self.renderer
            .core_resize(max_width, max_height)
            .map_err(|e| core_error(e, "resize", None))
    }

    fn begin(&mut self) -> Result<(), CoreRenderError> {
        match self.renderer.core_begin() {
            Ok(()) => Ok(()),
            // The GPU lost the context, so nothing is made any more: the
            // session makes the context again, as after a loss at the end.
            Err(GlesError::CoreContextLost { .. }) => Err(CoreRenderError::NoContext {
                operation: "begin after the GPU lost the context",
            }),
            Err(e) => Err(core_error(e, "begin", None)),
        }
    }

    fn end(&mut self) -> Result<CoreFrameStatus, CoreRenderError> {
        match self.renderer.core_end() {
            Ok(status) => Ok(CoreFrameStatus {
                gl_errors: status.gl_errors,
                context_lost: status.context_lost,
            }),
            // The loss is what the session acts on: it makes a new context,
            // and the failed fence or release goes with the old one.
            Err(GlesError::CoreContextLost { .. }) => Ok(CoreFrameStatus {
                gl_errors: 0,
                context_lost: true,
            }),
            Err(e) => Err(core_error(e, "end", None)),
        }
    }

    fn read_back(
        &mut self,
        out: &mut [u8],
        width: u16,
        height: u16,
        origin: FrameOrigin,
    ) -> Result<(), CoreRenderError> {
        self.renderer
            .core_read_back(out, width, height, origin)
            .map_err(|e| core_error(e, "read_back", None))
    }

    fn destroy(&mut self) {
        self.renderer.core_destroy();
    }
}

fn offer_of(offer: CoreOffer) -> ContextOffer {
    let version = |v: Option<(u8, u8)>| v.map(|(major, minor)| GlVersion::new(major, minor));
    ContextOffer {
        gles: version(offer.gles),
        gl_core: version(offer.gl_core),
        gl_compatibility: version(offer.gl_compatibility),
    }
}

fn spec_of(request: &ContextRequest) -> CoreSpec {
    CoreSpec {
        api: match request.api {
            GlApi::Gles => CoreApi::Gles,
            GlApi::GlCore => CoreApi::GlCore,
            GlApi::GlCompatibility => CoreApi::GlCompatibility,
        },
        major: request.version.major,
        minor: request.version.minor,
        depth: request.depth,
        stencil: request.stencil,
    }
}

/// The contract error of a renderer error in `operation`. `request` is the
/// request of a `create`.
fn core_error(
    e: GlesError,
    operation: &'static str,
    request: Option<&ContextRequest>,
) -> CoreRenderError {
    if let (GlesError::CoreNotOffered { .. }, Some(request)) = (&e, request) {
        return CoreRenderError::NotOffered { request: *request };
    }
    match e {
        GlesError::NoCoreContext { operation } => CoreRenderError::NoContext { operation },
        GlesError::CoreSize {
            operation,
            width,
            height,
            max_width,
            max_height,
        } => CoreRenderError::Size {
            operation,
            width,
            height,
            max_width,
            max_height,
        },
        GlesError::CoreBufferTooSmall { len, min } => CoreRenderError::BufferTooSmall { len, min },
        other => CoreRenderError::Driver {
            operation,
            reason: other.to_string(),
        },
    }
}
