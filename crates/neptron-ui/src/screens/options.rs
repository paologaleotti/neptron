//! The options of one choice row, each with its detail, before one
//! applies. An option the player cannot choose is dim, with its reason.

use core::fmt::Write;

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::{Intent, Value};
use crate::snapshot::{ChoiceOption, Level, MenuItem, PageId, SettingKey, SettingValue, Snapshot};
use crate::text::StackText;
use crate::theme::Theme;
use crate::tree::{RowKind, TreeRow, WidgetKind};
use crate::widgets::{List, ListEvent, ListItem};

use super::chrome::{self, Layout};
use super::page::Crumb;
use super::router::Nav;

/// Lines of the box under the list that shows the detail or the reason of
/// the focused option in full.
const HELP_LINES: u32 = 2;
/// The longest help text, in bytes: more than the lines of the help box
/// hold.
const HELP_BYTES: usize = 192;
/// Space between the boxes, in pixels.
const GAP_PX: u32 = 4;
/// Width of the column that marks the option in use, in pixels.
const MARK_COLUMN_PX: u32 = 8;
/// Size of the mark of the option in use, in pixels.
const MARK_PX: u32 = 4;

pub(super) struct Options {
    label: Crumb,
    page: PageId,
    key: SettingKey,
    level: Level,
    list: List,
    list_box: Rect,
    help_box: Rect,
    help: Rect,
    /// True once the cursor was put on the option in use.
    placed: bool,
}

impl Options {
    /// The options of the row `key` of `page`, whose label is `label`,
    /// written at `level`.
    pub(super) fn new(
        page: PageId,
        key: SettingKey,
        level: Level,
        label: Crumb,
        layout: &Layout,
        theme: &Theme,
    ) -> Self {
        let content = layout.content;
        let (help_box, help) = chrome::rows_box_bottom(content, theme, HELP_LINES);
        let above = Rect::new(
            content.x,
            content.y,
            content.w,
            content.h.saturating_sub(help_box.h + GAP_PX),
        );
        let (list_box, rows) = chrome::rows_box(above, theme, chrome::box_rows(above, theme));
        let list = Rect::new(
            chrome::offset(rows.x, MARK_COLUMN_PX),
            rows.y,
            rows.w.saturating_sub(MARK_COLUMN_PX),
            rows.h,
        );
        Self {
            label,
            page,
            key,
            level,
            list: List::new(list, theme.row_height_px()),
            list_box,
            help_box,
            help,
            placed: false,
        }
    }

    /// Puts the cursor on the option in use the first time the snapshot
    /// holds the row.
    pub(super) fn sync(&mut self, snap: &Snapshot) {
        if let Some((_, index, options)) = self.choice(snap) {
            let cursor = if self.placed {
                self.list.cursor()
            } else {
                index
            };
            self.list.select(cursor, options.len());
            self.placed = true;
        }
    }

    pub(super) fn handle(
        &mut self,
        action: UiAction,
        snap: &Snapshot,
        out: &mut Vec<Intent>,
    ) -> Nav {
        if action == UiAction::Back {
            return Nav::Back;
        }
        let Some((item, index, options)) = self.choice(snap) else {
            return Nav::Stay;
        };
        let ListEvent::Activated(i) = self.list.handle(action, options.len()) else {
            return Nav::Stay;
        };
        if !options.get(i).is_some_and(ChoiceOption::is_enabled) {
            return Nav::Stay;
        }
        if i != index && item.scopes.allows(self.level) {
            out.push(Intent::SetValue {
                key: self.key,
                level: self.level,
                value: Value::Choice(i),
            });
        }
        Nav::Back
    }

    /// The part of this screen in the breadcrumb: the label of the row
    /// that opened it.
    pub(super) fn label(&self) -> &str {
        self.label.as_str()
    }

    pub(super) fn request(&self, snap: &Snapshot) -> Option<Intent> {
        (!snap.settings.is_for(self.page)).then_some(Intent::LoadSettings { page: self.page })
    }

    /// Draws the list under the breadcrumb `path`.
    pub(super) fn draw(&self, snap: &Snapshot, theme: &Theme, path: &[&str], frame: &mut UiFrame) {
        let choice = self.choice(snap);
        chrome::frame_path(frame, theme, path, "", "Accept: use", "Back: cancel");
        chrome::boxed(frame, theme, self.list_box);
        chrome::boxed(frame, theme, self.help_box);
        let Some((_, index, options)) = choice else {
            return;
        };
        self.list.draw(frame, theme, options.len(), |i| {
            let option = options.get(i);
            ListItem {
                label: option.map_or("", |o| o.label.as_str()),
                detail: option
                    .map(|o| o.unavailable.as_deref().unwrap_or(&o.detail))
                    .filter(|d| !d.is_empty()),
                enabled: option.is_some_and(ChoiceOption::is_enabled),
                kind: RowKind::Action,
            }
        });
        self.draw_mark(frame, theme, index, options);
        let Some(option) = options.get(self.list.cursor()) else {
            return;
        };
        let help = chrome::padded(self.help, theme);
        match &option.unavailable {
            Some(reason) => {
                let mut text = StackText::<HELP_BYTES>::new();
                // `StackText` never fails; it drops what does not fit.
                let _ = write!(text, "Not available. {reason}");
                chrome::paragraph(frame, theme, help, theme.colors.text, text.as_str());
            }
            None => {
                chrome::paragraph(frame, theme, help, theme.colors.accent, &option.detail);
            }
        }
    }

    fn draw_mark(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        index: usize,
        options: &[ChoiceOption],
    ) {
        let Some(option) = options.get(index) else {
            return;
        };
        let top = self.list.top();
        let Some(slot) = index
            .checked_sub(top)
            .filter(|s| *s < self.list.visible_rows())
        else {
            return;
        };
        let row = theme.row_height_px();
        let y = chrome::offset(self.list.rect().y, slot as u32 * row + (row - MARK_PX) / 2);
        let x = self
            .list
            .rect()
            .x
            .saturating_sub(((MARK_COLUMN_PX + MARK_PX) / 2) as i32);
        let mark = Rect::new(x, y, MARK_PX, MARK_PX);
        frame.scene.rect(mark, theme.colors.accent);
        frame.tree.push(TreeRow {
            widget: WidgetKind::Label,
            kind: None,
            label: "In use",
            value: Some(&option.label),
            source: None,
            rect: mark,
            focused: false,
            clipped: false,
        });
    }

    fn choice<'a>(&self, snap: &'a Snapshot) -> Option<(&'a MenuItem, usize, &'a [ChoiceOption])> {
        if !snap.settings.is_for(self.page) {
            return None;
        }
        let item = snap.settings.rows.iter().find(|r| r.key == self.key)?;
        match &item.value {
            SettingValue::Choice { index, options } => Some((item, *index, options)),
            _ => None,
        }
    }
}
