use neptron_types::{Filter, Modeline, Rect, ui_dest_fill, ui_dest_uniform};

use super::OrientedSource;
use super::source::div_round;

/// Size of the UI canvas, in pixels.
pub const UI_CANVAS: (u16, u16) = (320, 240);

/// How the game picture sits in a mode: everything the renderer needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Placement {
    /// The part of the game frame to show, in pixels of the frame as the
    /// core draws it: the whole frame, less the lines of an overscan crop
    /// at both edges.
    pub crop: Rect,
    /// Where the game goes, in pixels of the mode. It lies inside the
    /// active area.
    pub dest: Rect,
    /// How the game is scaled from `crop` to `dest`.
    pub filter: Filter,
    /// Where the UI canvas goes, in pixels of the mode: the full active
    /// area on a CRT, one whole scale, centred, on a flat panel.
    pub ui_dest: Rect,
    /// The whole scale factors that hold exactly on each axis.
    pub scale: ScaleClaim,
}

/// The whole scale factor of each axis from the source on the screen
/// (after the rotation) to `dest`, or `None` where the scale is a fraction.
/// A claim of `Some(k)` means `dest` is exactly `k` times the crop on
/// that axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScaleClaim {
    /// Horizontal factor.
    pub x: Option<u16>,
    /// Vertical factor.
    pub y: Option<u16>,
}

/// How the policy fills the width of a CRT mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HorizontalScale {
    /// The mode is this whole multiple of the base width: the game width,
    /// or the bordered width for a source with too few lines.
    Multiple(u16),
    /// A fixed width: the game is scaled by a fraction. Also the case of a
    /// source wider than the GPU sends, at the GPU's widest mode.
    Fixed(u16),
}

/// The vertical part of a CRT placement, from the shape of the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Vertical {
    /// The game is `factor` times its lines, `lines` in all, centred, with
    /// `crop_per_edge` source lines left out at the top and the bottom.
    Exact {
        lines: u16,
        factor: u16,
        crop_per_edge: u16,
    },
    /// The game fills the active lines.
    Fill,
}

/// The placement of the game in a CRT mode. `base_width` is the width the
/// horizontal multiple applies to. The UI fills the active area.
pub(crate) fn crt_placement(
    source: &OrientedSource,
    mode: &Modeline,
    vertical: Vertical,
    horizontal: HorizontalScale,
    base_width: u16,
) -> Placement {
    let (y, h, claim_y) = match vertical {
        Vertical::Exact { lines, factor, .. } => (
            centre(mode.vactive, u32::from(lines)),
            u32::from(lines),
            Some(factor),
        ),
        Vertical::Fill => (
            0,
            u32::from(mode.vactive),
            whole(mode.vactive, source.height),
        ),
    };
    let w = match (vertical, horizontal) {
        (Vertical::Fill, _) => u32::from(mode.hactive),
        (_, HorizontalScale::Multiple(k)) => u32::from(source.width) * u32::from(k),
        (_, HorizontalScale::Fixed(width)) => div_round(
            u64::from(source.width) * u64::from(width),
            u64::from(base_width),
        ),
    }
    .min(u32::from(mode.hactive));
    let claim_x = whole_u32(w, source.width);
    let scale = ScaleClaim {
        x: claim_x,
        y: claim_y,
    };
    let crop_per_edge = match vertical {
        Vertical::Exact { crop_per_edge, .. } => crop_per_edge,
        Vertical::Fill => 0,
    };
    Placement {
        crop: line_crop(source, crop_per_edge),
        dest: Rect {
            x: centre(mode.hactive, w),
            y,
            w,
            h,
        },
        filter: filter_for(scale),
        ui_dest: ui_dest_fill(mode.hactive, mode.vactive),
        scale,
    }
}

/// The placement of the game in a flat-panel mode: the largest whole
/// vertical scale at which the game, at its display aspect, fits the mode,
/// centred. A game larger than the mode fits with a fractional scale.
pub(crate) fn panel_placement(source: &OrientedSource, mode: &Modeline) -> Placement {
    let largest = mode.vactive / source.height;
    let fits = (1..=largest)
        .rev()
        .map(|k| (Some(k), u32::from(source.height) * u32::from(k)))
        .find(|&(_, h)| aspect_width(source, h) <= u32::from(mode.hactive));
    let (claim_y, h) = fits.unwrap_or_else(|| (None, fitted_height(source, mode)));
    let w = aspect_width(source, h).min(u32::from(mode.hactive));
    let scale = ScaleClaim {
        x: whole_u32(w, source.width),
        y: claim_y,
    };
    Placement {
        crop: frame_rect(source),
        dest: Rect {
            x: centre(mode.hactive, w),
            y: centre(mode.vactive, h),
            w,
            h,
        },
        filter: filter_for(scale),
        ui_dest: ui_dest_uniform(mode.hactive, mode.vactive, UI_CANVAS.0, UI_CANVAS.1),
        scale,
    }
}

/// The width of the game at a height of `h` pixels, at its display aspect,
/// or with square pixels when the core gives no aspect.
fn aspect_width(source: &OrientedSource, h: u32) -> u32 {
    match source.aspect_milli {
        0 => div_round(
            u64::from(source.width) * u64::from(h),
            u64::from(source.height),
        ),
        aspect => div_round(u64::from(h) * u64::from(aspect), 1_000),
    }
}

/// The height at which the game fills the mode on one axis.
fn fitted_height(source: &OrientedSource, mode: &Modeline) -> u32 {
    let at_full_height = aspect_width(source, u32::from(mode.vactive));
    if at_full_height <= u32::from(mode.hactive) {
        return u32::from(mode.vactive);
    }
    div_round(
        u64::from(mode.vactive) * u64::from(mode.hactive),
        u64::from(at_full_height.max(1)),
    )
}

fn frame_rect(source: &OrientedSource) -> Rect {
    Rect {
        x: 0,
        y: 0,
        w: u32::from(source.frame_width),
        h: u32::from(source.frame_height),
    }
}

/// The frame without `per_edge` lines of the screen at each end. The
/// lines of a turned frame are its columns; the crop is the same at both
/// ends, so the direction of the turn does not matter.
fn line_crop(source: &OrientedSource, per_edge: u16) -> Rect {
    let whole = frame_rect(source);
    let cut = u32::from(per_edge);
    let edge = i32::from(per_edge);
    if source.turned {
        Rect {
            x: edge,
            w: whole.w.saturating_sub(cut * 2),
            ..whole
        }
    } else {
        Rect {
            y: edge,
            h: whole.h.saturating_sub(cut * 2),
            ..whole
        }
    }
}

fn filter_for(scale: ScaleClaim) -> Filter {
    if scale.x.is_some() && scale.y.is_some() {
        Filter::Nearest
    } else {
        Filter::SharpBilinear
    }
}

/// The offset that centres `inner` in `outer`, rounded down; 0 when it does
/// not fit.
fn centre(outer: u16, inner: u32) -> i32 {
    let margin = u32::from(outer).saturating_sub(inner) / 2;
    // A margin is at most half of a u16.
    margin as i32
}

fn whole(outer: u16, inner: u16) -> Option<u16> {
    whole_u32(u32::from(outer), inner)
}

fn whole_u32(outer: u32, inner: u16) -> Option<u16> {
    let inner = u32::from(inner);
    if inner == 0 || outer == 0 || !outer.is_multiple_of(inner) {
        return None;
    }
    u16::try_from(outer / inner).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(width: u16, height: u16, aspect_milli: u32) -> OrientedSource {
        OrientedSource {
            width,
            height,
            refresh_mhz: 60_000,
            interlaced: false,
            aspect_milli,
            frame_width: width,
            frame_height: height,
            turned: false,
        }
    }

    fn mode(hactive: u16, vactive: u16) -> Modeline {
        Modeline {
            pclock_hz: 25_000_000,
            hactive,
            hsync_start: hactive + 10,
            hsync_end: hactive + 20,
            htotal: hactive + 30,
            vactive,
            vsync_start: vactive + 3,
            vsync_end: vactive + 6,
            vtotal: vactive + 20,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn an_exact_crt_mode_fills_its_width_and_its_lines() {
        let p = crt_placement(
            &source(256, 224, 1_333),
            &mode(1_280, 224),
            Vertical::Exact {
                lines: 224,
                factor: 1,
                crop_per_edge: 0,
            },
            HorizontalScale::Multiple(5),
            256,
        );
        assert_eq!(p.crop, rect(0, 0, 256, 224));
        assert_eq!(p.dest, rect(0, 0, 1_280, 224));
        assert_eq!(
            p.scale,
            ScaleClaim {
                x: Some(5),
                y: Some(1)
            }
        );
        assert_eq!(p.filter, Filter::Nearest);
        assert_eq!(p.ui_dest, rect(0, 0, 1_280, 224));
    }

    #[test]
    fn an_overscan_crop_leaves_out_the_same_lines_at_both_edges() {
        // NES at 2x with 1 line per edge cropped: 238 lines, 476 at 2x.
        let p = crt_placement(
            &source(256, 240, 1_333),
            &mode(256, 476),
            Vertical::Exact {
                lines: 476,
                factor: 2,
                crop_per_edge: 1,
            },
            HorizontalScale::Multiple(1),
            256,
        );
        assert_eq!(p.crop, rect(0, 1, 256, 238));
        assert_eq!(p.dest, rect(0, 0, 256, 476));
        assert_eq!(p.filter, Filter::Nearest);
        // A turned frame of 256x224 shows 256 lines: its columns are cut.
        let turned = OrientedSource {
            frame_width: 256,
            frame_height: 224,
            turned: true,
            ..source(224, 256, 750)
        };
        let p = crt_placement(
            &turned,
            &mode(224, 500),
            Vertical::Exact {
                lines: 500,
                factor: 2,
                crop_per_edge: 3,
            },
            HorizontalScale::Multiple(1),
            224,
        );
        assert_eq!(p.crop, rect(3, 0, 250, 224));
        assert_eq!(p.dest, rect(0, 0, 224, 500));
    }

    #[test]
    fn a_bordered_source_is_centred_on_both_axes() {
        // A Game Boy in a 320x224 mode.
        let p = crt_placement(
            &source(160, 144, 1_111),
            &mode(320, 224),
            Vertical::Exact {
                lines: 144,
                factor: 1,
                crop_per_edge: 0,
            },
            HorizontalScale::Multiple(1),
            320,
        );
        assert_eq!(p.dest, rect(80, 40, 160, 144));
        assert_eq!(p.filter, Filter::Nearest);
    }

    #[test]
    fn a_fixed_super_width_scales_by_a_fraction_with_a_sharp_filter() {
        let p = crt_placement(
            &source(256, 224, 1_333),
            &mode(1_920, 224),
            Vertical::Exact {
                lines: 224,
                factor: 1,
                crop_per_edge: 0,
            },
            HorizontalScale::Fixed(1_920),
            256,
        );
        assert_eq!(p.dest, rect(0, 0, 1_920, 224));
        assert_eq!(
            p.scale,
            ScaleClaim {
                x: None,
                y: Some(1)
            }
        );
        assert_eq!(p.filter, Filter::SharpBilinear);
    }

    #[test]
    fn a_fitted_mode_fills_the_active_area() {
        let p = crt_placement(
            &source(384, 384, 1_333),
            &mode(768, 576),
            Vertical::Fill,
            HorizontalScale::Multiple(2),
            384,
        );
        assert_eq!(p.dest, rect(0, 0, 768, 576));
        assert_eq!(
            p.scale,
            ScaleClaim {
                x: Some(2),
                y: None
            }
        );
        assert_eq!(p.filter, Filter::SharpBilinear);
    }

    #[test]
    fn a_panel_scales_the_lines_by_the_largest_whole_factor_that_fits() {
        let p = panel_placement(&source(256, 224, 1_333), &mode(1_920, 1_080));
        // 224 x 4 = 896 lines, 896 x 4:3 = 1194 pixels.
        assert_eq!(p.dest, rect(363, 92, 1_194, 896));
        assert_eq!(
            p.scale,
            ScaleClaim {
                x: None,
                y: Some(4)
            }
        );
        assert_eq!(p.ui_dest, rect(320, 60, 1_280, 960));
    }

    #[test]
    fn a_panel_with_square_pixels_scales_both_axes_by_the_same_factor() {
        let p = panel_placement(&source(320, 240, 0), &mode(1_920, 1_080));
        assert_eq!(p.dest, rect(320, 60, 1_280, 960));
        assert_eq!(
            p.scale,
            ScaleClaim {
                x: Some(4),
                y: Some(4)
            }
        );
        assert_eq!(p.filter, Filter::Nearest);
    }

    #[test]
    fn a_game_larger_than_the_panel_fits_with_a_fraction() {
        let p = panel_placement(&source(640, 480, 1_333), &mode(640, 400));
        assert_eq!(p.dest, rect(53, 0, 533, 400));
        assert_eq!(p.scale.y, None);
        assert_eq!(p.filter, Filter::SharpBilinear);
    }
}
