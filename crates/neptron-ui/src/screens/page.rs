//! A settings page of the app in a framed box: the rows of the display
//! panel, of the quick menu and of every other page.

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{ActionId, Level, MenuItem, PageId, SettingKey, SettingValue, Snapshot};
use crate::text::StackText;
use crate::theme::Theme;
use crate::widgets::{SettingsEvent, SettingsList};

use super::RESERVED_ID_MIN;
use super::chrome;
use super::router::{Nav, Target};

/// The longest part of a breadcrumb that a screen keeps, in bytes. A row
/// label is shorter: the label column holds 19 characters.
const CRUMB_BYTES: usize = 64;

/// The label of the row that opened a screen: its part of the breadcrumb.
/// It lives on the stack, so opening a screen allocates nothing for it.
pub(super) type Crumb = StackText<CRUMB_BYTES>;

/// `text` as a [`Crumb`]. A longer text is cut at a character boundary and
/// ends with an ellipsis.
pub(super) fn crumb(text: &str) -> Crumb {
    let mut crumb = Crumb::new();
    crumb.push_cut(text);
    crumb
}

/// What one action did to a [`Page`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PageEvent {
    Stay,
    Back,
    /// A change or an action of the app.
    Intent(Intent),
    /// The player wants to see the options of this choice row.
    Pick(SettingKey, Crumb),
    /// A menu action that the UI carries out itself.
    Reserved(ActionId),
    /// The player wants to see this page, from the row with this label.
    Open(PageId, Crumb),
}

pub(super) struct Page {
    id: PageId,
    level: Level,
    list: SettingsList,
    list_box: Rect,
}

impl Page {
    /// The page `id` in a box at the top of `area`, writing at `level`.
    pub(super) fn new(id: PageId, level: Level, area: Rect, theme: &Theme) -> Self {
        let (list_box, list) = chrome::rows_box(area, theme, chrome::box_rows(area, theme));
        Self {
            id,
            level,
            list: SettingsList::new(list, theme.row_height_px()).with_pickers(),
            list_box,
        }
    }

    /// Moves the page into a box at the top of `area` and keeps the cursor.
    pub(super) fn place(&mut self, area: Rect, theme: &Theme) {
        let (list_box, list) = chrome::rows_box(area, theme, chrome::box_rows(area, theme));
        let cursor = self.list.cursor();
        self.list = SettingsList::new(list, theme.row_height_px()).with_pickers();
        // The rows may not be loaded now; the next action clamps the cursor
        // to the rows it sees, as without a move.
        self.list.select(cursor, usize::MAX);
        self.list_box = list_box;
    }

    pub(super) fn id(&self) -> PageId {
        self.id
    }

    /// The rows of the page, or none while the snapshot holds a different
    /// page.
    pub(super) fn rows<'a>(&self, snap: &'a Snapshot) -> &'a [MenuItem] {
        if snap.settings.is_for(self.id) {
            &snap.settings.rows
        } else {
            &[]
        }
    }

    /// The footer hint of the focused row: what accept or the arrows do
    /// there. Empty for a disabled row and while the rows are not loaded.
    pub(super) fn hint(&self, snap: &Snapshot) -> &'static str {
        self.list.hint(self.rows(snap))
    }

    /// Applies one action. While the snapshot holds a different page the
    /// page takes only back: any other action would clamp the cursor to no
    /// rows and lose the row the player came back to.
    pub(super) fn handle(&mut self, action: UiAction, snap: &Snapshot) -> PageEvent {
        let rows = self.rows(snap);
        if rows.is_empty() && action != UiAction::Back {
            return PageEvent::Stay;
        }
        match self.list.handle(action, rows, self.level) {
            SettingsEvent::Back => PageEvent::Back,
            SettingsEvent::Intent(Intent::Action(id)) if id.0 >= RESERVED_ID_MIN => {
                PageEvent::Reserved(id)
            }
            SettingsEvent::Intent(intent) => PageEvent::Intent(intent),
            SettingsEvent::Pick(i) => rows.get(i).map_or(PageEvent::Stay, |row| {
                PageEvent::Pick(row.key, crumb(&row.label))
            }),
            SettingsEvent::Open(i) => match rows.get(i) {
                Some(row) => match row.value {
                    SettingValue::Page(page) => PageEvent::Open(page, crumb(&row.label)),
                    _ => PageEvent::Stay,
                },
                None => PageEvent::Stay,
            },
            SettingsEvent::None | SettingsEvent::Moved => PageEvent::Stay,
        }
    }

    /// Where `event` takes the player: back, a change of the app in `out`,
    /// the options of a row or a page at the level of this page. A
    /// reserved action is for the screen to carry out, and stays here.
    pub(super) fn nav(&self, event: PageEvent, out: &mut Vec<Intent>) -> Nav {
        match event {
            PageEvent::Back => Nav::Back,
            PageEvent::Intent(intent) => {
                out.push(intent);
                Nav::Stay
            }
            PageEvent::Pick(key, label) => Nav::Open(Target::Options {
                page: self.id,
                key,
                level: self.level,
                label,
            }),
            PageEvent::Open(page, label) => Nav::Open(Target::Page {
                page,
                level: self.level,
                label,
            }),
            PageEvent::Reserved(_) | PageEvent::Stay => Nav::Stay,
        }
    }

    /// The request for the rows of the page, or `None` when the snapshot
    /// holds them.
    pub(super) fn request(&self, snap: &Snapshot) -> Option<Intent> {
        (!snap.settings.is_for(self.id)).then_some(Intent::LoadSettings { page: self.id })
    }

    pub(super) fn draw(&self, snap: &Snapshot, theme: &Theme, frame: &mut UiFrame) {
        chrome::boxed(frame, theme, self.list_box);
        self.list.draw(frame, theme, self.rows(snap));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_label_is_cut_with_an_ellipsis_on_a_character_boundary() {
        let label = "\u{c9}t\u{e9} \u{e0} la plage ".repeat(8);
        assert!(label.len() > CRUMB_BYTES);
        let cut = crumb(&label);
        let text = cut.as_str();
        assert!(text.ends_with("..."), "{text:?}");
        assert!(text.len() <= CRUMB_BYTES);
        let head = text.trim_end_matches("...");
        assert!(label.starts_with(head));
        assert!(head.len() + 4 > CRUMB_BYTES - 3, "the head fills the room");
        assert_eq!(crumb("Audio").as_str(), "Audio");
    }
}
