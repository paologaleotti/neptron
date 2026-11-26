use neptron_modeline::policy::{Placement, ui_placement, whole_multiple};
use neptron_types::{
    Filter, Form, Frame, GamePicture, Modeline, MonitorKind, Rect, Rotation, Scene, ScreenClass,
    SourceGeometry, scanlines_for,
};

use crate::ScanlineSettings;

/// What a scene is built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneInputs<'p> {
    /// The winner's placement.
    pub placement: &'p Placement,
    /// The source the placement is for.
    pub source: &'p SourceGeometry,
    /// The kind of the monitor: a CRT or a flat panel.
    pub monitor: &'p MonitorKind,
    /// The mode the scene shows in: the read back of the mode on screen,
    /// or the proposal of a preview.
    pub mode: &'p Modeline,
    /// The scanline settings.
    pub scanlines: ScanlineSettings,
    /// The turn of the UI canvas relative to the scan-out: the value of
    /// `Scene::ui_rotation`.
    pub ui_turn: Rotation,
    /// The form of the UI: its canvas is the size that the scene places
    /// when it has no canvas.
    pub form: Form,
    /// The internal resolution of the game picture: a whole number of
    /// picture pixels on each axis for one pixel of the source. 1 for a
    /// core that draws at its native size. A GL core that draws above it
    /// sends a picture `picture_scale` times the source.
    pub picture_scale: u16,
}

/// The one place where the engine builds a `Scene`: the run and the preview
/// both come here.
///
/// The crop, the destination and the filter come from the winner's
/// placement. The crop is used as it is when the game frame has the size
/// of the placement's source and the crop lies inside the frame: the policy
/// can cut overscan lines so that a whole multiple fits, and that picture
/// stays pixel exact.
///
/// The turn of the game picture is the rotation of `inputs.source`, the
/// source that the placement was planned for, so the placement and the
/// turn always agree.
///
/// Otherwise (a frame of another size while the debounce waits or while
/// no mode fits it, or a crop that reaches outside the frame) the frame
/// is shown turned inside the current destination, and the renderer never
/// reads outside the frame. A fit (no whole vertical scale in the
/// placement) shows the whole frame in the destination, with sharp
/// bilinear where the scale is not whole. A whole-step placement keeps
/// whole steps of lines: the largest whole multiple of the frame's lines
/// up to the planned one that fits, centred, with the width scaled by the
/// same step; a frame taller than the destination shows its middle lines
/// once each. Only the picture mode Fit scales lines by a fraction.
///
/// The UI canvas turns by `inputs.ui_turn`, and goes where
/// `neptron_modeline::policy::ui_placement` places the canvas after that
/// turn, of the size of its frame, in the mode of the scene, with the
/// filter of that function at every turn: nearest when each axis scales
/// by 1 or more, sharp bilinear when an axis is below 1.
///
/// The scanlines come from `neptron_types::scanlines_for` with the
/// settings, the whole vertical multiple from the lines of the turned
/// crop to the destination of this scene, the screen class of the mode on the monitor, and its
/// interlace flag.
///
/// A picture at an internal resolution `k` (`inputs.picture_scale`) is
/// `k` times its source on each axis. Every rule above runs on the native
/// size, the picture size over `k`, and the crop is then `k` times the
/// native crop: the placement and the scanlines see the native lines. The
/// filter is nearest only when the destination is a whole multiple of the
/// scaled crop on both axes. A picture whose sides are not multiples of
/// `k` counts as native.
pub fn build_scene<'a>(
    inputs: &SceneInputs<'_>,
    game: Option<GamePicture<'a>>,
    ui: Option<Frame<'a>>,
) -> Scene<'a> {
    let placement = inputs.placement;
    let rotation = inputs.source.rotation;
    let k = game.map_or(1, |g| {
        picture_scale(g.width(), g.height(), inputs.picture_scale)
    });
    let native = game.map(|g| (g.width() / k, g.height() / k));
    let (native_crop, dest, filter) = match native {
        Some(size) if !crop_holds(&placement.crop, inputs.source, size) => {
            other_frame(placement, rotation, size)
        }
        _ => (placement.crop, placement.dest, placement.filter),
    };
    let (_, turned_lines) = turned_size(&native_crop, rotation);
    let (crop, filter) = if k == 1 {
        (native_crop, filter)
    } else {
        let crop = scale_rect(&native_crop, k);
        let scaled = match filter_for(turned_size(&crop, rotation), &dest) {
            Filter::Nearest => filter,
            other => other,
        };
        (crop, scaled)
    };
    let scanlines = scanlines_for(
        inputs.scanlines.mode,
        inputs.scanlines.strength_percent,
        whole_multiple(turned_lines, dest.h),
        ScreenClass::of(inputs.monitor, inputs.mode),
        inputs.mode.interlace,
    );
    let (ui_dest, ui_filter) = ui_place(inputs, ui.as_ref());
    Scene {
        game,
        crop,
        dest,
        rotation,
        filter,
        ui,
        ui_dest,
        ui_rotation: inputs.ui_turn,
        ui_filter,
        scanlines,
    }
}

/// The destination and the filter of the UI canvas `ui` (the canvas of
/// the form when the scene has none) after the turn of the inputs.
fn ui_place(inputs: &SceneInputs<'_>, ui: Option<&Frame<'_>>) -> (Rect, Filter) {
    let (width, height) = ui.map_or(inputs.form.canvas(), |f| (f.width, f.height));
    let turned = if inputs.ui_turn.across() {
        (height, width)
    } else {
        (width, height)
    };
    let placed = ui_placement(inputs.monitor, inputs.mode, turned);
    (placed.dest, placed.filter)
}

/// The scale `k` of a picture of `width` by `height` at the internal
/// resolution `scale`: `scale` when both sides are multiples of it, else 1.
fn picture_scale(width: u16, height: u16, scale: u16) -> u16 {
    if scale > 1 && width.is_multiple_of(scale) && height.is_multiple_of(scale) {
        scale
    } else {
        1
    }
}

/// `rect` with every value `k` times larger.
fn scale_rect(rect: &Rect, k: u16) -> Rect {
    Rect {
        x: rect.x * i32::from(k),
        y: rect.y * i32::from(k),
        w: rect.w * u32::from(k),
        h: rect.h * u32::from(k),
    }
}

/// The frame of `(width, height)` has the size of the placement's source,
/// and the crop lies inside it.
fn crop_holds(crop: &Rect, source: &SourceGeometry, (frame_w, frame_h): (u16, u16)) -> bool {
    let (width, height) = (i64::from(frame_w), i64::from(frame_h));
    let same_size = frame_w == source.width && frame_h == source.height;
    let (x, y) = (i64::from(crop.x), i64::from(crop.y));
    same_size
        && x >= 0
        && y >= 0
        && crop.w > 0
        && crop.h > 0
        && x + i64::from(crop.w) <= width
        && y + i64::from(crop.h) <= height
}

/// The crop, the destination and the filter of a frame that the
/// placement does not fit: see [`build_scene`].
fn other_frame(
    placement: &Placement,
    rotation: Rotation,
    frame: (u16, u16),
) -> (Rect, Rect, Filter) {
    let all = whole(frame);
    let dest = placement.dest;
    let Some(planned_step) = placement.scale.y.filter(|&k| k > 0) else {
        return (all, dest, filter_for(turned_size(&all, rotation), &dest));
    };
    let (columns, lines) = turned_size(&all, rotation);
    let (planned_columns, _) = turned_size(&placement.crop, rotation);
    let step = (dest.h / lines.max(1)).min(u32::from(planned_step));
    let (crop, step, h) = if step == 0 {
        (middle_lines(all, rotation, lines - dest.h), 1, dest.h)
    } else {
        (all, step, lines * step)
    };
    // The width keeps the planned columns per game column, less the
    // vertical steps the frame does not get.
    let w = u64::from(columns) * u64::from(dest.w) * u64::from(step)
        / (u64::from(planned_columns.max(1)) * u64::from(planned_step));
    let w = u32::try_from(w).unwrap_or(dest.w).clamp(1, dest.w);
    let placed = Rect {
        x: dest.x + centre(dest.w, w),
        y: dest.y + centre(dest.h, h),
        w,
        h,
    };
    (
        crop,
        placed,
        filter_for(turned_size(&crop, rotation), &placed),
    )
}

/// `all` less `cut` lines of the turned picture, half at each edge.
fn middle_lines(all: Rect, rotation: Rotation, cut: u32) -> Rect {
    let first = i32::try_from(cut / 2).unwrap_or(0);
    match rotation {
        Rotation::R0 | Rotation::R180 => Rect {
            y: first,
            h: all.h - cut,
            ..all
        },
        Rotation::R90 | Rotation::R270 => Rect {
            x: first,
            w: all.w - cut,
            ..all
        },
    }
}

/// The offset that centres `inner` in `outer`.
fn centre(outer: u32, inner: u32) -> i32 {
    i32::try_from(outer.saturating_sub(inner) / 2).unwrap_or(0)
}

fn whole((width, height): (u16, u16)) -> Rect {
    Rect {
        x: 0,
        y: 0,
        w: u32::from(width),
        h: u32::from(height),
    }
}

/// The columns and the lines of `crop` after the turn of `rotation`.
fn turned_size(crop: &Rect, rotation: Rotation) -> (u32, u32) {
    match rotation {
        Rotation::R0 | Rotation::R180 => (crop.w, crop.h),
        Rotation::R90 | Rotation::R270 => (crop.h, crop.w),
    }
}

fn filter_for((columns, lines): (u32, u32), dest: &Rect) -> Filter {
    let exact = |from: u32, to: u32| from > 0 && to.is_multiple_of(from);
    if exact(columns, dest.w) && exact(lines, dest.h) {
        Filter::Nearest
    } else {
        Filter::SharpBilinear
    }
}

#[cfg(test)]
mod tests {
    use neptron_modeline::policy::ScaleClaim;
    use neptron_types::{
        FrameOrigin, GpuFrame, GpuImageId, PixelFormat, ScanlineKind, ScanlineMode, Scanlines,
    };

    use super::*;

    static PIXELS: [u8; 512 * 448 * 2] = [0; 512 * 448 * 2];

    fn frame(width: u16, height: u16) -> Frame<'static> {
        Frame {
            data: &PIXELS,
            width,
            height,
            pitch: usize::from(width) * 2,
            format: PixelFormat::Rgb565,
        }
    }

    fn placement() -> Placement {
        Placement {
            crop: Rect {
                x: 0,
                y: 0,
                w: 256,
                h: 224,
            },
            dest: Rect {
                x: 0,
                y: 8,
                w: 1_280,
                h: 224,
            },
            filter: Filter::Nearest,
            scale: ScaleClaim {
                x: Some(5),
                y: Some(1),
            },
        }
    }

    /// 1280x448p at 31.5 kHz on a CRT.
    const MODE_448P: Modeline = Modeline {
        pclock_hz: 40_320_000,
        hactive: 1_280,
        hsync_start: 1_312,
        hsync_end: 1_408,
        htotal: 1_280,
        vactive: 448,
        vsync_start: 450,
        vsync_end: 453,
        vtotal: 525,
        interlace: false,
        doublescan: false,
        hsync_positive: false,
        vsync_positive: false,
    };

    fn crt() -> MonitorKind {
        MonitorKind::Crt { ranges: Vec::new() }
    }

    fn build<'a>(
        p: &Placement,
        source: &SourceGeometry,
        game: Option<Frame<'a>>,
        ui: Option<Frame<'a>>,
    ) -> Scene<'a> {
        build_with(p, source, &MODE_448P, ScanlineMode::On, game, ui)
    }

    fn build_with<'a>(
        p: &Placement,
        source: &SourceGeometry,
        mode: &Modeline,
        scanline_mode: ScanlineMode,
        game: Option<Frame<'a>>,
        ui: Option<Frame<'a>>,
    ) -> Scene<'a> {
        let monitor = crt();
        let inputs = SceneInputs {
            placement: p,
            source,
            monitor: &monitor,
            mode,
            scanlines: ScanlineSettings {
                mode: scanline_mode,
                strength_percent: 75,
            },
            ui_turn: Rotation::R0,
            form: Form::Landscape,
            picture_scale: 1,
        };
        build_scene(&inputs, game.map(GamePicture::Memory), ui)
    }

    /// The scene of `p` with the canvas turned by `ui_turn` in `mode` on
    /// `monitor`, with the portrait form across and the landscape form
    /// else.
    fn build_turned<'a>(
        monitor: &MonitorKind,
        mode: &Modeline,
        ui_turn: Rotation,
        ui: Option<Frame<'a>>,
    ) -> Scene<'a> {
        let p = placement();
        let inputs = SceneInputs {
            placement: &p,
            source: &SOURCE,
            monitor,
            mode,
            scanlines: ScanlineSettings::OFF,
            ui_turn,
            form: if ui_turn.across() {
                Form::Portrait
            } else {
                Form::Landscape
            },
            picture_scale: 1,
        };
        build_scene(&inputs, Some(GamePicture::Memory(frame(256, 224))), ui)
    }

    fn mode(hactive: u16, vactive: u16, interlace: bool) -> Modeline {
        Modeline {
            hactive,
            vactive,
            interlace,
            ..MODE_448P
        }
    }

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    /// The portrait canvas turned across the lines is 320x240 in scan-out:
    /// it fills the active area of a CRT mode, as the landscape canvas
    /// does on a screen at 0.
    #[test]
    fn a_portrait_canvas_turned_across_a_crt_mode_fills_it() {
        for turn in [Rotation::R90, Rotation::R270] {
            for m in [mode(640, 480, true), mode(640, 240, false)] {
                let scene = build_turned(&crt(), &m, turn, Some(frame(240, 320)));
                assert_eq!(scene.ui_rotation, turn);
                let full = rect(0, 0, 640, u32::from(m.vactive));
                assert_eq!(scene.ui_dest, full);
                assert_eq!(scene.ui_filter, Filter::Nearest);
            }
        }
    }

    /// A landscape canvas turned across 240 lines puts 320 canvas columns
    /// on 240 lines, below a scale of 1: sharp bilinear, so no column of
    /// a glyph is dropped. The placement follows the size of the frame.
    #[test]
    fn a_landscape_canvas_turned_across_240_lines_is_soft() {
        let scene = build_turned(
            &crt(),
            &mode(640, 240, false),
            Rotation::R90,
            Some(frame(320, 240)),
        );
        assert_eq!(scene.ui_dest, rect(0, 0, 640, 240));
        assert_eq!(scene.ui_filter, Filter::SharpBilinear);
    }

    /// With no canvas the scene places the canvas of the form: the
    /// portrait canvas turned is 320x240, 4x in 1080p.
    #[test]
    fn a_canvas_turned_on_a_flat_panel_takes_the_largest_whole_scale() {
        let panel = MonitorKind::FlatPanel {
            modes: vec![mode(1_920, 1_080, false)],
        };
        let scene = build_turned(&panel, &mode(1_920, 1_080, false), Rotation::R270, None);
        assert_eq!(scene.ui_dest, rect(320, 60, 1_280, 960));
        assert_eq!(scene.ui_filter, Filter::Nearest);
    }

    #[test]
    fn a_canvas_upside_down_keeps_the_destination_of_the_placement() {
        let scene = build_turned(&crt(), &MODE_448P, Rotation::R180, Some(frame(320, 240)));
        assert_eq!(scene.ui_rotation, Rotation::R180);
        assert_eq!(scene.ui_dest, rect(0, 0, 1_280, 448), "the active area");
        assert_eq!(scene.ui_filter, Filter::Nearest);
    }

    #[test]
    fn a_whole_multiple_of_two_draws_the_scanlines_of_the_settings() {
        let (p, source) = cropped();
        let scene = build(&p, &source, Some(frame(256, 240)), None);
        assert_eq!(
            scene.scanlines,
            Scanlines {
                kind: ScanlineKind::Black,
                strength_percent: 75
            }
        );
        let off = build_with(&p, &source, &MODE_448P, ScanlineMode::Off, None, None);
        assert_eq!(off.scanlines, Scanlines::NONE);
    }

    #[test]
    fn an_interlaced_mode_draws_no_scanlines() {
        let (p, source) = cropped();
        let interlaced = Modeline {
            interlace: true,
            ..MODE_448P
        };
        let scene = build_with(&p, &source, &interlaced, ScanlineMode::On, None, None);
        assert_eq!(scene.scanlines, Scanlines::NONE);
    }

    #[test]
    fn a_fractional_or_single_scale_draws_no_scanlines() {
        let (p, _) = cropped();
        let fallback = build(&p, &SOURCE, Some(frame(256, 240)), None);
        assert_eq!(fallback.crop.h, 240, "448 lines over 240: fractional");
        assert_eq!(fallback.scanlines, Scanlines::NONE);
        let single = build(&placement(), &SOURCE, Some(frame(256, 224)), None);
        assert_eq!(single.dest.h, single.crop.h);
        assert_eq!(single.scanlines, Scanlines::NONE);
    }

    const SOURCE: SourceGeometry = SourceGeometry {
        width: 256,
        height: 224,
        refresh_mhz: 60_099,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    };

    #[test]
    fn a_frame_of_the_placement_size_takes_the_placement() {
        let p = placement();
        let scene = build(&p, &SOURCE, Some(frame(256, 224)), None);
        assert_eq!(scene.crop, p.crop);
        assert_eq!(scene.dest, p.dest);
        assert_eq!(scene.filter, p.filter);
        assert_eq!(scene.ui_dest, rect(0, 0, 1_280, 448));
    }

    #[test]
    fn the_ui_canvas_is_upright_with_nearest() {
        let p = placement();
        for rotation in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            let source = SourceGeometry { rotation, ..SOURCE };
            let scene = build(&p, &source, Some(frame(256, 224)), Some(frame(320, 240)));
            assert_eq!(scene.rotation, rotation);
            assert_eq!(
                (scene.ui_rotation, scene.ui_filter),
                (Rotation::R0, Filter::Nearest),
                "{rotation:?}"
            );
        }
    }

    /// 256x240 with 8 overscan lines cut at the top and the bottom: 224
    /// lines at 2x in a 448-line mode.
    fn cropped() -> (Placement, SourceGeometry) {
        let p = Placement {
            crop: Rect {
                x: 0,
                y: 8,
                w: 256,
                h: 224,
            },
            dest: Rect {
                x: 0,
                y: 0,
                w: 1_280,
                h: 448,
            },
            scale: ScaleClaim {
                x: Some(5),
                y: Some(2),
            },
            ..placement()
        };
        let source = SourceGeometry {
            height: 240,
            ..SOURCE
        };
        (p, source)
    }

    #[test]
    fn a_crop_inside_the_frame_is_kept_pixel_exact() {
        let (p, source) = cropped();
        let scene = build(&p, &source, Some(frame(256, 240)), None);
        assert_eq!(scene.crop, p.crop);
        assert_eq!(scene.filter, Filter::Nearest);
        assert_eq!(scene.dest.h, 2 * scene.crop.h);
    }

    /// The scene of a GPU picture of `size` at the internal resolution
    /// `k`, with scanlines on.
    fn build_gpu(
        p: &Placement,
        source: &SourceGeometry,
        size: (u16, u16),
        k: u16,
    ) -> Scene<'static> {
        let monitor = crt();
        let inputs = SceneInputs {
            placement: p,
            source,
            monitor: &monitor,
            mode: &MODE_448P,
            scanlines: ScanlineSettings {
                mode: ScanlineMode::On,
                strength_percent: 75,
            },
            ui_turn: Rotation::R0,
            form: Form::Landscape,
            picture_scale: k,
        };
        let gpu = GpuFrame {
            image: GpuImageId(1),
            width: size.0,
            height: size.1,
            storage_width: 1_024,
            storage_height: 1_024,
            origin: FrameOrigin::BottomLeft,
        };
        build_scene(&inputs, Some(GamePicture::Gpu(gpu)), None)
    }

    #[test]
    fn a_picture_at_an_internal_resolution_takes_the_native_placement_times_k() {
        let (p, source) = cropped();
        let native = build(&p, &source, Some(frame(256, 240)), None);
        for k in [2_u16, 4] {
            let scene = build_gpu(&p, &source, (256 * k, 240 * k), k);
            let k32 = u32::from(k);
            assert_eq!(
                scene.crop,
                rect(0, 8 * i32::from(k), 256 * k32, 224 * k32),
                "{k}x"
            );
            assert_eq!(
                scene.dest, native.dest,
                "the destination of the native lines"
            );
            assert_eq!(
                scene.scanlines, native.scanlines,
                "the scanlines follow the native lines"
            );
            assert_eq!(
                scene.filter,
                Filter::SharpBilinear,
                "{k}x columns do not divide the destination"
            );
            assert!(matches!(scene.game, Some(GamePicture::Gpu(g)) if g.width == 256 * k));
        }
    }

    #[test]
    fn a_picture_that_is_a_whole_step_of_the_scaled_crop_keeps_nearest() {
        let p = Placement {
            crop: rect(0, 0, 256, 224),
            dest: rect(0, 0, 512, 448),
            scale: ScaleClaim {
                x: Some(2),
                y: Some(2),
            },
            ..placement()
        };
        let scene = build_gpu(&p, &SOURCE, (512, 448), 2);
        assert_eq!(scene.crop, rect(0, 0, 512, 448));
        assert_eq!(scene.filter, Filter::Nearest);
    }

    #[test]
    fn a_picture_whose_sides_are_not_multiples_of_k_counts_as_native() {
        let (p, source) = cropped();
        let scene = build_gpu(&p, &source, (255, 240), 2);
        assert_eq!(
            scene.crop,
            rect(0, 0, 255, 240),
            "another size: shown whole"
        );
    }

    #[test]
    fn a_crop_partly_outside_the_frame_shows_the_whole_frame() {
        let (mut p, source) = cropped();
        p.crop.y = 24;
        let scene = build(&p, &source, Some(frame(256, 240)), None);
        assert_eq!(
            scene.crop,
            Rect {
                x: 0,
                y: 0,
                w: 256,
                h: 240
            }
        );
        assert_eq!(scene.dest, rect(320, 104, 640, 240), "1x, centred");
        assert_eq!(
            scene.dest.h, scene.crop.h,
            "whole steps of lines stay whole"
        );
        p.crop = Rect {
            x: -1,
            ..cropped().0.crop
        };
        let scene = build(&p, &source, Some(frame(256, 240)), None);
        assert_eq!(scene.crop.x, 0);
    }

    #[test]
    fn a_frame_of_another_size_is_shown_whole_in_the_destination() {
        let p = placement();
        let scene = build(&p, &SOURCE, Some(frame(320, 224)), None);
        assert_eq!(
            scene.crop,
            Rect {
                x: 0,
                y: 0,
                w: 320,
                h: 224
            }
        );
        assert_eq!(scene.dest, p.dest);
        assert_eq!(scene.filter, Filter::Nearest, "1280 is 4 times 320");
    }

    /// A frame taller than a whole-step destination shows its middle
    /// lines once each, not its lines pressed by a fraction.
    #[test]
    fn a_taller_frame_in_whole_steps_shows_its_middle_lines() {
        let p = placement();
        let scene = build(&p, &SOURCE, Some(frame(512, 448)), None);
        assert_eq!(scene.crop, rect(0, 112, 512, 224));
        assert_eq!(scene.dest, p.dest);
        assert_eq!(scene.dest.h, scene.crop.h);
    }

    /// A fit keeps the whole frame in its destination, smoothed.
    #[test]
    fn a_frame_of_another_size_in_a_fit_is_shown_whole_and_smoothed() {
        let p = Placement {
            filter: Filter::SharpBilinear,
            scale: ScaleClaim { x: None, y: None },
            ..placement()
        };
        let scene = build(&p, &SOURCE, Some(frame(512, 448)), None);
        assert_eq!(scene.crop, rect(0, 0, 512, 448));
        assert_eq!(scene.dest, p.dest);
        assert_eq!(scene.filter, Filter::SharpBilinear);
    }

    #[test]
    fn a_larger_frame_that_holds_the_crop_is_still_another_size() {
        let (p, source) = cropped();
        let scene = build(&p, &source, Some(frame(512, 448)), None);
        assert_eq!(scene.crop.w, 512);
        assert_eq!(scene.crop.h, 448);
    }

    #[test]
    fn a_scene_without_a_game_keeps_the_placement() {
        let p = placement();
        let scene = build(&p, &SOURCE, None, Some(frame(320, 240)));
        assert_eq!(scene.crop, p.crop);
        assert!(scene.game.is_none());
        assert_eq!(scene.ui.map(|u| u.width), Some(320));
    }

    /// DoDonPachi: a 320x240 frame on its side, upright 240x320, at 2x in
    /// 480x640.
    fn turned() -> (Placement, SourceGeometry) {
        let p = Placement {
            crop: Rect {
                x: 0,
                y: 0,
                w: 320,
                h: 240,
            },
            dest: Rect {
                x: 0,
                y: 0,
                w: 480,
                h: 640,
            },
            scale: ScaleClaim {
                x: Some(2),
                y: Some(2),
            },
            ..placement()
        };
        let source = SourceGeometry {
            width: 320,
            height: 240,
            rotation: Rotation::R90,
            ..SOURCE
        };
        (p, source)
    }

    #[test]
    fn the_scene_takes_the_turn_of_the_planned_source() {
        let (p, source) = turned();
        for rotation in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            let source = SourceGeometry { rotation, ..source };
            let scene = build(&p, &source, Some(frame(320, 240)), None);
            assert_eq!(scene.rotation, rotation);
            assert_eq!(scene.crop, p.crop, "the crop stays in frame pixels");
            assert_eq!(scene.dest, p.dest);
        }
    }

    /// 640 lines over the 320 lines of the turned picture are 2x, so the
    /// scanlines show; the 240 lines of the frame would give 2.67.
    #[test]
    fn the_scanlines_count_the_lines_of_the_turned_picture() {
        let (p, source) = turned();
        let scene = build(&p, &source, Some(frame(320, 240)), None);
        assert_eq!(scene.scanlines.kind, ScanlineKind::Black);
        let flat = SourceGeometry {
            rotation: Rotation::R180,
            ..source
        };
        let scene = build(&p, &flat, Some(frame(320, 240)), None);
        assert_eq!(scene.scanlines, Scanlines::NONE);
    }

    /// A frame of another size while the debounce waits is shown whole and
    /// turned, in whole steps of the lines of the turned picture.
    #[test]
    fn a_turned_frame_of_another_size_keeps_whole_steps_of_the_turn() {
        let (p, source) = turned();
        // 160 lines at 2x in 640, 240 columns at 2x.
        let scene = build(&p, &source, Some(frame(160, 240)), None);
        assert_eq!(scene.rotation, Rotation::R90);
        assert_eq!(scene.dest, rect(0, 160, 480, 320));
        assert_eq!(scene.filter, Filter::Nearest);
        // 240 lines at 2x in 640, 160 columns at 2x.
        let scene = build(&p, &source, Some(frame(240, 160)), None);
        assert_eq!(scene.dest, rect(80, 80, 320, 480));
        assert_eq!(scene.filter, Filter::Nearest);
        // 400 lines are taller than 640 at 2x: 1x, centred.
        let scene = build(&p, &source, Some(frame(400, 240)), None);
        assert_eq!(scene.dest, rect(120, 120, 240, 400));
        // 800 lines: the middle 640, once each.
        let scene = build(&p, &source, Some(frame(800, 240)), None);
        assert_eq!(scene.crop, rect(80, 0, 640, 240));
        assert_eq!(scene.dest, rect(120, 0, 240, 640));
    }
}
