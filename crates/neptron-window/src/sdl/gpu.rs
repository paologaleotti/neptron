use neptron_backend::DisplayError;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::{ScaleMode, Texture, WindowCanvas, create_renderer};
use sdl3::video::Window;

use super::display::device;

/// A renderer and its one streaming texture.
///
/// With the `unsafe_textures` feature of `sdl3`, a `Texture` is a raw
/// pointer that its renderer frees on drop. This type owns both and never
/// hands the texture out, so no texture is used after its renderer is gone.
pub(super) struct Gpu {
    canvas: WindowCanvas,
    texture: Texture,
}

/// Why [`Gpu::make`] failed, with the window when SDL gave it back.
pub(super) struct MakeError {
    pub(super) window: Option<Window>,
    pub(super) error: DisplayError,
}

impl Gpu {
    /// Makes a renderer on `window` with vsync, and a `width` by `height`
    /// streaming texture. A failed renderer closes the window, because
    /// SDL takes it by value; a failed texture gives it back.
    pub(super) fn make(
        window: Window,
        width: u32,
        height: u32,
        scale: ScaleMode,
        refuse: impl FnOnce(String) -> DisplayError,
    ) -> Result<Gpu, MakeError> {
        let mut canvas = create_renderer(window, None).map_err(|e| MakeError {
            window: None,
            error: device("renderer", e),
        })?;
        // The renderer writes XRGB8888 as little-endian bytes B, G, R, X.
        // BGRX32 names that byte order on every host.
        let mut texture = match canvas.create_texture_streaming(PixelFormat::BGRX32, width, height)
        {
            Ok(texture) => texture,
            Err(e) => {
                return Err(MakeError {
                    window: Some(canvas.into_window()),
                    error: refuse(format!("no texture for the active area: {e}")),
                });
            }
        };
        texture.set_scale_mode(scale);
        canvas.set_draw_color(Color::BLACK);
        Ok(Gpu { canvas, texture })
    }

    /// Copies `pixels`, rows `pitch` bytes apart, into the texture.
    pub(super) fn upload(&mut self, pixels: &[u8], pitch: usize) -> Result<(), DisplayError> {
        self.texture
            .update(None, pixels, pitch)
            .map_err(|e| device("texture update", e))
    }

    /// Draws the texture over the whole window and presents it with vsync.
    pub(super) fn show(&mut self) -> Result<(), DisplayError> {
        self.canvas.clear();
        self.canvas
            .copy(&self.texture, None, None)
            .map_err(|e| device("texture copy", e))?;
        if self.canvas.present() {
            Ok(())
        } else {
            Err(device("present", sdl3::get_error()))
        }
    }

    pub(super) fn window_mut(&mut self) -> &mut Window {
        self.canvas.window_mut()
    }

    /// Drops the renderer, which frees the texture, and gives the window
    /// back.
    pub(super) fn into_window(self) -> Window {
        self.canvas.into_window()
    }
}
