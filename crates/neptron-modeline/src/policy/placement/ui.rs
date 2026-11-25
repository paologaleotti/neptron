use neptron_types::{Filter, Modeline, MonitorKind, Rect, ui_dest_fill, ui_dest_uniform};

use super::centre;
use crate::policy::TV_SAFE_WIDTH_SAMPLES;

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

/// Where the UI canvas goes in `mode` on a display of `kind`, and its
/// filter. `canvas` is the size of `T`, the canvas after its turn, in
/// scan-out pixels: width, then lines.
///
/// - A CRT: the full active area ([`ui_dest_fill`]). Each axis scales on
///   its own, as the tube shows it.
/// - A TV: the largest whole factor across that fits
///   [`TV_SAFE_WIDTH_SAMPLES`] and the largest whole factor down that fits
///   the active lines, each at least 1, centred. A fraction would give
///   columns or lines of uneven size, which shows on text. A 320x240
///   canvas is 640x240 at (40, 0) on 240p, at (40, 24) on 288p, 640x480 at
///   (40, 0) on 480i and at (40, 48) on 576i.
/// - A flat panel: the largest whole scale that fits both axes, centred
///   ([`ui_dest_uniform`]).
///
/// The filter is [`Filter::Nearest`] when each axis has a scale of 1 or
/// more, and [`Filter::SharpBilinear`] when an axis is below 1: nearest
/// would drop canvas lines or columns there, and a stroke of a glyph one
/// pixel wide would vanish.
///
/// The function decides no mode: it places the canvas in the mode it is
/// given.
pub fn ui_placement(kind: &MonitorKind, mode: &Modeline, canvas: (u16, u16)) -> UiPlacement {
    let (canvas_w, canvas_h) = canvas;
    let dest = match kind {
        MonitorKind::Crt { .. } => ui_dest_fill(mode.hactive, mode.vactive),
        MonitorKind::Tv { .. } => tv_dest(mode, canvas),
        MonitorKind::FlatPanel { .. } => {
            ui_dest_uniform(mode.hactive, mode.vactive, canvas_w, canvas_h)
        }
    };
    UiPlacement {
        dest,
        filter: ui_filter(dest, canvas),
    }
}

fn tv_dest(mode: &Modeline, (canvas_w, canvas_h): (u16, u16)) -> Rect {
    let across = (TV_SAFE_WIDTH_SAMPLES.min(mode.hactive) / canvas_w.max(1)).max(1);
    let down = (mode.vactive / canvas_h.max(1)).max(1);
    let w = u32::from(canvas_w) * u32::from(across);
    let h = u32::from(canvas_h) * u32::from(down);
    Rect {
        x: centre(mode.hactive, w),
        y: centre(mode.vactive, h),
        w,
        h,
    }
}

/// [`Filter::Nearest`] when `dest` scales each axis of `canvas` by 1 or
/// more, or shows nothing; else [`Filter::SharpBilinear`].
pub(super) fn ui_filter(dest: Rect, canvas: (u16, u16)) -> Filter {
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

    const LANDSCAPE: (u16, u16) = (320, 240);
    const PORTRAIT: (u16, u16) = (240, 320);

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

    fn mode(hactive: u16, vactive: u16) -> Modeline {
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
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    fn at(x: i32, y: i32, w: u32, h: u32, filter: Filter) -> UiPlacement {
        UiPlacement {
            dest: Rect { x, y, w, h },
            filter,
        }
    }

    #[test]
    fn a_crt_fills_the_active_area() {
        let p = ui_placement(&crt(), &mode(1_280, 240), LANDSCAPE);
        assert_eq!(p, at(0, 0, 1_280, 240, Filter::Nearest));
        let p = ui_placement(&crt(), &mode(640, 480), LANDSCAPE);
        assert_eq!(p, at(0, 0, 640, 480, Filter::Nearest));
    }

    /// A 224-line game mode: 0.93 of a line per canvas line, so nearest
    /// would drop 16 canvas lines.
    #[test]
    fn a_crt_mode_of_fewer_lines_than_the_canvas_is_soft() {
        let p = ui_placement(&crt(), &mode(1_280, 224), LANDSCAPE);
        assert_eq!(p, at(0, 0, 1_280, 224, Filter::SharpBilinear));
        let p = ui_placement(&crt(), &mode(256, 240), LANDSCAPE);
        assert_eq!(p.filter, Filter::SharpBilinear, "fewer samples than 320");
    }

    #[test]
    fn a_tv_takes_whole_factors_inside_the_safe_width() {
        let tv = tv();
        let cases = [
            (TV_525_240P, at(40, 0, 640, 240, Filter::Nearest)),
            (TV_625_288P, at(40, 24, 640, 240, Filter::Nearest)),
            (TV_525_480I, at(40, 0, 640, 480, Filter::Nearest)),
            (TV_625_576I, at(40, 48, 640, 480, Filter::Nearest)),
        ];
        for (m, want) in cases {
            assert_eq!(ui_placement(&tv, &m, LANDSCAPE), want, "{m:?}");
        }
    }

    #[test]
    fn a_tv_gives_at_least_one_factor_on_each_axis() {
        let p = ui_placement(&tv(), &mode(200, 100), LANDSCAPE);
        assert_eq!(p.dest.w, 320);
        assert_eq!(p.dest.h, 240);
        assert_eq!(p.filter, Filter::Nearest);
    }

    #[test]
    fn a_flat_panel_takes_the_largest_whole_scale_of_either_canvas() {
        let p = ui_placement(&panel(), &mode(1_920, 1_080), LANDSCAPE);
        assert_eq!(p, at(320, 60, 1_280, 960, Filter::Nearest));
        let p = ui_placement(&panel(), &mode(1_080, 1_920), PORTRAIT);
        assert_eq!(p, at(60, 320, 960, 1_280, Filter::Nearest));
        let p = ui_placement(&panel(), &mode(1_920, 1_080), PORTRAIT);
        assert_eq!(p, at(600, 60, 720, 960, Filter::Nearest));
    }

    #[test]
    fn an_empty_canvas_or_mode_shows_no_ui_with_nearest() {
        let p = ui_placement(&panel(), &mode(1_920, 1_080), (0, 320));
        assert_eq!((p.dest.w, p.filter), (0, Filter::Nearest));
        let p = ui_placement(&crt(), &mode(0, 240), LANDSCAPE);
        assert_eq!((p.dest.w, p.filter), (0, Filter::Nearest));
    }

    #[test]
    fn the_16_bit_extremes_do_not_overflow() {
        let huge = mode(u16::MAX, u16::MAX);
        for kind in [crt(), tv(), panel()] {
            let p = ui_placement(&kind, &huge, (u16::MAX, u16::MAX));
            assert!(p.dest.w > 0 && p.dest.h > 0, "{kind:?}");
            let p = ui_placement(&kind, &mode(1, 1), (u16::MAX, u16::MAX));
            assert!(p.dest.w <= u32::from(u16::MAX), "{kind:?}");
        }
    }
}
