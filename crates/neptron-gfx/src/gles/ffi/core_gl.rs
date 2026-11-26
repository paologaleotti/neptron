//! The context of a core that draws with GL: a second EGL context on the
//! display of the renderer, with no surface, and the objects that carry its
//! picture to the renderer (`docs/spec/hw-render.md`, sections 2 and 3).
//!
//! The core context shares no object with the context of the renderer. Its
//! colour texture goes to the renderer as an EGL image, which crosses
//! client APIs. Two EGL fences order the work with no wait on the CPU: the
//! renderer waits on the fence of the core before it samples the image,
//! and the core waits on the fence of the last present before it draws.
//! With no server wait in EGL, each side calls `glFinish` instead.

use std::ffi::{CStr, c_void};
use std::num::NonZeroU32;
use std::sync::OnceLock;

use glow::HasContext as _;
use khronos_egl as egl;
use neptron_types::GpuImageId;

use super::{
    EglContext, EglInstance, Gl, Texture, egl_error, has_extension, load_egl, version_text,
};
use crate::gles::core_spec::{
    DepthFormat, ErrorCadence, MAX_GL_ERRORS_PER_CHECK, depth_format, end_outcome, keep_or_undo,
    picture_from_gl,
};
use crate::gles::{CoreApi, CoreOffer, CoreSpec, CoreStatus, GlesError, version};

/// The EGL extensions without which no core context is offered.
const EGL_NEEDS: [&str; 3] = [
    "EGL_KHR_surfaceless_context",
    "EGL_KHR_image_base",
    "EGL_KHR_gl_texture_2D_image",
];
/// The GL extension of the renderer that samples an EGL image.
const GL_EGL_IMAGE: &str = "GL_OES_EGL_image";
/// `EGL_CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_EXT`, from
/// `EGL_EXT_create_context_robustness`, for an OpenGL ES context.
const RESET_STRATEGY_EXT: egl::Int = 0x3138;
/// `GL_NO_ERROR`, which is also the reset status of a context with no
/// reset.
const GL_NO_ERROR: u32 = 0;
/// `GL_DRAW_FRAMEBUFFER_BINDING`, which is `GL_FRAMEBUFFER_BINDING`.
const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
/// `GL_READ_FRAMEBUFFER_BINDING` of GL 3 and ES 3.
const GL_READ_FRAMEBUFFER_BINDING: u32 = 0x8CAA;
/// `GL_PIXEL_UNPACK_BUFFER_BINDING` of GL 2.1 and ES 3.
const GL_PIXEL_UNPACK_BUFFER_BINDING: u32 = 0x88EF;
/// `GL_RGBA8`, the sized colour format of desktop GL.
const GL_RGBA8: u32 = 0x8058;

/// The EGL library of [`core_lookup`]: loaded once and kept for the
/// process, so the lookup reads no state of a renderer and a core can call
/// it on any thread.
static LOOKUP_EGL: OnceLock<EglInstance> = OnceLock::new();

/// The address of the GL function `name` through `eglGetProcAddress`, or 0.
/// With EGL 1.5 it gives core functions too. The address dispatches to the
/// context that is current on the calling thread.
pub(in crate::gles) fn core_lookup(name: &CStr) -> usize {
    let Some(lib) = LOOKUP_EGL.get() else {
        return 0;
    };
    let Ok(name) = name.to_str() else {
        return 0;
    };
    lib.get_proc_address(name).map_or(0, |f| f as usize)
}

/// `glEGLImageTargetTexture2DOES`.
type ImageTarget = unsafe extern "system" fn(u32, *const c_void);
/// `glGetGraphicsResetStatus` and its KHR, EXT and ARB names.
type ResetStatus = unsafe extern "system" fn() -> u32;

/// The context of a core, its framebuffer, and the image of its colour
/// texture in the renderer. It has no `Drop`: the renderer destroys it
/// with [`CoreGl::destroy`], which needs the EGL display.
pub(in crate::gles) struct CoreGl {
    context: egl::Context,
    gl: glow::Context,
    /// The context is OpenGL ES (2 or 3): unsized texture formats.
    es: bool,
    /// The context is desktop GL: EGL calls that act on the current
    /// context need the desktop GL API bound.
    desktop: bool,
    /// The context is OpenGL ES 2: no read framebuffer and no buffer
    /// objects for pixels.
    es2: bool,
    texture: Texture,
    depth: Option<(glow::NativeRenderbuffer, DepthFormat)>,
    framebuffer: glow::NativeFramebuffer,
    image: egl::Image,
    id: GpuImageId,
    width: u16,
    height: u16,
    /// The texture of the renderer that holds the image.
    imported: Texture,
    /// The framebuffer of the renderer that reads the image back.
    read_framebuffer: Option<glow::NativeFramebuffer>,
    read_pixels: Vec<u8>,
    image_target: ImageTarget,
    reset_status: Option<ResetStatus>,
    fences: bool,
    core_fence: Option<egl::Sync>,
    present_fence: Option<egl::Sync>,
    /// Colour textures of the core context that a resize replaced. GL
    /// deletes a texture only in its own context, so the next `begin`
    /// deletes them.
    retired: Vec<Texture>,
    begun: bool,
    cadence: ErrorCadence,
}

/// The highest version of each API that the display of `ctx` makes for a
/// core, or nothing when EGL or the renderer lacks an extension that the
/// path needs. It makes a context of each API, reads its version and
/// destroys it. `frontend` is the GL of the renderer, current.
pub(in crate::gles) fn probe_offer(ctx: &EglContext, frontend: &Gl<'_>) -> CoreOffer {
    if missing_extension(ctx, frontend).is_some() {
        return CoreOffer::default();
    }
    let probe = |api, major, minor| {
        let context = make_context(ctx, api, major, minor, false).ok()?;
        let desktop = api != CoreApi::Gles;
        let found = ctx
            .lib
            .make_current(ctx.display, None, None, Some(context))
            .ok()
            .and_then(|()| version_text(&ctx.lib).ok())
            .and_then(|text| version::api_version(&text));
        // The probe reads only the version; a failed release or destroy
        // leaves nothing that a later context uses.
        let _ = release(ctx, desktop);
        let _ = ctx.lib.destroy_context(ctx.display, context);
        let (es, major, minor) = found?;
        if es != (api == CoreApi::Gles) {
            return None;
        }
        Some((u8::try_from(major).ok()?, u8::try_from(minor).ok()?))
    };
    CoreOffer {
        gles: probe(CoreApi::Gles, 2, 0),
        gl_core: probe(CoreApi::GlCore, 3, 2).filter(|&v| v >= (3, 2)),
        gl_compatibility: probe(CoreApi::GlCompatibility, 2, 0),
    }
}

/// The first extension that the path lacks, in EGL or in the renderer.
fn missing_extension(ctx: &EglContext, frontend: &Gl<'_>) -> Option<GlesError> {
    let egl_list = ctx
        .lib
        .query_string(Some(ctx.display), egl::EXTENSIONS)
        .ok();
    for name in EGL_NEEDS {
        if !egl_list.is_some_and(|list| has_extension(list, name)) {
            return Some(GlesError::MissingEglExtension { name });
        }
    }
    frontend.require_extension(GL_EGL_IMAGE).err()
}

/// True when the display of `ctx` has `name`.
fn egl_has(ctx: &EglContext, name: &str) -> bool {
    ctx.lib
        .query_string(Some(ctx.display), egl::EXTENSIONS)
        .is_ok_and(|list| has_extension(list, name))
}

/// A context of `api` at `major.minor` or later on the display of `ctx`,
/// with no surface and no shared objects. `robust` asks for a reset
/// notice. The thread keeps OpenGL ES as its bound API, as the renderer
/// needs.
fn make_context(
    ctx: &EglContext,
    api: CoreApi,
    major: u8,
    minor: u8,
    robust: bool,
) -> Result<egl::Context, GlesError> {
    let (bind, renderable) = match api {
        CoreApi::Gles if major >= 3 => (egl::OPENGL_ES_API, egl::OPENGL_ES3_BIT),
        CoreApi::Gles => (egl::OPENGL_ES_API, egl::OPENGL_ES2_BIT),
        CoreApi::GlCore | CoreApi::GlCompatibility => (egl::OPENGL_API, egl::OPENGL_BIT),
    };
    let mut attributes = vec![
        egl::CONTEXT_MAJOR_VERSION,
        egl::Int::from(major),
        egl::CONTEXT_MINOR_VERSION,
        egl::Int::from(minor),
    ];
    match api {
        CoreApi::Gles => {
            if robust {
                attributes.extend([RESET_STRATEGY_EXT, egl::LOSE_CONTEXT_ON_RESET]);
            }
        }
        CoreApi::GlCore | CoreApi::GlCompatibility => {
            let profile = if api == CoreApi::GlCore {
                egl::CONTEXT_OPENGL_CORE_PROFILE_BIT
            } else {
                egl::CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT
            };
            attributes.extend([egl::CONTEXT_OPENGL_PROFILE_MASK, profile]);
            if robust {
                attributes.extend([
                    egl::CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY,
                    egl::LOSE_CONTEXT_ON_RESET,
                ]);
            }
        }
    }
    attributes.push(egl::NONE);
    ctx.lib
        .bind_api(bind)
        .map_err(|e| egl_error("eglBindAPI for the core", e))?;
    let made = ctx
        .lib
        .choose_first_config(
            ctx.display,
            &[
                egl::RENDERABLE_TYPE,
                renderable,
                egl::SURFACE_TYPE,
                egl::DONT_CARE,
                egl::NONE,
            ],
        )
        .map_err(|e| egl_error("eglChooseConfig for the core", e))
        .and_then(|config| config.ok_or(GlesError::NoConfig))
        .and_then(|config| {
            ctx.lib
                .create_context(ctx.display, config, None, &attributes)
                .map_err(|e| egl_error("eglCreateContext for the core", e))
        });
    // The renderer makes its contexts with OpenGL ES bound; a failure here
    // shows at its next eglCreateContext, not at a draw.
    let _ = ctx.lib.bind_api(egl::OPENGL_ES_API);
    made
}

/// The bindings of a core context that the frontend changes between core
/// calls, so it can put them back: the core may keep a binding from one
/// frame to the next.
struct Bindings {
    draw_framebuffer: u32,
    read_framebuffer: u32,
    renderbuffer: u32,
    texture: u32,
    unpack_buffer: u32,
}

impl CoreGl {
    /// Makes the context of `spec`, a colour texture of `width` by
    /// `height`, the depth and stencil buffer of `spec`, the framebuffer,
    /// and the image of the texture in the renderer. The caller checked
    /// the offer and the size. The renderer's context is current when
    /// this returns.
    pub(in crate::gles) fn create(
        ctx: &EglContext,
        spec: &CoreSpec,
        (width, height): (u16, u16),
        id: GpuImageId,
    ) -> Result<CoreGl, GlesError> {
        if let Some(missing) = missing_extension(ctx, &ctx.current()?) {
            return Err(missing);
        }
        if LOOKUP_EGL.get().is_none() {
            // A second thread may set it first; either library gives the
            // same addresses.
            let _ = LOOKUP_EGL.set(load_egl()?);
        }
        let image_target = ctx
            .lib
            .get_proc_address("glEGLImageTargetTexture2DOES")
            .ok_or(GlesError::MissingExtension { name: GL_EGL_IMAGE })?;
        // SAFETY: eglGetProcAddress gave the address of
        // glEGLImageTargetTexture2DOES, whose signature in GL_OES_EGL_image
        // is ImageTarget: a GLenum and a GLeglImageOES pointer.
        let image_target: ImageTarget = unsafe { std::mem::transmute(image_target) };
        let wanted_robust = egl_has(ctx, "EGL_EXT_create_context_robustness");
        let (context, robust) =
            match make_context(ctx, spec.api, spec.major, spec.minor, wanted_robust) {
                Ok(context) => (context, wanted_robust),
                Err(_) if wanted_robust => (
                    make_context(ctx, spec.api, spec.major, spec.minor, false)?,
                    false,
                ),
                Err(e) => return Err(e),
            };
        let made = Self::fill(
            ctx,
            context,
            spec,
            (width, height),
            id,
            image_target,
            robust,
        );
        if made.is_err() {
            // Destroying the context deletes every object made in it. The
            // error of the setup is the one to report.
            let _ = release(ctx, spec.api != CoreApi::Gles);
            let _ = ctx.lib.destroy_context(ctx.display, context);
        }
        let restored = ctx.current().map(|_| ());
        keep_or_undo(made, restored, |core| core.destroy(ctx))
    }

    fn fill(
        ctx: &EglContext,
        context: egl::Context,
        spec: &CoreSpec,
        (width, height): (u16, u16),
        id: GpuImageId,
        image_target: ImageTarget,
        robust: bool,
    ) -> Result<CoreGl, GlesError> {
        ctx.lib
            .make_current(ctx.display, None, None, Some(context))
            .map_err(|e| egl_error("eglMakeCurrent of the core with no surface", e))?;
        let text = version_text(&ctx.lib)?;
        let (es, major, _) = version::api_version(&text).ok_or(GlesError::GlVersion { text })?;
        let es2 = es && major == 2;
        // SAFETY: the core context is current, and its GL_VERSION parses
        // (checked above), so glow's own query does not panic. With EGL 1.5
        // eglGetProcAddress gives core functions too; a missing one is a
        // null pointer, which glow keeps as not loaded.
        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                ctx.lib
                    .get_proc_address(name)
                    .map_or(std::ptr::null(), |f| f as *const c_void)
            })
        };
        let reset_status = robust.then(|| reset_status_function(&ctx.lib)).flatten();
        let depth = depth_format(
            spec,
            es2,
            gl.supported_extensions().contains("GL_OES_depth24"),
            gl.supported_extensions()
                .contains("GL_OES_packed_depth_stencil"),
        )
        .map_err(|name| GlesError::MissingExtension { name })?;
        // SAFETY: the core context is current; every object below is made
        // in it, and the sizes come from the caller's check against the
        // limits, so they fit an i32.
        let (texture, depth, framebuffer) = unsafe {
            let texture = gl.create_texture().map_err(|message| GlesError::GlObject {
                call: "glGenTextures for the core",
                message,
            })?;
            colour_storage(&gl, texture, es, (width, height));
            let depth = match depth {
                Some(format) => {
                    let rb = gl
                        .create_renderbuffer()
                        .map_err(|message| GlesError::GlObject {
                            call: "glGenRenderbuffers for the core",
                            message,
                        })?;
                    depth_storage(&gl, rb, format, (width, height));
                    Some((rb, format))
                }
                None => None,
            };
            let framebuffer = gl
                .create_framebuffer()
                .map_err(|message| GlesError::GlObject {
                    call: "glGenFramebuffers for the core",
                    message,
                })?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            attach(&gl, texture, depth);
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_renderbuffer(glow::RENDERBUFFER, None);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(GlesError::Framebuffer { status });
            }
            let code = gl.get_error();
            if code != GL_NO_ERROR {
                return Err(GlesError::Gl {
                    call: "the framebuffer of the core",
                    code,
                });
            }
            (texture, depth, framebuffer)
        };
        let image = image_of(ctx, context, texture)?;
        let desktop = !es;
        if let Err(e) = release(ctx, desktop) {
            // The release error is the one to report.
            let _ = ctx.lib.destroy_image(ctx.display, image);
            return Err(e);
        }
        let imported = match import(ctx, image, image_target) {
            Ok(texture) => texture,
            Err(e) => {
                // The import error is the one to report.
                let _ = ctx.lib.destroy_image(ctx.display, image);
                return Err(e);
            }
        };
        Ok(CoreGl {
            context,
            gl,
            es,
            desktop,
            es2,
            texture,
            depth,
            framebuffer,
            image,
            id,
            width,
            height,
            imported,
            read_framebuffer: None,
            read_pixels: Vec::new(),
            image_target,
            reset_status,
            fences: egl_has(ctx, "EGL_KHR_fence_sync") && egl_has(ctx, "EGL_KHR_wait_sync"),
            core_fence: None,
            present_fence: None,
            retired: Vec::new(),
            begun: false,
            cadence: ErrorCadence::default(),
        })
    }

    /// The name of the framebuffer that the core draws into.
    pub(in crate::gles) fn framebuffer(&self) -> u32 {
        self.framebuffer.0.get()
    }

    /// The id of the image.
    pub(in crate::gles) fn id(&self) -> GpuImageId {
        self.id
    }

    /// The size of the colour texture, in texels.
    pub(in crate::gles) fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// The texture of the renderer that samples the image.
    pub(in crate::gles) fn imported(&self) -> Texture {
        self.imported
    }

    /// True between `begin` and `end`.
    pub(in crate::gles) fn begun(&self) -> bool {
        self.begun
    }

    /// True when the core context is current on this thread for its own
    /// client API.
    pub(in crate::gles) fn is_current(&self, ctx: &EglContext) -> bool {
        on_api(ctx, self.desktop, || {
            Ok(ctx.lib.get_current_context() == Some(self.context))
        })
        .unwrap_or(false)
    }

    /// Makes the core context current with no surface, and orders the
    /// draws of the core after the last present that read the image. On an
    /// error no context is current, so a core call draws nowhere.
    pub(in crate::gles) fn begin(&mut self, ctx: &EglContext) -> Result<(), GlesError> {
        let present_fence = self.present_fence.take();
        let current = on_api(ctx, self.desktop, || {
            ctx.lib
                .make_current(ctx.display, None, None, Some(self.context))
                .map_err(|e| egl_error("eglMakeCurrent of the core", e))?;
            match present_fence {
                Some(fence) => wait_and_destroy(ctx, fence),
                None => Ok(()),
            }
        });
        if let Err(e) = current {
            // The make-current or wait error is the one to report.
            let _ = release(ctx, self.desktop);
            return Err(e);
        }
        for texture in self.retired.drain(..) {
            // SAFETY: the core context is current, and the texture is its
            // own; no framebuffer holds it since the resize.
            unsafe { self.gl.delete_texture(texture) };
        }
        self.begun = true;
        Ok(())
    }

    /// Orders the next present after the draws of the core, reads the GL
    /// errors and the reset status now and then, and releases the core
    /// context.
    pub(in crate::gles) fn end(&mut self, ctx: &EglContext) -> Result<CoreStatus, GlesError> {
        if !self.begun {
            return Err(GlesError::NoCoreContext { operation: "end" });
        }
        self.begun = false;
        let mut status = CoreStatus::default();
        let due = self.cadence.tick();
        if due {
            for _ in 0..MAX_GL_ERRORS_PER_CHECK {
                // SAFETY: the core context is current from `begin`.
                if unsafe { self.gl.get_error() } == GL_NO_ERROR {
                    break;
                }
                status.gl_errors += 1;
            }
        }
        let fenced = on_api(ctx, self.desktop, || {
            fence(ctx, &self.gl, self.fences, "the core fence")
        });
        // A failed fence can come from a lost context, so the reset status
        // is read then too, while the context is still current.
        if (due || fenced.is_err())
            && let Some(reset_status) = self.reset_status
        {
            // SAFETY: the function came from eglGetProcAddress by one of
            // the names of glGetGraphicsResetStatus, which takes nothing
            // and returns a GLenum; the robust core context is current.
            status.context_lost = unsafe { reset_status() } != GL_NO_ERROR;
        }
        // The core context goes, so no later GL call reaches it by mistake.
        let released = release(ctx, self.desktop);
        let fenced = fenced.map(|fence| {
            if let Some(old) = std::mem::replace(&mut self.core_fence, fence) {
                destroy_sync(ctx, old);
            }
        });
        end_outcome(status, fenced, released)
    }

    /// Orders a read of the image by the renderer after the draws of the
    /// core. The renderer's context is current.
    pub(in crate::gles) fn before_read(&mut self, ctx: &EglContext) -> Result<(), GlesError> {
        match self.core_fence.take() {
            Some(fence) => wait_and_destroy(ctx, fence),
            None => Ok(()),
        }
    }

    /// Orders the next draws of the core after a read of the image by the
    /// renderer, whose context is current.
    pub(in crate::gles) fn after_read(
        &mut self,
        ctx: &EglContext,
        gl: &Gl<'_>,
    ) -> Result<(), GlesError> {
        let fence = fence(ctx, gl.gl, self.fences, "the present fence")?;
        if let Some(old) = std::mem::replace(&mut self.present_fence, fence) {
            destroy_sync(ctx, old);
        }
        Ok(())
    }

    /// Gives the colour texture and the depth and stencil buffer a new
    /// size. The framebuffer name and the image id stay, and the bindings
    /// of the core context come back as they were. After an error the old
    /// attachments and the old image stay, and nothing new is left. The
    /// renderer's context is current when it returns.
    pub(in crate::gles) fn resize(
        &mut self,
        ctx: &EglContext,
        size: (u16, u16),
    ) -> Result<(), GlesError> {
        ctx.lib
            .make_current(ctx.display, None, None, Some(self.context))
            .map_err(|e| egl_error("eglMakeCurrent of the core", e))?;
        let made = self.attach_new(ctx, size);
        // The renderer makes its context current next; a failed release
        // shows there.
        let _ = release(ctx, self.desktop);
        let made = made?;
        let imported = ctx.current().and_then(|gl| self.import_image(&gl, made.1));
        let (texture, image) = keep_or_undo(Ok(made), imported, |(texture, image)| {
            self.undo_resize(ctx, texture, image);
        })?;
        // The texture of the renderer holds the new image now, so the old
        // image has no user left; a failure frees nothing more.
        let _ = ctx.lib.destroy_image(ctx.display, self.image);
        self.retired.push(self.texture);
        self.texture = texture;
        self.image = image;
        (self.width, self.height) = size;
        self.read_pixels = Vec::new();
        Ok(())
    }

    /// Makes a colour texture of `size`, gives the depth and stencil
    /// buffer that size, attaches both, and makes the image of the new
    /// texture. The old texture and its image stay alive. After an error
    /// the old attachments are back. The core context is current.
    fn attach_new(
        &mut self,
        ctx: &EglContext,
        size: (u16, u16),
    ) -> Result<(Texture, egl::Image), GlesError> {
        let saved = self.bindings();
        // SAFETY: the core context is current. The new texture is made in
        // it; the sizes come from the caller's check, so they fit an i32.
        let made = unsafe {
            if !self.es2 {
                self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            }
            match self.gl.create_texture() {
                Ok(texture) => {
                    colour_storage(&self.gl, texture, self.es, size);
                    if let Some((rb, format)) = self.depth {
                        depth_storage(&self.gl, rb, format, size);
                    }
                    self.gl
                        .bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
                    attach(&self.gl, texture, self.depth);
                    let status = self.gl.check_framebuffer_status(glow::FRAMEBUFFER);
                    if status == glow::FRAMEBUFFER_COMPLETE {
                        Ok(texture)
                    } else {
                        self.attach_old();
                        self.gl.delete_texture(texture);
                        Err(GlesError::Framebuffer { status })
                    }
                }
                Err(message) => Err(GlesError::GlObject {
                    call: "glGenTextures for the core",
                    message,
                }),
            }
        };
        self.restore(&saved);
        let texture = made?;
        match image_of(ctx, self.context, texture) {
            Ok(image) => Ok((texture, image)),
            Err(e) => {
                // SAFETY: the core context is current and the texture is
                // its own; the old texture goes back on the framebuffer
                // first.
                unsafe {
                    self.attach_old();
                    self.gl.delete_texture(texture);
                }
                self.restore(&saved);
                Err(e)
            }
        }
    }

    /// Puts the old colour texture and the old depth size back on the
    /// framebuffer of the core.
    ///
    /// # Safety
    ///
    /// The core context is current.
    unsafe fn attach_old(&self) {
        // SAFETY: the caller's guarantee; every object is of the core
        // context.
        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
            if let Some((rb, format)) = self.depth {
                depth_storage(&self.gl, rb, format, (self.width, self.height));
            }
            attach(&self.gl, self.texture, self.depth);
        }
    }

    /// Gives the texture of the renderer, whose context `gl` is current,
    /// the storage of `image`.
    fn import_image(&self, gl: &Gl<'_>, image: egl::Image) -> Result<(), GlesError> {
        // SAFETY: the renderer's context is current; `imported` is its
        // texture, and the image is live.
        unsafe {
            gl.gl.bind_texture(glow::TEXTURE_2D, Some(self.imported));
            (self.image_target)(glow::TEXTURE_2D, image.as_ptr());
        }
        gl.check("glEGLImageTargetTexture2DOES after a resize")
    }

    /// Frees the texture and the image of a resize whose last step failed,
    /// and puts the old attachments and the old image back. A step that
    /// fails here leaves an object that the destroy of the context frees,
    /// and the error of the resize is the one to report.
    fn undo_resize(&mut self, ctx: &EglContext, texture: Texture, image: egl::Image) {
        if ctx
            .lib
            .make_current(ctx.display, None, None, Some(self.context))
            .is_ok()
        {
            let saved = self.bindings();
            // SAFETY: the core context is current, and `texture` is its
            // own; the old texture goes back on the framebuffer first.
            unsafe {
                self.attach_old();
                self.gl.delete_texture(texture);
            }
            self.restore(&saved);
            let _ = release(ctx, self.desktop);
        }
        let _ = ctx.lib.destroy_image(ctx.display, image);
        if let Ok(gl) = ctx.current() {
            let _ = self.import_image(&gl, self.image);
        }
    }

    /// The bindings that `resize` changes.
    fn bindings(&self) -> Bindings {
        // SAFETY: the core context is current; each name is a valid query
        // of its version.
        unsafe {
            let read = |p| u32::try_from(self.gl.get_parameter_i32(p)).unwrap_or(0);
            Bindings {
                draw_framebuffer: read(GL_DRAW_FRAMEBUFFER_BINDING),
                read_framebuffer: if self.es2 {
                    0
                } else {
                    read(GL_READ_FRAMEBUFFER_BINDING)
                },
                renderbuffer: read(glow::RENDERBUFFER_BINDING),
                texture: read(glow::TEXTURE_BINDING_2D),
                unpack_buffer: if self.es2 {
                    0
                } else {
                    read(GL_PIXEL_UNPACK_BUFFER_BINDING)
                },
            }
        }
    }

    fn restore(&self, saved: &Bindings) {
        let some = |v: u32| NonZeroU32::new(v);
        // SAFETY: the core context is current; each name came from its own
        // binding query, so it is one of its objects or 0.
        unsafe {
            if self.es2 {
                self.gl.bind_framebuffer(
                    glow::FRAMEBUFFER,
                    some(saved.draw_framebuffer).map(glow::NativeFramebuffer),
                );
            } else {
                self.gl.bind_framebuffer(
                    glow::DRAW_FRAMEBUFFER,
                    some(saved.draw_framebuffer).map(glow::NativeFramebuffer),
                );
                self.gl.bind_framebuffer(
                    glow::READ_FRAMEBUFFER,
                    some(saved.read_framebuffer).map(glow::NativeFramebuffer),
                );
                self.gl.bind_buffer(
                    glow::PIXEL_UNPACK_BUFFER,
                    some(saved.unpack_buffer).map(glow::NativeBuffer),
                );
            }
            self.gl.bind_renderbuffer(
                glow::RENDERBUFFER,
                some(saved.renderbuffer).map(glow::NativeRenderbuffer),
            );
            self.gl.bind_texture(
                glow::TEXTURE_2D,
                some(saved.texture).map(glow::NativeTexture),
            );
        }
    }

    /// Copies the `width` by `height` picture at texel `(0, 0)` into `out`
    /// as XRGB8888 rows from the top, by the rule of `bottom_left`. It
    /// reads the image through a framebuffer of the renderer, whose
    /// context `gl` is, so no state of the core changes. The caller checked
    /// the size and the buffer.
    pub(in crate::gles) fn read_back(
        &mut self,
        ctx: &EglContext,
        gl: &Gl<'_>,
        (width, height): (u16, u16),
        bottom_left: bool,
        out: &mut [u8],
    ) -> Result<(), GlesError> {
        self.before_read(ctx)?;
        let framebuffer = match self.read_framebuffer {
            Some(fb) => fb,
            None => {
                let fb = gl.framebuffer(self.imported)?;
                self.read_framebuffer = Some(fb);
                fb
            }
        };
        gl.check_framebuffer(framebuffer)?;
        let len = usize::from(width) * usize::from(height) * 4;
        self.read_pixels.resize(len, 0);
        gl.read_rgba(u32::from(width), u32::from(height), &mut self.read_pixels)?;
        gl.check("glReadPixels of the core image")?;
        picture_from_gl(&self.read_pixels, width, height, bottom_left, out);
        Ok(())
    }

    /// Destroys the context, the image, the fences, and the objects of the
    /// renderer that held the image. The renderer's context is current
    /// when it returns.
    pub(in crate::gles) fn destroy(self, ctx: &EglContext) {
        // Nothing can be done about a failure while the core goes away.
        if let Ok(gl) = ctx.current() {
            // SAFETY: the renderer's context is current; both objects are
            // its own.
            unsafe {
                gl.gl.delete_texture(self.imported);
                if let Some(fb) = self.read_framebuffer {
                    gl.gl.delete_framebuffer(fb);
                }
            }
        }
        if self.begun {
            // A frame of the core is open: its context is current, and EGL
            // destroys a current context only once it is released.
            let _ = release(ctx, self.desktop);
        }
        for fence in [self.core_fence, self.present_fence].into_iter().flatten() {
            destroy_sync(ctx, fence);
        }
        let _ = ctx.lib.destroy_image(ctx.display, self.image);
        // The core context is not current, so EGL destroys it and every
        // object in it at once.
        let _ = ctx.lib.destroy_context(ctx.display, self.context);
    }
}

/// Gives `texture` RGBA storage of `size` with no content, nearest
/// sampling and no mipmaps: an EGL image needs a complete texture.
///
/// # Safety
///
/// The context of `gl` is current, `texture` is its own, and both sides fit
/// an i32. No pixel unpack buffer is bound.
unsafe fn colour_storage(gl: &glow::Context, texture: Texture, es: bool, (w, h): (u16, u16)) {
    // SAFETY: the caller's guarantee.
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        for (name, value) in [
            (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
            (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
        }
        // OpenGL ES 2 needs the internal format equal to the format.
        let internal = if es { glow::RGBA } else { GL_RGBA8 };
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            internal as i32,
            i32::from(w),
            i32::from(h),
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
    }
}

/// Gives `rb` the storage of `format` and `size`.
///
/// # Safety
///
/// The context of `gl` is current and `rb` is its own.
unsafe fn depth_storage(
    gl: &glow::Context,
    rb: glow::NativeRenderbuffer,
    format: DepthFormat,
    (w, h): (u16, u16),
) {
    // SAFETY: the caller's guarantee; the sides fit an i32.
    unsafe {
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rb));
        gl.renderbuffer_storage(glow::RENDERBUFFER, format.gl(), i32::from(w), i32::from(h));
    }
}

/// Attaches the colour texture and the depth and stencil buffer to the
/// bound framebuffer. A packed buffer goes on both attachment points,
/// which every version takes.
///
/// # Safety
///
/// The context of `gl` is current, and the objects and the bound
/// framebuffer are its own.
unsafe fn attach(
    gl: &glow::Context,
    texture: Texture,
    depth: Option<(glow::NativeRenderbuffer, DepthFormat)>,
) {
    // SAFETY: the caller's guarantee.
    unsafe {
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        if let Some((rb, format)) = depth {
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::DEPTH_ATTACHMENT,
                glow::RENDERBUFFER,
                Some(rb),
            );
            if format.has_stencil() {
                gl.framebuffer_renderbuffer(
                    glow::FRAMEBUFFER,
                    glow::STENCIL_ATTACHMENT,
                    glow::RENDERBUFFER,
                    Some(rb),
                );
            }
        }
    }
}

/// The EGL image of level 0 of `texture` of `context`.
fn image_of(
    ctx: &EglContext,
    context: egl::Context,
    texture: Texture,
) -> Result<egl::Image, GlesError> {
    let name = texture.0.get() as usize;
    // SAFETY: for EGL_GL_TEXTURE_2D the client buffer is the texture name
    // cast to a pointer, as EGL_KHR_gl_texture_2D_image defines; EGL reads
    // it as a number and never dereferences it.
    let buffer = unsafe { egl::ClientBuffer::from_ptr(name as *mut c_void) };
    ctx.lib
        .create_image(
            ctx.display,
            context,
            egl::GL_TEXTURE_2D as egl::Enum,
            buffer,
            &[egl::GL_TEXTURE_LEVEL as egl::Attrib, 0, egl::ATTRIB_NONE],
        )
        .map_err(|e| egl_error("eglCreateImage of the core texture", e))
}

/// A texture of the renderer that holds `image`, with nearest sampling.
fn import(
    ctx: &EglContext,
    image: egl::Image,
    image_target: ImageTarget,
) -> Result<Texture, GlesError> {
    let gl = ctx.current()?;
    let texture = gl.texture()?;
    // SAFETY: the renderer's context is current, and `texture` is its own
    // and bound by `texture()`. `image` is a live image of this display.
    unsafe { image_target(glow::TEXTURE_2D, image.as_ptr()) };
    if let Err(e) = gl.check("glEGLImageTargetTexture2DOES") {
        // SAFETY: as above.
        unsafe { gl.gl.delete_texture(texture) };
        return Err(e);
    }
    Ok(texture)
}

/// The first of the names of `glGetGraphicsResetStatus` that EGL gives.
fn reset_status_function(lib: &EglInstance) -> Option<ResetStatus> {
    let f = [
        "glGetGraphicsResetStatus",
        "glGetGraphicsResetStatusKHR",
        "glGetGraphicsResetStatusEXT",
        "glGetGraphicsResetStatusARB",
    ]
    .into_iter()
    .find_map(|name| lib.get_proc_address(name))?;
    // SAFETY: each name is glGetGraphicsResetStatus in GL 4.5, ES 3.2 or an
    // extension, with the signature ResetStatus: no argument, a GLenum out.
    Some(unsafe { std::mem::transmute::<extern "system" fn(), ResetStatus>(f) })
}

/// A fence after the work of the current context, whose GL is `gl`,
/// flushed so that another context can wait on it; or, with no fences in
/// EGL (`fences` false), `None` after a `glFinish`.
fn fence(
    ctx: &EglContext,
    gl: &glow::Context,
    fences: bool,
    call: &'static str,
) -> Result<Option<egl::Sync>, GlesError> {
    // SAFETY: the context of `gl` is current, as the caller says. The
    // attribute list ends with ATTRIB_NONE.
    unsafe {
        if !fences {
            gl.finish();
            return Ok(None);
        }
        let sync = ctx
            .lib
            .create_sync(
                ctx.display,
                egl::SYNC_FENCE as egl::Enum,
                &[egl::ATTRIB_NONE],
            )
            .map_err(|e| egl_error(call, e))?;
        gl.flush();
        Ok(Some(sync))
    }
}

/// Makes the current context wait on `fence` in the GPU, then drops it.
fn wait_and_destroy(ctx: &EglContext, fence: egl::Sync) -> Result<(), GlesError> {
    let waited = ctx
        .lib
        .wait_sync(ctx.display, fence, 0)
        .map_err(|e| egl_error("eglWaitSync", e));
    destroy_sync(ctx, fence);
    waited
}

/// Runs `f` with desktop GL bound as the client API of the thread when
/// `desktop`, then binds OpenGL ES again. EGL applies `eglMakeCurrent` with
/// no context, `eglCreateSync` and `eglWaitSync` to the current context of
/// the bound API, so such a call on a desktop GL context needs that API
/// bound. Mesa takes the call with either API; EGL does not promise it.
fn on_api<R>(
    ctx: &EglContext,
    desktop: bool,
    f: impl FnOnce() -> Result<R, GlesError>,
) -> Result<R, GlesError> {
    if !desktop {
        return f();
    }
    ctx.lib
        .bind_api(egl::OPENGL_API)
        .map_err(|e| egl_error("eglBindAPI of desktop GL for the core", e))?;
    let out = f();
    // The renderer makes its contexts with OpenGL ES bound; a failure here
    // shows at its next eglCreateContext, not at a draw.
    let _ = ctx.lib.bind_api(egl::OPENGL_ES_API);
    out
}

/// Releases the core context of the thread, whose API is desktop GL when
/// `desktop`.
fn release(ctx: &EglContext, desktop: bool) -> Result<(), GlesError> {
    on_api(ctx, desktop, || {
        ctx.lib
            .make_current(ctx.display, None, None, None)
            .map_err(|e| egl_error("eglMakeCurrent to release the core", e))
    })
}

fn destroy_sync(ctx: &EglContext, fence: egl::Sync) {
    // SAFETY: the fence was made on this display. A failure leaves a fence
    // that EGL frees with the display.
    let _ = unsafe { ctx.lib.destroy_sync(ctx.display, fence) };
}
