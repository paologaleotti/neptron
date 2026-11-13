//! The quick menu over a paused game: the first page of the quick menu,
//! written at the game level, and the prompt that asks where to keep the
//! changes when the player leaves. The pages that open from it note their
//! changes here too.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::intent::{Intent, Value};
use crate::snapshot::{Level, MenuItem, Scopes, SettingKey, SettingValue, Snapshot};
use crate::theme::Theme;
use crate::widgets::{Dialog, DialogContent, DialogEvent, DialogTone};

use super::chrome::{self, Layout};
use super::overlays::draw_dialog;
use super::page::{Page, PageEvent};
use super::router::Nav;
use super::{QUICK_MENU_PAGE, QUIT_GAME, RESUME};

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

/// A change at the game level that the prompt can move: the setting, its
/// value, and the levels its row allows, read when the change was made.
type Change = (SettingKey, Value, Scopes);

pub(super) struct QuickMenu {
    page: Page,
    changes: [Option<Change>; MAX_CHANGES],
    prompt: Option<(Dialog, Leave)>,
    /// True after a quit request: the menu waits for the session to end
    /// and takes only back, which gives the menu back if the app did not
    /// stop the game.
    quitting: bool,
}

impl QuickMenu {
    pub(super) fn new(layout: &Layout, theme: &Theme) -> Self {
        Self {
            page: Page::new(QUICK_MENU_PAGE, Level::Game, layout.content, theme),
            changes: [None; MAX_CHANGES],
            prompt: None,
            quitting: false,
        }
    }

    /// Notes a change at the game level, made here or in a screen opened
    /// from here, so the prompt can move it on leaving. A reset removes the
    /// note: the value inherits again and there is nothing to move.
    ///
    /// The scopes come from the row of the change in `snap`: the page that
    /// made the change is in the snapshot of its frame, and the prompt may
    /// open later on a page that does not hold the row. A row that is not
    /// there allows no level, so its change stays at the game level.
    pub(super) fn record(&mut self, intent: &Intent, snap: &Snapshot) {
        match *intent {
            Intent::SetValue {
                key,
                level: Level::Game,
                value,
            } => {
                let scopes = snap
                    .settings
                    .rows
                    .iter()
                    .find(|r| r.key == key)
                    .map_or(Scopes::NONE, |r| r.scopes);
                let slot = self
                    .changes
                    .iter()
                    .position(|c| c.is_some_and(|(k, _, _)| k == key))
                    .or_else(|| self.changes.iter().position(Option::is_none));
                if let Some(entry) = slot.and_then(|i| self.changes.get_mut(i)) {
                    *entry = Some((key, value, scopes));
                }
            }
            Intent::ResetValue {
                key,
                level: Level::Game,
            } => {
                for entry in &mut self.changes {
                    if entry.is_some_and(|(k, _, _)| k == key) {
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
            PageEvent::Back | PageEvent::Reserved(RESUME) => self.ask(Leave::Resume, snap, out),
            PageEvent::Reserved(QUIT_GAME) => self.ask(Leave::Quit, snap, out),
            event => self.page.nav(event, out),
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
            self.page.hint(snap),
            "Back: resume",
        );
        self.page.draw(snap, theme, frame);
        if let Some((dialog, _)) = &self.prompt {
            frame.tree.clear();
            frame.tree.set_screen("quick-menu/keep");
            let content = DialogContent {
                title: "Keep the changes?",
                message: "Keep the new settings for this game only, for all games of this system, or for all games?",
                buttons: &KEEP_BUTTONS,
                tone: DialogTone::Normal,
            };
            draw_dialog(dialog, &content, theme, frame);
            chrome::hints(frame, theme, "Accept: choose", "Back: cancel");
        }
    }

    fn ask(&mut self, leave: Leave, snap: &Snapshot, out: &mut Vec<Intent>) -> Nav {
        self.forget_taken_back(snap);
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

    /// Forgets the noted changes that the app took back: the guard reverts
    /// a display change that the player did not keep, and the row then
    /// shows the value in effect. A change whose row is not in the page of
    /// the snapshot stays noted.
    pub(super) fn forget_taken_back(&mut self, snap: &Snapshot) {
        let rows = &snap.settings.rows;
        for entry in &mut self.changes {
            let gone = entry.is_some_and(|(key, value, _)| {
                rows.iter()
                    .find(|r| r.key == key)
                    .is_some_and(|row| !holds(row, value))
            });
            if gone {
                *entry = None;
            }
        }
    }

    /// Moves every noted change from the game level to `level`, when the
    /// row of the change allowed that level. A change whose row did not
    /// stays at the game level, the narrowest one. The game level is where
    /// the changes are, so for it there is nothing to send.
    fn keep_at(&mut self, level: Level, snap: &Snapshot, out: &mut Vec<Intent>) {
        self.forget_taken_back(snap);
        for (key, value, scopes) in self.changes.iter_mut().filter_map(Option::take) {
            if level == Level::Game || !scopes.allows(level) {
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

/// Whether `row` shows `value`, set at the game level.
fn holds(row: &MenuItem, value: Value) -> bool {
    let same = match (&row.value, value) {
        (SettingValue::Toggle(on), Value::Toggle(set)) => *on == set,
        (SettingValue::Number { value: now, .. }, Value::Number(set)) => *now == set,
        (SettingValue::Choice { index, .. }, Value::Choice(set)) => *index == set,
        _ => false,
    };
    same && row.source == Level::Game
}
