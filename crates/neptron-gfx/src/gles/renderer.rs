use std::rc::Rc;

use neptron_types::{Filter, Frame, Rect, Scene};

use super::GlesError;
use super::ffi::{
    Buffer, EglContext, Framebuffer, GbmTarget, Gl, Program, Storage, Texture, Uniform,
};
use super::quad::{Quad, quad};
use super::shader::{self, VARIANTS, Variant};
use super::texels;
use crate::reference::{Pattern, check_frame, check_output, check_rect, check_scanlines};
use crate::{Image, OutputBuffer, RectKind};

/// Draws a [`Scene`] with EGL and OpenGL ES 2, with the pixel rules of
/// [`ReferenceRenderer`](crate::ReferenceRenderer).
///
/// # Pixels
///
/// The renderer draws two quads: the game frame into `dest`, then the UI
/// canvas into `ui_dest`, over black. A quad covers only the output pixels
/// whose source pixel is inside the frame; the rest stays black, as in the
/// reference. Each quad uses the program of the axes that blend: sharp
/// bilinear at a scale that is not a whole number. The fragment shader
/// computes the taps of each output pixel from its position with the
/// integer rules of the reference, in 32-bit floats that hold every value
/// exactly, and reads each tap with one `NEAREST` texture read. Each source
/// image is a texture of 8-bit channels, expanded on upload with the code
/// of the reference. So nearest output equals the reference byte for byte.
/// The shader blends the taps and rounds half up, as the reference does,
/// but in floats: a blend whose exact value is within float error of a
/// half can round the other way, so a channel can differ by 1. The shader
/// needs highp floats with the 23-bit precision of an IEEE 32-bit float,
/// and [`headless`](Self::headless) checks that the driver gives it.
///
/// Scanlines follow the same split. The pattern of dark lines comes from
/// the reference code and reaches the shader as a uniform. On a nearest
/// pixel the shader darkens with exact integer steps, so it equals the
/// reference. On a blended pixel it scales the float blend before its one
/// rounding, so a channel can differ by 1.
///
/// The renderer refuses what the reference refuses, with
/// [`GlesError::Scene`]. It also refuses a scene that the shader cannot
/// compute exactly ([`GlesError::ScaleTooLarge`]), and images or outputs
/// above the GL limits.
///
/// # Target
///
/// [`headless`](Self::headless) makes a context with no display on the
/// Mesa surfaceless platform. It draws into a framebuffer object, and
/// [`render`](Self::render) reads the pixels back.
///
/// All surfaceless renderers of a process share one EGL display. When EGL
/// has `EGL_KHR_display_reference`, each renderer holds a reference to it,
/// and dropping one renderer leaves the others working. Without that
/// extension a renderer never terminates the display, so the display stays
/// initialized until the process ends.
///
/// [`on_gbm`](Self::on_gbm) makes a context on the GBM platform with a
/// window surface on the KMS backend's GBM surface, and
/// [`draw_to_window`](Self::draw_to_window) draws into it and swaps. That
/// display belongs to the renderer alone and ends with it. Both targets
/// share one draw function, so they give the same pixels.
///
/// # Threads and allocation
///
/// The renderer is not `Send`: its GL context stays on the thread that
/// made it. Each call makes the context current first, which replaces any
/// other context that is current on the thread. Dropping the renderer
/// releases the current context of the thread. Textures, the buffer of
/// source texels and the read-back buffer are made on the first call and
/// again when a size changes; other calls allocate nothing.
///
/// # Byte order
///
/// Source pixels are native-endian. An `XRGB8888` pixel goes to GL as the
/// bytes B, G, R, X, which is its little-endian layout, so the module does
/// not build for a big-endian target.
pub struct GlesRenderer {
    game: SourceTexture,
    ui: SourceTexture,
    target: Option<Offscreen>,
    /// One program for each [`Variant`], in [`Variant::index`] order.
    programs: [(Program, Uniforms); VARIANTS],
    corners: Buffer,
    max_texture: u32,
    max_output: u32,
    context: EglContext,
}

impl std::fmt::Debug for GlesRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlesRenderer")
            .field("output", &self.target.as_ref().map(|t| (t.width, t.height)))
            .finish_non_exhaustive()
    }
}

impl GlesRenderer {
    /// A renderer with no display: an OpenGL ES 2 context on the EGL
    /// surfaceless platform of Mesa.
    ///
    /// # Errors
    ///
    /// [`GlesError`] when EGL does not load, the platform or an OpenGL ES 2
    /// context is not available, highp floats are not 32-bit floats, GL has
    /// no `GL_EXT_texture_format_BGRA8888`, or the program does not compile.
    pub fn headless() -> Result<Self, GlesError> {
        Self::on(EglContext::surfaceless()?)
    }

    /// A renderer that draws into a window surface on the GBM surface of
    /// `target`, for the KMS backend. It also renders into an output buffer
    /// as [`headless`](Self::headless) does. The renderer holds `target`
    /// until it drops, so the GBM objects outlive its EGL surface.
    ///
    /// # Errors
    ///
    /// [`GlesError`] as for [`headless`](Self::headless), and when EGL has
    /// no GBM platform or no config of the GBM format.
    pub fn on_gbm(target: Rc<dyn GbmTarget>) -> Result<Self, GlesError> {
        Self::on(EglContext::on_gbm(target)?)
    }

    /// Draws `scene` into the window surface as a `width` by `height`
    /// output at the GL origin, the bottom left, and swaps: the frame
    /// becomes the front buffer of the GBM surface. The pixels are the
    /// pixels of [`render`](Self::render). A window surface stores the last
    /// GL row first, so the picture is in the last `height` rows of the
    /// surface buffer. The rest of the surface is black. An output with a
    /// zero side draws nothing and does not swap: the surface keeps its
    /// front buffer, as `render` leaves its output unchanged.
    ///
    /// # Errors
    ///
    /// The errors of [`render`](Self::render), and [`GlesError::NoWindow`]
    /// on a renderer with no window surface. Nothing is swapped after an
    /// error.
    pub fn draw_to_window(
        &mut self,
        scene: &Scene<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), GlesError> {
        if !self.context.has_window() {
            return Err(GlesError::NoWindow);
        }
        if !self.draw(scene, width, height, Surface::Window)? {
            return Ok(());
        }
        self.context.swap()
    }

    fn on(context: EglContext) -> Result<Self, GlesError> {
        let gl = context.current()?;
        gl.check_highp()?;
        gl.require_extension(BGRA_EXTENSION)?;
        // On an error below, dropping the context deletes every object
        // made in it.
        let [a, b, c, d] =
            Variant::ALL.map(|variant| gl.program(shader::VERTEX, &variant.fragment()));
        let programs = [a?, b?, c?, d?].map(|p| (p, Uniforms::of(&gl, p)));
        let corners = gl.corner_buffer()?;
        let game = SourceTexture::new(&gl)?;
        let ui = SourceTexture::new(&gl)?;
        let (max_texture, max_viewport) = gl.limits();
        gl.check("renderer setup")?;
        Ok(Self {
            game,
            ui,
            target: None,
            programs,
            corners,
            max_texture,
            max_output: max_texture.min(max_viewport),
            context,
        })
    }

    /// Draws `scene` into `out`, with the rules of
    /// [`ReferenceRenderer`](crate::ReferenceRenderer).
    ///
    /// # Errors
    ///
    /// [`GlesError::Scene`] for what the reference refuses, and the other
    /// [`GlesError`] values for a scene or an output above a limit of this
    /// renderer, or a failed GL call. The renderer checks the scene before
    /// it draws, and `out` is unchanged after an error.
    pub fn render(
        &mut self,
        scene: &Scene<'_>,
        out: &mut OutputBuffer<'_>,
    ) -> Result<(), GlesError> {
        check_output(out)?;
        if !self.draw(scene, out.width, out.height, Surface::Offscreen)? {
            return Ok(());
        }
        let gl = self.context.current()?;
        // The draw made the target at this size, so this changes nothing.
        let target = ensure_target(&gl, &mut self.target, out.width, out.height)?;
        gl.read_rgba(out.width, out.height, &mut target.pixels)?;
        gl.check("glReadPixels")?;
        copy_out(&target.pixels, out);
        Ok(())
    }

    /// Checks `scene` for an output of `width` by `height` and draws it into
    /// `surface`. Returns false, with nothing drawn, for an output with a
    /// zero side. Every target and every finish step goes through this one
    /// function, so they all get the same pixels.
    fn draw(
        &mut self,
        scene: &Scene<'_>,
        width: u32,
        height: u32,
        surface: Surface,
    ) -> Result<bool, GlesError> {
        let size = (width, height);
        let game = match &scene.game {
            Some(frame) => {
                check_frame(frame, Image::Game)?;
                check_rect(&scene.crop, RectKind::Crop)?;
                check_rect(&scene.dest, RectKind::Dest)?;
                check_scanlines(&scene.scanlines)?;
                let pattern = Pattern::of(scene.scanlines, scene.crop.h, scene.dest.h);
                let rects = (scene.crop, scene.dest);
                draw_of(frame, rects, scene.filter, pattern, RectKind::Dest, size)?
            }
            None => None,
        };
        let ui = match &scene.ui {
            Some(frame) => {
                check_frame(frame, Image::Ui)?;
                check_rect(&scene.ui_dest, RectKind::UiDest)?;
                let whole = Rect {
                    x: 0,
                    y: 0,
                    w: u32::from(frame.width),
                    h: u32::from(frame.height),
                };
                let (rects, kind) = ((whole, scene.ui_dest), RectKind::UiDest);
                draw_of(frame, rects, Filter::Nearest, None, kind, size)?
            }
            None => None,
        };
        if width == 0 || height == 0 {
            return Ok(false);
        }
        self.check_sizes(size, game.as_ref(), ui.as_ref())?;

        let gl = self.context.current()?;
        gl.drain_errors()?;
        match surface {
            Surface::Offscreen => {
                let target = ensure_target(&gl, &mut self.target, width, height)?;
                gl.begin(target.framebuffer, width, height);
            }
            Surface::Window => gl.begin_window(width, height),
        }
        for (draw, texture) in [(game, &mut self.game), (ui, &mut self.ui)] {
            if let Some(draw) = draw {
                texture.upload(&gl, &draw.frame)?;
                gl.check("source texture upload")?;
                // Variant::index is below VARIANTS for every variant: the
                // shader tests check it for each one.
                let (program, uniforms) = &self.programs[draw.quad.variant.index()];
                gl.bind_quad(*program, self.corners, texture.texture);
                uniforms.set(&gl, &draw.quad, width, height);
                gl.draw_quad();
            }
        }
        gl.check("draw")?;
        Ok(true)
    }

    /// The GL renderer and version strings, such as `softpipe` and
    /// `OpenGL ES 3.1 Mesa 26.1.8`, for logs and reports.
    ///
    /// # Errors
    ///
    /// [`GlesError::Egl`] when the context cannot be made current.
    pub fn gl_strings(&self) -> Result<(String, String), GlesError> {
        Ok(self.context.current()?.names())
    }

    /// `GL_MAX_TEXTURE_SIZE`: a source image with more pixels in a row, or
    /// more rows, gives [`GlesError::TextureTooLarge`].
    pub fn max_texture_side(&self) -> u32 {
        self.max_texture
    }

    /// The largest output side, from `GL_MAX_TEXTURE_SIZE` and
    /// `GL_MAX_VIEWPORT_DIMS`: a larger output gives
    /// [`GlesError::OutputTooLarge`].
    pub fn max_output_side(&self) -> u32 {
        self.max_output
    }

    fn check_sizes(
        &self,
        (width, height): (u32, u32),
        game: Option<&Draw<'_>>,
        ui: Option<&Draw<'_>>,
    ) -> Result<(), GlesError> {
        if width > self.max_output || height > self.max_output {
            return Err(GlesError::OutputTooLarge {
                width,
                height,
                limit: self.max_output,
            });
        }
        for (draw, image) in [(game, Image::Game), (ui, Image::Ui)] {
            let Some(draw) = draw else { continue };
            let (width, height) = texture_size(&draw.frame);
            if width > u64::from(self.max_texture) || height > self.max_texture {
                return Err(GlesError::TextureTooLarge {
                    image,
                    width,
                    height,
                    limit: self.max_texture,
                });
            }
        }
        Ok(())
    }
}

/// Where [`GlesRenderer::draw`] draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    /// The framebuffer object that `render` reads back.
    Offscreen,
    /// The window surface on GBM, from its origin.
    Window,
}

/// One quad to draw, with the image that it shows.
struct Draw<'a> {
    frame: Frame<'a>,
    quad: Quad,
}

fn draw_of<'a>(
    frame: &Frame<'a>,
    rects: (Rect, Rect),
    filter: Filter,
    pattern: Option<Pattern>,
    kind: RectKind,
    size: (u32, u32),
) -> Result<Option<Draw<'a>>, GlesError> {
    Ok(
        quad(frame, rects, filter, pattern, kind, size)?.map(|quad| Draw {
            frame: *frame,
            quad,
        }),
    )
}

/// The texture of a source image: one texel per pixel.
fn texture_size(frame: &Frame<'_>) -> (u64, u32) {
    (u64::from(frame.width), u32::from(frame.height))
}

/// The GL extension that uploads `XRGB8888` with no conversion. Mesa has
/// it on every driver.
const BGRA_EXTENSION: &str = "GL_EXT_texture_format_BGRA8888";

/// A texture that holds one source image, with the size of its storage,
/// and the buffer that holds texels that GL cannot read from the frame.
struct SourceTexture {
    texture: Texture,
    size: (u32, u32),
    staging: Vec<u8>,
}

impl SourceTexture {
    fn new(gl: &Gl<'_>) -> Result<Self, GlesError> {
        Ok(Self {
            texture: gl.texture()?,
            size: (0, 0),
            staging: Vec::new(),
        })
    }

    /// Copies `frame` into the texture. The caller checked the frame and
    /// the texture limit, so both sides fit a `u32`.
    fn upload(&mut self, gl: &Gl<'_>, frame: &Frame<'_>) -> Result<(), GlesError> {
        let size = (u32::from(frame.width), u32::from(frame.height));
        if self.size != size {
            gl.allocate(self.texture, Storage::Source, size.0, size.1);
            self.size = size;
        }
        let data = texels::packed(frame, &mut self.staging)?;
        gl.upload(self.texture, data, size.0, size.1)
    }
}

/// The framebuffer object that the headless renderer draws into, and the
/// buffer that receives its pixels.
struct Offscreen {
    framebuffer: Framebuffer,
    texture: Texture,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// The offscreen target of `width` by `height`. Its storage changes only
/// with the size. GL objects live as long as the context, which deletes
/// them when it is destroyed.
fn ensure_target<'t>(
    gl: &Gl<'_>,
    target: &'t mut Option<Offscreen>,
    width: u32,
    height: u32,
) -> Result<&'t mut Offscreen, GlesError> {
    let t = match target {
        Some(t) => t,
        None => {
            let texture = gl.texture()?;
            let framebuffer = gl.framebuffer(texture)?;
            target.insert(Offscreen {
                framebuffer,
                texture,
                width: 0,
                height: 0,
                pixels: Vec::new(),
            })
        }
    };
    if (t.width, t.height) != (width, height) {
        gl.allocate(t.texture, Storage::Target, width, height);
        gl.check_framebuffer(t.framebuffer)?;
        gl.check("framebuffer setup")?;
        t.pixels.clear();
        t.pixels.resize(width as usize * height as usize * 4, 0);
        (t.width, t.height) = (width, height);
    }
    Ok(t)
}

/// Writes the RGBA rows from GL, bottom row first, into `out` as XRGB8888
/// rows from the top.
fn copy_out(pixels: &[u8], out: &mut OutputBuffer<'_>) {
    let row_bytes = out.width as usize * 4;
    for (y, gl_row) in pixels.chunks_exact(row_bytes).rev().enumerate() {
        let start = y * out.pitch;
        let Some(row) = out.data.get_mut(start..start + row_bytes) else {
            continue;
        };
        for (px, rgba) in row.chunks_exact_mut(4).zip(gl_row.chunks_exact(4)) {
            px.copy_from_slice(&[rgba[2], rgba[1], rgba[0], 0xFF]);
        }
    }
}

/// The uniform locations of the program.
struct Uniforms {
    output: Option<Uniform>,
    visible: Option<Uniform>,
    dest: Option<Uniform>,
    crop: Option<Uniform>,
    frame: Option<Uniform>,
    axis: Option<Uniform>,
    bounds: Option<Uniform>,
    scan: Option<Uniform>,
    texture: Option<Uniform>,
}

impl Uniforms {
    fn of(gl: &Gl<'_>, program: Program) -> Self {
        Self {
            output: gl.uniform(program, "u_output"),
            visible: gl.uniform(program, "u_visible"),
            dest: gl.uniform(program, "u_dest"),
            crop: gl.uniform(program, "u_crop"),
            frame: gl.uniform(program, "u_frame"),
            axis: gl.uniform(program, "u_axis"),
            bounds: gl.uniform(program, "u_bounds"),
            scan: gl.uniform(program, "u_scan"),
            texture: gl.uniform(program, "u_texture"),
        }
    }

    fn set(&self, gl: &Gl<'_>, quad: &Quad, width: u32, height: u32) {
        gl.set_vec4(
            self.output.as_ref(),
            [width as f32, height as f32, 0.0, 0.0],
        );
        gl.set_vec4(self.visible.as_ref(), quad.visible);
        gl.set_vec4(self.dest.as_ref(), quad.dest);
        gl.set_vec4(self.crop.as_ref(), quad.crop);
        gl.set_vec4(self.frame.as_ref(), quad.frame);
        gl.set_vec4(self.axis.as_ref(), quad.axis);
        gl.set_vec4(self.bounds.as_ref(), quad.bounds);
        gl.set_vec4(self.scan.as_ref(), quad.scan);
        gl.set_int(self.texture.as_ref(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn headless_without_libegl_is_a_load_error() {
        let err = GlesRenderer::headless().unwrap_err();
        assert!(matches!(err, GlesError::LoadEgl { .. }), "{err}");
    }

    #[test]
    fn copy_out_flips_rows_and_swaps_channels() {
        // Two rows of one pixel, bottom row first as GL gives them.
        let pixels = [1, 2, 3, 0, 4, 5, 6, 0];
        let mut data = [0xAA_u8; 12];
        let mut out = OutputBuffer {
            data: &mut data,
            width: 1,
            height: 2,
            pitch: 8,
        };
        copy_out(&pixels, &mut out);
        assert_eq!(data[..4], [6, 5, 4, 0xFF]);
        assert_eq!(data[4..8], [0xAA; 4], "row padding is not written");
        assert_eq!(data[8..], [3, 2, 1, 0xFF]);
    }
}
