use neptron_types::{Filter, Frame, Rect};

use super::GlesError;
use super::shader::Variant;
use crate::RectKind;
use crate::reference::{Pattern, visible};

/// The largest product of the source side and the destination side on one
/// axis. The shader computes `(2 * i + 1) * source` for destination pixel
/// `i`, which stays below `2^24`, where every integer is exact in a 32-bit
/// float.
pub const MAX_AXIS_PRODUCT: u64 = 1 << 23;

/// A crop position beyond this many pixels puts every sample outside any
/// frame, as at the limit itself, because a frame side is below `2^16`.
/// The clamp keeps the value exact in a 32-bit float.
const CROP_POSITION_LIMIT_PX: i32 = 1 << 20;

/// The uniforms of one draw of the renderer program. Every value is an
/// integer below `2^24`, so a 32-bit float holds it exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Quad {
    /// The visible part of the destination, in output pixels from the top
    /// left: x, y, width, height.
    pub(super) visible: [f32; 4],
    /// The destination index of the first visible column and line, then
    /// the destination width and height.
    pub(super) dest: [f32; 4],
    /// The crop: x, y, width, height, in source pixels.
    pub(super) crop: [f32; 4],
    /// The source width and height in pixels, then the texture width (bytes
    /// per row) and height.
    pub(super) frame: [f32; 4],
    /// The program that draws it: the pixel format and the filter.
    pub(super) variant: Variant,
    /// The scanline pattern: the period in destination lines (0 for no
    /// scanlines), the first dark line of a period, the percent that a dark
    /// line keeps, then 0.
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
    let (x0, x1) = visible(dest.x, dest.w, out_width);
    let (y0, y1) = visible(dest.y, dest.h, out_height);
    if x0 == x1 || y0 == y1 {
        return Ok(None);
    }
    let first = |start: u32, pos: i32| (i64::from(start) - i64::from(pos)) as f32;
    let clamp = |v: i32| v.clamp(-CROP_POSITION_LIMIT_PX, CROP_POSITION_LIMIT_PX) as f32;
    let bpp = frame.format.bytes_per_pixel() as f32;
    Ok(Some(Quad {
        visible: [x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32],
        dest: [
            first(x0, dest.x),
            first(y0, dest.y),
            dest.w as f32,
            dest.h as f32,
        ],
        crop: [clamp(crop.x), clamp(crop.y), crop.w as f32, crop.h as f32],
        frame: [
            f32::from(frame.width),
            f32::from(frame.height),
            f32::from(frame.width) * bpp,
            f32::from(frame.height),
        ],
        variant: Variant::of(frame.format, filter),
        // A period is at most dest.h and the percent at most 100, so the
        // floats are exact.
        scan: pattern.map_or([0.0; 4], |p| {
            [
                p.period as f32,
                p.first_dark as f32,
                p.keep_percent as f32,
                0.0,
            ]
        }),
    }))
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
        assert_eq!(q.frame, [10.0, 8.0, 20.0, 8.0]);
        assert_eq!(
            q.variant,
            Variant::of(PixelFormat::Rgb565, Filter::SharpBilinear)
        );
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
        assert_eq!(q.scan, [4.0, 2.0, 25.0, 0.0]);
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
    fn far_crop_positions_are_clamped() {
        let q = game(rect(i32::MIN, i32::MAX, 8, 6), rect(0, 0, 8, 6))
            .unwrap()
            .unwrap();
        assert_eq!(q.crop[..2], [-1_048_576.0, 1_048_576.0]);
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

    #[test]
    fn format_and_filter_codes() {
        let f = frame(PixelFormat::Xrgb1555);
        let q = quad(
            &f,
            (rect(0, 0, 1, 1), rect(0, 0, 1, 1)),
            Filter::Nearest,
            None,
            RectKind::UiDest,
            OUT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            q.variant,
            Variant::of(PixelFormat::Xrgb1555, Filter::Nearest)
        );
        let f = frame(PixelFormat::Xrgb8888);
        let q = quad(
            &f,
            (rect(0, 0, 1, 1), rect(0, 0, 1, 1)),
            Filter::Nearest,
            None,
            RectKind::UiDest,
            OUT,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            q.variant,
            Variant::of(PixelFormat::Xrgb8888, Filter::Nearest)
        );
        assert_eq!(q.frame[2], 40.0);
    }
}
