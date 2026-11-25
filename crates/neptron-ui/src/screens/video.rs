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
use crate::metrics::Metrics;
use crate::snapshot::{Snapshot, VideoStatus};
use crate::text::{self, Align, StackText};
use crate::theme::Theme;
use crate::tree::{TreeRow, WidgetKind};

use super::chrome;
use super::{VIDEO_GAME_ROW, VIDEO_MODE_ROW, VIDEO_RESULT_ROW};

/// The fewest lines of the result.
const RESULT_LINES_MIN: u32 = 2;
/// The lines of the block with one line for each fact: the heading, the
/// game, the mode and the result.
const BASE_LINES: u32 = 3 + RESULT_LINES_MIN;
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
pub(super) fn fits(area: Rect, metrics: &Metrics, theme: &Theme) -> bool {
    chrome::box_rows(below(area, metrics), theme) >= MIN_PAGE_ROWS
}

/// The area under a block at the top of `area`.
pub(super) fn below(area: Rect, metrics: &Metrics) -> Rect {
    let (outer, _) = chrome::lines_box(area, LINE_PX, metrics.video_lines);
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
///
/// A block with more lines than [`BASE_LINES`] gives a fact that does not
/// fit its line a second line, the game first, and the rest to the result.
pub(super) fn draw(
    frame: &mut UiFrame,
    theme: &Theme,
    metrics: &Metrics,
    area: Rect,
    status: &VideoStatus,
) {
    let (outer, inner) = chrome::lines_box(area, LINE_PX, metrics.video_lines);
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
    let mut spare = lines.saturating_sub(BASE_LINES);
    let roomy = spare > 0;
    let mut next = 1;
    for (name, id, value) in [
        ("Game", VIDEO_GAME_ROW, status.game.as_str()),
        ("Mode", VIDEO_MODE_ROW, status.mode.as_str()),
    ] {
        let Some(rect) = line(next) else {
            return;
        };
        let (_, value_box) = columns(rect);
        let mut compact = StackText::<FACT_BYTES>::new();
        let shown = if roomy && font::text_width(value) > value_box.w {
            single_spaces(value, &mut compact);
            compact.as_str()
        } else {
            value
        };
        let fact_lines = if spare > 0 && font::text_width(shown) > value_box.w {
            spare -= 1;
            2
        } else {
            1
        };
        field(frame, theme, rect, fact_lines, (name, id), (value, shown));
        next += fact_lines;
    }
    if let Some(first) = line(next) {
        let more = (next + 1..lines).filter_map(line);
        result(frame, theme, first, more, status);
    }
}

/// The longest fact that a block with room writes with single spaces, in
/// bytes. The formatter writes fewer than 48.
const FACT_BYTES: usize = 96;

/// Writes `text` into `out` with each run of spaces made one space. The
/// fixed columns of the formatter line up the two facts on a wide line;
/// on a narrow line they would only push a fact on to a second line.
fn single_spaces<const N: usize>(text: &str, out: &mut StackText<N>) {
    let mut words = text.split(' ').filter(|w| !w.is_empty());
    if let Some(first) = words.next() {
        out.push_cut(first);
    }
    for word in words {
        out.push_cut(" ");
        out.push_cut(word);
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

/// Draws the fact `name` of the block as `shown` on `lines` lines from
/// `rect`, its first line. The tree row has the stable `id` as its label
/// and the whole text, `value`, as its value.
fn field(
    frame: &mut UiFrame,
    theme: &Theme,
    rect: Rect,
    lines: u32,
    (name, id): (&str, &str),
    (value, shown): (&str, &str),
) {
    let (name_box, value_box) = columns(rect);
    text::draw_in(
        &mut frame.scene,
        name_box,
        Align::Left,
        theme.colors.text_dim,
        name,
    );
    let clipped = text::draw_lines(
        &mut frame.scene,
        value_box,
        lines,
        LINE_PX,
        theme.colors.text,
        shown,
    );
    let whole = Rect::new(rect.x, rect.y, rect.w, rect.h.saturating_mul(lines));
    push(frame, id, value, whole, clipped);
}

/// Draws the result in the value column of `first` and, when the text needs
/// it, of the lines of `more`. Text left over ends with an ellipsis. An
/// alert result is drawn in the warning colour, which differs from the
/// focus colour, so it never reads as a focused row.
fn result(
    frame: &mut UiFrame,
    theme: &Theme,
    first: Rect,
    more: impl Iterator<Item = Rect>,
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
    let mut more = more.peekable();
    while let (Some(line), Some(next)) = (lines.peek().copied(), more.next()) {
        lines.next();
        let (_, next_box) = columns(next);
        if lines.peek().is_some() && more.peek().is_none() {
            chrome::draw_cut(frame, next_box, color, line);
        } else {
            clipped |= text::draw_in(&mut frame.scene, next_box, Align::Left, color, line);
        }
        rect.h = rect.h.saturating_add(next.h);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Form;

    #[test]
    fn single_spaces_join_the_words_with_one_space() {
        let mut out = StackText::<FACT_BYTES>::new();
        single_spaces("768x448p  30.05 kHz 60.099 Hz  2x", &mut out);
        assert_eq!(out.as_str(), "768x448p 30.05 kHz 60.099 Hz 2x");
        let mut empty = StackText::<FACT_BYTES>::new();
        single_spaces("   ", &mut empty);
        assert_eq!(empty.as_str(), "");
    }

    fn status() -> VideoStatus {
        VideoStatus {
            game: "256x224             60.099 Hz".into(),
            mode: "768x448p  30.05 kHz 60.099 Hz  2x".into(),
            result: "[!] cropped 2 lines, speed (60.099 to 59.940 Hz)".into(),
            ..VideoStatus::default()
        }
    }

    #[test]
    fn a_block_with_room_gives_a_long_fact_a_second_line_and_cuts_nothing() {
        let theme = Theme::default();
        let metrics = Metrics::of(Form::Portrait, &theme);
        let mut frame = UiFrame::new();
        draw(&mut frame, &theme, &metrics, metrics.content, &status());
        let rows: Vec<_> = frame
            .tree
            .rows()
            .map(|r| (r.label, r.rect.h / LINE_PX, r.clipped))
            .collect();
        assert_eq!(
            rows,
            [
                ("Video", 1, false),
                (VIDEO_GAME_ROW, 1, false),
                (VIDEO_MODE_ROW, 2, false),
                (VIDEO_RESULT_ROW, 3, false)
            ]
        );
    }

    #[test]
    fn a_block_of_five_lines_keeps_one_line_for_each_fact() {
        let theme = Theme::default();
        let metrics = Metrics::of(Form::Landscape, &theme);
        let mut frame = UiFrame::new();
        draw(&mut frame, &theme, &metrics, metrics.content, &status());
        let lines: Vec<_> = frame.tree.rows().map(|r| r.rect.h / LINE_PX).collect();
        assert_eq!(lines, [1, 1, 1, 2]);
    }
}
