use serde::{Deserialize, Serialize};

/// The layout of one pixel in memory. Each format is one native-endian
/// integer per pixel, as libretro defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// 32 bits: 8 unused, then 8 each of red, green and blue.
    Xrgb8888,
    /// 16 bits: 5 red, 6 green, 5 blue.
    Rgb565,
    /// 16 bits: 1 unused, then 5 each of red, green and blue.
    Xrgb1555,
}

impl PixelFormat {
    /// Bytes per pixel: 4 or 2.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Xrgb8888 => 4,
            PixelFormat::Rgb565 | PixelFormat::Xrgb1555 => 2,
        }
    }
}

/// A borrowed image: a core frame or the UI canvas.
///
/// Row `y` starts at byte `y * pitch` of `data`. This type does not check
/// that `data` holds `height` rows; a consumer checks it before it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frame<'a> {
    /// The pixel bytes.
    pub data: &'a [u8],
    /// Width, in pixels.
    pub width: u16,
    /// Height, in lines.
    pub height: u16,
    /// Distance from the start of one row to the start of the next, in
    /// bytes.
    pub pitch: usize,
    /// The layout of each pixel.
    pub format: PixelFormat,
}

/// A rectangle, in pixels. `x` and `y` are the top-left corner and can be
/// negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

/// How the game frame is scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    /// Nearest neighbour.
    Nearest,
    /// Sharp bilinear: sharp pixels, with a bilinear blend only where a
    /// non-integer scale factor puts a pixel edge inside a target pixel.
    SharpBilinear,
}

/// Everything one flip shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scene<'a> {
    /// The game frame, or `None` for no game picture.
    pub game: Option<Frame<'a>>,
    /// The part of the game frame to show, in game frame pixels.
    pub crop: Rect,
    /// Where the cropped game frame goes, in pixels of the display mode.
    pub dest: Rect,
    /// How the game frame is scaled from `crop` to `dest`.
    pub filter: Filter,
    /// The 320x240 UI canvas, drawn over the game, or `None` for no UI.
    pub ui: Option<Frame<'a>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_per_pixel() {
        assert_eq!(PixelFormat::Xrgb8888.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Rgb565.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Xrgb1555.bytes_per_pixel(), 2);
    }
}
