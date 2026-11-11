//! The display panel: what is on screen now, and the rows of the display
//! page. The panel never changes the display itself: a change is an
//! intent, the app validates it and switches, and the snapshot then holds
//! a confirmation that the confirm dialog shows.

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{Level, Snapshot};
use crate::theme::Theme;

use super::chrome::{self, Layout};
use super::page::{Page, PageEvent};
use super::router::{Nav, Target};
use super::{DISPLAY_PAGE, GRID_PATTERN};

/// The lines of the box that shows the output, the mode and the monitor.
const INFO_LINES: u32 = 3;
/// Space between the boxes, in pixels.
const GAP_PX: u32 = 4;

/// The display panel. It writes at the global level: the output and the
/// monitor are global, and the picture style of one game is changed from
/// the quick menu.
pub(super) struct DisplayPanel {
    page: Page,
    info_box: Rect,
    info: Rect,
}

impl DisplayPanel {
    pub(super) fn new(layout: &Layout, theme: &Theme) -> Self {
        let content = layout.content;
        let (info_box, info) = chrome::rows_box(content, theme, INFO_LINES);
        let below = info_box.h + GAP_PX;
        let area = Rect::new(
            content.x,
            chrome::offset(content.y, below),
            content.w,
            content.h.saturating_sub(below),
        );
        Self {
            page: Page::new(DISPLAY_PAGE, Level::Global, area, theme),
            info_box,
            info,
        }
    }

    pub(super) fn handle(
        &mut self,
        action: UiAction,
        snap: &Snapshot,
        out: &mut Vec<Intent>,
    ) -> Nav {
        match self.page.handle(action, snap) {
            PageEvent::Back => Nav::Back,
            PageEvent::Intent(intent) => {
                out.push(intent);
                Nav::Stay
            }
            PageEvent::Pick(key) => Nav::Open(Target::Options {
                page: self.page.id(),
                key,
                level: self.page.level(),
            }),
            PageEvent::Reserved(GRID_PATTERN) => Nav::Open(Target::Grid),
            PageEvent::Reserved(_) | PageEvent::Stay => Nav::Stay,
        }
    }

    pub(super) fn request(&self, snap: &Snapshot) -> Option<Intent> {
        self.page.request(snap)
    }

    pub(super) fn draw(&self, snap: &Snapshot, theme: &Theme, frame: &mut UiFrame) {
        chrome::frame(
            frame,
            theme,
            "Display",
            "",
            "Accept: change",
            "Back: return",
        );
        chrome::boxed(frame, theme, self.info_box);
        let (output, mode) = snap
            .display
            .as_ref()
            .map_or(("none", "none"), |d| (d.output.as_str(), d.mode.as_str()));
        let monitor = snap
            .active_profile
            .and_then(|id| snap.profiles.iter().find(|p| p.id == id))
            .map_or("not set", |p| p.name.as_str());
        let row = theme.row_height_px();
        let line = |i: u32| {
            chrome::padded(
                Rect::new(
                    self.info.x,
                    chrome::offset(self.info.y, i * row),
                    self.info.w,
                    row,
                ),
                theme,
            )
        };
        chrome::field(frame, theme, line(0), "Output", output);
        chrome::field(frame, theme, line(1), "Mode", mode);
        chrome::field(frame, theme, line(2), "Monitor", monitor);
        self.page.draw(snap, theme, frame);
    }
}
