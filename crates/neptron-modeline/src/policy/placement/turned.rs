use neptron_types::{Filter, Modeline, MonitorKind, Rect, ui_dest_uniform};

use super::centre;
use crate::Aspect;
use crate::policy::TV_SAFE_WIDTH_SAMPLES;
use crate::policy::source::div_round;

/// Where a canvas goes in a mode and how it is scaled into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiPlacement {
    /// Where the turned canvas goes, in pixels of the mode: the value of
    /// `Scene::ui_dest`.
    pub dest: Rect,
    /// How the turned canvas is scaled into `dest`: the value of
    /// `Scene::ui_filter`.
    pub filter: Filter,
}

/// Where a UI canvas turned by 90 or 270 degrees goes in `mode`, and its
/// filter. `canvas` is the size of the turned canvas `T` in scan-out
/// pixels, width then lines: 240 x 320 for the 320x240 canvas.
/// `monitor_aspect` is the shape of the face of a CRT; a TV and a flat
/// panel do not read it.
///
/// - A CRT: the lines are `T`'s lines times the largest whole `k` that
///   fits the active lines, or all the active lines when not even `k = 1`
///   fits. The width keeps the shape of `T` on the face, at most the
///   active width and at least 1. Centred.
/// - A TV: the lines as on a CRT. The width is the whole multiple of `T`'s
///   width nearest to the width of its shape, where
///   [`TV_SAFE_WIDTH_SAMPLES`] over the active lines is 4:3. It is at
///   least one multiple and at most the safe width. A fraction would give
///   columns of uneven width, which shows on text. Centred.
/// - A flat panel: the largest whole scale that fits both axes, centred
///   ([`ui_dest_uniform`]).
///
/// The filter is [`Filter::Nearest`] when each axis has a scale of 1 or
/// more, and [`Filter::SharpBilinear`] when an axis is below 1: nearest
/// would drop canvas columns there, and a stroke of a glyph one pixel
/// wide would vanish. A canvas or a CRT or TV mode with a zero side gives
/// a rectangle with a zero side, which shows no UI.
///
/// The function decides no mode: it places the canvas in the mode it is
/// given.
pub fn ui_dest_turned(
    kind: &MonitorKind,
    mode: &Modeline,
    canvas: (u16, u16),
    monitor_aspect: Aspect,
) -> UiPlacement {
    let (canvas_w, canvas_h) = canvas;
    let dest = match kind {
        MonitorKind::FlatPanel { .. } => {
            ui_dest_uniform(mode.hactive, mode.vactive, canvas_w, canvas_h)
        }
        _ if canvas_w == 0 || canvas_h == 0 || mode.hactive == 0 || mode.vactive == 0 => Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        MonitorKind::Crt { .. } => {
            let h = turned_lines(mode.vactive, canvas_h);
            let w = shape_width(canvas, h, mode.hactive, mode.vactive, monitor_aspect)
                .clamp(1, u32::from(mode.hactive));
            centred(mode, w, h)
        }
        MonitorKind::Tv { .. } => {
            let h = turned_lines(mode.vactive, canvas_h);
            let shape = shape_width(
                canvas,
                h,
                TV_SAFE_WIDTH_SAMPLES,
                mode.vactive,
                Aspect::STANDARD_CRT,
            );
            let safe = u32::from(TV_SAFE_WIDTH_SAMPLES.min(mode.hactive));
            let unit = u32::from(canvas_w);
            let multiple = div_round(u64::from(shape), u64::from(unit))
                .min(safe / unit)
                .max(1);
            centred(mode, (unit * multiple).min(safe), h)
        }
    };
    UiPlacement {
        dest,
        filter: ui_filter(dest, canvas),
    }
}

/// The lines of the turned canvas: its lines at the largest whole scale
/// that fits `vactive`, or all of `vactive` when the canvas is taller.
fn turned_lines(vactive: u16, canvas_h: u16) -> u32 {
    match vactive / canvas_h {
        0 => u32::from(vactive),
        k => u32::from(canvas_h) * u32::from(k),
    }
}

/// The width at which a canvas of `canvas` pixels, shown on `h` of the
/// `vactive` lines, keeps its shape on a face of shape `face` that the
/// `hactive` pixels span: `canvas_w * h * hactive * face.h /
/// (canvas_h * vactive * face.w)`, rounded.
fn shape_width(canvas: (u16, u16), h: u32, hactive: u16, vactive: u16, face: Aspect) -> u32 {
    // Each factor is at most 65 535 (h is at most vactive), and the
    // product of four such factors is below 2^64.
    let numerator =
        u64::from(canvas.0) * u64::from(h) * u64::from(hactive) * u64::from(face.height);
    let denominator = u64::from(canvas.1) * u64::from(vactive) * u64::from(face.width.max(1));
    div_round(numerator, denominator)
}

fn centred(mode: &Modeline, w: u32, h: u32) -> Rect {
    Rect {
        x: centre(mode.hactive, w),
        y: centre(mode.vactive, h),
        w,
        h,
    }
}

fn ui_filter(dest: Rect, canvas: (u16, u16)) -> Filter {
    let empty = dest.w == 0 || dest.h == 0;
    if empty || (dest.w >= u32::from(canvas.0) && dest.h >= u32::from(canvas.1)) {
        Filter::Nearest
    } else {
        Filter::SharpBilinear
    }
}

#[cfg(test)]
mod tests {
    use neptron_types::{TV_525_240P, TV_525_480I, TV_625_288P, TV_625_576I, TvStandard};

    use super::*;

    const TURNED: (u16, u16) = (240, 320);

    fn crt() -> MonitorKind {
        MonitorKind::Crt { ranges: Vec::new() }
    }

    fn panel() -> MonitorKind {
        MonitorKind::FlatPanel { modes: Vec::new() }
    }

    fn tv() -> MonitorKind {
        MonitorKind::Tv {
            standard: TvStandard::Ntsc,
        }
    }

    fn mode(hactive: u16, vactive: u16, interlace: bool) -> Modeline {
        Modeline {
            pclock_hz: 25_000_000,
            hactive,
            hsync_start: hactive.saturating_add(10),
            hsync_end: hactive.saturating_add(20),
            htotal: hactive.saturating_add(30),
            vactive,
            vsync_start: vactive.saturating_add(3),
            vsync_end: vactive.saturating_add(6),
            vtotal: vactive.saturating_add(20),
            interlace,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    fn placed(kind: &MonitorKind, mode: &Modeline) -> UiPlacement {
        ui_dest_turned(kind, mode, TURNED, Aspect::STANDARD_CRT)
    }

    fn at(x: i32, y: i32, w: u32, h: u32, filter: Filter) -> UiPlacement {
        UiPlacement {
            dest: Rect { x, y, w, h },
            filter,
        }
    }

    #[test]
    fn a_15_khz_crt_at_448i_shows_320_lines_one_to_one() {
        // The mode of a 320-line source at rung 3 on generic_15.
        let p = placed(&crt(), &mode(597, 448, true));
        assert_eq!(p, at(178, 64, 240, 320, Filter::Nearest));
    }

    #[test]
    fn a_15_khz_crt_at_240p_fits_the_canvas_into_its_lines_soft() {
        // 0.75 of a line per canvas column, at the shape of the canvas.
        let p = placed(&crt(), &mode(640, 240, false));
        assert_eq!(p, at(140, 0, 360, 240, Filter::SharpBilinear));
    }

    #[test]
    fn a_31_khz_crt_shows_320_lines_one_to_one_with_borders() {
        let p = placed(&crt(), &mode(643, 482, false));
        assert_eq!(p, at(201, 81, 240, 320, Filter::Nearest));
    }

    #[test]
    fn a_multi_sync_crt_at_640_lines_shows_the_canvas_at_2x() {
        let p = placed(&crt(), &mode(427, 640, false));
        assert_eq!(p, at(93, 0, 240, 640, Filter::Nearest));
    }

    #[test]
    fn a_super_width_crt_mode_keeps_the_shape_on_the_face() {
        // 1920 pixels span the face at 240 lines: 4.5 pixels per canvas
        // column across, 0.75 along.
        let p = placed(&crt(), &mode(1920, 240, false));
        assert_eq!(p, at(420, 0, 1080, 240, Filter::SharpBilinear));
    }

    #[test]
    fn a_wide_face_takes_fewer_pixels_for_the_same_shape() {
        let wide = Aspect {
            width: 16,
            height: 9,
        };
        let p = ui_dest_turned(&crt(), &mode(640, 480, false), TURNED, wide);
        // 240 x 320 on 320 of 480 lines: 3/4 x 2/3 x 640 x 9/16.
        assert_eq!(p, at(230, 80, 180, 320, Filter::SharpBilinear));
    }

    #[test]
    fn ntsc_480i_shows_240_samples_on_320_lines() {
        // The shape width is 264 samples; one whole multiple of 240 is
        // the nearest, 9 % narrower.
        let p = placed(&tv(), &TV_525_480I);
        assert_eq!(p, at(240, 80, 240, 320, Filter::Nearest));
    }

    #[test]
    fn pal_576i_shows_240_samples_on_320_lines() {
        let p = placed(&tv(), &TV_625_576I);
        assert_eq!(p, at(240, 128, 240, 320, Filter::Nearest));
    }

    #[test]
    fn a_tv_at_240p_or_288p_fits_the_lines_soft() {
        // The shape is 396 samples wide at 240 or 288 lines: two whole
        // multiples of 240 are the nearest.
        let p = placed(&tv(), &TV_525_240P);
        assert_eq!(p, at(120, 0, 480, 240, Filter::SharpBilinear));
        let p = placed(&tv(), &TV_625_288P);
        assert_eq!(p, at(120, 0, 480, 288, Filter::SharpBilinear));
    }

    #[test]
    fn a_tv_never_goes_beyond_the_safe_width() {
        let p = ui_dest_turned(&tv(), &TV_525_480I, (1000, 100), Aspect::STANDARD_CRT);
        assert_eq!(p.dest.w, u32::from(TV_SAFE_WIDTH_SAMPLES));
        assert_eq!(p.dest.x, 8);
        assert_eq!(p.filter, Filter::SharpBilinear);
        let p = ui_dest_turned(&tv(), &TV_525_480I, (300, 100), Aspect::STANDARD_CRT);
        assert_eq!(p.dest.w, 600, "two whole multiples fit the safe width");
    }

    #[test]
    fn flat_panels_take_the_largest_whole_scale_centred() {
        let p = placed(&panel(), &mode(1920, 1080, false));
        assert_eq!(p, at(600, 60, 720, 960, Filter::Nearest));
        let p = placed(&panel(), &mode(1280, 720, false));
        assert_eq!(p, at(400, 40, 480, 640, Filter::Nearest));
        let p = placed(&panel(), &mode(3840, 2160, false));
        assert_eq!(p, at(1200, 120, 1440, 1920, Filter::Nearest));
    }

    #[test]
    fn a_portrait_panel_mode_takes_the_scale_of_its_width() {
        let p = placed(&panel(), &mode(1080, 1920, false));
        assert_eq!(p, at(60, 320, 960, 1280, Filter::Nearest));
    }

    #[test]
    fn zero_sizes_give_no_ui() {
        for kind in [crt(), tv()] {
            for (m, canvas) in [
                (mode(640, 480, false), (0, 320)),
                (mode(640, 480, false), (240, 0)),
                (mode(0, 480, false), TURNED),
                (mode(640, 0, false), TURNED),
            ] {
                let p = ui_dest_turned(&kind, &m, canvas, Aspect::STANDARD_CRT);
                assert_eq!(p, at(0, 0, 0, 0, Filter::Nearest), "{kind:?} {canvas:?}");
            }
        }
        let p = ui_dest_turned(
            &panel(),
            &mode(1920, 1080, false),
            (0, 320),
            Aspect::STANDARD_CRT,
        );
        assert_eq!(p.dest.w, 0);
    }

    #[test]
    fn a_face_with_a_zero_side_still_gives_a_rectangle() {
        let flat = Aspect {
            width: 0,
            height: 0,
        };
        let p = ui_dest_turned(&crt(), &mode(640, 480, false), TURNED, flat);
        assert_eq!(
            p.dest,
            Rect {
                x: 319,
                y: 80,
                w: 1,
                h: 320
            }
        );
    }

    #[test]
    fn the_16_bit_extremes_do_not_overflow() {
        let huge = mode(u16::MAX, u16::MAX, false);
        let p = ui_dest_turned(&crt(), &huge, (u16::MAX, u16::MAX), Aspect::STANDARD_CRT);
        assert_eq!(p, at(8_192, 0, 49_151, 65_535, Filter::SharpBilinear));
        let p = ui_dest_turned(&crt(), &huge, (1, 1), Aspect::STANDARD_CRT);
        assert_eq!(p, at(8_192, 0, 49_151, 65_535, Filter::Nearest));
        // The safe width spans the face, so the shape is 3/4 of it.
        let p = ui_dest_turned(&tv(), &huge, (1, 1), Aspect::STANDARD_CRT);
        assert_eq!(p.dest.w, 528);
        let p = ui_dest_turned(&panel(), &huge, (1, 1), Aspect::STANDARD_CRT);
        assert_eq!(p.dest.w, 65_535);
        let tiny = mode(1, 1, false);
        let p = ui_dest_turned(&crt(), &tiny, (u16::MAX, u16::MAX), Aspect::STANDARD_CRT);
        assert_eq!(p, at(0, 0, 1, 1, Filter::SharpBilinear));
    }
}
