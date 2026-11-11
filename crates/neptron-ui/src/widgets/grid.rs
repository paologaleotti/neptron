use crate::canvas::SpriteId;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::text::{self, Align};
use crate::theme::Theme;
use crate::tree::{TreeRow, WidgetKind};

use super::list::MAX_VISIBLE_ROWS;
use super::{add, framed, to_u32};

/// The most columns a grid has: one per canvas column.
pub const MAX_COLUMNS: usize = crate::canvas::UI_WIDTH as usize;

const CELL_GAP_PX: u32 = 2;
const FOCUS_FRAME_PX: u32 = 2;

/// One cell of a [`Grid`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridCell<'a> {
    /// The label, at the bottom of the cell.
    pub label: &'a str,
    /// An image at the top-left of the cell, clipped to the cell.
    pub sprite: Option<SpriteId>,
}

/// What one action did to a [`Grid`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridEvent {
    /// Nothing changed.
    None,
    /// The cursor moved.
    Moved,
    /// The player accepted the cell at this index.
    Activated(usize),
    /// The player pressed back.
    Back,
    /// The player pressed menu.
    Menu,
}

/// Cells in rows of a fixed number of columns, filled left to right. The
/// D-pad moves in two directions; the grid scrolls by whole rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    rect: Rect,
    columns: usize,
    cell_height_px: u32,
    cursor: usize,
    top_row: usize,
}

impl Grid {
    /// A grid in `rect` with `columns` columns (1 to [`MAX_COLUMNS`]) of cells
    /// `cell_height_px` tall.
    pub fn new(rect: Rect, columns: usize, cell_height_px: u32) -> Self {
        Self {
            rect,
            columns: columns.clamp(1, MAX_COLUMNS),
            cell_height_px: cell_height_px.max(1),
            cursor: 0,
            top_row: 0,
        }
    }

    /// The focused cell.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The number of cell rows that fit, at least 1 and at most
    /// [`MAX_VISIBLE_ROWS`].
    pub fn visible_rows(&self) -> usize {
        ((self.rect.h / self.cell_height_px) as usize).clamp(1, MAX_VISIBLE_ROWS)
    }

    /// Moves the cursor to `index` and scrolls it into view.
    pub fn select(&mut self, index: usize, len: usize) {
        self.cursor = index;
        (self.cursor, self.top_row) = self.window(len);
    }

    /// Applies one action to a grid of `len` cells.
    pub fn handle(&mut self, action: UiAction, len: usize) -> GridEvent {
        self.select(self.cursor, len);
        let last = len.saturating_sub(1);
        let c = self.cursor;
        let target = match action {
            UiAction::Left => c.saturating_sub(1),
            UiAction::Right => c.saturating_add(1).min(last),
            UiAction::Up => c.checked_sub(self.columns).unwrap_or(c),
            UiAction::Down if c.saturating_add(self.columns) <= last => c + self.columns,
            UiAction::Down if c / self.columns < last / self.columns => last,
            UiAction::Down => c,
            UiAction::Accept if len > 0 => return GridEvent::Activated(c),
            UiAction::Accept => return GridEvent::None,
            UiAction::Back => return GridEvent::Back,
            UiAction::Menu => return GridEvent::Menu,
        };
        if target == c {
            return GridEvent::None;
        }
        self.select(target, len);
        GridEvent::Moved
    }

    /// Draws the visible cells of a grid of `len` cells. `cell` gives the
    /// cell at an index.
    pub fn draw<'a>(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        len: usize,
        cell: impl Fn(usize) -> GridCell<'a>,
    ) {
        let (cursor, top_row) = self.window(len);
        let first = top_row.saturating_mul(self.columns);
        let count = self.visible_rows() * self.columns;
        let cell_w = self.rect.w / to_u32(self.columns);
        for index in first..len.min(first.saturating_add(count)) {
            let slot = index - first;
            let x = add(
                self.rect.x,
                to_u32(slot % self.columns).saturating_mul(cell_w),
            );
            let row_offset = to_u32(slot / self.columns).saturating_mul(self.cell_height_px);
            let y = add(self.rect.y, row_offset);
            let outer = Rect::new(x, y, cell_w, self.cell_height_px).inset(CELL_GAP_PX / 2);
            let focused = index == cursor;
            let frame_color = if focused {
                theme.colors.focus
            } else {
                theme.colors.panel
            };
            framed(
                &mut frame.scene,
                outer,
                frame_color,
                theme.colors.panel,
                FOCUS_FRAME_PX,
            );
            let inner = outer.inset(FOCUS_FRAME_PX);
            let entry = cell(index);
            if let Some(id) = entry.sprite {
                frame.scene.clip(Some(inner));
                frame.scene.sprite(
                    id,
                    add(inner.x, theme.padding_px()),
                    add(inner.y, theme.padding_px()),
                );
                frame.scene.clip(None);
            }
            let label_y =
                i32::try_from(inner.bottom() - i64::from(theme.row_height_px())).unwrap_or(inner.y);
            let label_box = Rect::new(inner.x, label_y, inner.w, theme.row_height_px());
            let color = if focused {
                theme.colors.text
            } else {
                theme.colors.text_dim
            };
            let clipped = text::draw_in(
                &mut frame.scene,
                label_box,
                Align::Center,
                color,
                entry.label,
            );
            frame.tree.push(TreeRow {
                widget: WidgetKind::Grid,
                label: entry.label,
                value: None,
                source: None,
                rect: outer,
                focused,
                clipped,
            });
        }
    }

    fn window(&self, len: usize) -> (usize, usize) {
        let rows = self.visible_rows();
        let cursor = self.cursor.min(len.saturating_sub(1));
        let row = cursor / self.columns;
        let mut top = self.top_row.min(row);
        if row >= top.saturating_add(rows) {
            top = row - (rows - 1);
        }
        (cursor, top)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_huge_column_count_is_capped() {
        let mut g = Grid::new(Rect::new(16, 12, 288, 120), usize::MAX, u32::MAX);
        assert_eq!(g.handle(UiAction::Down, usize::MAX), GridEvent::Moved);
        assert_eq!(g.cursor(), MAX_COLUMNS);
        assert_eq!(g.handle(UiAction::Right, usize::MAX), GridEvent::Moved);
        g.select(usize::MAX, usize::MAX);
        let mut frame = UiFrame::new();
        g.draw(&mut frame, &Theme::default(), usize::MAX, |_| GridCell {
            label: "x",
            sprite: None,
        });
        let drawn = frame.tree.rows().len();
        assert!((1..=MAX_COLUMNS).contains(&drawn), "{drawn}");
    }

    fn grid() -> Grid {
        Grid::new(Rect::new(16, 12, 288, 120), 3, 60)
    }

    #[test]
    fn the_d_pad_moves_in_two_directions() {
        let mut g = grid();
        assert_eq!(g.handle(UiAction::Right, 7), GridEvent::Moved);
        assert_eq!(g.handle(UiAction::Down, 7), GridEvent::Moved);
        assert_eq!(g.cursor(), 4);
        assert_eq!(g.handle(UiAction::Down, 7), GridEvent::Moved);
        assert_eq!(g.cursor(), 6);
        assert_eq!(g.handle(UiAction::Down, 7), GridEvent::None);
        assert_eq!(g.handle(UiAction::Up, 7), GridEvent::Moved);
        assert_eq!(g.cursor(), 3);
        assert_eq!(g.handle(UiAction::Left, 7), GridEvent::Moved);
        assert_eq!(g.cursor(), 2);
        assert_eq!(g.handle(UiAction::Accept, 7), GridEvent::Activated(2));
    }

    #[test]
    fn down_into_a_short_last_row_goes_to_the_last_cell() {
        let mut g = grid();
        g.select(2, 5);
        assert_eq!(g.handle(UiAction::Down, 5), GridEvent::Moved);
        assert_eq!(g.cursor(), 4);
    }

    #[test]
    fn the_grid_scrolls_by_rows() {
        let mut g = grid();
        g.select(8, 9);
        let mut frame = UiFrame::new();
        g.draw(&mut frame, &Theme::default(), 9, |_| GridCell {
            label: "Cell",
            sprite: None,
        });
        assert_eq!(frame.tree.rows().len(), 6);
        assert_eq!(
            frame.tree.focused().map(|r| r.rect),
            Some(Rect::new(209, 73, 94, 58))
        );
    }

    #[test]
    fn back_menu_and_empty_grids() {
        let mut g = grid();
        assert_eq!(g.handle(UiAction::Accept, 0), GridEvent::None);
        assert_eq!(g.handle(UiAction::Back, 0), GridEvent::Back);
        assert_eq!(g.handle(UiAction::Menu, 0), GridEvent::Menu);
    }

    #[test]
    fn a_sprite_is_clipped_to_its_cell() {
        let g = grid();
        let mut frame = UiFrame::new();
        g.draw(&mut frame, &Theme::default(), 1, |_| GridCell {
            label: "NES",
            sprite: Some(SpriteId(0)),
        });
        let clips: Vec<_> = frame
            .scene
            .commands()
            .iter()
            .filter_map(|c| match c {
                crate::DrawCommand::Clip(clip) => Some(*clip),
                _ => None,
            })
            .collect();
        assert_eq!(clips, [Some(Rect::new(19, 15, 90, 54)), None]);
    }
}
