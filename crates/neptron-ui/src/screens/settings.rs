//! The Settings page: one row for each settings panel of
//! [`super::SETTINGS_PANELS`]. It writes nothing; its panels write at the
//! global level, also when the page opens from the quick menu.

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::metrics::Metrics;
use crate::snapshot::Level;
use crate::theme::Theme;
use crate::widgets::{HINT_OPEN, LabelRule, List, ListEvent, ListItem};

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
    pub(super) fn new(label: Crumb, metrics: &Metrics, theme: &Theme) -> Self {
        let content = Layout::of(metrics).content;
        let (list_box, list) = chrome::rows_box(content, theme, chrome::box_rows(content, theme));
        Self {
            label,
            list: List::new(list, theme.row_height_px()).with_labels(LabelRule::Wrap),
            list_box,
        }
    }

    /// Lays the page out again for `metrics`, with the cursor kept.
    pub(super) fn relayout(&mut self, metrics: &Metrics, theme: &Theme) {
        let fresh = Self::new(self.label, metrics, theme);
        self.list.set_rect(fresh.list.rect());
        self.list_box = fresh.list_box;
    }

    /// Puts the cursor on the row of panel `index`.
    pub(super) fn select(&mut self, index: usize) {
        self.list.select(index, SETTINGS_PANELS.len());
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
    pub(super) fn draw(
        &self,
        theme: &Theme,
        metrics: &Metrics,
        path: &[&str],
        frame: &mut UiFrame,
    ) {
        chrome::frame_path(frame, theme, metrics, path, "", HINT_OPEN, "Back: return");
        chrome::boxed(frame, theme, self.list_box);
        self.list.draw(frame, theme, SETTINGS_PANELS.len(), |i| {
            ListItem::page(SETTINGS_PANELS.get(i).map_or("", |p| p.label))
        });
    }
}
