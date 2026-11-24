//! The grid test pattern, to adjust the geometry of the picture and to see
//! how much of the canvas the monitor shows.
//!
//! The pattern does not follow the skin: its pixels are fixed, so one
//! photo of a monitor compares with another. It covers the canvas of the
//! form. On the 320x240 canvas it draws, in this order:
//!
//! 1. Black on every pixel.
//! 2. The grid, grey `#808080`, lines two pixels wide, centred on the
//!    canvas: columns `16k - 1` and `16k` for `k` = 1 to 19 (15 and 16 up
//!    to 303 and 304), rows `120 + 16j - 1` and `120 + 16j` for `j` = -7
//!    to 7 (7 and 8 up to 231 and 232). The canvas centre lies between
//!    columns 159 and 160 and between rows 119 and 120, on grid lines.
//! 3. The border, white, one pixel on the exact edge: column 0, column
//!    319, row 0 and row 239, each the full length.
//! 4. The centre cross, white, two pixels wide: columns 159 and 160 from
//!    row 104 to 135, rows 119 and 120 from column 144 to 175.
//! 5. The corner marks, amber `#F0C030`: an L at each corner of the safe
//!    area (columns 16 to 303, rows 12 to 227), arms 16 pixels long and 2
//!    pixels thick, along the edges of the safe area and inside it.
//! 6. The text `Back: return`, white, on a black box of columns 120 to
//!    199 and rows 215 to 227, at the bottom of the safe area.
//!
//! A monitor that shows the white border shows the whole canvas. A monitor
//! that cuts the border but shows the four corner marks shows the safe
//! area, so every menu is visible.

use crate::frame::UiFrame;
use crate::geom::{Color, Rect};
use crate::input::UiAction;
use crate::metrics::Metrics;
use crate::text::Align;

use super::chrome;
use super::router::Nav;

/// The pitch of the grid, in pixels.
pub const GRID_PITCH_PX: u32 = 16;
/// The width of a grid line and of the cross, in pixels.
const LINE_PX: u32 = 2;
/// The length of the arms of the centre cross, in pixels.
const CROSS_PX: u32 = 32;
/// The length of the arms of a corner mark, in pixels.
const MARK_ARM_PX: u32 = 16;
/// Space on each side of the caption text, in pixels.
const CAPTION_PAD_PX: u32 = 4;

const GRID: Color = Color::rgb(0x80, 0x80, 0x80);
const BORDER: Color = Color::WHITE;
const CROSS: Color = Color::WHITE;
const MARK: Color = Color::rgb(0xF0, 0xC0, 0x30);
const CAPTION: &str = "Back: return";

/// Accept or back closes the pattern. The other actions do nothing, so a
/// player can touch the pad while adjusting the monitor.
pub(super) fn handle(action: UiAction) -> Nav {
    match action {
        UiAction::Accept | UiAction::Back => Nav::Back,
        _ => Nav::Stay,
    }
}

pub(super) fn draw(frame: &mut UiFrame, metrics: &Metrics) {
    let (width, height) = (metrics.canvas.width(), metrics.canvas.height());
    let (centre_x, centre_y) = ((width / 2) as i32, (height / 2) as i32);
    let scene = &mut frame.scene;
    scene.rect(metrics.canvas.rect(), Color::BLACK);
    for k in 1..width / GRID_PITCH_PX {
        let x = (k * GRID_PITCH_PX) as i32 - 1;
        scene.rect(Rect::new(x, 0, LINE_PX, height), GRID);
    }
    let rows = centre_y / GRID_PITCH_PX as i32;
    for j in -rows..=rows {
        let y = centre_y + j * GRID_PITCH_PX as i32 - 1;
        scene.rect(Rect::new(0, y, width, LINE_PX), GRID);
    }
    for edge in [
        Rect::new(0, 0, width, 1),
        Rect::new(0, height as i32 - 1, width, 1),
        Rect::new(0, 0, 1, height),
        Rect::new(width as i32 - 1, 0, 1, height),
    ] {
        scene.rect(edge, BORDER);
    }
    let half = (CROSS_PX / 2) as i32;
    scene.rect(
        Rect::new(centre_x - 1, centre_y - half, LINE_PX, CROSS_PX),
        CROSS,
    );
    scene.rect(
        Rect::new(centre_x - half, centre_y - 1, CROSS_PX, LINE_PX),
        CROSS,
    );
    draw_corner_marks(frame, metrics.safe);
    draw_caption(frame, metrics.safe, centre_x);
}

fn draw_corner_marks(frame: &mut UiFrame, safe: Rect) {
    let left = safe.x;
    let top = safe.y;
    let right = (safe.right() - i64::from(LINE_PX)) as i32;
    let bottom = (safe.bottom() - i64::from(LINE_PX)) as i32;
    let arm = MARK_ARM_PX as i32 - LINE_PX as i32;
    for (x, y, dx, dy) in [
        (left, top, 0, 0),
        (right, top, -arm, 0),
        (left, bottom, 0, -arm),
        (right, bottom, -arm, -arm),
    ] {
        frame
            .scene
            .rect(Rect::new(x + dx, y, MARK_ARM_PX, LINE_PX), MARK);
        frame
            .scene
            .rect(Rect::new(x, y + dy, LINE_PX, MARK_ARM_PX), MARK);
    }
}

fn draw_caption(frame: &mut UiFrame, safe: Rect, centre_x: i32) {
    let width = crate::font::text_width(CAPTION) + 2 * CAPTION_PAD_PX;
    let height = crate::font::GLYPH_HEIGHT + 1;
    let x = centre_x - (width / 2) as i32;
    let y = (safe.bottom() - i64::from(height)) as i32;
    let caption = Rect::new(x, y, width, height);
    frame.scene.rect(caption, Color::BLACK);
    let text = Rect::new(x, y + 1, width, height - 1);
    chrome::label(frame, text, Align::Center, Color::WHITE, CAPTION);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::CpuCanvas;
    use crate::metrics::Form;

    const UI_WIDTH: u32 = Form::Landscape.size().width();
    const UI_HEIGHT: u32 = Form::Landscape.size().height();

    fn canvas() -> CpuCanvas {
        canvas_of(Form::Landscape)
    }

    fn canvas_of(form: Form) -> CpuCanvas {
        let mut frame = UiFrame::new();
        draw(&mut frame, &Metrics::of(form, &crate::Theme::default()));
        let mut canvas = CpuCanvas::with_size(form.size());
        frame.render(&mut canvas);
        canvas
    }

    fn at(canvas: &CpuCanvas, x: u32, y: u32) -> Color {
        canvas.pixel(x, y).unwrap_or(Color::BLACK)
    }

    #[test]
    fn the_border_is_on_the_exact_edge() {
        let c = canvas();
        for x in 0..UI_WIDTH {
            assert_eq!(at(&c, x, 0), BORDER, "{x}");
            assert_eq!(at(&c, x, UI_HEIGHT - 1), BORDER, "{x}");
        }
        for y in 0..UI_HEIGHT {
            assert_eq!(at(&c, 0, y), BORDER, "{y}");
            assert_eq!(at(&c, UI_WIDTH - 1, y), BORDER, "{y}");
        }
        assert_ne!(at(&c, 1, 3), BORDER);
    }

    #[test]
    fn the_grid_is_symmetric_about_the_centre() {
        let c = canvas();
        let y = 60;
        for x in 1..UI_WIDTH - 1 {
            let mirror = UI_WIDTH - 1 - x;
            assert_eq!(at(&c, x, y), at(&c, mirror, y), "column {x}");
        }
        let x = 72;
        for y in 1..UI_HEIGHT - 1 {
            let mirror = UI_HEIGHT - 1 - y;
            assert_eq!(at(&c, x, y), at(&c, x, mirror), "row {y}");
        }
        assert_eq!(at(&c, 15, 60), GRID);
        assert_eq!(at(&c, 16, 60), GRID);
        assert_eq!(at(&c, 17, 60), Color::BLACK);
        assert_eq!(at(&c, 72, 7), GRID);
        assert_eq!(at(&c, 72, 232), GRID);
        assert_eq!(at(&c, 72, 233), Color::BLACK);
    }

    #[test]
    fn the_cross_marks_the_centre() {
        let c = canvas();
        for (x, y) in [(159, 104), (160, 135), (144, 119), (175, 120), (159, 119)] {
            assert_eq!(at(&c, x, y), CROSS, "{x},{y}");
        }
        assert_eq!(at(&c, 159, 103), GRID);
        assert_eq!(at(&c, 143, 119), GRID);
    }

    #[test]
    fn the_corner_marks_are_at_the_safe_area_corners() {
        let c = canvas();
        for (x, y) in [
            (16, 12),
            (31, 12),
            (16, 27),
            (303, 12),
            (288, 13),
            (302, 27),
            (16, 227),
            (17, 212),
            (303, 227),
            (288, 226),
        ] {
            assert_eq!(at(&c, x, y), MARK, "{x},{y}");
        }
        assert_ne!(at(&c, 32, 12), MARK);
        assert_ne!(at(&c, 304, 12), MARK);
        assert_ne!(at(&c, 16, 228), MARK);
    }

    #[test]
    fn the_caption_box_is_where_the_doc_says() {
        let mut frame = UiFrame::new();
        draw(&mut frame, &Metrics::default());
        let boxes: Vec<_> = frame
            .scene
            .commands()
            .iter()
            .filter_map(|c| match *c {
                crate::DrawCommand::Rect { rect, color } if color == Color::BLACK => Some(rect),
                _ => None,
            })
            .collect();
        assert_eq!(boxes.last(), Some(&Rect::new(120, 215, 80, 13)));
        assert!(
            Form::Landscape
                .safe_area()
                .contains(&Rect::new(120, 215, 80, 13))
        );
    }

    #[test]
    fn the_portrait_pattern_covers_the_portrait_canvas() {
        let c = canvas_of(Form::Portrait);
        for (x, y) in [(0, 0), (239, 0), (0, 319), (239, 319)] {
            assert_eq!(at(&c, x, y), BORDER, "{x},{y}");
        }
        for (x, y) in [(119, 144), (120, 175), (104, 159), (135, 160)] {
            assert_eq!(at(&c, x, y), CROSS, "{x},{y}");
        }
        for (x, y) in [(12, 16), (227, 16), (12, 303), (227, 303)] {
            assert_eq!(at(&c, x, y), MARK, "{x},{y}");
        }
        assert_eq!(at(&c, 15, 60), GRID);
        assert_eq!(at(&c, 72, 15), GRID);
    }

    #[test]
    fn only_accept_and_back_close_the_pattern() {
        assert!(matches!(handle(UiAction::Back), Nav::Back));
        assert!(matches!(handle(UiAction::Accept), Nav::Back));
        for action in [UiAction::Up, UiAction::Left, UiAction::Menu] {
            assert!(matches!(handle(action), Nav::Stay));
        }
    }
}
