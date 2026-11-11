//! The CPU reference renderer. Its output is the definition of the correct
//! pixels for every other renderer.

mod axis;
mod error;
mod pixel;

pub use error::{Image, MAX_RECT_SIDE_PX, RectKind, RenderError};

use neptron_types::{Frame, Scene};

use axis::{AxisMap, Tap};
use error::{check_frame, check_output, check_rect};
use pixel::Rgb;

/// The buffer that [`ReferenceRenderer::render`] writes, owned by the
/// caller.
#[derive(Debug)]
pub struct OutputBuffer<'a> {
    /// The pixel bytes, XRGB8888 little-endian.
    pub data: &'a mut [u8],
    /// Width, in pixels.
    pub width: u32,
    /// Height, in lines.
    pub height: u32,
    /// Distance from the start of one row to the start of the next, in
    /// bytes. At least `width * 4`.
    pub pitch: usize,
}

/// The CPU renderer that defines the correct pixels of a [`Scene`].
///
/// # Output
///
/// The output is XRGB8888 as DRM defines it: one little-endian `u32` per
/// pixel, `0xXXRRGGBB`. The renderer writes every pixel of the `width` by
/// `height` area, with the X byte set to `0xFF`. It does not write the bytes
/// between the end of a row and the next pitch. A pixel that shows nothing
/// is black.
///
/// # Source pixels
///
/// A source pixel is one native-endian integer, as libretro defines it.
/// The renderer expands each channel to 8 bits as a GPU does for a
/// normalised channel: a 5-bit value `v` becomes `round(v * 255 / 31)` and
/// a 6-bit value becomes `round(v * 255 / 63)`. This is not bit
/// replication, which differs by 1 for some values. The X bits of `Xrgb8888` and `Xrgb1555` are
/// ignored. Filtering uses the expanded 8-bit values.
///
/// # Scale and crop
///
/// The crop rectangle, in game frame pixels, scales to the destination
/// rectangle, in output pixels. Each axis scales on its own, by
/// `dest.w / crop.w` and `dest.h / crop.h`. A crop or destination
/// rectangle with a zero side shows no game picture. A part of the
/// destination outside the output is clipped; the scale does not change.
///
/// Pixel `i` of the destination, counted from `dest.x`, has its centre at
/// `i + 1/2`. That centre maps to the source coordinate
/// `u = crop.x + (i + 1/2) * crop.w / dest.w`. The renderer computes `u` as
/// an exact fraction. The same holds for lines, with `y`, `h` and `v`.
///
/// - When the source pixel `(floor(u), floor(v))` is outside the game
///   frame, the destination pixel is black. A crop can extend past the
///   frame, and that part shows black.
/// - Nearest: the destination pixel is the source pixel
///   `(floor(u), floor(v))`. A centre exactly on a source pixel edge takes
///   the pixel on the right, or the pixel below.
/// - Sharp bilinear: see below.
///
/// # Sharp bilinear
///
/// This is the filter of the libretro `sharp-bilinear` shader, with a
/// prescale per axis. For one axis, with `n = crop.w` and `m = dest.w`:
///
/// - `p = max(1, floor(m / n))`, the integer prescale.
/// - `f = u - floor(u)`, and `d = f - 1/2`, the distance from the centre of
///   the source pixel.
/// - `w = max(0, |d| * p - (p - 1) / 2)`. `w` is between 0 and 1/2.
/// - The near pixel is `floor(u)`. The far pixel is `floor(u) - 1` when
///   `d < 0` and `floor(u) + 1` otherwise, clamped to the part of the crop
///   that is inside the frame. The axis value is
///   `near * (1 - w) + far * w`.
///
/// The two axes combine as a bilinear blend of four source pixels, with the
/// weights `(1 - wx) * (1 - wy)`, `wx * (1 - wy)`, `(1 - wx) * wy` and
/// `wx * wy`. Each channel is the exact rational value of that sum, rounded
/// to the nearest integer, with a half rounded up.
///
/// At an integer scale `w` is always 0, so sharp bilinear equals nearest.
/// Below a scale of 1, `p` is 1 and the filter is a plain bilinear blend at
/// the pixel centre.
///
/// # Integers
///
/// The pixel path uses integer maths only, so the output is the same on
/// every host. With crop and destination sides up to [`MAX_RECT_SIDE_PX`],
/// no value overflows a `u64`.
///
/// # Allocation
///
/// It keeps two tables of sample positions, one entry per output column and
/// one per output line. The first call to [`render`](Self::render) grows
/// them, and so does a call with a larger output. Other calls allocate
/// nothing.
#[derive(Debug, Default)]
pub struct ReferenceRenderer {
    columns: Vec<Tap>,
    lines: Vec<Tap>,
}

impl ReferenceRenderer {
    /// A renderer with empty tables.
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws `scene` into `out`, with the rules of [`ReferenceRenderer`].
    ///
    /// # Errors
    ///
    /// [`RenderError`] when a frame or the output buffer is smaller than
    /// its size and pitch need, or when a rectangle side of a scene with a
    /// game frame is above [`MAX_RECT_SIDE_PX`]. The renderer checks
    /// everything before it writes, so `out` is unchanged after an error.
    pub fn render(
        &mut self,
        scene: &Scene<'_>,
        out: &mut OutputBuffer<'_>,
    ) -> Result<(), RenderError> {
        check_output(out)?;
        if let Some(game) = &scene.game {
            check_frame(game, Image::Game)?;
            check_rect(&scene.crop, RectKind::Crop)?;
            check_rect(&scene.dest, RectKind::Dest)?;
        }
        fill_black(out);
        if let Some(game) = &scene.game {
            self.draw_game(game, scene, out);
        }
        Ok(())
    }

    fn draw_game(&mut self, game: &Frame<'_>, scene: &Scene<'_>, out: &mut OutputBuffer<'_>) {
        let (crop, dest) = (scene.crop, scene.dest);
        if crop.w == 0 || crop.h == 0 || dest.w == 0 || dest.h == 0 {
            return;
        }
        let x_map = AxisMap {
            crop_start: i64::from(crop.x),
            crop_len: crop.w,
            dest_len: dest.w,
            frame_len: u32::from(game.width),
            filter: scene.filter,
        };
        let y_map = AxisMap {
            crop_start: i64::from(crop.y),
            crop_len: crop.h,
            dest_len: dest.h,
            frame_len: u32::from(game.height),
            filter: scene.filter,
        };
        let (x0, x1) = visible(dest.x, dest.w, out.width);
        let (y0, y1) = visible(dest.y, dest.h, out.height);
        fill_taps(&mut self.columns, &x_map, x0, x1, dest.x);
        fill_taps(&mut self.lines, &y_map, y0, y1, dest.y);

        let den = (x_map.denominator(), y_map.denominator());
        for (y, line) in (y0..y1).zip(&self.lines) {
            let Some(row) = output_row(out, y, x0, x1) else {
                continue;
            };
            for (px, column) in row.chunks_exact_mut(4).zip(&self.columns) {
                px.copy_from_slice(&sample(game, *column, *line, den).to_xrgb8888_le());
            }
        }
    }
}

/// The output pixels `[start, end)` of an axis that a rectangle at `pos`
/// with length `len` covers.
fn visible(pos: i32, len: u32, out_len: u32) -> (u32, u32) {
    let start = i64::from(pos).clamp(0, i64::from(out_len));
    let end = (i64::from(pos) + i64::from(len)).clamp(start, i64::from(out_len));
    (start as u32, end as u32)
}

fn fill_taps(taps: &mut Vec<Tap>, map: &AxisMap, start: u32, end: u32, dest_pos: i32) {
    taps.clear();
    taps.extend((start..end).map(|o| map.tap((i64::from(o) - i64::from(dest_pos)) as u32)));
}

/// The bytes of output pixels `[x0, x1)` on line `y`. The output check
/// proves that they exist, so `None` never happens.
fn output_row<'b>(out: &'b mut OutputBuffer<'_>, y: u32, x0: u32, x1: u32) -> Option<&'b mut [u8]> {
    let start = (y as usize).checked_mul(out.pitch)?;
    out.data
        .get_mut(start + x0 as usize * 4..start + x1 as usize * 4)
}

fn fill_black(out: &mut OutputBuffer<'_>) {
    let black = Rgb::BLACK.to_xrgb8888_le();
    for y in 0..out.height {
        if let Some(row) = output_row(out, y, 0, out.width) {
            for px in row.chunks_exact_mut(4) {
                px.copy_from_slice(&black);
            }
        }
    }
}

fn sample(frame: &Frame<'_>, column: Tap, line: Tap, (den_x, den_y): (u64, u64)) -> Rgb {
    let (
        Tap::Inside {
            near: x_near,
            far: x_far,
            weight: wx,
        },
        Tap::Inside {
            near: y_near,
            far: y_far,
            weight: wy,
        },
    ) = (column, line)
    else {
        return Rgb::BLACK;
    };
    if wx == 0 && wy == 0 {
        return pixel::read(frame, x_near, y_near);
    }
    let taps = [
        (
            pixel::read(frame, x_near, y_near),
            (den_x - wx) * (den_y - wy),
        ),
        (pixel::read(frame, x_far, y_near), wx * (den_y - wy)),
        (pixel::read(frame, x_near, y_far), (den_x - wx) * wy),
        (pixel::read(frame, x_far, y_far), wx * wy),
    ];
    let total = den_x * den_y;
    let channel = |get: fn(&Rgb) -> u8| {
        let sum: u64 = taps.iter().map(|(c, w)| u64::from(get(c)) * w).sum();
        // The weights sum to `total`, so the rounded quotient is at most 255.
        ((2 * sum + total) / (2 * total)) as u8
    };
    Rgb {
        r: channel(|c| c.r),
        g: channel(|c| c.g),
        b: channel(|c| c.b),
    }
}

#[cfg(test)]
mod tests;
