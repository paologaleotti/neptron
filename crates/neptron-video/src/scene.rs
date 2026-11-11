use neptron_modeline::policy::Placement;
use neptron_types::{Filter, Frame, Rect, Scanlines, Scene, SourceGeometry};

/// The one place where the engine builds a `Scene`: the run and the preview
/// both come here.
///
/// The crop, the destination and the filter come from the winner's
/// placement, for `source`, the source the placement is for. The crop is
/// used as it is when the game frame has the size of that source and the
/// crop lies inside the frame: the policy can cut overscan lines so that a
/// whole multiple fits, and that picture stays pixel exact.
///
/// Otherwise the frame is shown whole in the current destination, with
/// sharp-bilinear when the scale is not whole. That covers a frame of
/// another size (a new size while the debounce waits, or while no mode
/// fits it) and a crop that reaches outside the frame, so the renderer
/// never reads outside the frame. The UI goes to the placement's UI
/// destination.
///
/// Every value of the scene that depends on the mode is set here, so a new
/// field of `Scene` (such as a scanline value) is set here too.
pub fn build_scene<'a>(
    placement: &Placement,
    source: &SourceGeometry,
    game: Option<Frame<'a>>,
    ui: Option<Frame<'a>>,
) -> Scene<'a> {
    let (crop, filter) = match game {
        Some(frame) if !crop_holds(&placement.crop, source, &frame) => {
            let crop = whole(&frame);
            (crop, filter_for(&crop, &placement.dest))
        }
        _ => (placement.crop, placement.filter),
    };
    Scene {
        game,
        crop,
        dest: placement.dest,
        filter,
        ui,
        ui_dest: placement.ui_dest,
        scanlines: Scanlines::NONE,
    }
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
    use neptron_types::{PixelFormat, Rotation};

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
        let scene = build_scene(&p, &SOURCE, Some(frame(256, 224)), None);
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
        let scene = build_scene(&p, &source, Some(frame(256, 240)), None);
        assert_eq!(scene.crop, p.crop);
        assert_eq!(scene.filter, Filter::Nearest);
        assert_eq!(scene.dest.h, 2 * scene.crop.h);
    }

    #[test]
    fn a_crop_partly_outside_the_frame_shows_the_whole_frame() {
        let (mut p, source) = cropped();
        p.crop.y = 24;
        let scene = build_scene(&p, &source, Some(frame(256, 240)), None);
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
        let scene = build_scene(&p, &source, Some(frame(256, 240)), None);
        assert_eq!(scene.crop.x, 0);
    }

    #[test]
    fn a_frame_of_another_size_is_shown_whole_in_the_destination() {
        let p = placement();
        let scene = build_scene(&p, &SOURCE, Some(frame(320, 224)), None);
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
        let scene = build_scene(&p, &SOURCE, Some(frame(512, 448)), None);
        assert_eq!(scene.filter, Filter::SharpBilinear);
    }

    #[test]
    fn a_larger_frame_that_holds_the_crop_is_still_another_size() {
        let (p, source) = cropped();
        let scene = build_scene(&p, &source, Some(frame(512, 448)), None);
        assert_eq!(scene.crop.w, 512);
        assert_eq!(scene.crop.h, 448);
    }

    #[test]
    fn a_scene_without_a_game_keeps_the_placement() {
        let p = placement();
        let scene = build_scene(&p, &SOURCE, None, Some(frame(320, 240)));
        assert_eq!(scene.crop, p.crop);
        assert!(scene.game.is_none());
        assert_eq!(scene.ui.map(|u| u.width), Some(320));
    }
}
