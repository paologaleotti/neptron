//! A settings page in the generic screen: the rows of the page in one
//! framed box, written at the level of the screen that opened it. The
//! audio panel and the video page of the quick menu are such pages, and
//! so is any new panel of the Settings page.

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{Level, PageId, Snapshot};
use crate::theme::Theme;

use super::chrome::{self, Layout};
use super::page::{Crumb, Page};
use super::router::Nav;
use super::video;
use super::{AUDIO_PAGE, LIBRARY_PAGE, VIDEO_PAGE};

/// The screen id of a page that has no id of its own.
const PAGE_SCREEN: &str = "page";

pub(super) struct PagePanel {
    page: Page,
    label: Crumb,
    content: Rect,
    /// True for a page that shows the video block above its rows.
    wants_video: bool,
    /// True while the layout has the video block above the rows.
    video: bool,
}

impl PagePanel {
    /// The page `id`, opened from the row `label`, writing at `level`.
    pub(super) fn new(
        id: PageId,
        level: Level,
        label: Crumb,
        layout: &Layout,
        theme: &Theme,
    ) -> Self {
        Self {
            page: Page::new(id, level, layout.content, theme),
            label,
            content: layout.content,
            wants_video: id == VIDEO_PAGE,
            video: false,
        }
    }

    /// The id of the screen in the tree.
    pub(super) fn screen_id(&self) -> &'static str {
        match self.page.id() {
            AUDIO_PAGE => "audio",
            LIBRARY_PAGE => "library",
            VIDEO_PAGE => "quick-menu/video",
            _ => PAGE_SCREEN,
        }
    }

    /// The part of this screen in the breadcrumb: the label of the row
    /// that opened it.
    pub(super) fn label(&self) -> &str {
        self.label.as_str()
    }

    /// Puts the video block above the rows of a page that shows it, while
    /// the snapshot has a video status and the block leaves room for the
    /// rows, and removes it otherwise.
    pub(super) fn sync(&mut self, snap: &Snapshot, theme: &Theme) {
        let video =
            self.wants_video && video::status(snap).is_some() && video::fits(self.content, theme);
        if video == self.video {
            return;
        }
        self.video = video;
        let area = if video {
            video::below(self.content)
        } else {
            self.content
        };
        self.page.place(area, theme);
    }

    pub(super) fn handle(
        &mut self,
        action: UiAction,
        snap: &Snapshot,
        out: &mut Vec<Intent>,
    ) -> Nav {
        let event = self.page.handle(action, snap);
        self.page.nav(event, out)
    }

    pub(super) fn request(&self, snap: &Snapshot) -> Option<Intent> {
        self.page.request(snap)
    }

    /// Draws the page under the breadcrumb `path`.
    pub(super) fn draw(&self, snap: &Snapshot, theme: &Theme, path: &[&str], frame: &mut UiFrame) {
        chrome::frame_path(frame, theme, path, "", self.page.hint(snap), "Back: return");
        if let Some(status) = video::status(snap).filter(|_| self.video) {
            video::draw(frame, theme, self.content, status);
        }
        self.page.draw(snap, theme, frame);
    }
}
