//! The scenes of the goldens. `tests/goldens.rs` compares the reference
//! renderer with its golden images; `examples/gles_compare.rs` compares
//! the GLES renderer with the reference on the same scenes.

use neptron_types::{
    Filter, Frame, PixelFormat, Rect, ScanlineKind, Scanlines, Scene, ui_dest_fill, ui_dest_uniform,
};

/// Extra bytes at the end of each source row, so that the scenes also
/// check the pitch.
pub const ROW_PADDING_BYTES: usize = 6;

/// A source image that owns its bytes.
pub struct Source {
    /// The pixel bytes.
    pub data: Vec<u8>,
    /// Width, in pixels.
    pub width: u16,
    /// Height, in lines.
    pub height: u16,
    /// Bytes from one row to the next.
    pub pitch: usize,
    /// The layout of each pixel.
    pub format: PixelFormat,
}

impl Source {
    /// An image with the native-endian pixel value `pixel(x, y)`, cut to
    /// the size of `format`, and rows padded by [`ROW_PADDING_BYTES`].
    pub fn from_fn(
        width: u16,
        height: u16,
        format: PixelFormat,
        pixel: impl Fn(u16, u16) -> u32,
    ) -> Self {
        let bpp = format.bytes_per_pixel();
        let pitch = usize::from(width) * bpp + ROW_PADDING_BYTES;
        let mut data = vec![0x5A_u8; pitch * usize::from(height)];
        for y in 0..height {
            for x in 0..width {
                let start = usize::from(y) * pitch + usize::from(x) * bpp;
                let v = pixel(x, y);
                let px = &mut data[start..start + bpp];
                match format {
                    PixelFormat::Xrgb8888 => px.copy_from_slice(&v.to_ne_bytes()),
                    PixelFormat::Rgb565 | PixelFormat::Xrgb1555 => {
                        px.copy_from_slice(&(v as u16).to_ne_bytes());
                    }
                }
            }
        }
        Self {
            data,
            width,
            height,
            pitch,
            format,
        }
    }

    /// A test pattern: red grows to the right, blue grows down, and green
    /// alternates as a checkerboard. Every X bit that a format has is set
    /// on some pixels, so the scenes show that it is ignored.
    pub fn pattern(width: u16, height: u16, format: PixelFormat) -> Self {
        Self::from_fn(width, height, format, |x, y| {
            let r = ramp(x, width);
            let g = if (x + y) % 2 == 0 { 0xFF } else { 0x40 };
            let b = ramp(y, height);
            let x_bits = (x + 2 * y) % 3 == 0;
            match format {
                PixelFormat::Xrgb8888 => {
                    let x_byte = if x_bits { 0xA5 } else { 0 };
                    u32::from_be_bytes([x_byte, r, g, b])
                }
                PixelFormat::Rgb565 => {
                    (u32::from(r >> 3) << 11) | (u32::from(g >> 2) << 5) | u32::from(b >> 3)
                }
                PixelFormat::Xrgb1555 => {
                    let x_bit = if x_bits { 0x8000 } else { 0 };
                    x_bit | (u32::from(r >> 3) << 10) | (u32::from(g >> 3) << 5) | u32::from(b >> 3)
                }
            }
        })
    }

    /// The image as a frame of a scene.
    pub fn frame(&self) -> Frame<'_> {
        Frame {
            data: &self.data,
            width: self.width,
            height: self.height,
            pitch: self.pitch,
            format: self.format,
        }
    }
}

fn ramp(i: u16, len: u16) -> u8 {
    (u32::from(i) * 255 / u32::from(len.max(2) - 1)) as u8
}

/// A rectangle.
pub fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

/// A named scene with its images and its output size.
pub struct Case {
    /// The name, also the golden file name.
    pub name: &'static str,
    /// The game frame.
    pub game: Option<Source>,
    /// The crop of the game frame.
    pub crop: Rect,
    /// The destination of the game frame.
    pub dest: Rect,
    /// The game filter.
    pub filter: Filter,
    /// The scanlines over the game.
    pub scanlines: Scanlines,
    /// The UI canvas.
    pub ui: Option<Source>,
    /// The destination of the UI canvas.
    pub ui_dest: Rect,
    /// Output width.
    pub width: u32,
    /// Output height.
    pub height: u32,
}

impl Case {
    /// A scene with a game frame and no UI.
    pub fn game(
        name: &'static str,
        game: Source,
        crop: Rect,
        dest: Rect,
        filter: Filter,
        (width, height): (u32, u32),
    ) -> Self {
        Self {
            name,
            game: Some(game),
            crop,
            dest,
            filter,
            scanlines: Scanlines::NONE,
            ui: None,
            ui_dest: rect(0, 0, 0, 0),
            width,
            height,
        }
    }

    /// This case with `ui` drawn into `ui_dest`.
    pub fn with_ui(self, ui: Source, ui_dest: Rect) -> Self {
        Self {
            ui: Some(ui),
            ui_dest,
            ..self
        }
    }

    /// This case with black scanlines at `strength_percent`.
    pub fn with_black_lines(self, strength_percent: u8) -> Self {
        Self {
            scanlines: Scanlines {
                kind: ScanlineKind::Black,
                strength_percent,
            },
            ..self
        }
    }

    /// The scene, borrowing the images of the case.
    pub fn scene(&self) -> Scene<'_> {
        Scene {
            game: self.game.as_ref().map(Source::frame),
            crop: self.crop,
            dest: self.dest,
            filter: self.filter,
            scanlines: self.scanlines,
            ui: self.ui.as_ref().map(Source::frame),
            ui_dest: self.ui_dest,
        }
    }
}

/// The scenes that have a golden image.
pub fn golden_cases() -> Vec<Case> {
    use Filter::{Nearest, SharpBilinear};
    use PixelFormat::{Rgb565, Xrgb1555, Xrgb8888};
    let pattern = Source::pattern;
    vec![
        Case::game(
            "nearest_integer_scale",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(4, 3, 24, 18),
            Nearest,
            (32, 24),
        ),
        Case::game(
            "nearest_fractional_scale",
            pattern(10, 7, Xrgb8888),
            rect(0, 0, 10, 7),
            rect(1, 2, 29, 19),
            Nearest,
            (32, 24),
        ),
        Case::game(
            "sharp_bilinear_fractional_scale",
            pattern(10, 7, Xrgb8888),
            rect(0, 0, 10, 7),
            rect(1, 2, 29, 19),
            SharpBilinear,
            (32, 24),
        ),
        Case::game(
            "sharp_bilinear_downscale",
            pattern(16, 12, Xrgb8888),
            rect(0, 0, 16, 12),
            rect(3, 2, 11, 9),
            SharpBilinear,
            (16, 12),
        ),
        Case::game(
            "crop_outside_the_frame",
            pattern(8, 8, Rgb565),
            rect(-2, -1, 12, 10),
            rect(-5, 1, 40, 23),
            SharpBilinear,
            (32, 24),
        ),
        Case::game(
            "rgb565_crop",
            pattern(8, 6, Rgb565),
            rect(1, 1, 6, 4),
            rect(2, 2, 12, 8),
            Nearest,
            (16, 12),
        ),
        Case::game(
            "xrgb1555_crop",
            pattern(8, 6, Xrgb1555),
            rect(1, 1, 6, 4),
            rect(2, 2, 12, 8),
            Nearest,
            (16, 12),
        ),
        Case::game(
            "ui_over_the_game",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(0, 0, 40, 30),
            Nearest,
            (40, 30),
        )
        .with_ui(pattern(16, 12, Xrgb1555), ui_dest_uniform(40, 30, 16, 12)),
        // A CRT mode six times wider than the canvas, as 1920x240 is for
        // 320x240.
        Case::game(
            "ui_fills_a_super_width_mode",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(0, 0, 96, 12),
            Nearest,
            (96, 12),
        )
        .with_ui(pattern(16, 12, Xrgb1555), ui_dest_fill(96, 12)),
        // A canvas stretched by 6.25 and 1.5, partly outside the output.
        Case::game(
            "ui_fractional_scale_clipped",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(0, 0, 96, 24),
            Nearest,
            (96, 24),
        )
        .with_ui(pattern(16, 12, Xrgb8888), rect(-10, 4, 100, 18)),
        // Line doubling with black scanlines. Borders at the left and the
        // right show that only the game lines darken.
        Case::game(
            "scanlines_2x_strength_100",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(4, 0, 16, 12),
            Nearest,
            (24, 12),
        )
        .with_black_lines(100),
        Case::game(
            "scanlines_2x_strength_50",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(4, 0, 16, 12),
            Nearest,
            (24, 12),
        )
        .with_black_lines(50),
        Case::game(
            "scanlines_2x_strength_0",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(4, 0, 16, 12),
            Nearest,
            (24, 12),
        )
        .with_black_lines(0),
        Case::game(
            "scanlines_3x_rgb565",
            pattern(8, 6, Rgb565),
            rect(0, 0, 8, 6),
            rect(0, 0, 24, 18),
            Nearest,
            (24, 18),
        )
        .with_black_lines(100),
        Case::game(
            "scanlines_4x_xrgb1555",
            pattern(6, 4, Xrgb1555),
            rect(0, 0, 6, 4),
            rect(0, 0, 24, 16),
            Nearest,
            (24, 16),
        )
        .with_black_lines(75),
        // 6 lines to 15: a scale of 2.5 shows no scanlines.
        Case::game(
            "scanlines_fractional_scale",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(0, 0, 16, 15),
            Nearest,
            (16, 15),
        )
        .with_black_lines(100),
        Case::game(
            "scanlines_1x_scale",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(4, 3, 8, 6),
            Nearest,
            (16, 12),
        )
        .with_black_lines(100),
        // The destination starts on line 3, so the dark lines are the even
        // output lines 4 to 12.
        Case::game(
            "scanlines_crop_and_odd_offset",
            pattern(8, 8, Rgb565),
            rect(1, 1, 6, 5),
            rect(2, 3, 12, 10),
            Nearest,
            (16, 15),
        )
        .with_black_lines(100),
        Case::game(
            "scanlines_under_the_ui",
            pattern(16, 12, Xrgb8888),
            rect(0, 0, 16, 12),
            rect(0, 0, 32, 24),
            Nearest,
            (32, 24),
        )
        .with_black_lines(100)
        .with_ui(pattern(16, 12, Xrgb1555), rect(8, 6, 16, 12)),
        // A width of 29 for 10 pixels blends columns; the height is 2x.
        Case::game(
            "scanlines_sharp_bilinear",
            pattern(10, 6, Xrgb8888),
            rect(0, 0, 10, 6),
            rect(1, 0, 29, 12),
            SharpBilinear,
            (32, 12),
        )
        .with_black_lines(60),
    ]
}
