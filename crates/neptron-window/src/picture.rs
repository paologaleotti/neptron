//! The size of the picture in the desktop window.
//!
//! A CRT shows every mode as a 4:3 picture, whatever its pixel count. The
//! window does the same: the vertical axis scales by a whole number, so
//! each line keeps the same height, and the horizontal axis scales on its
//! own to give the picture its 4:3 shape. A 320x240 mode at factor 3 is a
//! 960x720 picture, and a 2560x240 super-width mode is also 960x720.

/// A width and a height, in pixels or in window points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Size {
    pub(crate) w: u32,
    pub(crate) h: u32,
}

/// The shape of the picture, as width to height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Aspect {
    pub(crate) w: u32,
    pub(crate) h: u32,
}

/// The shape of a CRT picture.
pub(crate) const CRT_ASPECT: Aspect = Aspect { w: 4, h: 3 };

/// The part of the usable desktop area, in points, that the window frame
/// and the title bar can take on each axis.
pub(crate) const FRAME_MARGIN_PT: u32 = 64;

/// The space a picture can take on a desktop area of `usable`, after the
/// window frame. It is at least 1 by 1.
pub(crate) fn picture_bound(usable: Size) -> Size {
    Size {
        w: usable.w.saturating_sub(FRAME_MARGIN_PT).max(1),
        h: usable.h.saturating_sub(FRAME_MARGIN_PT).max(1),
    }
}

/// The picture size for a mode with `active` pixels, inside `bound`, with
/// the shape `aspect`.
///
/// The height is `active.h * k` for the largest whole `k` at which the
/// picture fits `bound`. The width is `height * aspect.w / aspect.h`,
/// rounded to the nearest point, with a half rounded up. When no whole `k`
/// fits, the picture is the largest box of that shape inside `bound`. A
/// side of 0 in `active` or in `aspect` gives the size 1 by 1.
pub(crate) fn picture_size(active: Size, bound: Size, aspect: Aspect) -> Size {
    if active.w == 0 || active.h == 0 || aspect.w == 0 || aspect.h == 0 {
        return Size { w: 1, h: 1 };
    }
    let (lines, aw, ah) = (
        u64::from(active.h),
        u64::from(aspect.w),
        u64::from(aspect.h),
    );
    let (bw, bh) = (u64::from(bound.w), u64::from(bound.h));
    let k = (bh / lines).min(bw * ah / (aw * lines));
    let height = if k >= 1 {
        lines * k
    } else {
        bh.min(bw * ah / aw).max(1)
    };
    let width = ((2 * height * aw + ah) / (2 * ah)).max(1);
    // Both sides are at most their bound, or 1 for an empty bound, and
    // each bound came from a `u32`, so the conversions never fail.
    Size {
        w: u32::try_from(width).unwrap_or(u32::MAX),
        h: u32::try_from(height).unwrap_or(u32::MAX),
    }
}

/// True when the picture is a whole multiple of the mode on both axes, so
/// that nearest scaling keeps every pixel the same size. Otherwise the
/// window scales with a linear filter, because nearest scaling at a
/// fraction drops or doubles whole columns.
pub(crate) fn is_whole_scale(active: Size, picture: Size) -> bool {
    active.w != 0
        && active.h != 0
        && picture.w.is_multiple_of(active.w)
        && picture.h.is_multiple_of(active.h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn size(w: u32, h: u32) -> Size {
        Size { w, h }
    }

    /// A 1440x900 desktop with its menu bar and dock taken off.
    const LAPTOP: Size = size(1376, 836);

    #[test]
    fn a_240_line_mode_scales_by_the_largest_whole_factor() {
        assert_eq!(
            picture_size(size(320, 240), LAPTOP, CRT_ASPECT),
            size(960, 720)
        );
        assert_eq!(
            picture_size(size(320, 240), size(1856, 1016), CRT_ASPECT),
            size(1280, 960)
        );
    }

    #[test]
    fn a_super_width_mode_keeps_the_4_3_shape() {
        let picture = picture_size(size(2560, 240), LAPTOP, CRT_ASPECT);
        assert_eq!(picture, size(960, 720));
        assert!(!is_whole_scale(size(2560, 240), picture));
    }

    #[test]
    fn every_mode_of_one_line_count_gets_the_same_picture() {
        for width in [256, 320, 384, 640, 1280, 2560, 3840] {
            assert_eq!(
                picture_size(size(width, 224), LAPTOP, CRT_ASPECT),
                size(896, 672),
                "{width}"
            );
        }
    }

    #[test]
    fn the_width_limit_can_lower_the_factor() {
        // Factor 3 is 720 lines high but 960 points wide.
        assert_eq!(
            picture_size(size(320, 240), size(900, 2000), CRT_ASPECT),
            size(640, 480)
        );
    }

    #[test]
    fn an_odd_width_rounds_to_the_nearest_point() {
        // 239 * 4 / 3 = 318.67.
        assert_eq!(
            picture_size(size(320, 239), size(1000, 300), CRT_ASPECT),
            size(319, 239)
        );
        // 5 * 4 / 3 = 6.67.
        assert_eq!(
            picture_size(size(1, 5), size(100, 5), CRT_ASPECT),
            size(7, 5)
        );
    }

    #[test]
    fn a_mode_taller_than_the_desktop_shrinks_to_the_largest_fitting_box() {
        assert_eq!(
            picture_size(size(1920, 1080), size(1376, 836), CRT_ASPECT),
            size(1115, 836)
        );
        assert_eq!(
            picture_size(size(640, 480), size(400, 836), CRT_ASPECT),
            size(400, 300)
        );
    }

    #[test]
    fn a_480_line_interlaced_mode_fits_at_factor_1() {
        assert_eq!(
            picture_size(size(640, 480), LAPTOP, CRT_ASPECT),
            size(640, 480)
        );
        assert!(is_whole_scale(size(640, 480), size(640, 480)));
    }

    #[test]
    fn another_aspect_gives_another_width() {
        let wide = Aspect { w: 16, h: 9 };
        assert_eq!(picture_size(size(320, 240), LAPTOP, wide), size(1280, 720));
    }

    #[test]
    fn empty_inputs_give_a_one_point_picture() {
        let one = size(1, 1);
        assert_eq!(picture_size(size(0, 240), LAPTOP, CRT_ASPECT), one);
        assert_eq!(picture_size(size(320, 0), LAPTOP, CRT_ASPECT), one);
        assert_eq!(
            picture_size(size(320, 240), LAPTOP, Aspect { w: 0, h: 3 }),
            one
        );
        assert_eq!(picture_size(size(320, 240), size(0, 0), CRT_ASPECT), one);
    }

    #[test]
    fn the_bound_leaves_room_for_the_window_frame() {
        assert_eq!(picture_bound(size(1440, 900)), size(1376, 836));
        assert_eq!(picture_bound(size(10, 64)), size(1, 1));
    }

    #[test]
    fn whole_scale_needs_a_whole_factor_on_both_axes() {
        assert!(is_whole_scale(size(320, 240), size(960, 720)));
        assert!(!is_whole_scale(size(256, 224), size(896, 672)));
        assert!(!is_whole_scale(size(320, 240), size(960, 721)));
        assert!(!is_whole_scale(size(0, 240), size(960, 720)));
    }
}
