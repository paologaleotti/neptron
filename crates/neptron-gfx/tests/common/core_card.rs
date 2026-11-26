//! A stand-in for a GL core in the tier T3 checks: it gets the eight GL
//! functions of the test card of `neptron-testcore` through the lookup that
//! a display gives a core, and draws the card with them into the
//! framebuffer of the display, as the GL test core does. `gles_compare`
//! and `kms_check` share it.
//!
//! This is the only `unsafe` code of the checks: a call through an address
//! that the lookup gives is a call into C, as it is for a real core.
//!
//! The including program also includes `neptron-testcore/src/gl_card.rs`
//! as the module `gl_card`, with its `Size` in a module `script`, so the
//! card is the one of the test core and the checks need no new dependency.

use std::ffi::CStr;

use crate::gl_card::{
    GL_BLEND, GL_COLOR_BUFFER_BIT, GL_FRAMEBUFFER, GL_FUNCTIONS, GL_SCISSOR_TEST, gl_card,
    gl_card_pixel,
};
use crate::script::Size;

/// The GL functions of the card.
pub struct CoreCard {
    bind_framebuffer: unsafe extern "C" fn(u32, u32),
    viewport: unsafe extern "C" fn(i32, i32, i32, i32),
    enable: unsafe extern "C" fn(u32),
    disable: unsafe extern "C" fn(u32),
    scissor: unsafe extern "C" fn(i32, i32, i32, i32),
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    clear: unsafe extern "C" fn(u32),
    get_error: unsafe extern "C" fn() -> u32,
}

/// How a draw of the card leaves the context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    /// Clean: the scissor test off.
    Clean,
    /// Dirty, as the `hw_dirty` action of the test core: a scissor, a
    /// blend, a viewport, a clear colour, and framebuffer 0 bound.
    Dirty,
}

impl CoreCard {
    /// The functions through `lookup`, or the name of the first one that
    /// it does not give.
    pub fn resolve(lookup: fn(&CStr) -> usize) -> Result<Self, String> {
        let mut found = [0_usize; GL_FUNCTIONS.len()];
        for (slot, name) in found.iter_mut().zip(GL_FUNCTIONS) {
            *slot = lookup(name);
            if *slot == 0 {
                return Err(format!("the lookup gives no {name:?}"));
            }
        }
        let [
            bind,
            viewport,
            enable,
            disable,
            scissor,
            clear_color,
            clear,
            get_error,
        ] = found;
        // SAFETY: each address is not 0 and came from the lookup of a
        // display for the name of GL_FUNCTIONS in that order; the contract
        // of the lookup makes it a C function with the GL signature of
        // that name, which is the type it gets here.
        unsafe {
            Ok(Self {
                bind_framebuffer: std::mem::transmute::<usize, unsafe extern "C" fn(u32, u32)>(
                    bind,
                ),
                viewport: std::mem::transmute::<usize, unsafe extern "C" fn(i32, i32, i32, i32)>(
                    viewport,
                ),
                enable: std::mem::transmute::<usize, unsafe extern "C" fn(u32)>(enable),
                disable: std::mem::transmute::<usize, unsafe extern "C" fn(u32)>(disable),
                scissor: std::mem::transmute::<usize, unsafe extern "C" fn(i32, i32, i32, i32)>(
                    scissor,
                ),
                clear_color: std::mem::transmute::<usize, unsafe extern "C" fn(f32, f32, f32, f32)>(
                    clear_color,
                ),
                clear: std::mem::transmute::<usize, unsafe extern "C" fn(u32)>(clear),
                get_error: std::mem::transmute::<usize, unsafe extern "C" fn() -> u32>(get_error),
            })
        }
    }

    /// Draws the upright card of `size` into `framebuffer` with GL line 0
    /// at the top, or at the bottom with `bottom_left`, and returns the GL
    /// error, 0 for none. The caller made the core context current.
    pub fn draw(&self, framebuffer: u32, size: Size, bottom_left: bool, leave: Leave) -> u32 {
        let side = |v: u32| i32::try_from(v).unwrap_or(i32::MAX);
        // SAFETY: the functions of `resolve`, called with plain values,
        // while the caller holds the core context current between
        // `core_begin` and `core_end`.
        unsafe {
            (self.bind_framebuffer)(GL_FRAMEBUFFER, framebuffer);
            (self.viewport)(0, 0, side(size.width), side(size.height));
            (self.enable)(GL_SCISSOR_TEST);
            for rect in gl_card(size, 0, bottom_left) {
                let channel = |shift: u32| ((rect.rgb >> shift) & 0xFF) as f32 / 255.0;
                (self.scissor)(
                    side(rect.x),
                    side(rect.y),
                    side(rect.width),
                    side(rect.height),
                );
                (self.clear_color)(channel(16), channel(8), channel(0), 1.0);
                (self.clear)(GL_COLOR_BUFFER_BIT);
            }
            match leave {
                Leave::Clean => (self.disable)(GL_SCISSOR_TEST),
                Leave::Dirty => {
                    (self.scissor)(1, 2, 3, 4);
                    (self.enable)(GL_BLEND);
                    (self.viewport)(5, 6, 7, 8);
                    (self.clear_color)(1.0, 0.0, 1.0, 0.5);
                    (self.bind_framebuffer)(GL_FRAMEBUFFER, 0);
                }
            }
            (self.get_error)()
        }
    }

    /// Binds framebuffer 0 and clears it to red, as the `hw_fbo0` action
    /// of the test core does, and returns the GL error. The core context
    /// has no surface, so the clear reaches no picture.
    pub fn draw_into_zero(&self) -> u32 {
        // SAFETY: as in `draw`.
        unsafe {
            (self.bind_framebuffer)(GL_FRAMEBUFFER, 0);
            (self.disable)(GL_SCISSOR_TEST);
            (self.clear_color)(1.0, 0.0, 0.0, 1.0);
            (self.clear)(GL_COLOR_BUFFER_BIT);
            (self.get_error)()
        }
    }
}

/// The colour, `0x00RRGGBB`, of pixel `(x, y)` of the upright card of
/// `size`: what a display must show for the core.
pub fn card_pixel(size: Size, x: u32, y: u32) -> u32 {
    gl_card_pixel(size, 0, x, y)
}
