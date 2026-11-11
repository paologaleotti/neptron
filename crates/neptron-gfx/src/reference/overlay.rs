use neptron_types::{Filter, Frame, Rect};

use super::axis::AxisMap;
use super::scanline::FULL_PERCENT;
use super::{OutputBuffer, ReferenceRenderer, fill_taps, output_row, sample, visible};

impl ReferenceRenderer {
    /// Draws the whole canvas into `dest`, opaque, with nearest scaling.
    /// `dest` has sides up to [`MAX_RECT_SIDE_PX`](super::MAX_RECT_SIDE_PX).
    pub(super) fn draw_ui(&mut self, ui: &Frame<'_>, dest: Rect, out: &mut OutputBuffer<'_>) {
        let (ui_w, ui_h) = (u32::from(ui.width), u32::from(ui.height));
        if ui_w == 0 || ui_h == 0 || dest.w == 0 || dest.h == 0 {
            return;
        }
        let map = |len: u32, dest_len: u32| AxisMap {
            crop_start: 0,
            crop_len: len,
            dest_len,
            frame_len: len,
            filter: Filter::Nearest,
        };
        let (x0, x1) = visible(dest.x, dest.w, out.width);
        let (y0, y1) = visible(dest.y, dest.h, out.height);
        fill_taps(&mut self.columns, &map(ui_w, dest.w), x0, x1, dest.x);
        fill_taps(&mut self.lines, &map(ui_h, dest.h), y0, y1, dest.y);

        for (y, line) in (y0..y1).zip(&self.lines) {
            let Some(row) = output_row(out, y, x0, x1) else {
                continue;
            };
            for (px, column) in row.chunks_exact_mut(4).zip(&self.columns) {
                px.copy_from_slice(
                    &sample(ui, *column, *line, (1, 1), FULL_PERCENT).to_xrgb8888_le(),
                );
            }
        }
    }
}
