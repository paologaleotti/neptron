//! Every `unsafe` call of the GLES renderer: loading libEGL, the platform
//! display, the window surface on GBM, the GL function table, and each GL
//! call.
//!
//! A [`Gl`] exists only while [`EglContext::current`] holds: the context is
//! current on the calling thread, and the GL function table belongs to it.
//! Each GL call below relies on that, and on the checks of its arguments
//! that the wrapper makes.

use std::ffi::{CStr, c_void};
use std::marker::PhantomData;
use std::rc::Rc;

use glow::HasContext as _;
use khronos_egl as egl;

use super::GlesError;
use super::texels::{BYTES_PER_TEXEL, Texels};
use super::version;

/// EGL 1.5, loaded at run time from `libEGL.so.1`.
pub(super) type EglInstance = egl::DynamicInstance<egl::EGL1_5>;

/// `EGL_PLATFORM_SURFACELESS_MESA`, from `EGL_MESA_platform_surfaceless`.
const PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;
/// `EGL_PLATFORM_GBM_KHR`, from `EGL_KHR_platform_gbm`.
const PLATFORM_GBM_KHR: egl::Enum = 0x31D7;
/// `GBM_FORMAT_XRGB8888`, the fourcc `XR24`: the native visual of an EGL
/// config whose buffers a GBM surface of that format can take.
const GBM_FORMAT_XRGB8888: egl::Int = 0x3432_5258;
/// Configs read to find the one with the GBM format. Mesa lists a few
/// dozen.
const MAX_CONFIGS: usize = 256;
/// `EGL_TRACK_REFERENCES_KHR`, from `EGL_KHR_display_reference`.
const TRACK_REFERENCES_KHR: egl::Attrib = 0x3352;
/// The fewest mantissa bits of a highp float that keep every integer of
/// the shader below 2^24 exact: the bits of an IEEE 32-bit float.
pub(super) const MIN_HIGHP_PRECISION_BITS: i32 = 23;
/// More pending GL errors than this means a fault that draining does not
/// clear.
const MAX_PENDING_GL_ERRORS: usize = 16;

/// A GBM device and a GBM surface on it, which the renderer draws into
/// through EGL. The KMS backend owns them; the renderer sees two pointers.
///
/// # Safety
///
/// [`gbm_device`](Self::gbm_device) returns a live `struct gbm_device *`,
/// and [`gbm_surface`](Self::gbm_surface) a live `struct gbm_surface *` made
/// on that device with the format `GBM_FORMAT_XRGB8888`. Both stay live and
/// unchanged for as long as the value lives.
pub unsafe trait GbmTarget {
    /// The `struct gbm_device *`.
    fn gbm_device(&self) -> *mut c_void;
    /// The `struct gbm_surface *`.
    fn gbm_surface(&self) -> *mut c_void;
}

/// Opens `libEGL.so.1` and checks that it provides EGL 1.5.
fn load_egl() -> Result<EglInstance, GlesError> {
    // SAFETY: the library is the system EGL library, found by its standard
    // name. load_required checks that every EGL 1.5 entry point exists; the
    // EGL ABI defines their signatures.
    unsafe { EglInstance::load_required() }.map_err(|e| GlesError::LoadEgl {
        reason: e.to_string(),
    })
}

/// Maps an EGL error to [`GlesError::Egl`] with the name of the call.
fn egl_error(call: &'static str, e: egl::Error) -> GlesError {
    GlesError::Egl {
        call,
        code: e.native(),
        message: e.to_string(),
    }
}

/// An OpenGL ES 2 context on an EGL display, with its GL function table.
///
/// It is neither `Send` nor `Sync`: GL calls go to the context that is
/// current on the calling thread, so the context stays on the thread that
/// made it.
pub(super) struct EglContext {
    // Drop order: the function table, then the surface and the context (in
    // Drop), then the library that holds the code of both, then the GBM
    // objects under the surface.
    gl: glow::Context,
    context: egl::Context,
    /// The window surface on GBM, or `None` with no surface.
    surface: Option<egl::Surface>,
    display: egl::Display,
    /// True when drop terminates the display: the display counts its
    /// references, so terminate releases only this renderer's reference,
    /// or the display belongs to this context alone (GBM).
    counted: bool,
    lib: EglInstance,
    _target: Option<Rc<dyn GbmTarget>>,
    not_send: PhantomData<*const ()>,
}

impl EglContext {
    /// A context on the Mesa surfaceless platform: no window system and no
    /// display device. The renderer draws into a framebuffer object.
    pub(super) fn surfaceless() -> Result<Self, GlesError> {
        let lib = load_egl()?;
        // Every surfaceless display of the process is the same EGLDisplay.
        // With reference tracking, initialize and terminate count, so one
        // renderer that goes away leaves the others working.
        let counted = lib
            .query_string(None, egl::EXTENSIONS)
            .is_ok_and(|ext| has_extension(ext, "EGL_KHR_display_reference"));
        let attributes: &[egl::Attrib] = if counted {
            &[
                TRACK_REFERENCES_KHR,
                egl::TRUE as egl::Attrib,
                egl::ATTRIB_NONE,
            ]
        } else {
            &[egl::ATTRIB_NONE]
        };
        // SAFETY: the surfaceless platform takes EGL_DEFAULT_DISPLAY as its
        // native display and needs no native object. The attribute list
        // ends with ATTRIB_NONE.
        let display = unsafe {
            lib.get_platform_display(PLATFORM_SURFACELESS_MESA, egl::DEFAULT_DISPLAY, attributes)
        }
        .map_err(|e| egl_error("eglGetPlatformDisplay(surfaceless)", e))?;
        lib.initialize(display)
            .map_err(|e| egl_error("eglInitialize", e))?;
        let made = context_on(&lib, display).and_then(|context| {
            gl_on(&lib).map(|gl| (gl, context)).inspect_err(|_| {
                // The GL error is the one to report.
                let _ = lib.make_current(display, None, None, None);
                let _ = lib.destroy_context(display, context);
            })
        });
        match made {
            Ok((gl, context)) => Ok(Self {
                gl,
                context,
                surface: None,
                display,
                counted,
                lib,
                _target: None,
                not_send: PhantomData,
            }),
            Err(e) => {
                if counted {
                    // The setup error is the one to report.
                    let _ = lib.terminate(display);
                }
                Err(e)
            }
        }
    }

    /// A context on the GBM platform with a window surface on the GBM
    /// surface of `target`. The EGL display belongs to this context alone,
    /// because each GBM device gets its own display.
    pub(super) fn on_gbm(target: Rc<dyn GbmTarget>) -> Result<Self, GlesError> {
        let lib = load_egl()?;
        // SAFETY: the GbmTarget contract makes the native display a live
        // gbm_device. The attribute list ends with ATTRIB_NONE.
        let display = unsafe {
            lib.get_platform_display(PLATFORM_GBM_KHR, target.gbm_device(), &[egl::ATTRIB_NONE])
        }
        .map_err(|e| egl_error("eglGetPlatformDisplay(gbm)", e))?;
        lib.initialize(display)
            .map_err(|e| egl_error("eglInitialize", e))?;
        let made = window_on(&lib, display, target.as_ref()).and_then(|(context, surface)| {
            gl_on(&lib)
                .map(|gl| (gl, context, surface))
                .inspect_err(|_| {
                    // The GL error is the one to report.
                    let _ = lib.make_current(display, None, None, None);
                    let _ = lib.destroy_surface(display, surface);
                    let _ = lib.destroy_context(display, context);
                })
        });
        match made {
            Ok((gl, context, surface)) => Ok(Self {
                gl,
                context,
                surface: Some(surface),
                display,
                counted: true,
                lib,
                _target: Some(target),
                not_send: PhantomData,
            }),
            Err(e) => {
                // The setup error is the one to report.
                let _ = lib.terminate(display);
                Err(e)
            }
        }
    }

    /// Makes the context current on this thread, with its window surface
    /// when it has one, and gives the GL calls.
    pub(super) fn current(&self) -> Result<Gl<'_>, GlesError> {
        self.lib
            .make_current(self.display, self.surface, self.surface, Some(self.context))
            .map_err(|e| egl_error("eglMakeCurrent", e))?;
        Ok(Gl { gl: &self.gl })
    }

    /// True when the context has a window surface.
    pub(super) fn has_window(&self) -> bool {
        self.surface.is_some()
    }

    /// Shows what was drawn into the window surface: on GBM, it finishes
    /// the frame and makes it the surface's front buffer.
    pub(super) fn swap(&self) -> Result<(), GlesError> {
        let surface = self.surface.ok_or(GlesError::NoWindow)?;
        self.lib
            .swap_buffers(self.display, surface)
            .map_err(|e| egl_error("eglSwapBuffers", e))
    }
}

/// Binds OpenGL ES, and makes an ES 2 context current on a window surface
/// on the GBM surface of `target`.
fn window_on(
    lib: &EglInstance,
    display: egl::Display,
    target: &dyn GbmTarget,
) -> Result<(egl::Context, egl::Surface), GlesError> {
    lib.bind_api(egl::OPENGL_ES_API)
        .map_err(|e| egl_error("eglBindAPI", e))?;
    let attributes = [
        egl::SURFACE_TYPE,
        egl::WINDOW_BIT,
        egl::RENDERABLE_TYPE,
        egl::OPENGL_ES2_BIT,
        egl::RED_SIZE,
        8,
        egl::GREEN_SIZE,
        8,
        egl::BLUE_SIZE,
        8,
        egl::NONE,
    ];
    let mut configs = Vec::with_capacity(MAX_CONFIGS);
    lib.choose_config(display, &attributes, &mut configs)
        .map_err(|e| egl_error("eglChooseConfig", e))?;
    let config = configs
        .into_iter()
        .find(|&c| {
            lib.get_config_attrib(display, c, egl::NATIVE_VISUAL_ID)
                .is_ok_and(|id| id == GBM_FORMAT_XRGB8888)
        })
        .ok_or(GlesError::NoConfig)?;
    let context = lib
        .create_context(
            display,
            config,
            None,
            &[egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE],
        )
        .map_err(|e| egl_error("eglCreateContext", e))?;
    // SAFETY: the GbmTarget contract makes the native window a live
    // gbm_surface of format XRGB8888 on the display's device, the format of
    // the config. The attribute list ends with ATTRIB_NONE.
    let surface = unsafe {
        lib.create_platform_window_surface(
            display,
            config,
            target.gbm_surface(),
            &[egl::ATTRIB_NONE],
        )
    };
    let surface = match surface {
        Ok(s) => s,
        Err(e) => {
            // The surface error is the one to report.
            let _ = lib.destroy_context(display, context);
            return Err(egl_error("eglCreatePlatformWindowSurface", e));
        }
    };
    if let Err(e) = lib.make_current(display, Some(surface), Some(surface), Some(context)) {
        // The make-current error is the one to report.
        let _ = lib.destroy_surface(display, surface);
        let _ = lib.destroy_context(display, context);
        return Err(egl_error("eglMakeCurrent with the window surface", e));
    }
    Ok((context, surface))
}

/// Binds OpenGL ES, and makes an ES 2 context current with no surface.
fn context_on(lib: &EglInstance, display: egl::Display) -> Result<egl::Context, GlesError> {
    lib.bind_api(egl::OPENGL_ES_API)
        .map_err(|e| egl_error("eglBindAPI", e))?;
    let attributes = [
        egl::RENDERABLE_TYPE,
        egl::OPENGL_ES2_BIT,
        egl::SURFACE_TYPE,
        egl::PBUFFER_BIT,
        egl::NONE,
    ];
    let config = lib
        .choose_first_config(display, &attributes)
        .map_err(|e| egl_error("eglChooseConfig", e))?
        .ok_or(GlesError::NoConfig)?;
    let context = lib
        .create_context(
            display,
            config,
            None,
            &[egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE],
        )
        .map_err(|e| egl_error("eglCreateContext", e))?;
    if let Err(e) = lib.make_current(display, None, None, Some(context)) {
        // The make-current error is the one to report.
        let _ = lib.destroy_context(display, context);
        return Err(egl_error("eglMakeCurrent with no surface", e));
    }
    Ok(context)
}

/// True when the space-separated list `extensions` holds `name`.
fn has_extension(extensions: &CStr, name: &str) -> bool {
    extensions
        .to_str()
        .is_ok_and(|list| list.split_ascii_whitespace().any(|e| e == name))
}

/// The GL function table of the current context. glow panics when
/// glGetString is missing or GL_VERSION is null, not UTF-8 or not a version
/// it parses, so this checks all of that first with its own call.
fn gl_on(lib: &EglInstance) -> Result<glow::Context, GlesError> {
    type GetString = unsafe extern "system" fn(u32) -> *const u8;
    let get_string = lib
        .get_proc_address("glGetString")
        .ok_or(GlesError::GlVersion {
            text: "no glGetString".to_owned(),
        })?;
    // SAFETY: eglGetProcAddress gave the address of glGetString, whose
    // signature in the GL ES ABI is GetString: one GLenum in, a pointer to
    // a static string out.
    let get_string: GetString = unsafe { std::mem::transmute(get_string) };
    // SAFETY: a context is current (context_on), as glGetString needs.
    let text = unsafe { get_string(glow::VERSION) };
    if text.is_null() {
        return Err(GlesError::GlVersion {
            text: "GL_VERSION is null".to_owned(),
        });
    }
    // SAFETY: GL returns a null-terminated string that lives as long as the
    // context; it is copied before the next call.
    let text = unsafe { CStr::from_ptr(text.cast()) };
    let text = text.to_str().map_err(|_| GlesError::GlVersion {
        text: text.to_string_lossy().into_owned(),
    })?;
    version::check(text)?;
    // SAFETY: the context is current and glow's GL_VERSION query succeeds,
    // as checked above. With EGL 1.5 eglGetProcAddress returns core GL ES
    // functions too. A missing function is a null pointer, which glow
    // stores as not loaded.
    Ok(unsafe {
        glow::Context::from_loader_function(|name| {
            lib.get_proc_address(name)
                .map_or(std::ptr::null(), |f| f as *const c_void)
        })
    })
}

impl Drop for EglContext {
    fn drop(&mut self) {
        // Nothing can be done about a failure while the renderer goes away.
        let _ = self.lib.make_current(self.display, None, None, None);
        if let Some(surface) = self.surface {
            let _ = self.lib.destroy_surface(self.display, surface);
        }
        let _ = self.lib.destroy_context(self.display, self.context);
        if self.counted {
            let _ = self.lib.terminate(self.display);
        }
    }
}

/// A GL object name.
pub(super) type Texture = glow::NativeTexture;
/// A GL framebuffer name.
pub(super) type Framebuffer = glow::NativeFramebuffer;
/// A GL buffer name.
pub(super) type Buffer = glow::NativeBuffer;
/// A GL program name.
pub(super) type Program = glow::NativeProgram;
/// A GL uniform location.
pub(super) type Uniform = glow::NativeUniformLocation;

/// What a texture holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Storage {
    /// The colour buffer of the offscreen target: `RGBA` bytes.
    Target,
    /// A source image: `BGRA_EXT` bytes.
    Source,
}

impl Storage {
    /// The GL format; the type is `UNSIGNED_BYTE`.
    fn format(self) -> u32 {
        match self {
            Self::Target => glow::RGBA,
            Self::Source => glow::BGRA,
        }
    }
}

/// The GL calls of the renderer, valid while its context is current.
pub(super) struct Gl<'a> {
    gl: &'a glow::Context,
}

/// The vertex attribute index of the quad corners.
pub(super) const CORNER_ATTRIBUTE: u32 = 0;

impl Gl<'_> {
    /// The first pending GL error, as [`GlesError::Gl`] with `call`.
    pub(super) fn check(&self, call: &'static str) -> Result<(), GlesError> {
        // SAFETY: the context is current (see the type).
        let code = unsafe { self.gl.get_error() };
        if code == glow::NO_ERROR {
            Ok(())
        } else {
            Err(GlesError::Gl { call, code })
        }
    }

    /// The `GL_RENDERER` and `GL_VERSION` strings.
    pub(super) fn names(&self) -> (String, String) {
        // SAFETY: the context is current; both names are valid string
        // queries.
        unsafe {
            (
                self.gl.get_parameter_string(glow::RENDERER),
                self.gl.get_parameter_string(glow::VERSION),
            )
        }
    }

    /// Clears every pending GL error flag, so that a later check reports
    /// only its own call.
    pub(super) fn drain_errors(&self) -> Result<(), GlesError> {
        for _ in 0..MAX_PENDING_GL_ERRORS {
            // SAFETY: the context is current (see the type).
            if unsafe { self.gl.get_error() } == glow::NO_ERROR {
                return Ok(());
            }
        }
        self.check("draining old GL errors")
    }

    /// Checks that a highp float of the fragment shader has the precision
    /// of an IEEE 32-bit float.
    pub(super) fn check_highp(&self) -> Result<(), GlesError> {
        // SAFETY: the context is current; both enums are valid for
        // glGetShaderPrecisionFormat.
        let format = unsafe {
            self.gl
                .get_shader_precision_format(glow::FRAGMENT_SHADER, glow::HIGH_FLOAT)
        };
        let bits = format.map(|f| f.precision);
        match bits {
            Some(bits) if bits >= MIN_HIGHP_PRECISION_BITS => Ok(()),
            _ => Err(GlesError::LowPrecision {
                bits,
                needed: MIN_HIGHP_PRECISION_BITS,
            }),
        }
    }

    /// `GL_MAX_TEXTURE_SIZE` and the smaller side of `GL_MAX_VIEWPORT_DIMS`.
    pub(super) fn limits(&self) -> (u32, u32) {
        let mut viewport = [0_i32; 2];
        // SAFETY: the context is current. GL_MAX_VIEWPORT_DIMS writes two
        // integers, and the slice holds two.
        let texture = unsafe {
            self.gl
                .get_parameter_i32_slice(glow::MAX_VIEWPORT_DIMS, &mut viewport);
            self.gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE)
        };
        let positive = |v: i32| u32::try_from(v).unwrap_or(0);
        (positive(texture), positive(viewport[0].min(viewport[1])))
    }

    /// Compiles and links the program from its two sources. The corner
    /// attribute is bound to [`CORNER_ATTRIBUTE`].
    pub(super) fn program(&self, vertex: &str, fragment: &str) -> Result<Program, GlesError> {
        let vs = self.shader(glow::VERTEX_SHADER, "vertex", vertex)?;
        let fs = match self.shader(glow::FRAGMENT_SHADER, "fragment", fragment) {
            Ok(fs) => fs,
            Err(e) => {
                // SAFETY: the context is current and vs is its shader.
                unsafe { self.gl.delete_shader(vs) };
                return Err(e);
            }
        };
        // SAFETY: the context is current; vs and fs are its compiled shaders.
        // They are deleted after the link, which keeps them alive in the
        // program as GL defines.
        unsafe {
            let program = self
                .gl
                .create_program()
                .map_err(|message| GlesError::GlObject {
                    call: "glCreateProgram",
                    message,
                });
            if let Ok(p) = program {
                self.gl.attach_shader(p, vs);
                self.gl.attach_shader(p, fs);
                self.gl
                    .bind_attrib_location(p, CORNER_ATTRIBUTE, "a_corner");
                self.gl.link_program(p);
            }
            self.gl.delete_shader(vs);
            self.gl.delete_shader(fs);
            let p = program?;
            if self.gl.get_program_link_status(p) {
                Ok(p)
            } else {
                let log = self.gl.get_program_info_log(p);
                self.gl.delete_program(p);
                Err(GlesError::Link { log })
            }
        }
    }

    fn shader(
        &self,
        kind: u32,
        stage: &'static str,
        source: &str,
    ) -> Result<glow::NativeShader, GlesError> {
        // SAFETY: the context is current, and kind is a shader type.
        unsafe {
            let shader = self
                .gl
                .create_shader(kind)
                .map_err(|message| GlesError::GlObject {
                    call: "glCreateShader",
                    message,
                })?;
            self.gl.shader_source(shader, source);
            self.gl.compile_shader(shader);
            if self.gl.get_shader_compile_status(shader) {
                Ok(shader)
            } else {
                let log = self.gl.get_shader_info_log(shader);
                self.gl.delete_shader(shader);
                Err(GlesError::Compile { stage, log })
            }
        }
    }

    /// The location of a uniform of `program`, or `None` when the linker
    /// removed it.
    pub(super) fn uniform(&self, program: Program, name: &str) -> Option<Uniform> {
        // SAFETY: the context is current and program is its linked program.
        unsafe { self.gl.get_uniform_location(program, name) }
    }

    /// A buffer with the four corners of the unit square, in triangle strip
    /// order, as two floats each.
    pub(super) fn corner_buffer(&self) -> Result<Buffer, GlesError> {
        const CORNERS: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let mut bytes = [0_u8; 32];
        for (chunk, v) in bytes.chunks_exact_mut(4).zip(CORNERS) {
            chunk.copy_from_slice(&v.to_ne_bytes());
        }
        // SAFETY: the context is current; the data slice is the whole
        // buffer content.
        unsafe {
            let buffer = self
                .gl
                .create_buffer()
                .map_err(|message| GlesError::GlObject {
                    call: "glCreateBuffer",
                    message,
                })?;
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
            Ok(buffer)
        }
    }

    /// A texture with nearest sampling and clamped edges, and no storage.
    pub(super) fn texture(&self) -> Result<Texture, GlesError> {
        // SAFETY: the context is current; the parameters are valid for a 2D
        // texture.
        unsafe {
            let texture = self
                .gl
                .create_texture()
                .map_err(|message| GlesError::GlObject {
                    call: "glGenTextures",
                    message,
                })?;
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (name, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
                (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                self.gl
                    .tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
            }
            Ok(texture)
        }
    }

    /// Gives `texture` new storage of `width` by `height` texels of
    /// `storage`, with no content.
    pub(super) fn allocate(&self, texture: Texture, storage: Storage, width: u32, height: u32) {
        let format = storage.format();
        // SAFETY: the context is current. The caller checked the size
        // against GL_MAX_TEXTURE_SIZE, so both sides fit an i32. A null
        // pointer asks for storage with no content. GL ES 2 needs the
        // internal format equal to the format.
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                format as i32,
                width as i32,
                height as i32,
                0,
                format,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
        }
    }

    /// Writes `rows` packed rows of `width` `BGRA_EXT` texels from `texels`
    /// into the source texture `texture`, whose storage holds at least
    /// that many.
    pub(super) fn upload(
        &self,
        texture: Texture,
        texels: Texels<'_>,
        width: u32,
        rows: u32,
    ) -> Result<(), GlesError> {
        let data = match texels {
            Texels::Bytes(bytes) => bytes,
            // SAFETY: a u32 is 4 bytes with no padding, every byte is a
            // valid u8, and u8 needs no alignment, so the bytes of the
            // words are a valid slice for the same lifetime.
            Texels::Words(words) => unsafe {
                std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), size_of_val(words))
            },
        };
        let row_bytes = width as usize * BYTES_PER_TEXEL;
        if row_bytes.checked_mul(rows as usize) != Some(data.len()) {
            return Err(GlesError::UploadOutOfBounds {
                len: data.len(),
                width: row_bytes as u32,
                rows,
                pitch: row_bytes,
            });
        }
        // SAFETY: the context is current. UNPACK_ALIGNMENT 1 makes GL read
        // rows of exactly row_bytes bytes, and the check above proves that
        // data holds `rows` of them. The texture has storage of at least
        // width by rows texels, from the caller, so both sides fit an i32.
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                width as i32,
                rows as i32,
                Storage::Source.format(),
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
        }
        Ok(())
    }

    /// Checks that the context has the GL extension `name`.
    pub(super) fn require_extension(&self, name: &'static str) -> Result<(), GlesError> {
        if self.gl.supported_extensions().contains(name) {
            Ok(())
        } else {
            Err(GlesError::MissingExtension { name })
        }
    }

    /// A framebuffer with `texture` as its colour attachment.
    pub(super) fn framebuffer(&self, texture: Texture) -> Result<Framebuffer, GlesError> {
        // SAFETY: the context is current and texture is its texture.
        unsafe {
            let fb = self
                .gl
                .create_framebuffer()
                .map_err(|message| GlesError::GlObject {
                    call: "glGenFramebuffers",
                    message,
                })?;
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fb));
            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            Ok(fb)
        }
    }

    /// Checks that `fb` is complete.
    pub(super) fn check_framebuffer(&self, fb: Framebuffer) -> Result<(), GlesError> {
        // SAFETY: the context is current and fb is its framebuffer.
        let status = unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fb));
            self.gl.check_framebuffer_status(glow::FRAMEBUFFER)
        };
        if status == glow::FRAMEBUFFER_COMPLETE {
            Ok(())
        } else {
            Err(GlesError::Framebuffer { status })
        }
    }

    /// Binds `fb`, sets the viewport to `width` by `height` and clears it to
    /// opaque black.
    pub(super) fn begin(&self, fb: Framebuffer, width: u32, height: u32) {
        // SAFETY: the context is current. The caller checked the size
        // against the GL limits, so it fits an i32.
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fb));
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.disable(glow::BLEND);
            self.gl.disable(glow::DITHER);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.disable(glow::DEPTH_TEST);
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    /// Binds the window surface's framebuffer, sets the viewport to `width`
    /// by `height` at the origin and clears the whole surface to opaque
    /// black, as [`begin`](Self::begin) does for a framebuffer object.
    pub(super) fn begin_window(&self, width: u32, height: u32) {
        // SAFETY: the context is current with its window surface. The
        // caller checked the size against the GL limits, so it fits an i32.
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.disable(glow::BLEND);
            self.gl.disable(glow::DITHER);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.disable(glow::DEPTH_TEST);
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    /// Selects `program`, the corner buffer and `texture` on unit 0.
    pub(super) fn bind_quad(&self, program: Program, corners: Buffer, texture: Texture) {
        // SAFETY: the context is current; all three objects are its own.
        // The corner buffer holds 4 vertices of 2 floats.
        unsafe {
            self.gl.use_program(Some(program));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(corners));
            self.gl.enable_vertex_attrib_array(CORNER_ATTRIBUTE);
            self.gl
                .vertex_attrib_pointer_f32(CORNER_ATTRIBUTE, 2, glow::FLOAT, false, 0, 0);
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        }
    }

    /// Sets a `vec4` uniform of the program in use.
    pub(super) fn set_vec4(&self, at: Option<&Uniform>, v: [f32; 4]) {
        // SAFETY: the context is current; a None location is ignored by GL.
        unsafe { self.gl.uniform_4_f32(at, v[0], v[1], v[2], v[3]) };
    }

    /// Sets an `int` (or sampler) uniform of the program in use.
    pub(super) fn set_int(&self, at: Option<&Uniform>, v: i32) {
        // SAFETY: the context is current; a None location is ignored by GL.
        unsafe { self.gl.uniform_1_i32(at, v) };
    }

    /// Draws the corner buffer as one quad.
    pub(super) fn draw_quad(&self) {
        // SAFETY: the context is current and bind_quad set up 4 vertices.
        unsafe { self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4) };
    }

    /// Reads the `width` by `height` RGBA pixels of the bound framebuffer
    /// into `out`, bottom row first.
    pub(super) fn read_rgba(
        &self,
        width: u32,
        height: u32,
        out: &mut [u8],
    ) -> Result<(), GlesError> {
        let needed = width as usize * height as usize * 4;
        if out.len() < needed {
            return Err(GlesError::UploadOutOfBounds {
                len: out.len(),
                width: width * 4,
                rows: height,
                pitch: width as usize * 4,
            });
        }
        // SAFETY: the context is current. PACK_ALIGNMENT 1 and RGBA bytes
        // make GL write width * height * 4 bytes, which out holds.
        unsafe {
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut out[..needed])),
            );
        }
        Ok(())
    }
}
