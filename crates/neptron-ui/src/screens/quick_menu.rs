//! The quick menu over a paused game: the rows of the quick menu page,
//! written at the game level, and the prompt that asks where to keep the
//! changes when the player leaves.

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::{Intent, Value};
use crate::snapshot::{Level, SettingKey, Snapshot};
use crate::theme::Theme;
use crate::widgets::{Dialog, DialogContent, DialogEvent};

use super::chrome::{self, Layout};
use super::overlays::draw_dialog;
use super::page::{Page, PageEvent};
use super::router::{Nav, Target};
use super::video;
use super::{GRID_PATTERN, OPEN_DISPLAY, QUICK_MENU_PAGE, QUIT_GAME, RESUME};

/// The settings changed in one visit of the quick menu that the prompt can
/// move. A further setting stays at the game level.
pub const MAX_CHANGES: usize = 16;

/// The buttons of the prompt, from the narrowest level to the widest. The
/// first has the focus, so a quick press keeps a change for this game only.
const KEEP_BUTTONS: [&str; 3] = ["This game", "System", "All games"];
const KEEP_LEVELS: [Level; 3] = [Level::Game, Level::System, Level::Global];

/// How the player leaves the quick menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leave {
    Resume,
    Quit,
}

pub(super) struct QuickMenu {
    page: Page,
    changes: [Option<(SettingKey, Value)>; MAX_CHANGES],
    prompt: Option<(Dialog, Leave)>,
    /// True after a quit request: the menu waits for the session to end
    /// and takes only back, which gives the menu back if the app did not
    /// stop the game.
    quitting: bool,
    content: Rect,
    /// True while the layout has the video block above the rows.
    video: bool,
}

impl QuickMenu {
    pub(super) fn new(layout: &Layout, theme: &Theme) -> Self {
        Self {
            page: Page::new(QUICK_MENU_PAGE, Level::Game, layout.content, theme),
            changes: [None; MAX_CHANGES],
            prompt: None,
            quitting: false,
            content: layout.content,
            video: false,
        }
    }

    /// Puts the video block above the rows while the snapshot has a video
    /// status and the block leaves room for the rows, and removes it
    /// otherwise.
    pub(super) fn sync(&mut self, snap: &Snapshot, theme: &Theme) {
        let video = video::status(snap).is_some() && video::fits(self.content, theme);
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

    /// Notes a change at the game level, made here or in a screen opened
    /// from here, so the prompt can move it on leaving. A reset removes the
    /// note: the value inherits again and there is nothing to move.
    pub(super) fn record(&mut self, intent: &Intent) {
        match *intent {
            Intent::SetValue {
                key,
                level: Level::Game,
                value,
            } => {
                let slot = self
                    .changes
                    .iter()
                    .position(|c| c.is_some_and(|(k, _)| k == key))
                    .or_else(|| self.changes.iter().position(Option::is_none));
                if let Some(entry) = slot.and_then(|i| self.changes.get_mut(i)) {
                    *entry = Some((key, value));
                }
            }
            Intent::ResetValue {
                key,
                level: Level::Game,
            } => {
                for entry in &mut self.changes {
                    if entry.is_some_and(|(k, _)| k == key) {
                        *entry = None;
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn handle(
        &mut self,
        action: UiAction,
        snap: &Snapshot,
        out: &mut Vec<Intent>,
    ) -> Nav {
        if self.quitting {
            if action == UiAction::Back {
                self.quitting = false;
            }
            return Nav::Stay;
        }
        if let Some((dialog, leave)) = &mut self.prompt {
            let leave = *leave;
            return match dialog.handle(action, KEEP_BUTTONS.len()) {
                DialogEvent::Chosen(i) => {
                    let level = KEEP_LEVELS.get(i).copied().unwrap_or(Level::Game);
                    self.keep_at(level, snap, out);
                    self.prompt = None;
                    self.leave(leave, out)
                }
                DialogEvent::Cancelled => {
                    self.prompt = None;
                    Nav::Stay
                }
                DialogEvent::None | DialogEvent::Moved => Nav::Stay,
            };
        }
        match self.page.handle(action, snap) {
            PageEvent::Back | PageEvent::Reserved(RESUME) => self.ask(Leave::Resume, out),
            PageEvent::Reserved(QUIT_GAME) => self.ask(Leave::Quit, out),
            PageEvent::Reserved(OPEN_DISPLAY) => Nav::Open(Target::Display),
            PageEvent::Reserved(GRID_PATTERN) => Nav::Open(Target::Grid),
            PageEvent::Intent(intent) => {
                out.push(intent);
                Nav::Stay
            }
            PageEvent::Pick(key) => Nav::Open(Target::Options {
                page: QUICK_MENU_PAGE,
                key,
                level: Level::Game,
            }),
            PageEvent::Reserved(_) | PageEvent::Stay => Nav::Stay,
        }
    }

    pub(super) fn request(&self, snap: &Snapshot) -> Option<Intent> {
        self.page.request(snap)
    }

    pub(super) fn draw(&self, snap: &Snapshot, theme: &Theme, frame: &mut UiFrame) {
        let title = snap
            .session
            .as_ref()
            .map_or("Quick menu", |s| s.title.as_str());
        chrome::frame(
            frame,
            theme,
            title,
            "Paused",
            "Accept: choose",
            "Back: resume",
        );
        if let Some(status) = video::status(snap).filter(|_| self.video) {
            video::draw(frame, theme, self.content, status);
        }
        self.page.draw(snap, theme, frame);
        if let Some((dialog, _)) = &self.prompt {
            frame.tree.clear();
            frame.tree.set_screen("quick-menu/keep");
            let content = DialogContent {
                title: "Keep the changes?",
                message: "Keep the new settings for this game only, for all games of this system, or for all games?",
                buttons: &KEEP_BUTTONS,
            };
            draw_dialog(dialog, &content, theme, frame);
            chrome::hints(frame, theme, "Accept: choose", "Back: cancel");
        }
    }

    fn ask(&mut self, leave: Leave, out: &mut Vec<Intent>) -> Nav {
        if self.changes.iter().any(Option::is_some) {
            self.prompt = Some((Dialog::new(0), leave));
            return Nav::Stay;
        }
        self.leave(leave, out)
    }

    fn leave(&mut self, leave: Leave, out: &mut Vec<Intent>) -> Nav {
        match leave {
            Leave::Resume => {
                out.push(Intent::Resume);
                Nav::Back
            }
            Leave::Quit => {
                out.push(Intent::QuitGame);
                self.quitting = true;
                Nav::Stay
            }
        }
    }

    /// Moves every noted change from the game level to `level`, when the
    /// row of the change is in the snapshot and allows that level. A change
    /// whose row is gone stays at the game level, the narrowest one. The
    /// game level is where the changes are, so for it there is nothing to
    /// send.
    fn keep_at(&mut self, level: Level, snap: &Snapshot, out: &mut Vec<Intent>) {
        let rows = self.page.rows(snap);
        for (key, value) in self.changes.iter_mut().filter_map(Option::take) {
            let allowed = rows
                .iter()
                .find(|r| r.key == key)
                .is_some_and(|r| r.scopes.allows(level));
            if level == Level::Game || !allowed {
                continue;
            }
            out.push(Intent::SetValue { key, level, value });
            out.push(Intent::ResetValue {
                key,
                level: Level::Game,
            });
        }
    }
}
