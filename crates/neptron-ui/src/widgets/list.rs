use core::cell::Cell;

use crate::font;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::text::{self, Align};
use crate::theme::Theme;
use crate::tree::{RowKind, TreeRow, WidgetKind};

use super::{ARROW_COLUMN_PX, add, draw_arrow, to_u32};

/// Width of the scroll bar, and the gap before it, in pixels.
pub const SCROLLBAR_WIDTH_PX: u32 = 4;

/// The most lines a list draws: one per line of the tallest canvas. A
/// taller rectangle draws no more, so a huge rectangle costs no more work
/// than the canvas.
pub const MAX_VISIBLE_ROWS: usize = 320;

/// The most lines of one row. A label that needs more is shortened on its
/// last line and reported as clipped.
pub const MAX_ROW_LINES: u32 = 2;

/// One row of a [`List`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListItem<'a> {
    /// The label, on the left.
    pub label: &'a str,
    /// Extra text on the right, for example a game count.
    pub detail: Option<&'a str>,
    /// False shows the row as not available: dim text, and a bar in the
    /// border colour instead of the focus colour when it has the focus.
    pub enabled: bool,
    /// What accept does. A [`RowKind::Page`] row shows an arrow at its
    /// right edge while it is enabled.
    pub kind: RowKind,
    /// The detail of an enabled row is what the row gives, such as the tag
    /// of an option: it shows in the accent colour, apart from the dim
    /// reason of a row that is not available.
    pub accent_detail: bool,
}

/// What one action did to a [`List`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEvent {
    /// Nothing changed.
    None,
    /// The cursor moved.
    Moved,
    /// The player accepted the row at this index.
    Activated(usize),
    /// The player pressed back.
    Back,
    /// The player pressed menu.
    Menu,
}

/// What a [`List`] does with a label wider than its row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelRule {
    /// The row keeps one line and the label is cut: a fault of the layout,
    /// reported as clipped in the tree.
    #[default]
    Cut,
    /// The row keeps one line and the label is shortened with `...`. Not a
    /// fault: the text shows whole in another place, or has no bound. Every
    /// row has the same height, so a window of any length costs the same.
    Shorten,
    /// The label wraps on to a second line of the row.
    Wrap,
}

/// A vertical list with one focused row. Up and down move by one row; left
/// and right move by one page, the rows that fit the box.
///
/// A row has one line, or two when its label wraps ([`LabelRule::Wrap`]).
/// The window of visible rows keeps the focused row whole, and the scroll
/// bar measures rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    rect: Rect,
    row_height_px: u32,
    cursor: usize,
    /// The first visible row. A draw moves it down when rows of two lines
    /// push the focused row out of the box.
    top: Cell<usize>,
    /// The rows that the last draw showed whole: a page for a list with
    /// rows of two lines.
    shown: Cell<usize>,
    labels: LabelRule,
    /// True when a row can have more than one line.
    tall_rows: bool,
}

/// The rows in view of a list of some length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct View {
    pub cursor: usize,
    pub top: usize,
    pub scrolls: bool,
}

impl<'a> ListItem<'a> {
    /// An enabled action row with a label only.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            detail: None,
            enabled: true,
            kind: RowKind::Action,
            accent_detail: false,
        }
    }

    /// An enabled row with a label only that opens a page.
    pub fn page(label: &'a str) -> Self {
        Self {
            kind: RowKind::Page,
            accent_detail: false,
            ..Self::new(label)
        }
    }
}

impl List {
    /// A list in `rect` with rows of `row_height_px`, cursor on the first
    /// row. A label that does not fit is cut ([`LabelRule::Cut`]).
    pub fn new(rect: Rect, row_height_px: u32) -> Self {
        Self {
            rect,
            row_height_px: row_height_px.max(1),
            cursor: 0,
            top: Cell::new(0),
            shown: Cell::new(0),
            labels: LabelRule::Cut,
            tall_rows: false,
        }
    }

    /// The same list with the rule `labels` for a label wider than its row.
    pub fn with_labels(mut self, labels: LabelRule) -> Self {
        self.labels = labels;
        self.tall_rows = labels == LabelRule::Wrap;
        self
    }

    /// The same list, with rows of more than one line that another widget
    /// lays out.
    pub(super) fn with_tall_rows(mut self) -> Self {
        self.tall_rows = true;
        self
    }

    /// The area of the list.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Moves the list into `rect` and keeps its cursor.
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    /// The focused row.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The first visible row.
    pub fn top(&self) -> usize {
        self.top.get()
    }

    /// The number of lines that fit, at least 1 and at most
    /// [`MAX_VISIBLE_ROWS`]. With rows of one line, the rows that fit.
    pub fn visible_rows(&self) -> usize {
        ((self.rect.h / self.row_height_px) as usize).clamp(1, MAX_VISIBLE_ROWS)
    }

    /// Moves the cursor to `index` and scrolls it into view.
    pub fn select(&mut self, index: usize, len: usize) {
        self.cursor = index;
        self.settle(len);
    }

    /// Applies one action to a list of `len` rows.
    pub fn handle(&mut self, action: UiAction, len: usize) -> ListEvent {
        self.settle(len);
        let last = len.saturating_sub(1);
        let page = self.page();
        let target = match action {
            UiAction::Up => self.cursor.saturating_sub(1),
            UiAction::Down => self.cursor.saturating_add(1).min(last),
            UiAction::Left => self.cursor.saturating_sub(page),
            UiAction::Right => self.cursor.saturating_add(page).min(last),
            UiAction::Accept if len > 0 => return ListEvent::Activated(self.cursor),
            UiAction::Accept => return ListEvent::None,
            UiAction::Back => return ListEvent::Back,
            UiAction::Menu => return ListEvent::Menu,
        };
        if target == self.cursor {
            return ListEvent::None;
        }
        self.select(target, len);
        ListEvent::Moved
    }

    /// Draws the visible rows of a list of `len` rows. `item` gives the row
    /// at an index.
    pub fn draw<'a>(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        len: usize,
        item: impl Fn(usize) -> ListItem<'a>,
    ) {
        let lines = |i: usize, scrolls: bool| self.item_lines(theme, &item(i), scrolls);
        let view = self.view(len, &lines);
        let mut shown = 0;
        for (index, row, row_lines) in self.rows(len, view, &lines) {
            self.draw_row(
                frame,
                theme,
                row,
                row_lines,
                &item(index),
                index == view.cursor,
            );
            shown += 1;
        }
        self.finish(frame, theme, len, view, shown);
    }

    /// The rows in view of a list of `len` rows, whose row `i` has
    /// `lines(i, scrolls)` lines. The first visible row moves down until
    /// the focused row fits whole.
    pub(super) fn view(&self, len: usize, lines: &impl Fn(usize, bool) -> u32) -> View {
        let capacity = self.visible_rows();
        let cursor = self.cursor.min(len.saturating_sub(1));
        let scrolls = len > capacity
            || (self.tall_rows
                && (0..len).map(|i| lines(i, false) as usize).sum::<usize>() > capacity);
        let top = self.top.get().min(cursor);
        let mut first = cursor;
        let mut used = (lines(cursor, scrolls) as usize).min(capacity);
        while first > top {
            let above = lines(first - 1, scrolls) as usize;
            if used + above > capacity {
                break;
            }
            used += above;
            first -= 1;
        }
        View {
            cursor,
            top: first,
            scrolls,
        }
    }

    /// The visible rows of `view`: the index, the rectangle and the lines
    /// of each. A row that does not fit whole under the others is not in
    /// view; the first row always is, cut to the box.
    pub(super) fn rows<'l>(
        &'l self,
        len: usize,
        view: View,
        lines: &'l impl Fn(usize, bool) -> u32,
    ) -> impl Iterator<Item = (usize, Rect, u32)> + 'l {
        let capacity = self.visible_rows();
        let mut used = 0usize;
        (view.top..len).map_while(move |index| {
            let mut row_lines = lines(index, view.scrolls).max(1) as usize;
            if index == view.top {
                row_lines = row_lines.min(capacity);
            }
            if used + row_lines > capacity {
                return None;
            }
            let row = self.row_rect(used, row_lines, view.scrolls);
            used += row_lines;
            Some((index, row, to_u32(row_lines)))
        })
    }

    /// Keeps the window of the last draw and draws the scroll bar.
    pub(super) fn finish(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        len: usize,
        view: View,
        shown: usize,
    ) {
        self.top.set(view.top);
        self.shown.set(shown);
        if view.scrolls {
            let rows = if self.tall_rows {
                shown
            } else {
                self.visible_rows()
            };
            self.draw_scrollbar(frame, theme, len, view.top, rows);
        }
    }

    /// The lines of `entry` in this list: two when its label wraps.
    fn item_lines(&self, theme: &Theme, entry: &ListItem<'_>, scrolls: bool) -> u32 {
        if self.labels != LabelRule::Wrap {
            return 1;
        }
        let row = self.row_rect(0, 1, scrolls);
        let label_w = Columns::of(row, theme, entry).label.w;
        if font::text_width(entry.label) > label_w {
            MAX_ROW_LINES
        } else {
            1
        }
    }

    fn draw_row(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        row: Rect,
        lines: u32,
        entry: &ListItem<'_>,
        focused: bool,
    ) {
        let color = if focused && !entry.enabled {
            frame.scene.rect(row, theme.colors.border);
            theme.colors.text
        } else if focused {
            frame.scene.rect(row, theme.colors.focus);
            theme.colors.focus_text
        } else if entry.enabled {
            theme.colors.text
        } else {
            theme.colors.text_dim
        };
        let first = Rect::new(row.x, row.y, row.w, self.row_height_px.min(row.h));
        let columns = Columns::of(first, theme, entry);
        if columns.arrow {
            let arrow_color = if focused {
                color
            } else {
                theme.colors.text_dim
            };
            draw_arrow(&mut frame.scene, columns.padded, arrow_color);
        }
        if let Some(detail) = entry.detail {
            let detail_color = if focused {
                color
            } else if entry.enabled && entry.accent_detail {
                theme.colors.accent
            } else {
                theme.colors.text_dim
            };
            text::draw_in(
                &mut frame.scene,
                columns.detail,
                Align::Right,
                detail_color,
                detail,
            );
        }
        let clipped = match self.labels {
            LabelRule::Cut => text::draw_in(
                &mut frame.scene,
                columns.label,
                Align::Left,
                color,
                entry.label,
            ),
            LabelRule::Shorten => {
                text::draw_short(
                    &mut frame.scene,
                    columns.label,
                    Align::Left,
                    color,
                    entry.label,
                );
                false
            }
            LabelRule::Wrap => text::draw_lines(
                &mut frame.scene,
                columns.label,
                lines,
                self.row_height_px,
                color,
                entry.label,
            ),
        };
        frame.tree.push(TreeRow {
            widget: WidgetKind::List,
            kind: Some(if entry.enabled {
                entry.kind
            } else {
                RowKind::Disabled
            }),
            label: entry.label,
            value: entry.detail,
            source: None,
            rect: row,
            focused,
            clipped,
        });
    }

    fn page(&self) -> usize {
        match self.shown.get() {
            shown if self.tall_rows && shown > 0 => shown,
            _ => self.visible_rows(),
        }
    }

    /// The rectangle of a row of `lines` lines that starts `slot` lines
    /// below the top of the list.
    pub(super) fn row_rect(&self, slot: usize, lines: usize, scrolls: bool) -> Rect {
        let bar = if scrolls { SCROLLBAR_WIDTH_PX } else { 0 };
        let offset = to_u32(slot).saturating_mul(self.row_height_px);
        Rect::new(
            self.rect.x,
            add(self.rect.y, offset),
            self.rect.w.saturating_sub(bar),
            to_u32(lines).saturating_mul(self.row_height_px),
        )
    }

    /// The height of one line, in pixels.
    pub(super) fn line_px(&self) -> u32 {
        self.row_height_px
    }

    fn draw_scrollbar(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        len: usize,
        top: usize,
        rows: usize,
    ) {
        let track_h = to_u32(self.visible_rows())
            .saturating_mul(self.row_height_px)
            .min(self.rect.h);
        let x = add(
            self.rect.x,
            self.rect.w.saturating_sub(SCROLLBAR_WIDTH_PX - 1),
        );
        let track = Rect::new(x, self.rect.y, SCROLLBAR_WIDTH_PX - 1, track_h);
        frame.scene.rect(track, theme.colors.panel);
        let track_h = u128::from(track_h);
        let thumb_h = (track_h * rows as u128 / len.max(1) as u128)
            .max(4)
            .min(track_h);
        let travel = track_h - thumb_h;
        let span = len.saturating_sub(rows).max(1) as u128;
        let thumb_y = (travel * top.min(len) as u128 / span).min(travel);
        // Both values are at most `track_h`, which came from a `u32`.
        let thumb = Rect::new(x, add(self.rect.y, thumb_y as u32), track.w, thumb_h as u32);
        frame.scene.rect(thumb, theme.colors.border);
    }

    fn settle(&mut self, len: usize) {
        let view = self.view(len, &|_, _| 1);
        self.cursor = view.cursor;
        self.top.set(view.top);
    }
}

/// The columns of the first line of a row: the row less its padding, the
/// arrow, the detail and the label.
struct Columns {
    padded: Rect,
    arrow: bool,
    detail: Rect,
    label: Rect,
}

impl Columns {
    fn of(row: Rect, theme: &Theme, entry: &ListItem<'_>) -> Self {
        let padded = Rect::new(
            add(row.x, theme.padding_px()),
            row.y,
            row.w.saturating_sub(2 * theme.padding_px()),
            row.h,
        );
        let arrow = entry.enabled && entry.kind == RowKind::Page;
        let inner = if arrow {
            Rect::new(
                padded.x,
                padded.y,
                padded.w.saturating_sub(ARROW_COLUMN_PX),
                padded.h,
            )
        } else {
            padded
        };
        let detail_w = entry
            .detail
            .map_or(0, |d| font::text_width(d).min(inner.w / 2));
        let detail = Rect::new(add(inner.x, inner.w - detail_w), inner.y, detail_w, inner.h);
        let gap = if detail_w > 0 { font::ADVANCE } else { 0 };
        let label = Rect::new(
            inner.x,
            inner.y,
            inner.w.saturating_sub(detail_w + gap),
            inner.h,
        );
        Self {
            padded,
            arrow,
            detail,
            label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_lists_and_rects_stay_bounded() {
        let mut l = List::new(Rect::new(i32::MAX, i32::MIN, u32::MAX, u32::MAX), 1);
        assert_eq!(l.visible_rows(), MAX_VISIBLE_ROWS);
        l.select(usize::MAX, usize::MAX);
        assert_eq!(l.handle(UiAction::Down, usize::MAX), ListEvent::None);
        assert_eq!(l.handle(UiAction::Right, usize::MAX), ListEvent::None);
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), usize::MAX, |_| {
            ListItem::new("x")
        });
        assert_eq!(frame.tree.rows().len(), MAX_VISIBLE_ROWS);
    }

    fn list() -> List {
        List::new(Rect::new(16, 12, 288, 48), 12)
    }

    fn draw(list: &List, len: usize) -> UiFrame {
        let labels = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"];
        let mut frame = UiFrame::new();
        list.draw(&mut frame, &Theme::default(), len, |i| {
            ListItem::new(labels[i % labels.len()])
        });
        frame
    }

    #[test]
    fn up_and_down_move_and_stop_at_the_ends() {
        let mut l = list();
        assert_eq!(l.handle(UiAction::Up, 3), ListEvent::None);
        assert_eq!(l.handle(UiAction::Down, 3), ListEvent::Moved);
        assert_eq!(l.handle(UiAction::Down, 3), ListEvent::Moved);
        assert_eq!(l.handle(UiAction::Down, 3), ListEvent::None);
        assert_eq!(l.cursor(), 2);
        assert_eq!(l.handle(UiAction::Accept, 3), ListEvent::Activated(2));
    }

    #[test]
    fn left_and_right_move_by_a_page() {
        let mut l = list();
        assert_eq!(l.visible_rows(), 4);
        assert_eq!(l.handle(UiAction::Right, 10), ListEvent::Moved);
        assert_eq!((l.cursor(), l.top()), (4, 1));
        l.handle(UiAction::Right, 10);
        l.handle(UiAction::Right, 10);
        assert_eq!((l.cursor(), l.top()), (9, 6));
        l.handle(UiAction::Left, 10);
        assert_eq!((l.cursor(), l.top()), (5, 5));
    }

    #[test]
    fn back_menu_and_empty_lists() {
        let mut l = list();
        assert_eq!(l.handle(UiAction::Back, 0), ListEvent::Back);
        assert_eq!(l.handle(UiAction::Menu, 0), ListEvent::Menu);
        assert_eq!(l.handle(UiAction::Accept, 0), ListEvent::None);
        assert_eq!(l.handle(UiAction::Down, 0), ListEvent::None);
        assert_eq!(draw(&l, 0).tree.rows().len(), 0);
    }

    #[test]
    fn a_shorter_list_moves_the_cursor_back_in() {
        let mut l = list();
        l.select(9, 10);
        assert_eq!(l.handle(UiAction::Up, 3), ListEvent::Moved);
        assert_eq!((l.cursor(), l.top()), (1, 1));
    }

    #[test]
    fn draw_shows_the_visible_rows_with_the_focus() {
        let mut l = list();
        l.select(5, 6);
        let frame = draw(&l, 6);
        let rows: Vec<_> = frame.tree.rows().map(|r| (r.label, r.focused)).collect();
        assert_eq!(
            rows,
            [
                ("Gamma", false),
                ("Delta", false),
                ("Epsilon", false),
                ("Zeta", true)
            ]
        );
        let focused = frame.tree.focused().expect("a focused row");
        assert_eq!(focused.rect, Rect::new(16, 48, 284, 12));
    }

    #[test]
    fn a_focused_row_that_is_not_available_has_no_focus_colour() {
        let theme = Theme::default();
        let l = list();
        let fills = |enabled| {
            let mut frame = UiFrame::new();
            l.draw(&mut frame, &theme, 1, |_| ListItem {
                accent_detail: false,
                enabled,
                ..ListItem::new("Native")
            });
            frame
                .scene
                .commands()
                .iter()
                .filter_map(|c| match *c {
                    crate::DrawCommand::Rect { color, .. } => Some(color),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(fills(true), [theme.colors.focus]);
        assert_eq!(fills(false), [theme.colors.border]);
    }

    fn rects(frame: &UiFrame) -> Vec<Rect> {
        frame
            .scene
            .commands()
            .iter()
            .filter_map(|c| match *c {
                crate::DrawCommand::Rect { rect, .. } => Some(rect),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_row_that_opens_a_page_shows_an_arrow_at_its_right_edge() {
        let l = list();
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 3, |i| match i {
            0 => ListItem::page("Settings"),
            1 => ListItem::new("Resume"),
            _ => ListItem {
                accent_detail: false,
                enabled: false,
                ..ListItem::page("Video")
            },
        });
        let kinds: Vec<_> = frame.tree.rows().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            [
                Some(RowKind::Page),
                Some(RowKind::Action),
                Some(RowKind::Disabled)
            ]
        );
        // The focus bar, then the four columns of the arrow of row 0 only:
        // 7, 5, 3 and 1 pixels high, against the padding of the row.
        let arrow: Vec<_> = rects(&frame).into_iter().skip(1).collect();
        let right = 16 + 288 - 4 - 4;
        assert_eq!(
            arrow,
            [
                Rect::new(right, 14, 1, 7),
                Rect::new(right + 1, 15, 1, 5),
                Rect::new(right + 2, 16, 1, 3),
                Rect::new(right + 3, 17, 1, 1),
            ]
        );
    }

    #[test]
    fn the_detail_moves_left_of_the_arrow() {
        let l = list();
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 1, |_| ListItem {
            accent_detail: false,
            detail: Some("1 game"),
            ..ListItem::page("Mega Drive")
        });
        let detail_end = frame
            .scene
            .commands()
            .iter()
            .find_map(|c| match *c {
                crate::DrawCommand::Text { x, span, .. } if frame.scene.text(span) == "1 game" => {
                    Some(x + crate::font::text_width("1 game") as i32)
                }
                _ => None,
            })
            .expect("the detail");
        assert_eq!(detail_end, 16 + 288 - 4 - super::ARROW_COLUMN_PX as i32);
    }

    #[test]
    fn long_labels_are_reported_as_clipped() {
        let l = List::new(Rect::new(0, 0, 60, 12), 12);
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 1, |_| {
            ListItem::new("A very long game title")
        });
        assert!(frame.tree.focused().expect("one row").clipped);
    }

    fn texts(frame: &UiFrame) -> Vec<(i32, i32, &str)> {
        frame
            .scene
            .commands()
            .iter()
            .filter_map(|c| match *c {
                crate::DrawCommand::Text { x, y, span, .. } => Some((x, y, frame.scene.text(span))),
                _ => None,
            })
            .collect()
    }

    fn wrapping(h: u32) -> List {
        List::new(Rect::new(0, 0, 96, h), 12).with_labels(LabelRule::Wrap)
    }

    #[test]
    fn a_label_that_does_not_fit_wraps_to_a_second_line() {
        let l = wrapping(48);
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 2, |i| ListItem {
            detail: Some("12"),
            ..ListItem::page(["Nintendo System", "Mega Drive"][i])
        });
        let rows: Vec<_> = frame.tree.rows().map(|r| (r.rect, r.clipped)).collect();
        assert_eq!(
            rows,
            [
                (Rect::new(0, 0, 96, 24), false),
                (Rect::new(0, 24, 96, 12), false)
            ]
        );
        let t = texts(&frame);
        assert!(t.contains(&(4, 0, "Nintendo")), "{t:?}");
        assert!(t.contains(&(4, 12, "System")), "{t:?}");
        assert!(t.contains(&(4, 24, "Mega Drive")), "{t:?}");
    }

    #[test]
    fn a_label_of_more_than_two_lines_is_cut_on_the_second() {
        let l = wrapping(48);
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 1, |_| {
            ListItem::new("one two three four five six seven")
        });
        assert!(frame.tree.focused().expect("one row").clipped);
        let t = texts(&frame);
        assert_eq!(t.first(), Some(&(4, 0, "one two three")));
        assert!(t.iter().any(|&(_, y, s)| y == 12 && s == "..."), "{t:?}");
    }

    #[test]
    fn the_window_keeps_a_focused_row_of_two_lines_whole() {
        let labels = ["a", "long long long long", "b", "long long long long", "c"];
        let mut l = wrapping(48);
        let theme = Theme::default();
        let draw = |l: &List| {
            let mut frame = UiFrame::new();
            l.draw(&mut frame, &theme, labels.len(), |i| {
                ListItem::new(labels[i])
            });
            frame
        };
        l.select(3, labels.len());
        let frame = draw(&l);
        assert_eq!(l.top(), 2);
        let focused = frame.tree.focused().expect("focus");
        assert_eq!(focused.label, "long long long long");
        assert!(l.rect().contains(&focused.rect), "{:?}", focused.rect);
        assert!(!focused.clipped);
        l.handle(UiAction::Up, labels.len());
        draw(&l);
        assert_eq!((l.cursor(), l.top()), (2, 2), "the window stays");
        l.handle(UiAction::Up, labels.len());
        draw(&l);
        assert_eq!((l.cursor(), l.top()), (1, 1));
    }

    #[test]
    fn a_page_of_rows_of_two_lines_is_the_rows_shown() {
        let mut l = wrapping(48);
        let theme = Theme::default();
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &theme, 10, |_| {
            ListItem::new("long long long long")
        });
        assert_eq!(frame.tree.rows().len(), 2);
        assert_eq!(l.handle(UiAction::Right, 10), ListEvent::Moved);
        assert_eq!(l.cursor(), 2);
    }

    #[test]
    fn rows_that_fit_in_number_but_not_in_lines_scroll() {
        let l = wrapping(48);
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 3, |_| {
            ListItem::new("long long long long")
        });
        let rows: Vec<_> = frame.tree.rows().map(|r| r.rect.w).collect();
        assert_eq!(rows, [96 - SCROLLBAR_WIDTH_PX, 96 - SCROLLBAR_WIDTH_PX]);
    }

    #[test]
    fn a_shortened_label_is_not_reported_as_clipped() {
        let l = List::new(Rect::new(0, 0, 60, 12), 12).with_labels(LabelRule::Shorten);
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 1, |_| {
            ListItem::new("A very long game title")
        });
        let row = frame.tree.focused().expect("one row");
        assert!(!row.clipped);
        assert_eq!(row.rect.h, 12);
    }
}
