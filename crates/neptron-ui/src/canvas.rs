//! The drawing target of a scene, and the CPU canvas that draws into an
//! XRGB8888 buffer.

use crate::font;
use crate::geom::{Color, Rect};

/// Width of the logical UI canvas in pixels.
pub const UI_WIDTH: u32 = 320;
/// Height of the logical UI canvas in pixels.
pub const UI_HEIGHT: u32 = 240;
/// Bytes per pixel of the XRGB8888 buffer.
pub const BYTES_PER_PIXEL: usize = 4;

/// A target that can draw the four scene primitives. Every method clips to
/// the canvas and to the current clip rectangle.
pub trait Canvas {
    /// Width in pixels.
    fn width(&self) -> u32;
    /// Height in pixels.
    fn height(&self) -> u32;
    /// Sets the clip rectangle. `None` clips to the canvas only.
    fn set_clip(&mut self, clip: Option<Rect>);
    /// Fills a rectangle with one colour.
    fn fill_rect(&mut self, rect: Rect, color: Color);
    /// Draws a sprite with its top-left corner at `x`, `y`.
    fn draw_sprite(&mut self, id: SpriteId, x: i32, y: i32);
    /// Draws `text` with the built-in font. `x`, `y` is the top-left corner
    /// of the first glyph box.
    fn draw_text(&mut self, x: i32, y: i32, color: Color, text: &str);
}

/// The number of a sprite, given by [`CpuCanvas::add_sprite`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpriteId(pub u16);

/// An image for sprite commands, in ARGB8888 words. A pixel with alpha 0 is
/// not drawn; every other pixel is drawn opaque. There is no blending, so
/// every renderer gives the same pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

/// An error of a sprite or of the sprite store.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CanvasError {
    /// The pixel count does not match the size.
    #[error("a {width}x{height} sprite needs {expected} pixels, got {actual}")]
    SpriteSize {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Width times height.
        expected: u64,
        /// The pixels given.
        actual: usize,
    },
    /// The canvas holds the most sprites that a [`SpriteId`] can name.
    #[error("the canvas already holds {limit} sprites")]
    TooManySprites {
        /// The limit.
        limit: usize,
    },
}

/// The colour that marks a sprite command whose sprite is missing.
pub const MISSING_SPRITE_COLOR: Color = Color::rgb(0xFF, 0x00, 0xFF);
/// Size in pixels of the square drawn for a missing sprite.
pub const MISSING_SPRITE_SIZE_PX: u32 = 8;

/// A canvas in memory: XRGB8888, little-endian, rows without padding. The
/// unused byte of each pixel is zero.
///
/// The buffer is made once in [`CpuCanvas::new`]. Drawing never allocates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuCanvas {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    clip: Rect,
    sprites: Vec<Sprite>,
}

impl Sprite {
    /// A sprite from its size and `width * height` ARGB8888 pixels, rows
    /// first.
    pub fn from_argb(width: u32, height: u32, pixels: Vec<u32>) -> Result<Self, CanvasError> {
        let expected = u64::from(width) * u64::from(height);
        if u64::try_from(pixels.len()).ok() != Some(expected) {
            return Err(CanvasError::SpriteSize {
                width,
                height,
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl CpuCanvas {
    /// A black canvas of [`UI_WIDTH`] by [`UI_HEIGHT`] pixels.
    pub fn new() -> Self {
        let len = UI_WIDTH as usize * UI_HEIGHT as usize * BYTES_PER_PIXEL;
        Self {
            width: UI_WIDTH,
            height: UI_HEIGHT,
            bytes: vec![0; len],
            clip: full(UI_WIDTH, UI_HEIGHT),
            sprites: Vec::new(),
        }
    }

    /// Stores a sprite and returns its number.
    pub fn add_sprite(&mut self, sprite: Sprite) -> Result<SpriteId, CanvasError> {
        let limit = usize::from(u16::MAX) + 1;
        let id =
            u16::try_from(self.sprites.len()).map_err(|_| CanvasError::TooManySprites { limit })?;
        self.sprites.push(sprite);
        Ok(SpriteId(id))
    }

    /// The pixel buffer: XRGB8888, little-endian, `width * 4` bytes per row.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Bytes per row.
    pub fn pitch(&self) -> usize {
        self.width as usize * BYTES_PER_PIXEL
    }

    /// The colour of one pixel, or `None` outside the canvas.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = self.index(x, y);
        let b = self.bytes.get(at..at + BYTES_PER_PIXEL)?;
        Some(Color::from_xrgb(u32::from_le_bytes([
            b[0], b[1], b[2], b[3],
        ])))
    }

    /// Fills the whole canvas with one colour. The clip does not apply.
    pub fn clear(&mut self, color: Color) {
        let word = color.xrgb().to_le_bytes();
        for px in self.bytes.chunks_exact_mut(BYTES_PER_PIXEL) {
            px.copy_from_slice(&word);
        }
    }

    fn index(&self, x: u32, y: u32) -> usize {
        (y as usize * self.width as usize + x as usize) * BYTES_PER_PIXEL
    }

    // The caller has checked that `x`, `y` is inside the clip, and the clip
    // is inside the canvas, so the range exists; `get_mut` keeps it panic
    // free all the same.
    fn put(&mut self, x: u32, y: u32, color: Color) {
        let at = self.index(x, y);
        if let Some(px) = self.bytes.get_mut(at..at + BYTES_PER_PIXEL) {
            px.copy_from_slice(&color.xrgb().to_le_bytes());
        }
    }

    fn in_clip(&self, x: i64, y: i64) -> bool {
        x >= i64::from(self.clip.x)
            && y >= i64::from(self.clip.y)
            && x < self.clip.right()
            && y < self.clip.bottom()
    }

    fn put_clipped(&mut self, x: i64, y: i64, color: Color) {
        if self.in_clip(x, y) {
            // `in_clip` proves 0 <= x < width and 0 <= y < height.
            self.put(x as u32, y as u32, color);
        }
    }

    fn draw_glyph(&mut self, x: i64, y: i64, color: Color, glyph: &font::Glyph) {
        for (row, bits) in (0_i64..).zip(glyph.iter()) {
            for col in 0..i64::from(font::GLYPH_WIDTH) {
                if bits & (0x80 >> col) != 0 {
                    self.put_clipped(x + col, y + row, color);
                }
            }
        }
    }

    fn blit(&mut self, sprite: &Sprite, x: i64, y: i64) {
        let width = sprite.width as usize;
        for (row, line) in (0_i64..).zip(sprite.pixels.chunks_exact(width.max(1))) {
            for (col, &argb) in (0_i64..).zip(line) {
                if argb >> 24 != 0 {
                    self.put_clipped(x + col, y + row, Color::from_xrgb(argb));
                }
            }
        }
    }

    fn draw_missing_sprite(&mut self, x: i32, y: i32) {
        let size = MISSING_SPRITE_SIZE_PX;
        self.fill_rect(Rect::new(x, y, size, size), MISSING_SPRITE_COLOR);
    }
}

impl Default for CpuCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas for CpuCanvas {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn set_clip(&mut self, clip: Option<Rect>) {
        let canvas = full(self.width, self.height);
        self.clip = match clip {
            Some(c) => c.intersect(&canvas).unwrap_or_default(),
            None => canvas,
        };
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let Some(area) = rect.intersect(&self.clip) else {
            return;
        };
        let word = color.xrgb().to_le_bytes();
        let pitch = self.pitch();
        // `area` is inside the clip, which is inside the canvas, so every
        // coordinate here is non-negative and in range.
        let x0 = area.x as usize * BYTES_PER_PIXEL;
        let x1 = x0 + area.w as usize * BYTES_PER_PIXEL;
        for y in area.y as usize..area.y as usize + area.h as usize {
            let start = y * pitch;
            if let Some(row) = self.bytes.get_mut(start + x0..start + x1) {
                for px in row.chunks_exact_mut(BYTES_PER_PIXEL) {
                    px.copy_from_slice(&word);
                }
            }
        }
    }

    fn draw_sprite(&mut self, id: SpriteId, x: i32, y: i32) {
        // The store moves out while the pixels are written, so the sprite and
        // the buffer are not borrowed at the same time. A move of a `Vec`
        // does not allocate.
        let sprites = core::mem::take(&mut self.sprites);
        match sprites.get(usize::from(id.0)) {
            Some(sprite) => self.blit(sprite, i64::from(x), i64::from(y)),
            None => self.draw_missing_sprite(x, y),
        }
        self.sprites = sprites;
    }

    fn draw_text(&mut self, x: i32, y: i32, color: Color, text: &str) {
        let advance = i64::from(font::ADVANCE);
        let mut pen = i64::from(x);
        let y = i64::from(y);
        for c in text.chars() {
            if c != ' ' {
                self.draw_glyph(pen, y, color, font::glyph(c));
            }
            pen += advance;
        }
    }
}

fn full(width: u32, height: u32) -> Rect {
    Rect::new(0, 0, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(0xFF, 0, 0);

    fn count(canvas: &CpuCanvas, color: Color) -> usize {
        (0..UI_HEIGHT)
            .flat_map(|y| (0..UI_WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.pixel(x, y) == Some(color))
            .count()
    }

    #[test]
    fn new_canvas_is_black_xrgb() {
        let canvas = CpuCanvas::new();
        assert_eq!(canvas.as_bytes().len(), 320 * 240 * 4);
        assert_eq!(canvas.pitch(), 1280);
        assert_eq!(count(&canvas, Color::BLACK), 320 * 240);
    }

    #[test]
    fn pixels_are_little_endian_with_a_zero_top_byte() {
        let mut canvas = CpuCanvas::new();
        canvas.fill_rect(Rect::new(0, 0, 1, 1), Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(&canvas.as_bytes()[..4], &[0x33, 0x22, 0x11, 0x00]);
    }

    #[test]
    fn fill_is_clipped_to_the_canvas() {
        let mut canvas = CpuCanvas::new();
        canvas.fill_rect(Rect::new(-5, -5, 10, 10), RED);
        canvas.fill_rect(Rect::new(318, 238, 50, 50), RED);
        assert_eq!(count(&canvas, RED), 25 + 4);
        assert_eq!(canvas.pixel(4, 4), Some(RED));
        assert_eq!(canvas.pixel(5, 5), Some(Color::BLACK));
        assert_eq!(canvas.pixel(320, 0), None);
    }

    #[test]
    fn fill_is_clipped_to_the_clip_rectangle() {
        let mut canvas = CpuCanvas::new();
        canvas.set_clip(Some(Rect::new(10, 10, 3, 2)));
        canvas.fill_rect(Rect::new(0, 0, 320, 240), RED);
        assert_eq!(count(&canvas, RED), 6);
        canvas.set_clip(Some(Rect::new(400, 400, 3, 2)));
        canvas.fill_rect(Rect::new(0, 0, 320, 240), Color::WHITE);
        assert_eq!(count(&canvas, Color::WHITE), 0);
        canvas.set_clip(None);
        canvas.fill_rect(Rect::new(0, 0, 320, 240), Color::WHITE);
        assert_eq!(count(&canvas, Color::WHITE), 320 * 240);
    }

    #[test]
    fn text_draws_glyph_ink_only() {
        let mut canvas = CpuCanvas::new();
        canvas.draw_text(0, 0, RED, "I");
        let ink: u32 = font::glyph('I').iter().map(|r| r.count_ones()).sum();
        assert_eq!(count(&canvas, RED), ink as usize);
        for (y, bits) in (0..).zip(font::glyph('I')) {
            for x in 0..8 {
                let expected = if bits & (0x80 >> x) != 0 {
                    RED
                } else {
                    Color::BLACK
                };
                assert_eq!(canvas.pixel(x, y), Some(expected), "({x}, {y})");
            }
        }
    }

    #[test]
    fn text_advances_six_pixels_and_clips() {
        let mut canvas = CpuCanvas::new();
        canvas.draw_text(-6, 0, RED, "XI");
        let mut reference = CpuCanvas::new();
        reference.draw_text(0, 0, RED, "I");
        assert_eq!(canvas, reference);
    }

    #[test]
    fn sprites_skip_transparent_pixels() {
        let mut canvas = CpuCanvas::new();
        let sprite = Sprite::from_argb(2, 2, vec![0xFF00_FF00, 0, 0x0100_00FF, 0x00FF_FFFF])
            .expect("valid sprite");
        let id = canvas.add_sprite(sprite).expect("room for a sprite");
        canvas.draw_sprite(id, 1, 1);
        assert_eq!(canvas.pixel(1, 1), Some(Color::rgb(0, 0xFF, 0)));
        assert_eq!(canvas.pixel(2, 1), Some(Color::BLACK));
        assert_eq!(canvas.pixel(1, 2), Some(Color::rgb(0, 0, 0xFF)));
        assert_eq!(canvas.pixel(2, 2), Some(Color::BLACK));
    }

    #[test]
    fn a_missing_sprite_draws_a_marker() {
        let mut canvas = CpuCanvas::new();
        canvas.draw_sprite(SpriteId(3), 0, 0);
        assert_eq!(count(&canvas, MISSING_SPRITE_COLOR), 64);
    }

    #[test]
    fn sprite_size_must_match_the_pixels() {
        assert_eq!(
            Sprite::from_argb(3, 2, vec![0; 5]),
            Err(CanvasError::SpriteSize {
                width: 3,
                height: 2,
                expected: 6,
                actual: 5
            })
        );
        assert!(Sprite::from_argb(0, 0, Vec::new()).is_ok());
    }

    #[test]
    fn the_sprite_store_is_limited_by_the_id_type() {
        let mut canvas = CpuCanvas::new();
        let tiny = Sprite::from_argb(0, 0, Vec::new()).expect("empty sprite");
        for _ in 0..=u16::MAX {
            canvas.add_sprite(tiny.clone()).expect("room for a sprite");
        }
        assert_eq!(
            canvas.add_sprite(tiny),
            Err(CanvasError::TooManySprites { limit: 65536 })
        );
    }

    #[test]
    fn clear_ignores_the_clip() {
        let mut canvas = CpuCanvas::new();
        canvas.set_clip(Some(Rect::new(0, 0, 1, 1)));
        canvas.clear(RED);
        assert_eq!(count(&canvas, RED), 320 * 240);
    }
}
