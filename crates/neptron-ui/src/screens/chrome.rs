//! The frame around every menu screen: background, title band and hint
//! band, and the area between them.

use crate::canvas::{UI_HEIGHT, UI_WIDTH};
use crate::font;
use crate::frame::UiFrame;
use crate::geom::{Color, Rect};
use crate::text::{self, Align};
use crate::theme::{SAFE_AREA, Theme};
use crate::tree::{TreeRow, WidgetKind};

/// Space between the bands and the content, in pixels.
const GAP_PX: u32 = 4;
/// Height of the line under the title band. Two lines, so that an
/// interlaced mode shows it in both fields and it does not flicker.
const RULE_PX: u32 = 2;

/// The areas of a menu screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Layout {
    pub title: Rect,
    pub content: Rect,
    pub footer: Rect,
}

impl Layout {
    pub(super) fn new(theme: &Theme) -> Self {
        let row = theme.row_height_px();
        let title = Rect::new(SAFE_AREA.x, SAFE_AREA.y, SAFE_AREA.w, row + 4);
        let footer_h = row + 2;
        let footer = Rect::new(
            SAFE_AREA.x,
            offset(SAFE_AREA.y, SAFE_AREA.h - footer_h),
            SAFE_AREA.w,
            footer_h,
        );
        let top = title.h + RULE_PX + GAP_PX;
        let content = Rect::new(
            SAFE_AREA.x,
            offset(SAFE_AREA.y, top),
            SAFE_AREA.w,
            SAFE_AREA.h - top - footer_h - GAP_PX,
        );
        Self {
            title,
            content,
            footer,
        }
    }
}

/// Fills the whole canvas with the background colour.
pub(super) fn background(frame: &mut UiFrame, theme: &Theme) {
    frame.scene.rect(
        Rect::new(0, 0, UI_WIDTH, UI_HEIGHT),
        theme.colors.background,
    );
}

/// Draws the background, the title band with `title` on the left and
/// `right` on the right, and the hint band with `hint_left` and
/// `hint_right`.
pub(super) fn frame(
    frame: &mut UiFrame,
    theme: &Theme,
    title: &str,
    right: &str,
    hint_left: &str,
    hint_right: &str,
) {
    let layout = Layout::new(theme);
    background(frame, theme);
    frame.scene.rect(layout.title, theme.colors.panel);
    let rule = Rect::new(
        layout.title.x,
        offset(layout.title.y, layout.title.h),
        layout.title.w,
        RULE_PX,
    );
    frame.scene.rect(rule, theme.colors.accent);
    let inner = padded(layout.title, theme);
    let right_w = match right {
        "" => 0,
        text => font::text_width(text).saturating_add(2 * font::ADVANCE),
    };
    let title_box = Rect::new(inner.x, inner.y, inner.w.saturating_sub(right_w), inner.h);
    label(frame, title_box, Align::Left, theme.colors.text, title);
    label(frame, inner, Align::Right, theme.colors.accent, right);

    frame.scene.rect(layout.footer, theme.colors.panel);
    let inner = padded(layout.footer, theme);
    label(frame, inner, Align::Left, theme.colors.text_dim, hint_left);
    label(
        frame,
        inner,
        Align::Right,
        theme.colors.text_dim,
        hint_right,
    );
}

/// A framed box at the top of `area` that holds `rows` rows of the theme,
/// and the rectangle of those rows inside it.
pub(super) fn rows_box(area: Rect, theme: &Theme, rows: u32) -> (Rect, Rect) {
    let row = theme.row_height_px();
    let rows = rows.min(area.h.saturating_sub(BOX_EDGE_H_PX) / row);
    let outer = Rect::new(
        area.x,
        area.y,
        area.w,
        rows.saturating_mul(row).saturating_add(BOX_EDGE_H_PX),
    );
    let inner = Rect::new(
        offset(area.x, BOX_SIDE_PX + 1),
        offset(area.y, BOX_EDGE_H_PX / 2),
        area.w.saturating_sub(2 * (BOX_SIDE_PX + 1)),
        rows.saturating_mul(row),
    );
    (outer, inner)
}

/// A [`rows_box`] at the bottom of `area`.
pub(super) fn rows_box_bottom(area: Rect, theme: &Theme, rows: u32) -> (Rect, Rect) {
    let (outer, _) = rows_box(area, theme, rows);
    let y = offset(area.y, area.h.saturating_sub(outer.h));
    rows_box(Rect::new(area.x, y, area.w, outer.h), theme, rows)
}

/// The most rows that a [`rows_box`] in `area` holds.
pub(super) fn box_rows(area: Rect, theme: &Theme) -> u32 {
    area.h.saturating_sub(BOX_EDGE_H_PX) / theme.row_height_px()
}

/// Draws a framed box: a frame line around a panel fill. The top and bottom
/// lines are two pixels, so an interlaced mode shows them in both fields.
pub(super) fn boxed(frame: &mut UiFrame, theme: &Theme, rect: Rect) {
    frame.scene.rect(rect, theme.colors.border);
    let fill = Rect::new(
        offset(rect.x, BOX_SIDE_PX),
        offset(rect.y, RULE_PX),
        rect.w.saturating_sub(2 * BOX_SIDE_PX),
        rect.h.saturating_sub(2 * RULE_PX),
    );
    frame.scene.rect(fill, theme.colors.panel);
}

/// Width of the frame line at the sides of a box, in pixels.
const BOX_SIDE_PX: u32 = 1;
/// Height of the frame lines and the gaps above and below the rows of a
/// box, in pixels.
const BOX_EDGE_H_PX: u32 = 2 * (RULE_PX + 1);

/// Draws `text` in `rect` and adds it to the tree as a label. Empty text
/// draws nothing and adds no row.
pub(super) fn label(frame: &mut UiFrame, rect: Rect, align: Align, color: Color, text: &str) {
    if text.is_empty() {
        return;
    }
    let clipped = text::draw_in(&mut frame.scene, rect, align, color, text);
    frame.tree.push(TreeRow {
        widget: WidgetKind::Label,
        label: text,
        value: None,
        source: None,
        rect,
        focused: false,
        clipped,
    });
}

/// Draws `message` wrapped into the lines of `rect`, one line per row of
/// the theme. The last line that fits ends with an ellipsis when text is
/// left over. Returns the number of lines drawn.
pub(super) fn paragraph(
    frame: &mut UiFrame,
    theme: &Theme,
    rect: Rect,
    color: Color,
    message: &str,
) -> u32 {
    let row = theme.row_height_px();
    let max_lines = (rect.h / row) as usize;
    let mut drawn = 0;
    let mut lines = text::wrap(message, rect.w).peekable();
    while let Some(line) = lines.next() {
        if drawn >= max_lines {
            break;
        }
        let last = drawn + 1 == max_lines && lines.peek().is_some();
        let shown = if last { rest_of(message, line) } else { line };
        let y = offset(rect.y, row.saturating_mul(drawn as u32));
        label(
            frame,
            Rect::new(rect.x, y, rect.w, row),
            Align::Left,
            color,
            shown,
        );
        drawn += 1;
    }
    drawn as u32
}

/// `rect` with the theme padding taken off the left and the right.
pub(super) fn padded(rect: Rect, theme: &Theme) -> Rect {
    let pad = theme.padding_px();
    Rect::new(
        offset(rect.x, pad),
        rect.y,
        rect.w.saturating_sub(2 * pad),
        rect.h,
    )
}

/// `x` moved by `by` pixels, saturating.
pub(super) fn offset(x: i32, by: u32) -> i32 {
    i32::try_from(i64::from(x) + i64::from(by)).unwrap_or(i32::MAX)
}

/// The part of `message` from the start of `line` to the end. `line` is a
/// slice of `message`, as `text::wrap` gives it.
fn rest_of<'a>(message: &'a str, line: &'a str) -> &'a str {
    let start = (line.as_ptr() as usize).wrapping_sub(message.as_ptr() as usize);
    message.get(start..).unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_stays_in_the_safe_area_for_every_row_height() {
        for row in crate::theme::ROW_HEIGHT_MIN_PX..=crate::theme::ROW_HEIGHT_MAX_PX {
            let theme = Theme::new(Default::default(), row, 4).expect("valid");
            let l = Layout::new(&theme);
            for rect in [l.title, l.content, l.footer] {
                assert!(SAFE_AREA.contains(&rect), "{row}: {rect:?}");
            }
            assert!(l.title.bottom() < i64::from(l.content.y));
            assert!(l.content.bottom() < i64::from(l.footer.y));
        }
    }

    #[test]
    fn a_paragraph_ends_with_an_ellipsis_when_text_is_left() {
        let theme = Theme::default();
        let mut frame = UiFrame::new();
        let rect = Rect::new(16, 40, 60, 24);
        let lines = paragraph(
            &mut frame,
            &theme,
            rect,
            theme.colors.text,
            "one two three four five",
        );
        assert_eq!(lines, 2);
        let rows: Vec<_> = frame.tree.rows().map(|r| (r.label, r.clipped)).collect();
        assert_eq!(rows, [("one two", false), ("three four five", true)]);
    }
}
