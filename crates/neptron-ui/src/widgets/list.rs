use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::text::{self, Align};
use crate::theme::Theme;
use crate::tree::{RowKind, TreeRow, WidgetKind};

use super::{ARROW_COLUMN_PX, add, draw_arrow, to_u32};

/// Width of the scroll bar, and the gap before it, in pixels.
pub const SCROLLBAR_WIDTH_PX: u32 = 4;

/// The most rows a list draws: one per canvas line. A taller rectangle
/// draws no more, so a huge rectangle costs no more work than the canvas.
pub const MAX_VISIBLE_ROWS: usize = crate::canvas::UI_HEIGHT as usize;

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

/// A vertical list with one focused row. Up and down move by one row; left
/// and right move by one page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct List {
    rect: Rect,
    row_height_px: u32,
    cursor: usize,
    top: usize,
    arrows: bool,
}

impl<'a> ListItem<'a> {
    /// An enabled action row with a label only.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            detail: None,
            enabled: true,
            kind: RowKind::Action,
        }
    }

    /// An enabled row with a label only that opens a page.
    pub fn page(label: &'a str) -> Self {
        Self {
            kind: RowKind::Page,
            ..Self::new(label)
        }
    }
}

impl List {
    /// A list in `rect` with rows of `row_height_px`, cursor on the first
    /// row.
    pub fn new(rect: Rect, row_height_px: u32) -> Self {
        Self {
            rect,
            row_height_px: row_height_px.max(1),
            cursor: 0,
            top: 0,
            arrows: true,
        }
    }

    /// The same list, with no arrow on its [`RowKind::Page`] rows. Only the
    /// home screen with no games uses it: its one row is in a golden image
    /// of tier 3 that a change must not touch.
    pub fn without_arrows(mut self) -> Self {
        self.arrows = false;
        self
    }

    /// The area of the list.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// The focused row.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The first visible row.
    pub fn top(&self) -> usize {
        self.top
    }

    /// The number of rows that fit, at least 1 and at most
    /// [`MAX_VISIBLE_ROWS`].
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
        let page = self.visible_rows();
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
        let (cursor, top) = self.window(len);
        let rows = self.visible_rows();
        let scrolls = self.scrolls(len);
        for index in top..len.min(top.saturating_add(rows)) {
            let row = self.row_rect(index - top, scrolls);
            let entry = item(index);
            let focused = index == cursor;
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
            let padded = Rect::new(
                add(row.x, theme.padding_px()),
                row.y,
                row.w.saturating_sub(2 * theme.padding_px()),
                row.h,
            );
            let arrow = self.arrows && entry.enabled && entry.kind == RowKind::Page;
            let inner = if arrow {
                let arrow_color = if focused {
                    color
                } else {
                    theme.colors.text_dim
                };
                draw_arrow(&mut frame.scene, padded, arrow_color);
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
                .map_or(0, |d| crate::font::text_width(d).min(inner.w / 2));
            if let Some(detail) = entry.detail {
                let at = Rect::new(add(inner.x, inner.w - detail_w), inner.y, detail_w, inner.h);
                let detail_color = if focused {
                    color
                } else {
                    theme.colors.text_dim
                };
                text::draw_in(&mut frame.scene, at, Align::Right, detail_color, detail);
            }
            let gap = if detail_w > 0 {
                crate::font::ADVANCE
            } else {
                0
            };
            let label_box = Rect::new(
                inner.x,
                inner.y,
                inner.w.saturating_sub(detail_w + gap),
                inner.h,
            );
            let clipped =
                text::draw_in(&mut frame.scene, label_box, Align::Left, color, entry.label);
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
        if scrolls {
            self.draw_scrollbar(frame, theme, len, top);
        }
    }

    pub(super) fn scrolls(&self, len: usize) -> bool {
        len > self.visible_rows()
    }

    pub(super) fn row_rect(&self, slot: usize, scrolls: bool) -> Rect {
        let bar = if scrolls { SCROLLBAR_WIDTH_PX } else { 0 };
        let offset = to_u32(slot).saturating_mul(self.row_height_px);
        Rect::new(
            self.rect.x,
            add(self.rect.y, offset),
            self.rect.w.saturating_sub(bar),
            self.row_height_px,
        )
    }

    pub(super) fn draw_scrollbar(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        len: usize,
        top: usize,
    ) {
        let rows = self.visible_rows();
        let track_h = to_u32(rows)
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
        (self.cursor, self.top) = self.window(len);
    }

    pub(super) fn window(&self, len: usize) -> (usize, usize) {
        let rows = self.visible_rows();
        let cursor = self.cursor.min(len.saturating_sub(1));
        let mut top = self.top.min(cursor);
        if cursor >= top.saturating_add(rows) {
            top = cursor - (rows - 1);
        }
        (cursor, top)
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
    fn a_list_without_arrows_draws_none_and_keeps_the_kind() {
        let l = list().without_arrows();
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 1, |_| {
            ListItem::page("Display")
        });
        assert_eq!(rects(&frame).len(), 1, "the focus bar only");
        assert_eq!(
            frame.tree.focused().and_then(|r| r.kind),
            Some(RowKind::Page)
        );
    }

    #[test]
    fn the_detail_moves_left_of_the_arrow() {
        let l = list();
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), 1, |_| ListItem {
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
}
