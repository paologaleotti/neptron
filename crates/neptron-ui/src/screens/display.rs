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
use super::video;
use super::{DISPLAY_PAGE, GRID_PATTERN};

/// The lines of the box that shows the output, the mode and the monitor.
const INFO_LINES: u32 = 3;
/// The lines of that box while the video block shows the mode instead.
const INFO_LINES_WITH_VIDEO: u32 = 2;
/// Space between the boxes, in pixels.
const GAP_PX: u32 = 4;

/// The display panel. It writes at the global level: the output and the
/// monitor are global, and the picture style of one game is changed from
/// the quick menu.
///
/// While a game runs with a video status, the info box shows the output
/// and the monitor, and the video block under it shows the mode as the
/// kernel reports it. A skin with rows too tall for the block keeps the
/// panel without it.
pub(super) struct DisplayPanel {
    page: Page,
    content: Rect,
    places: Places,
}

/// Where the boxes of the panel are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Places {
    info_box: Rect,
    info: Rect,
    /// The area of the video block, `None` without it.
    video: Option<Rect>,
    page: Rect,
}

impl Places {
    /// The places with the video block when `video` is true and the block
    /// leaves room for the rows, else the places without it.
    fn new(content: Rect, theme: &Theme, video: bool) -> Self {
        if video {
            let (info_box, info, rest) = Self::info(content, theme, INFO_LINES_WITH_VIDEO);
            if video::fits(rest, theme) {
                return Self {
                    info_box,
                    info,
                    video: Some(rest),
                    page: video::below(rest),
                };
            }
        }
        let (info_box, info, rest) = Self::info(content, theme, INFO_LINES);
        Self {
            info_box,
            info,
            video: None,
            page: rest,
        }
    }

    /// The info box of `lines` lines at the top of `content`, its lines,
    /// and the area under it.
    fn info(content: Rect, theme: &Theme, lines: u32) -> (Rect, Rect, Rect) {
        let (info_box, info) = chrome::rows_box(content, theme, lines);
        let below = info_box.h.saturating_add(GAP_PX).min(content.h);
        let rest = Rect::new(
            content.x,
            chrome::offset(content.y, below),
            content.w,
            content.h.saturating_sub(below),
        );
        (info_box, info, rest)
    }
}

impl DisplayPanel {
    pub(super) fn new(layout: &Layout, theme: &Theme) -> Self {
        let content = layout.content;
        let places = Places::new(content, theme, false);
        Self {
            page: Page::new(DISPLAY_PAGE, Level::Global, places.page, theme),
            content,
            places,
        }
    }

    /// Shows the video block while the snapshot has a video status and the
    /// block leaves room for the rows, and removes it otherwise.
    pub(super) fn sync(&mut self, snap: &Snapshot, theme: &Theme) {
        let places = Places::new(self.content, theme, video::status(snap).is_some());
        if places == self.places {
            return;
        }
        self.places = places;
        self.page.place(places.page, theme);
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
            self.page.accept_hint(snap, "Accept: change"),
            "Back: return",
        );
        let places = &self.places;
        chrome::boxed(frame, theme, places.info_box);
        let (output, mode) = snap
            .display
            .as_ref()
            .map_or(("none", "none"), |d| (d.output.as_str(), d.mode.as_str()));
        let monitor = snap
            .active_profile
            .and_then(|id| snap.profiles.iter().find(|p| p.id == id))
            .map(|p| p.name.as_str())
            .or_else(|| {
                snap.display
                    .as_ref()
                    .map(|d| d.monitor.as_str())
                    .filter(|m| !m.is_empty())
            })
            .unwrap_or("not set");
        let row = theme.row_height_px();
        let line = |i: u32| {
            chrome::padded(
                Rect::new(
                    places.info.x,
                    chrome::offset(places.info.y, i * row),
                    places.info.w,
                    row,
                ),
                theme,
            )
        };
        chrome::field(frame, theme, line(0), "Output", output);
        match (places.video, video::status(snap)) {
            (Some(area), Some(status)) => {
                chrome::field(frame, theme, line(1), "Monitor", monitor);
                video::draw(frame, theme, area, status);
            }
            _ => {
                chrome::field(frame, theme, line(1), "Mode", mode);
                chrome::field(frame, theme, line(2), "Monitor", monitor);
            }
        }
        self.page.draw(snap, theme, frame);
    }
}
