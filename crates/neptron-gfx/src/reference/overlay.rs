use neptron_types::{Frame, Rect, Scene};

use super::scanline::FULL_PERCENT;
use super::{
    OutputBuffer, ReferenceRenderer, Turn, fill_taps, output_row, sample, to_frame, visible,
};

impl ReferenceRenderer {
    /// Draws the whole canvas into `scene.ui_dest`, opaque, turned by
    /// `scene.ui_rotation` and scaled with `scene.ui_filter`. `ui_dest` has
    /// sides up to [`MAX_RECT_SIDE_PX`](super::MAX_RECT_SIDE_PX).
    pub(super) fn draw_ui(
        &mut self,
        ui: &Frame<'_>,
        scene: &Scene<'_>,
        out: &mut OutputBuffer<'_>,
    ) {
        let dest = scene.ui_dest;
        if ui.width == 0 || ui.height == 0 || dest.w == 0 || dest.h == 0 {
            return;
        }
        let whole = Rect {
            x: 0,
            y: 0,
            w: u32::from(ui.width),
            h: u32::from(ui.height),
        };
        let turn = Turn::of(whole, (ui.width, ui.height), scene.ui_rotation);
        let x_map = turn.columns.map(dest.w, scene.ui_filter);
        let y_map = turn.lines.map(dest.h, scene.ui_filter);
        let (x0, x1) = visible(dest.x, dest.w, out.width);
        let (y0, y1) = visible(dest.y, dest.h, out.height);
        fill_taps(&mut self.columns, &x_map, x0, x1, dest.x);
        fill_taps(&mut self.lines, &y_map, y0, y1, dest.y);
        to_frame(&mut self.columns, &turn.columns);
        to_frame(&mut self.lines, &turn.lines);

        let den = (x_map.denominator(), y_map.denominator());
        for (y, line) in (y0..y1).zip(&self.lines) {
            let Some(row) = output_row(out, y, x0, x1) else {
                continue;
            };
            for (px, column) in row.chunks_exact_mut(4).zip(&self.columns) {
                let rgb = sample(ui, (*column, *line), turn.swap, den, FULL_PERCENT);
                px.copy_from_slice(&rgb.to_xrgb8888_le());
            }
        }
    }
}
