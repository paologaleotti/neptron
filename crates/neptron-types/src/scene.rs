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

/// The UI destination for a CRT: the full active area of a
/// `mode_width` by `mode_height` mode.
///
/// On a super-width mode such as 1920x240, the canvas then stretches on
/// the horizontal axis only, as the tube shows it.
pub fn ui_dest_fill(mode_width: u16, mode_height: u16) -> Rect {
    Rect {
        x: 0,
        y: 0,
        w: u32::from(mode_width),
        h: u32::from(mode_height),
    }
}

/// The UI destination for a flat panel: the canvas at the largest whole
/// scale `k` at which it fits the mode on both axes, at least 1, centred.
///
/// The top-left corner is at `floor((mode_width - k * canvas_width) / 2)`
/// and `floor((mode_height - k * canvas_height) / 2)`. A canvas larger than
/// the mode gets `k = 1` and a negative corner, so the output clips the
/// same amount on both sides, one pixel more on the left or the top for an
/// odd margin. A canvas with a zero side gives a rectangle with a zero
/// side, which shows no UI.
pub fn ui_dest_uniform(
    mode_width: u16,
    mode_height: u16,
    canvas_width: u16,
    canvas_height: u16,
) -> Rect {
    let fit = |mode: u16, canvas: u16| u32::from(mode) / u32::from(canvas.max(1));
    let scale = fit(mode_width, canvas_width)
        .min(fit(mode_height, canvas_height))
        .max(1);
    // With 16-bit inputs, a side is at most the larger of the mode side and
    // the canvas side, and an offset is inside the i32 range.
    let side = |canvas: u16| u32::from(canvas) * scale;
    let offset = |mode: u16, canvas: u16| (i64::from(mode) - i64::from(side(canvas))).div_euclid(2);
    Rect {
        x: offset(mode_width, canvas_width) as i32,
        y: offset(mode_height, canvas_height) as i32,
        w: side(canvas_width),
        h: side(canvas_height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn bytes_per_pixel() {
        assert_eq!(PixelFormat::Xrgb8888.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Rgb565.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Xrgb1555.bytes_per_pixel(), 2);
    }

    #[test]
    fn fill_covers_the_active_area() {
        assert_eq!(ui_dest_fill(1920, 240), rect(0, 0, 1920, 240));
        assert_eq!(ui_dest_fill(320, 240), rect(0, 0, 320, 240));
        assert_eq!(ui_dest_fill(0, 0), rect(0, 0, 0, 0));
        assert_eq!(ui_dest_fill(u16::MAX, u16::MAX), rect(0, 0, 65_535, 65_535));
    }

    #[test]
    fn uniform_on_the_canvas_size_is_one_to_one() {
        assert_eq!(ui_dest_uniform(320, 240, 320, 240), rect(0, 0, 320, 240));
    }

    #[test]
    fn uniform_takes_the_largest_whole_scale_and_centres() {
        assert_eq!(ui_dest_uniform(640, 480, 320, 240), rect(0, 0, 640, 480));
        assert_eq!(
            ui_dest_uniform(1920, 1080, 320, 240),
            rect(320, 60, 1280, 960)
        );
        assert_eq!(ui_dest_uniform(720, 480, 320, 240), rect(40, 0, 640, 480));
    }

    #[test]
    fn uniform_on_a_wide_mode_keeps_the_scale_of_its_height() {
        assert_eq!(ui_dest_uniform(1920, 240, 320, 240), rect(800, 0, 320, 240));
    }

    #[test]
    fn uniform_crops_a_large_canvas_around_the_centre() {
        assert_eq!(ui_dest_uniform(256, 224, 320, 240), rect(-32, -8, 320, 240));
        assert_eq!(ui_dest_uniform(255, 240, 320, 240), rect(-33, 0, 320, 240));
    }

    #[test]
    fn uniform_rounds_an_odd_margin_toward_the_top_left() {
        assert_eq!(ui_dest_uniform(5, 3, 2, 2), rect(1, 0, 2, 2));
    }

    #[test]
    fn uniform_handles_zero_sizes() {
        assert_eq!(ui_dest_uniform(320, 240, 0, 240), rect(160, 0, 0, 240));
        assert_eq!(ui_dest_uniform(320, 240, 0, 0), rect(160, 120, 0, 0));
        assert_eq!(ui_dest_uniform(0, 0, 320, 240), rect(-160, -120, 320, 240));
        assert_eq!(ui_dest_uniform(0, 0, 0, 0), rect(0, 0, 0, 0));
    }

    #[test]
    fn uniform_handles_the_extremes() {
        let max = u16::MAX;
        assert_eq!(ui_dest_uniform(max, max, 1, 1), rect(0, 0, 65_535, 65_535));
        assert_eq!(ui_dest_uniform(max, max, 2, 2), rect(0, 0, 65_534, 65_534));
        assert_eq!(
            ui_dest_uniform(1, 1, max, max),
            rect(-32_767, -32_767, 65_535, 65_535)
        );
        assert_eq!(
            ui_dest_uniform(max, 1, 1, max),
            rect(32_767, -32_767, 1, 65_535)
        );
    }
}
