//! The Settings page: one row for each settings panel of
//! [`super::SETTINGS_PANELS`]. It writes nothing; its panels write at the
//! global level, also when the page opens from the quick menu.

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::snapshot::Level;
use crate::theme::Theme;
use crate::widgets::{HINT_OPEN, List, ListEvent, ListItem};

use super::SETTINGS_PANELS;
use super::chrome::{self, Layout};
use super::page::{Crumb, crumb};
use super::router::{Nav, Target};

pub(super) struct SettingsMenu {
    label: Crumb,
    list: List,
    list_box: Rect,
}

impl SettingsMenu {
    /// The Settings page, opened from the row `label`.
    pub(super) fn new(label: Crumb, layout: &Layout, theme: &Theme) -> Self {
        let content = layout.content;
        let (list_box, list) = chrome::rows_box(content, theme, chrome::box_rows(content, theme));
        Self {
            label,
            list: List::new(list, theme.row_height_px()),
            list_box,
        }
    }

    /// The part of this screen in the breadcrumb: the label of the row
    /// that opened it.
    pub(super) fn label(&self) -> &str {
        self.label.as_str()
    }

    pub(super) fn handle(&mut self, action: UiAction) -> Nav {
        match self.list.handle(action, SETTINGS_PANELS.len()) {
            ListEvent::Back => Nav::Back,
            ListEvent::Activated(i) => SETTINGS_PANELS.get(i).map_or(Nav::Stay, |panel| {
                Nav::Open(Target::Page {
                    page: panel.page,
                    level: Level::Global,
                    label: crumb(panel.label),
                })
            }),
            ListEvent::None | ListEvent::Moved | ListEvent::Menu => Nav::Stay,
        }
    }

    /// Draws the page under the breadcrumb `path`.
    pub(super) fn draw(&self, theme: &Theme, path: &[&str], frame: &mut UiFrame) {
        chrome::frame_path(frame, theme, path, "", HINT_OPEN, "Back: return");
        chrome::boxed(frame, theme, self.list_box);
        self.list.draw(frame, theme, SETTINGS_PANELS.len(), |i| {
            ListItem::page(SETTINGS_PANELS.get(i).map_or("", |p| p.label))
        });
    }
}
