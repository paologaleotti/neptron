//! The widget tree: what the control API shows of a frame, so that a test
//! can check a screen without pixels.

use core::ops::Range;

use crate::geom::Rect;
use crate::snapshot::Level;

/// The kind of widget that made a tree row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    /// A row of a list.
    List,
    /// A row of a settings list.
    SettingsList,
    /// The text of a dialog.
    Dialog,
    /// A button of a dialog.
    Button,
    /// A cell of a grid.
    Grid,
    /// A slider.
    Slider,
    /// Text that is not part of a widget, for example a title.
    Label,
}

/// What accept does on a row of a list. The kind decides how the row looks
/// and which hint the footer shows, so the player can tell a row that opens
/// a page from a row that changes a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowKind {
    /// Accept opens a page, an option list or a screen. The row shows an
    /// arrow at its right edge.
    Page,
    /// The row changes its value in place.
    Value,
    /// Accept does something: resume, quit, play, use an option.
    Action,
    /// The row takes no input now.
    Disabled,
}

/// One visible row of the tree, as a view into the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRow<'a> {
    /// The widget that made the row.
    pub widget: WidgetKind,
    /// What accept does, for a row of a list. `None` for a label, a dialog
    /// text, a button, a grid cell or a slider.
    pub kind: Option<RowKind>,
    /// The label.
    pub label: &'a str,
    /// The value as shown, if the row has one.
    pub value: Option<&'a str>,
    /// The level the value comes from, for a settings row.
    pub source: Option<Level>,
    /// Where the row is on the canvas.
    pub rect: Rect,
    /// True when the row has the focus.
    pub focused: bool,
    /// True when the layout cut the text of the row.
    pub clipped: bool,
}

/// The visible rows of one frame and the id of the screen.
///
/// [`UiTree::clear`] keeps the memory, like [`crate::UiScene::clear`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiTree {
    screen: &'static str,
    rows: Vec<StoredRow>,
    text: String,
}

impl UiTree {
    /// An empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes every row and the screen id, and keeps the memory.
    pub fn clear(&mut self) {
        self.screen = "";
        self.rows.clear();
        self.text.clear();
    }

    /// Sets the id of the screen, for example `home` or `settings/display`.
    pub fn set_screen(&mut self, screen: &'static str) {
        self.screen = screen;
    }

    /// The id of the screen. Empty when no screen set it.
    pub fn screen(&self) -> &'static str {
        self.screen
    }

    /// Adds a row. The tree copies the text.
    pub fn push(&mut self, row: TreeRow<'_>) {
        let label = self.store(row.label);
        let value = row.value.map(|v| self.store(v));
        self.rows.push(StoredRow {
            widget: row.widget,
            kind: row.kind,
            label,
            value,
            source: row.source,
            rect: row.rect,
            focused: row.focused,
            clipped: row.clipped,
        });
    }

    /// The rows, in the order the widgets added them.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = TreeRow<'_>> + '_ {
        self.rows.iter().map(|row| self.view(row))
    }

    /// The first row with the focus.
    pub fn focused(&self) -> Option<TreeRow<'_>> {
        self.rows
            .iter()
            .find(|r| r.focused)
            .map(|row| self.view(row))
    }

    fn store(&mut self, text: &str) -> Range<usize> {
        let start = self.text.len();
        self.text.push_str(text);
        start..self.text.len()
    }

    fn view(&self, row: &StoredRow) -> TreeRow<'_> {
        TreeRow {
            widget: row.widget,
            kind: row.kind,
            label: self.text.get(row.label.clone()).unwrap_or(""),
            value: row.value.clone().map(|v| self.text.get(v).unwrap_or("")),
            source: row.source,
            rect: row.rect,
            focused: row.focused,
            clipped: row.clipped,
        }
    }

    #[cfg(test)]
    pub(crate) fn capacity(&self) -> (usize, usize) {
        (self.rows.capacity(), self.text.capacity())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredRow {
    widget: WidgetKind,
    kind: Option<RowKind>,
    label: Range<usize>,
    value: Option<Range<usize>>,
    source: Option<Level>,
    rect: Rect,
    focused: bool,
    clipped: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row<'a>(label: &'a str, value: Option<&'a str>, focused: bool) -> TreeRow<'a> {
        TreeRow {
            widget: WidgetKind::SettingsList,
            kind: Some(RowKind::Value),
            label,
            value,
            source: Some(Level::Global),
            rect: Rect::new(1, 2, 3, 4),
            focused,
            clipped: false,
        }
    }

    #[test]
    fn rows_come_back_as_pushed() {
        let mut tree = UiTree::new();
        tree.set_screen("settings/display");
        tree.push(row("Refresh", Some("60 Hz"), false));
        tree.push(row("Interlace", None, true));
        assert_eq!(tree.screen(), "settings/display");
        let rows: Vec<_> = tree.rows().collect();
        assert_eq!(
            rows,
            [
                row("Refresh", Some("60 Hz"), false),
                row("Interlace", None, true)
            ]
        );
        assert_eq!(tree.focused(), Some(row("Interlace", None, true)));
    }

    #[test]
    fn clear_empties_the_tree_and_keeps_the_memory() {
        let mut tree = UiTree::new();
        tree.set_screen("home");
        tree.push(row("A", Some("B"), true));
        let capacity = tree.capacity();
        tree.clear();
        assert_eq!(tree.screen(), "");
        assert_eq!(tree.rows().len(), 0);
        assert_eq!(tree.focused(), None);
        assert_eq!(tree.capacity(), capacity);
    }
}
