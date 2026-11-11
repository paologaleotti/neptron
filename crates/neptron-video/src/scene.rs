use neptron_modeline::policy::Placement;
use neptron_types::{Filter, Frame, Rect, Scanlines, Scene};

/// The one place where the engine builds a `Scene`: the run and the preview
/// both come here.
///
/// The crop, the destination and the filter come from the winner's
/// placement. A game frame of another size than the placement's source
/// (the frames that a new size shows while the debounce waits, or while no
/// mode fits it) is shown whole in the current destination, with
/// sharp-bilinear when the scale is not whole, so the renderer never reads
/// outside the frame. The UI goes to the placement's UI destination.
///
/// Every value of the scene that depends on the mode is set here, so a new
/// field of `Scene` (such as a scanline value) is set here too.
pub fn build_scene<'a>(
    placement: &Placement,
    game: Option<Frame<'a>>,
    ui: Option<Frame<'a>>,
) -> Scene<'a> {
    let (crop, filter) = match game {
        Some(frame) if !fits(&placement.crop, &frame) => {
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

fn fits(crop: &Rect, frame: &Frame<'_>) -> bool {
    crop.x == 0
        && crop.y == 0
        && crop.w == u32::from(frame.width)
        && crop.h == u32::from(frame.height)
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
    use neptron_types::PixelFormat;

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

    #[test]
    fn a_frame_of_the_placement_size_takes_the_placement() {
        let p = placement();
        let scene = build_scene(&p, Some(frame(256, 224)), None);
        assert_eq!(scene.crop, p.crop);
        assert_eq!(scene.dest, p.dest);
        assert_eq!(scene.filter, p.filter);
        assert_eq!(scene.ui_dest, p.ui_dest);
    }

    #[test]
    fn a_frame_of_another_size_is_shown_whole_in_the_destination() {
        let p = placement();
        let scene = build_scene(&p, Some(frame(320, 224)), None);
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
        let scene = build_scene(&p, Some(frame(512, 448)), None);
        assert_eq!(scene.filter, Filter::SharpBilinear);
    }

    #[test]
    fn a_scene_without_a_game_keeps_the_placement() {
        let p = placement();
        let scene = build_scene(&p, None, Some(frame(320, 240)));
        assert_eq!(scene.crop, p.crop);
        assert!(scene.game.is_none());
        assert_eq!(scene.ui.map(|u| u.width), Some(320));
    }
}
