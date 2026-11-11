use neptron_types::Frame;

use super::{OutputBuffer, output_row, pixel, visible};

/// Where the UI canvas goes in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) scale: u32,
    pub(crate) x: i64,
    pub(crate) y: i64,
}

/// The largest integer scale at which the canvas fits the output, at least
/// 1, centred. The offset rounds toward the top left.
pub(crate) fn placement(ui_w: u32, ui_h: u32, out_w: u32, out_h: u32) -> Placement {
    let scale = (out_w / ui_w.max(1)).min(out_h / ui_h.max(1)).max(1);
    let offset =
        |out: u32, ui: u32| (i64::from(out) - i64::from(ui) * i64::from(scale)).div_euclid(2);
    Placement {
        scale,
        x: offset(out_w, ui_w),
        y: offset(out_h, ui_h),
    }
}

/// Draws the canvas over the output, opaque, with nearest scaling.
pub(crate) fn draw(ui: &Frame<'_>, out: &mut OutputBuffer<'_>) {
    let (ui_w, ui_h) = (u32::from(ui.width), u32::from(ui.height));
    if ui_w == 0 || ui_h == 0 {
        return;
    }
    let at = placement(ui_w, ui_h, out.width, out.height);
    let (x0, x1) = span(at.x, ui_w * at.scale, out.width);
    let (y0, y1) = span(at.y, ui_h * at.scale, out.height);
    for y in y0..y1 {
        let v = ((i64::from(y) - at.y) / i64::from(at.scale)) as u32;
        let Some(row) = output_row(out, y, x0, x1) else {
            continue;
        };
        for (x, px) in (x0..x1).zip(row.chunks_exact_mut(4)) {
            let u = ((i64::from(x) - at.x) / i64::from(at.scale)) as u32;
            px.copy_from_slice(&pixel::read(ui, u, v).to_xrgb8888_le());
        }
    }
}

/// [`visible`] for an offset that can be outside the `i32` range.
fn span(pos: i64, len: u32, out_len: u32) -> (u32, u32) {
    let pos = pos.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    visible(pos, len, out_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_on_its_own_size_is_one_to_one() {
        assert_eq!(
            placement(320, 240, 320, 240),
            Placement {
                scale: 1,
                x: 0,
                y: 0
            }
        );
    }

    #[test]
    fn canvas_takes_the_largest_integer_scale_and_centres() {
        assert_eq!(
            placement(320, 240, 640, 480),
            Placement {
                scale: 2,
                x: 0,
                y: 0
            }
        );
        assert_eq!(
            placement(320, 240, 1920, 1080),
            Placement {
                scale: 4,
                x: 320,
                y: 60
            }
        );
        assert_eq!(
            placement(320, 240, 720, 480),
            Placement {
                scale: 2,
                x: 40,
                y: 0
            }
        );
    }

    #[test]
    fn a_wide_mode_keeps_the_scale_of_its_height() {
        assert_eq!(
            placement(320, 240, 1920, 240),
            Placement {
                scale: 1,
                x: 800,
                y: 0
            }
        );
    }

    #[test]
    fn an_output_smaller_than_the_canvas_crops_it_around_the_centre() {
        assert_eq!(
            placement(320, 240, 256, 224),
            Placement {
                scale: 1,
                x: -32,
                y: -8
            }
        );
        assert_eq!(
            placement(320, 240, 255, 240),
            Placement {
                scale: 1,
                x: -33,
                y: 0
            }
        );
    }

    #[test]
    fn an_odd_margin_rounds_toward_the_top_left() {
        assert_eq!(
            placement(2, 2, 5, 3),
            Placement {
                scale: 1,
                x: 1,
                y: 0
            }
        );
    }
}
