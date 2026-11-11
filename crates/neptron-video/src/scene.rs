use neptron_modeline::policy::Placement;
use neptron_types::{
    Filter, Frame, Modeline, MonitorKind, Rect, Scene, ScreenClass, SourceGeometry, scanlines_for,
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
/// Otherwise the frame is shown whole in the current destination, with
/// sharp-bilinear when the scale is not whole. That covers a frame of
/// another size (a new size while the debounce waits, or while no mode
/// fits it) and a crop that reaches outside the frame, so the renderer
/// never reads outside the frame. The UI goes to the placement's UI
/// destination.
///
/// The scanlines come from `neptron_types::scanlines_for` with the
/// settings, the whole vertical multiple from the crop to the destination
/// of this scene, the screen class of the mode on the monitor, and its
/// interlace flag.
pub fn build_scene<'a>(
    inputs: &SceneInputs<'_>,
    game: Option<Frame<'a>>,
    ui: Option<Frame<'a>>,
) -> Scene<'a> {
    let placement = inputs.placement;
    let (crop, filter) = match game {
        Some(frame) if !crop_holds(&placement.crop, inputs.source, &frame) => {
            let crop = whole(&frame);
            (crop, filter_for(&crop, &placement.dest))
        }
        _ => (placement.crop, placement.filter),
    };
    let scanlines = scanlines_for(
        inputs.scanlines.mode,
        inputs.scanlines.strength_percent,
        whole_multiple(crop.h, placement.dest.h),
        ScreenClass::of(inputs.monitor, inputs.mode),
        inputs.mode.interlace,
    );
    Scene {
        game,
        crop,
        dest: placement.dest,
        filter,
        ui,
        ui_dest: placement.ui_dest,
        scanlines,
    }
}

/// `to / from` when it is a whole number, as the renderer's scanline rule
/// reads the scale.
fn whole_multiple(from: u32, to: u32) -> Option<u16> {
    (from > 0 && to.is_multiple_of(from))
        .then(|| u16::try_from(to / from).ok())
        .flatten()
}

/// The frame has the size of the placement's source, and the crop lies
/// inside it.
fn crop_holds(crop: &Rect, source: &SourceGeometry, frame: &Frame<'_>) -> bool {
    let (width, height) = (i64::from(frame.width), i64::from(frame.height));
    let same_size = frame.width == source.width && frame.height == source.height;
    let (x, y) = (i64::from(crop.x), i64::from(crop.y));
    same_size
        && x >= 0
        && y >= 0
        && crop.w > 0
        && crop.h > 0
        && x + i64::from(crop.w) <= width
        && y + i64::from(crop.h) <= height
}

fn whole(frame: &Frame<'_>) -> Rect {
    Rect {
        x: 0,
        y: 0,
        w: u32::from(frame.width),
        h: u32::from(frame.height),
    }
}

fn filter_for(crop: &Rect, dest: &Rect) -> Filter {
    let exact = |from: u32, to: u32| from > 0 && to.is_multiple_of(from);
    if exact(crop.w, dest.w) && exact(crop.h, dest.h) {
        Filter::Nearest
    } else {
        Filter::SharpBilinear
    }
}

#[cfg(test)]
mod tests {
    use neptron_modeline::policy::ScaleClaim;
    use neptron_types::{PixelFormat, Rotation, ScanlineKind, ScanlineMode, Scanlines};

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
            ui_dest: Rect {
                x: 0,
                y: 0,
                w: 1_280,
                h: 240,
            },
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
        };
        build_scene(&inputs, game, ui)
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
        assert_eq!(scene.ui_dest, p.ui_dest);
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
        assert_eq!(scene.dest, p.dest);
        assert_eq!(
            scene.filter,
            Filter::SharpBilinear,
            "448 is not a multiple of 240"
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
        let scene = build(&p, &SOURCE, Some(frame(512, 448)), None);
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
}
