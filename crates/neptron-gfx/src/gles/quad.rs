use neptron_types::{Filter, Frame, Rect};

use super::GlesError;
use super::shader::Variant;
use crate::RectKind;
use crate::reference::{AxisMap, Pattern, visible};

/// The largest product of the source side and the destination side on one
/// axis. The shader computes `(2 * i + 1) * source` for destination pixel
/// `i`, which stays below `2^24`, where every integer is exact in a 32-bit
/// float.
pub const MAX_AXIS_PRODUCT: u64 = 1 << 23;

/// The uniforms of one draw of the renderer program, and the part of the
/// output it covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Quad {
    /// The output pixels that show a source pixel, from the top left: x, y,
    /// width, height. Every other pixel of the destination samples outside
    /// the frame and stays black.
    pub(super) visible: [f32; 4],
    /// The destination index of the first visible column and line, then
    /// the destination width and height.
    pub(super) dest: [f32; 4],
    /// The crop: x, y, width, height, in source pixels.
    pub(super) crop: [f32; 4],
    /// The source width and height in pixels, then 0 and 0: the renderer
    /// sets the inverse size of the texture storage, which can be larger
    /// than the frame.
    pub(super) frame: [f32; 4],
    /// `1 / (2 * dest.w)` and `1 / (2 * dest.h)`, for the exact quotients,
    /// then the prescale `p` of each axis.
    pub(super) axis: [f32; 4],
    /// The first and the last pixel of the crop inside the frame, x and y,
    /// then x and y. The far tap of a blend clamps to them, as in the
    /// reference.
    pub(super) bounds: [f32; 4],
    /// The program that draws it: the axes that blend.
    pub(super) variant: Variant,
    /// The scanline pattern: the period in destination lines (0 for no
    /// scanlines), the first dark line of a period, the percent that a dark
    /// line keeps, then the inverse of the period.
    pub(super) scan: [f32; 4],
}

/// The draw that maps `crop` of `frame` to `dest` on an output of
/// `out_width` by `out_height`, with the scanline `pattern` of the
/// destination lines, or `None` when it shows nothing.
pub(super) fn quad(
    frame: &Frame<'_>,
    (crop, dest): (Rect, Rect),
    filter: Filter,
    pattern: Option<Pattern>,
    kind: RectKind,
    (out_width, out_height): (u32, u32),
) -> Result<Option<Quad>, GlesError> {
    // A frame with a zero side has no pixel to sample, so every sample is
    // outside it and the destination stays black, as in the reference.
    if frame.width == 0 || frame.height == 0 {
        return Ok(None);
    }
    if crop.w == 0 || crop.h == 0 || dest.w == 0 || dest.h == 0 {
        return Ok(None);
    }
    for (source_len, dest_len) in [(crop.w, dest.w), (crop.h, dest.h)] {
        if u64::from(source_len) * u64::from(dest_len) > MAX_AXIS_PRODUCT {
            return Err(GlesError::ScaleTooLarge {
                rect: kind,
                source_len,
                dest_len,
                limit: MAX_AXIS_PRODUCT,
            });
        }
    }
    let x = Axis::of(
        crop.x,
        crop.w,
        dest.x,
        dest.w,
        frame.width,
        filter,
        out_width,
    );
    let y = Axis::of(
        crop.y,
        crop.h,
        dest.y,
        dest.h,
        frame.height,
        filter,
        out_height,
    );
    let (Some(x), Some(y)) = (x, y) else {
        return Ok(None);
    };
    Ok(Some(Quad {
        visible: [x.first_out, y.first_out, x.len, y.len],
        dest: [x.first_index, y.first_index, dest.w as f32, dest.h as f32],
        crop: [x.crop_start, y.crop_start, crop.w as f32, crop.h as f32],
        frame: [f32::from(frame.width), f32::from(frame.height), 0.0, 0.0],
        axis: [x.recip, y.recip, x.prescale, y.prescale],
        bounds: [x.bounds.0, y.bounds.0, x.bounds.1, y.bounds.1],
        variant: Variant {
            blend_x: x.blends,
            blend_y: y.blends,
        },
        // A period is at most dest.h and the percent at most 100, so the
        // floats are exact.
        scan: pattern.map_or([0.0; 4], |p| {
            [
                p.period as f32,
                p.first_dark as f32,
                p.keep_percent as f32,
                1.0 / p.period as f32,
            ]
        }),
    }))
}

/// The values of one axis of a [`Quad`].
struct Axis {
    first_out: f32,
    len: f32,
    first_index: f32,
    crop_start: f32,
    recip: f32,
    prescale: f32,
    bounds: (f32, f32),
    blends: bool,
}

impl Axis {
    /// One axis of the draw, or `None` when no output pixel of it shows a
    /// source pixel. The caller checked that both lengths are not 0.
    fn of(
        crop_pos: i32,
        crop_len: u32,
        dest_pos: i32,
        dest_len: u32,
        frame_len: u16,
        filter: Filter,
        out_len: u32,
    ) -> Option<Self> {
        let map = AxisMap {
            crop_start: i64::from(crop_pos),
            crop_len,
            dest_len,
            frame_len: u32::from(frame_len),
            filter,
        };
        let (inside_start, inside_end) = map.inside();
        let (out_start, out_end) = visible(dest_pos, dest_len, out_len);
        let dest_pos = i64::from(dest_pos);
        let start = i64::from(out_start).max(dest_pos + i64::from(inside_start));
        let end = i64::from(out_end).min(dest_pos + i64::from(inside_end));
        if start >= end {
            return None;
        }
        // A source pixel is inside the frame, so the crop starts less than
        // its length before the frame and inside 2^16 after it: every
        // value below is an integer below 2^24, exact in an f32.
        let crop_start = i64::from(crop_pos);
        let first = crop_start.max(0);
        let last = (crop_start + i64::from(crop_len)).min(i64::from(frame_len)) - 1;
        Some(Self {
            first_out: start as f32,
            len: (end - start) as f32,
            first_index: (start - dest_pos) as f32,
            crop_start: crop_start as f32,
            recip: (1.0 / (2.0 * f64::from(dest_len))) as f32,
            prescale: (dest_len / crop_len).max(1) as f32,
            bounds: (first as f32, last as f32),
            // At a whole scale every weight of the axis is 0.
            blends: filter == Filter::SharpBilinear && !dest_len.is_multiple_of(crop_len),
        })
    }
}

#[cfg(test)]
mod tests {
    use neptron_types::PixelFormat;

    use super::*;

    const OUT: (u32, u32) = (320, 240);

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    fn frame(format: PixelFormat) -> Frame<'static> {
        Frame {
            data: &[],
            width: 10,
            height: 8,
            pitch: 64,
            format,
        }
    }

    fn game(crop: Rect, dest: Rect) -> Result<Option<Quad>, GlesError> {
        let f = frame(PixelFormat::Rgb565);
        let filter = Filter::SharpBilinear;
        quad(&f, (crop, dest), filter, None, RectKind::Dest, OUT)
    }

    #[test]
    fn a_full_destination_maps_every_value() {
        let q = game(rect(1, 2, 8, 6), rect(4, 3, 24, 18)).unwrap().unwrap();
        assert_eq!(q.visible, [4.0, 3.0, 24.0, 18.0]);
        assert_eq!(q.dest, [0.0, 0.0, 24.0, 18.0]);
        assert_eq!(q.crop, [1.0, 2.0, 8.0, 6.0]);
        assert_eq!(q.frame, [10.0, 8.0, 0.0, 0.0]);
        assert_eq!(q.axis, [1.0 / 48.0, 1.0 / 36.0, 3.0, 3.0]);
        assert_eq!(q.bounds, [1.0, 2.0, 8.0, 7.0]);
        assert_eq!(q.variant, Variant::ALL[0], "a whole scale blends nothing");
        assert_eq!(q.scan, [0.0; 4]);
    }

    #[test]
    fn the_scanline_pattern_goes_to_its_uniform() {
        let f = frame(PixelFormat::Xrgb8888);
        let pattern = Pattern {
            period: 4,
            first_dark: 2,
            keep_percent: 25,
        };
        let rects = (rect(0, 0, 8, 6), rect(0, 0, 8, 24));
        let q = quad(
            &f,
            rects,
            Filter::Nearest,
            Some(pattern),
            RectKind::Dest,
            OUT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(q.scan, [4.0, 2.0, 25.0, 0.25]);
    }

    #[test]
    fn a_clipped_destination_starts_inside() {
        let q = game(rect(0, 0, 8, 6), rect(-5, 230, 40, 30))
            .unwrap()
            .unwrap();
        assert_eq!(q.visible, [0.0, 230.0, 35.0, 10.0]);
        assert_eq!(q.dest, [5.0, 0.0, 40.0, 30.0]);
    }

    #[test]
    fn nothing_to_draw_is_none() {
        assert_eq!(game(rect(0, 0, 0, 6), rect(0, 0, 8, 8)), Ok(None));
        assert_eq!(game(rect(0, 0, 8, 6), rect(0, 0, 8, 0)), Ok(None));
        assert_eq!(game(rect(0, 0, 8, 6), rect(320, 0, 8, 8)), Ok(None));
        assert_eq!(game(rect(0, 0, 8, 6), rect(-8, 0, 8, 8)), Ok(None));
    }

    #[test]
    fn a_frame_with_a_zero_side_draws_nothing() {
        for (width, height, pitch) in [(0, 240, 640), (320, 0, 0)] {
            let f = Frame {
                data: &[],
                width,
                height,
                pitch,
                format: PixelFormat::Rgb565,
            };
            let q = quad(
                &f,
                (rect(0, 0, 8, 6), rect(0, 0, 8, 6)),
                Filter::Nearest,
                None,
                RectKind::Dest,
                OUT,
            );
            assert_eq!(q, Ok(None), "{width}x{height}");
        }
    }

    #[test]
    fn a_crop_wholly_outside_the_frame_draws_nothing() {
        for crop in [
            rect(i32::MIN, 0, 8, 6),
            rect(0, i32::MAX, 8, 6),
            rect(10, 0, 8, 6),
        ] {
            assert_eq!(game(crop, rect(0, 0, 8, 6)), Ok(None), "{crop:?}");
        }
    }

    /// Only the output pixels that sample inside the frame are drawn; the
    /// clear leaves the others black, as in the reference.
    #[test]
    fn the_quad_covers_only_pixels_inside_the_frame() {
        let q = game(rect(-2, 0, 8, 6), rect(0, 0, 8, 6)).unwrap().unwrap();
        assert_eq!(q.visible, [2.0, 0.0, 6.0, 6.0]);
        assert_eq!(q.dest[..2], [2.0, 0.0]);
        assert_eq!(q.bounds, [0.0, 0.0, 5.0, 5.0]);
        let q = game(rect(6, 3, 8, 6), rect(0, 0, 8, 6)).unwrap().unwrap();
        assert_eq!(q.visible, [0.0, 0.0, 4.0, 5.0]);
        assert_eq!(q.bounds, [6.0, 3.0, 9.0, 7.0]);
    }

    #[test]
    fn the_axis_product_limit_is_inclusive() {
        assert!(game(rect(0, 0, 2048, 6), rect(0, 0, 4096, 6)).is_ok());
        assert_eq!(
            game(rect(0, 0, 8, 2049), rect(0, 0, 8, 4096)),
            Err(GlesError::ScaleTooLarge {
                rect: RectKind::Dest,
                source_len: 2049,
                dest_len: 4096,
                limit: MAX_AXIS_PRODUCT
            })
        );
    }

    /// Only sharp bilinear at a scale that is not whole blends an axis.
    #[test]
    fn the_axes_that_blend_pick_the_program() {
        let cases = [
            (Filter::Nearest, rect(0, 0, 5, 6), (false, false)),
            (Filter::SharpBilinear, rect(0, 0, 16, 12), (false, false)),
            (Filter::SharpBilinear, rect(0, 0, 20, 6), (true, false)),
            (Filter::SharpBilinear, rect(0, 0, 8, 9), (false, true)),
            (Filter::SharpBilinear, rect(0, 0, 4, 3), (true, true)),
        ];
        for (filter, dest, (blend_x, blend_y)) in cases {
            let f = frame(PixelFormat::Xrgb1555);
            let rects = (rect(0, 0, 8, 6), dest);
            let q = quad(&f, rects, filter, None, RectKind::Dest, OUT)
                .unwrap()
                .unwrap();
            assert_eq!(
                q.variant,
                Variant { blend_x, blend_y },
                "{filter:?} {dest:?}"
            );
        }
    }
}
