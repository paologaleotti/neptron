//! The reference core GL: the eight GL functions of the test card of
//! `neptron-testcore` in plain Rust, on images in memory
//! (`docs/spec/hw-render.md`, section 7.2).
//!
//! The functions are `extern "C"`, as a core calls them, and keep their
//! state in a thread-local: GL has no handle, and one thread runs the core
//! and the display. A call while no context is current changes nothing and
//! is counted, as a GL call with no current context does nothing.
//!
//! An image stores texel rows from GL `y = 0` upward, one `0x00RRGGBB`
//! value per texel. A clear with the scissor test on covers the scissor
//! box; the viewport does not limit a clear, as in GL.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CStr;

use neptron_types::FrameOrigin;

/// `GL_NO_ERROR`.
pub const GL_NO_ERROR: u32 = 0;
/// `GL_INVALID_ENUM`.
pub const GL_INVALID_ENUM: u32 = 0x0500;
/// `GL_INVALID_VALUE`.
pub const GL_INVALID_VALUE: u32 = 0x0501;
/// `GL_INVALID_OPERATION`.
pub const GL_INVALID_OPERATION: u32 = 0x0502;

const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_DRAW_FRAMEBUFFER: u32 = 0x8CA9;
const GL_SCISSOR_TEST: u32 = 0x0C11;
const GL_BLEND: u32 = 0x0BE2;
const GL_DEPTH_TEST: u32 = 0x0B71;
const GL_STENCIL_TEST: u32 = 0x0B90;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
const GL_DEPTH_BUFFER_BIT: u32 = 0x0100;
const GL_STENCIL_BUFFER_BIT: u32 = 0x0400;

/// The address of the reference GL function of `name`, or 0 for a name it
/// does not have. It is the lookup of the `CoreTarget` of the fake
/// display.
pub fn gl_lookup(name: &CStr) -> usize {
    let address = match name.to_bytes() {
        b"glBindFramebuffer" => bind_framebuffer as extern "C" fn(u32, u32) as *const (),
        b"glViewport" => viewport as extern "C" fn(i32, i32, i32, i32) as *const (),
        b"glEnable" => enable as extern "C" fn(u32) as *const (),
        b"glDisable" => disable as extern "C" fn(u32) as *const (),
        b"glScissor" => scissor as extern "C" fn(i32, i32, i32, i32) as *const (),
        b"glClearColor" => clear_color as extern "C" fn(f32, f32, f32, f32) as *const (),
        b"glClear" => clear as extern "C" fn(u32) as *const (),
        b"glGetError" => get_error as extern "C" fn() -> u32 as *const (),
        _ => return 0,
    };
    address as usize
}

/// The GL calls on this thread that came while no core context was
/// current: each one changed nothing.
pub fn gl_calls_without_context() -> u64 {
    with(|gl| gl.calls_without_context).unwrap_or(0)
}

/// The GL state that a core can leave behind, on this thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlLeftState {
    /// The bound framebuffer.
    pub framebuffer: u32,
    /// The viewport: x, y, width, height.
    pub viewport: [i32; 4],
    /// The scissor box: x, y, width, height.
    pub scissor: [i32; 4],
    /// The scissor test is on.
    pub scissor_test: bool,
    /// Blending is on.
    pub blend: bool,
}

/// The GL state on this thread now.
pub fn gl_left_state() -> GlLeftState {
    with(|gl| GlLeftState {
        framebuffer: gl.bound,
        viewport: gl.viewport,
        scissor: gl.scissor,
        scissor_test: gl.scissor_test,
        blend: gl.blend,
    })
    .unwrap_or_default()
}

/// The GL state of this thread.
#[derive(Debug, Default)]
struct Gl {
    current: bool,
    images: BTreeMap<u32, Image>,
    next_name: u32,
    bound: u32,
    viewport: [i32; 4],
    scissor: [i32; 4],
    scissor_test: bool,
    blend: bool,
    clear_rgb: u32,
    error: u32,
    errors_raised: u32,
    calls_without_context: u64,
}

#[derive(Debug)]
struct Image {
    width: u16,
    height: u16,
    texels: Vec<u32>,
}

thread_local! {
    static GL: RefCell<Gl> = RefCell::new(Gl::default());
}

/// Runs `f` on the GL state of this thread. `None` when the state is gone
/// (thread teardown) or borrowed, which a GL function never nests.
fn with<R>(f: impl FnOnce(&mut Gl) -> R) -> Option<R> {
    GL.try_with(|gl| gl.try_borrow_mut().ok().map(|mut gl| f(&mut gl)))
        .ok()
        .flatten()
}

/// Runs a GL call on the state when a context is current.
fn call(f: impl FnOnce(&mut Gl)) {
    with(|gl| {
        if gl.current {
            f(gl);
        } else {
            gl.calls_without_context = gl.calls_without_context.saturating_add(1);
        }
    });
}

/// `glBindFramebuffer`: `GL_FRAMEBUFFER` or `GL_DRAW_FRAMEBUFFER`, and 0
/// or the framebuffer of an image.
pub extern "C" fn bind_framebuffer(target: u32, framebuffer: u32) {
    call(|gl| {
        if target != GL_FRAMEBUFFER && target != GL_DRAW_FRAMEBUFFER {
            gl.raise(GL_INVALID_ENUM);
        } else if framebuffer != 0 && !gl.images.contains_key(&framebuffer) {
            gl.raise(GL_INVALID_OPERATION);
        } else {
            gl.bound = framebuffer;
        }
    });
}

/// `glViewport`. A clear does not read it.
pub extern "C" fn viewport(x: i32, y: i32, width: i32, height: i32) {
    call(|gl| {
        if width < 0 || height < 0 {
            gl.raise(GL_INVALID_VALUE);
        } else {
            gl.viewport = [x, y, width, height];
        }
    });
}

/// `glEnable` of the scissor test, blending, the depth test or the
/// stencil test.
pub extern "C" fn enable(cap: u32) {
    call(|gl| gl.set_cap(cap, true));
}

/// `glDisable`, as [`enable`].
pub extern "C" fn disable(cap: u32) {
    call(|gl| gl.set_cap(cap, false));
}

/// `glScissor`: the box of a clear while the scissor test is on.
pub extern "C" fn scissor(x: i32, y: i32, width: i32, height: i32) {
    call(|gl| {
        if width < 0 || height < 0 {
            gl.raise(GL_INVALID_VALUE);
        } else {
            gl.scissor = [x, y, width, height];
        }
    });
}

/// `glClearColor`. Alpha is not stored: the image has no alpha.
pub extern "C" fn clear_color(red: f32, green: f32, blue: f32, _alpha: f32) {
    call(|gl| gl.clear_rgb = (unorm(red) << 16) | (unorm(green) << 8) | unorm(blue));
}

/// `glClear`: the colour bit fills the bound image, inside the scissor
/// box when the test is on. Framebuffer 0 is no surface.
pub extern "C" fn clear(mask: u32) {
    call(|gl| {
        if mask & !(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT) != 0 {
            gl.raise(GL_INVALID_VALUE);
        } else if mask & GL_COLOR_BUFFER_BIT != 0 {
            gl.clear_colour();
        }
    });
}

/// `glGetError`: the first error since the last call, then
/// `GL_NO_ERROR`.
pub extern "C" fn get_error() -> u32 {
    let mut error = GL_NO_ERROR;
    call(|gl| error = std::mem::replace(&mut gl.error, GL_NO_ERROR));
    error
}

/// A colour channel as GL stores it in 8 bits: clamped, then rounded.
fn unorm(value: f32) -> u32 {
    let clamped = if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    };
    (clamped * 255.0).round() as u32
}

impl Gl {
    fn raise(&mut self, error: u32) {
        if self.error == GL_NO_ERROR {
            self.error = error;
        }
        self.errors_raised = self.errors_raised.saturating_add(1);
    }

    fn set_cap(&mut self, cap: u32, on: bool) {
        match cap {
            GL_SCISSOR_TEST => self.scissor_test = on,
            GL_BLEND => self.blend = on,
            // The card draws with clears only, which neither test changes.
            GL_DEPTH_TEST | GL_STENCIL_TEST => {}
            _ => self.raise(GL_INVALID_ENUM),
        }
    }

    /// Fills the bound image, inside the scissor box when the test is on.
    /// Framebuffer 0 is no surface: the context of a core has none.
    fn clear_colour(&mut self) {
        let rgb = self.clear_rgb;
        let box_ = self.scissor_test.then_some(self.scissor);
        let Some(image) = self.images.get_mut(&self.bound) else {
            return;
        };
        let (w, h) = (i64::from(image.width), i64::from(image.height));
        let (x0, y0, x1, y1) = match box_ {
            Some([x, y, bw, bh]) => {
                let (x, y) = (i64::from(x), i64::from(y));
                (
                    x.clamp(0, w),
                    y.clamp(0, h),
                    (x + i64::from(bw)).clamp(0, w),
                    (y + i64::from(bh)).clamp(0, h),
                )
            }
            None => (0, 0, w, h),
        };
        for row in y0..y1 {
            let start = (row * w + x0) as usize;
            let end = (row * w + x1.max(x0)) as usize;
            if let Some(texels) = image.texels.get_mut(start..end) {
                texels.fill(rgb);
            }
        }
    }
}

/// Makes an image of `width` by `height` black texels and its framebuffer,
/// and returns the framebuffer name.
pub(crate) fn make_image(width: u16, height: u16) -> u32 {
    with(|gl| {
        gl.next_name = gl.next_name.saturating_add(1);
        let name = gl.next_name;
        gl.images.insert(name, Image::black(width, height));
        name
    })
    .unwrap_or(0)
}

/// Gives the image of framebuffer `name` new black texels of a new size.
pub(crate) fn resize_image(name: u32, width: u16, height: u16) {
    with(|gl| {
        if let Some(image) = gl.images.get_mut(&name) {
            *image = Image::black(width, height);
        }
    });
}

/// Drops the image of framebuffer `name`.
pub(crate) fn drop_image(name: u32) {
    with(|gl| {
        gl.images.remove(&name);
        if gl.bound == name {
            gl.bound = 0;
        }
    });
}

/// Makes the context current or not.
pub(crate) fn set_current(current: bool) {
    with(|gl| gl.current = current);
}

/// The GL errors raised since the last call.
pub(crate) fn take_errors_raised() -> u32 {
    with(|gl| std::mem::take(&mut gl.errors_raised)).unwrap_or(0)
}

/// Copies the `width` by `height` picture of framebuffer `name` into `out`
/// as XRGB8888 native-endian pixels with no padding, upright by the rule of
/// `origin`. False when the image does not exist, the picture does not fit
/// it, or `out` is too short.
pub(crate) fn read_picture(
    name: u32,
    out: &mut [u8],
    width: u16,
    height: u16,
    origin: FrameOrigin,
) -> bool {
    with(|gl| {
        let Some(image) = gl.images.get(&name) else {
            return false;
        };
        if width > image.width || height > image.height {
            return false;
        }
        let row_bytes = usize::from(width) * 4;
        if out.len() < row_bytes * usize::from(height) {
            return false;
        }
        for (line, dst) in (0..height).zip(out.chunks_exact_mut(row_bytes)) {
            let Some(row) = origin.storage_row(line, height) else {
                return false;
            };
            let start = usize::from(row) * usize::from(image.width);
            let Some(texels) = image.texels.get(start..start + usize::from(width)) else {
                return false;
            };
            for (px, texel) in dst.chunks_exact_mut(4).zip(texels) {
                px.copy_from_slice(&texel.to_ne_bytes());
            }
        }
        true
    })
    .unwrap_or(false)
}

impl Image {
    fn black(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            texels: vec![0; usize::from(width) * usize::from(height)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Calls the function of `name` through its address, as a core does.
    fn lookup(name: &CStr) -> usize {
        let address = gl_lookup(name);
        assert_ne!(address, 0, "{name:?}");
        address
    }

    fn read(name: u32, w: u16, h: u16, origin: FrameOrigin) -> Vec<u32> {
        let mut out = vec![0_u8; usize::from(w) * usize::from(h) * 4];
        assert!(read_picture(name, &mut out, w, h, origin));
        out.chunks_exact(4)
            .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            .collect()
    }

    #[test]
    fn the_lookup_knows_the_eight_functions() {
        for name in [
            c"glBindFramebuffer",
            c"glViewport",
            c"glEnable",
            c"glDisable",
            c"glScissor",
            c"glClearColor",
            c"glClear",
            c"glGetError",
        ] {
            lookup(name);
        }
        assert_eq!(gl_lookup(c"glDrawArrays"), 0);
        assert_eq!(gl_lookup(c""), 0);
    }

    #[test]
    fn a_scissored_clear_fills_its_box() {
        let fb = make_image(4, 3);
        set_current(true);
        bind_framebuffer(GL_FRAMEBUFFER, fb);
        clear_color(1.0, 0.0, 0.0, 1.0);
        clear(GL_COLOR_BUFFER_BIT);
        enable(GL_SCISSOR_TEST);
        scissor(1, 0, 2, 1);
        clear_color(0.0, 0.5, 2.0, 1.0);
        clear(GL_COLOR_BUFFER_BIT);
        scissor(-5, 2, 100, 100);
        clear_color(0.0, 0.0, 0.0, 1.0);
        clear(GL_COLOR_BUFFER_BIT);
        assert_eq!(get_error(), GL_NO_ERROR);
        let red = 0xFF_00_00;
        let blue = 0x00_80_FF;
        // Row 0 is GL y = 0: the first picture line with a top-left origin.
        assert_eq!(
            read(fb, 4, 3, FrameOrigin::TopLeft),
            [red, blue, blue, red, red, red, red, red, 0, 0, 0, 0]
        );
        assert_eq!(
            read(fb, 4, 3, FrameOrigin::BottomLeft),
            [0, 0, 0, 0, red, red, red, red, red, blue, blue, red]
        );
        assert_eq!(read(fb, 2, 1, FrameOrigin::TopLeft), [red, blue]);
        assert_eq!(take_errors_raised(), 0);
    }

    #[test]
    fn framebuffer_zero_and_no_context_change_nothing() {
        let fb = make_image(2, 1);
        clear(GL_COLOR_BUFFER_BIT);
        assert_eq!(gl_calls_without_context(), 1);
        set_current(true);
        bind_framebuffer(GL_FRAMEBUFFER, 0);
        clear_color(1.0, 1.0, 1.0, 1.0);
        clear(GL_COLOR_BUFFER_BIT);
        assert_eq!(read(fb, 2, 1, FrameOrigin::TopLeft), [0, 0]);
        set_current(false);
        assert_eq!(get_error(), GL_NO_ERROR, "no context: no error either");
        assert_eq!(gl_calls_without_context(), 2);
    }

    #[test]
    fn bad_arguments_raise_the_first_error() {
        let fb = make_image(1, 1);
        set_current(true);
        bind_framebuffer(0x1234, fb);
        bind_framebuffer(GL_FRAMEBUFFER, fb + 99);
        viewport(0, 0, -1, 1);
        scissor(0, 0, 1, -1);
        enable(0xFFFF);
        clear(0x1);
        assert_eq!(get_error(), GL_INVALID_ENUM, "the first one stays");
        assert_eq!(get_error(), GL_NO_ERROR);
        assert_eq!(take_errors_raised(), 6);
        bind_framebuffer(GL_FRAMEBUFFER, fb + 99);
        assert_eq!(get_error(), GL_INVALID_OPERATION);
        viewport(1, 2, 3, 4);
        enable(GL_BLEND);
        disable(GL_BLEND);
        assert_eq!(get_error(), GL_NO_ERROR);
    }

    #[test]
    fn a_resize_and_a_drop_replace_the_image() {
        let fb = make_image(1, 1);
        set_current(true);
        bind_framebuffer(GL_FRAMEBUFFER, fb);
        clear_color(1.0, 1.0, 1.0, 1.0);
        clear(GL_COLOR_BUFFER_BIT);
        resize_image(fb, 2, 2);
        assert_eq!(read(fb, 2, 2, FrameOrigin::TopLeft), [0; 4]);
        let mut out = [0_u8; 3];
        assert!(
            !read_picture(fb, &mut out, 1, 1, FrameOrigin::TopLeft),
            "short"
        );
        assert!(
            !read_picture(fb, &mut [0; 64], 3, 1, FrameOrigin::TopLeft),
            "wide"
        );
        drop_image(fb);
        assert!(!read_picture(fb, &mut [0; 4], 1, 1, FrameOrigin::TopLeft));
        clear(GL_COLOR_BUFFER_BIT);
        assert_eq!(get_error(), GL_NO_ERROR, "the drop unbinds it");
    }

    #[test]
    fn colour_channels_round_and_clamp() {
        assert_eq!(unorm(0.5), 128);
        assert_eq!(unorm(-1.0), 0);
        assert_eq!(unorm(7.0), 255);
        assert_eq!(unorm(f32::NAN), 0);
        assert_eq!(unorm(1.0 / 255.0), 1);
    }
}
