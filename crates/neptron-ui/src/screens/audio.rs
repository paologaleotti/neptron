//! The audio panel: the rows of the audio page, written at the global
//! level. A choice row opens the list of its options.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{Level, Snapshot};
use crate::theme::Theme;

use super::AUDIO_PAGE;
use super::chrome::{self, Layout};
use super::page::{Page, PageEvent};
use super::router::{Nav, Target};

/// The audio panel: the volume and the output device of every game.
pub(super) struct AudioPanel {
    page: Page,
}

impl AudioPanel {
    pub(super) fn new(layout: &Layout, theme: &Theme) -> Self {
        Self {
            page: Page::new(AUDIO_PAGE, Level::Global, layout.content, theme),
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
                page: AUDIO_PAGE,
                key,
                level: Level::Global,
            }),
            PageEvent::Open(_) | PageEvent::Reserved(_) | PageEvent::Stay => Nav::Stay,
        }
    }

    pub(super) fn request(&self, snap: &Snapshot) -> Option<Intent> {
        self.page.request(snap)
    }

    pub(super) fn draw(&self, snap: &Snapshot, theme: &Theme, frame: &mut UiFrame) {
        chrome::frame(
            frame,
            theme,
            "Audio",
            "",
            self.page.hint(snap),
            "Back: return",
        );
        self.page.draw(snap, theme, frame);
    }
}
