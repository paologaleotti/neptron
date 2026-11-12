//! A settings page of the app in a framed box: the rows of the display
//! panel and of the quick menu.

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{ActionId, Level, MenuItem, PageId, SettingKey, Snapshot};
use crate::theme::Theme;
use crate::widgets::{SettingsEvent, SettingsList};

use super::RESERVED_ID_MIN;
use super::chrome;

/// What one action did to a [`Page`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PageEvent {
    Stay,
    Back,
    /// A change or an action of the app.
    Intent(Intent),
    /// The player wants to see the options of this choice.
    Pick(SettingKey),
    /// A menu action that the UI carries out itself.
    Reserved(ActionId),
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

    pub(super) fn level(&self) -> Level {
        self.level
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

    pub(super) fn handle(&mut self, action: UiAction, snap: &Snapshot) -> PageEvent {
        let rows = self.rows(snap);
        match self.list.handle(action, rows, self.level) {
            SettingsEvent::Back => PageEvent::Back,
            SettingsEvent::Intent(Intent::Action(id)) if id.0 >= RESERVED_ID_MIN => {
                PageEvent::Reserved(id)
            }
            SettingsEvent::Intent(intent) => PageEvent::Intent(intent),
            SettingsEvent::Pick(i) => rows
                .get(i)
                .map_or(PageEvent::Stay, |row| PageEvent::Pick(row.key)),
            SettingsEvent::None | SettingsEvent::Moved => PageEvent::Stay,
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
