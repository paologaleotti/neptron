//! The video block: what the running game asks for, the mode on screen and
//! the result, as text from the app. The block lays the text out and
//! decides nothing about it.
//!
//! The layout is fixed, so a digit that changes never moves a line: the
//! heading, the game line, the mode line, and two lines for the result
//! under the mode text. A result that needs more than two lines ends with
//! an ellipsis.

use crate::font;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::snapshot::{Snapshot, VideoStatus};
use crate::text::{self, Align};
use crate::theme::Theme;
use crate::tree::{TreeRow, WidgetKind};

use super::chrome;
use super::{VIDEO_GAME_ROW, VIDEO_MODE_ROW, VIDEO_RESULT_ROW};

/// The lines of the block: the heading, the game, the mode, and two lines
/// for the result.
const LINES: u32 = 5;
/// The height of a line of the block. It is one glyph box for every row
/// height of the theme, so the block leaves room for the menu under it
/// also with the tallest rows.
const LINE_PX: u32 = font::GLYPH_HEIGHT;
/// Space between the block and the box under it, in pixels.
const GAP_PX: u32 = 4;
/// The fewest rows that a page keeps under the block. A screen with less
/// room shows no block, so the player can always see and move the cursor.
const MIN_PAGE_ROWS: u32 = 2;

/// The video status to show: the one of the snapshot while a game runs.
pub(super) fn status(snap: &Snapshot) -> Option<&VideoStatus> {
    snap.session.as_ref().and(snap.video.as_ref())
}

/// True when a block at the top of `area` leaves room for
/// [`MIN_PAGE_ROWS`] rows of a page under it.
pub(super) fn fits(area: Rect, theme: &Theme) -> bool {
    chrome::box_rows(below(area), theme) >= MIN_PAGE_ROWS
}

/// The area under a block at the top of `area`.
pub(super) fn below(area: Rect) -> Rect {
    let (outer, _) = chrome::lines_box(area, LINE_PX, LINES);
    let used = outer.h.saturating_add(GAP_PX).min(area.h);
    Rect::new(
        area.x,
        chrome::offset(area.y, used),
        area.w,
        area.h.saturating_sub(used),
    )
}

/// Draws the block at the top of `area`: a box with the heading and the
/// lines of `status`. A line that does not fit in `area` is not drawn.
pub(super) fn draw(frame: &mut UiFrame, theme: &Theme, area: Rect, status: &VideoStatus) {
    let (outer, inner) = chrome::lines_box(area, LINE_PX, LINES);
    chrome::boxed(frame, theme, outer);
    let inner = chrome::padded(inner, theme);
    let lines = inner.h / LINE_PX;
    let line = |i: u32| {
        (i < lines).then(|| {
            Rect::new(
                inner.x,
                chrome::offset(inner.y, i * LINE_PX),
                inner.w,
                LINE_PX,
            )
        })
    };
    if let Some(rect) = line(0) {
        chrome::label(frame, rect, Align::Left, theme.colors.accent, "Video");
    }
    if let Some(rect) = line(1) {
        field(frame, theme, rect, "Game", VIDEO_GAME_ROW, &status.game);
    }
    if let Some(rect) = line(2) {
        field(frame, theme, rect, "Mode", VIDEO_MODE_ROW, &status.mode);
    }
    if let Some(first) = line(3) {
        result(frame, theme, first, line(4), status);
    }
}

/// The name column and the value column of a line.
fn columns(rect: Rect) -> (Rect, Rect) {
    let name_w = font::char_count_width(chrome::FIELD_NAME_CHARS).min(rect.w);
    let name = Rect::new(rect.x, rect.y, name_w, rect.h);
    let value = Rect::new(
        chrome::offset(rect.x, name_w),
        rect.y,
        rect.w.saturating_sub(name_w),
        rect.h,
    );
    (name, value)
}

/// Draws a named line of the block. The tree row has the stable `id` as its
/// label and the whole text as its value.
fn field(frame: &mut UiFrame, theme: &Theme, rect: Rect, name: &str, id: &str, value: &str) {
    let (name_box, value_box) = columns(rect);
    text::draw_in(
        &mut frame.scene,
        name_box,
        Align::Left,
        theme.colors.text_dim,
        name,
    );
    let clipped = text::draw_in(
        &mut frame.scene,
        value_box,
        Align::Left,
        theme.colors.text,
        value,
    );
    push(frame, id, value, rect, clipped);
}

/// Draws the result in the value column of `first` and, when the text needs
/// it and the block has it, of `second`. Text left over ends with an
/// ellipsis. An alert result is drawn in the warning colour, which differs
/// from the focus colour, so it never reads as a focused row.
fn result(
    frame: &mut UiFrame,
    theme: &Theme,
    first: Rect,
    second: Option<Rect>,
    status: &VideoStatus,
) {
    let color = if status.alert {
        theme.colors.warning
    } else {
        theme.colors.text
    };
    let (_, first_box) = columns(first);
    let mut lines = text::wrap(&status.result, first_box.w).peekable();
    let mut clipped = false;
    let mut rect = first;
    if let Some(line) = lines.next() {
        clipped |= text::draw_in(&mut frame.scene, first_box, Align::Left, color, line);
    }
    if let (Some(line), Some(second)) = (lines.peek().copied(), second) {
        lines.next();
        let (_, second_box) = columns(second);
        if lines.peek().is_some() {
            chrome::draw_cut(frame, second_box, color, line);
        } else {
            clipped |= text::draw_in(&mut frame.scene, second_box, Align::Left, color, line);
        }
        rect.h = rect.h.saturating_add(second.h);
    }
    clipped |= lines.peek().is_some();
    push(frame, VIDEO_RESULT_ROW, &status.result, rect, clipped);
}

fn push(frame: &mut UiFrame, id: &str, value: &str, rect: Rect, clipped: bool) {
    frame.tree.push(TreeRow {
        widget: WidgetKind::Label,
        kind: None,
        label: id,
        value: Some(value),
        source: None,
        rect,
        focused: false,
        clipped,
    });
}
