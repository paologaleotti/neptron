//! The router: the screen stack, the overlays, and the order of work in one
//! frame.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{GameSource, Level, PageId, SettingKey, Snapshot};
use crate::theme::Theme;

use super::chrome::{self, Layout};
use super::display::DisplayPanel;
use super::game_list::GameList;
use super::home::Home;
use super::options::Options;
use super::overlays::{Confirm, Notices};
use super::page::Crumb;
use super::panel::PagePanel;
use super::pattern;
use super::quick_menu::QuickMenu;
use super::settings::SettingsMenu;
use super::{DISPLAY_PAGE, GRID_PAGE, SETTINGS_PAGE};

/// The deepest screen stack. The longest path is home, game list, game,
/// quick menu, Settings, display panel, option list: seven screens.
pub const MAX_DEPTH: usize = 8;

/// The screens and their state. The app makes one at launch, keeps it, and
/// calls [`Screens::update`] once per frame.
///
/// The state is the stack of open screens with their cursors. Everything
/// else comes from the snapshot of the frame, so a screen is a function of
/// that state, the snapshot and the actions.
pub struct Screens {
    theme: Theme,
    layout: Layout,
    stack: Vec<Screen>,
    confirm: Confirm,
    notices: Notices,
    visible: bool,
}

/// What a screen asks the router to do after an action.
pub(super) enum Nav {
    /// Stay on the screen.
    Stay,
    /// Close the screen.
    Back,
    /// Open a screen on top.
    Open(Target),
}

/// A screen that a different screen opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Games(GameSource),
    QuickMenu,
    /// A page, opened from the row `label`, that writes at `level`. A
    /// reserved page id opens the screen of the UI with that id.
    Page {
        page: PageId,
        level: Level,
        label: Crumb,
    },
    /// The options of the choice row `label` of a page, written at a level.
    Options {
        page: PageId,
        key: SettingKey,
        level: Level,
        label: Crumb,
    },
}

pub(super) enum Screen {
    Home(Home),
    Games(GameList),
    Settings(SettingsMenu),
    Display(DisplayPanel),
    Page(PagePanel),
    Options(Options),
    /// Boxed: its list of changes makes it much larger than the others.
    QuickMenu(Box<QuickMenu>),
    Grid,
    /// A game runs and the UI draws nothing.
    Game,
}

impl Screens {
    /// The home screen, drawn with `theme`.
    pub fn new(theme: Theme) -> Self {
        let layout = Layout::new(&theme);
        let mut stack = Vec::with_capacity(MAX_DEPTH);
        stack.push(Screen::Home(Home::new(&layout, &theme)));
        Self {
            theme,
            layout,
            stack,
            confirm: Confirm::new(),
            notices: Notices::new(),
            visible: true,
        }
    }

    /// Runs one frame: follows the snapshot, applies `actions` in order,
    /// asks for missing data, and draws. `out` is cleared first; it then
    /// holds the scene, the widget tree and the intents of the frame.
    ///
    /// An action goes to the topmost of: the confirm dialog of a pending
    /// signal change, the oldest notice (not while a game runs alone), the
    /// screen on top of the stack.
    ///
    /// The UI has no clock. A held direction repeats only when the app
    /// sends the action again.
    pub fn update(&mut self, snap: &Snapshot, actions: &[UiAction], out: &mut UiFrame) {
        out.clear();
        self.follow(snap);
        self.confirm.begin_frame();
        for &action in actions {
            self.handle(action, snap, &mut out.intents);
            self.follow(snap);
        }
        self.confirm.end_frame(actions);
        if let Some(request) = self.request(snap) {
            out.intents.push(request);
        }
        self.draw(snap, out);
    }

    /// False while a game runs with no menu and no confirm dialog over it.
    /// The app then shows the game alone, sends the UI no actions but the
    /// menu button, and keeps the core running. True otherwise: the core is
    /// paused. Notices wait while it is false.
    pub fn shows_ui(&self) -> bool {
        self.visible
    }

    /// The number of open screens, home included.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    fn follow(&mut self, snap: &Snapshot) {
        let game = self.stack.iter().position(|s| matches!(s, Screen::Game));
        match (snap.session.is_some(), game) {
            (true, None) => {
                self.stack.truncate(MAX_DEPTH - 1);
                self.stack.push(Screen::Game);
            }
            (false, Some(at)) => self.stack.truncate(at.max(1)),
            _ => {}
        }
        match self.stack.last_mut() {
            Some(Screen::Games(list)) => list.sync(snap),
            Some(Screen::Options(options)) => options.sync(snap),
            Some(Screen::Display(panel)) => panel.sync(snap, &self.theme),
            Some(Screen::Page(panel)) => panel.sync(snap, &self.theme),
            _ => {}
        }
        self.confirm.follow(snap.confirm.as_ref());
        self.notices.follow(snap);
        self.visible = !self.in_game() || snap.confirm.is_some();
    }

    fn in_game(&self) -> bool {
        matches!(self.stack.last(), Some(Screen::Game))
    }

    fn handle(&mut self, action: UiAction, snap: &Snapshot, out: &mut Vec<Intent>) {
        if snap.confirm.is_some() {
            self.confirm.handle(action, out);
            return;
        }
        if !self.in_game() && self.notices.current(snap).is_some() {
            self.notices.handle(action, snap, out);
            return;
        }
        let first_new = out.len();
        let nav = match self.stack.last_mut() {
            Some(Screen::Home(home)) => home.handle(action, snap),
            Some(Screen::Games(list)) => list.handle(action, snap, out),
            Some(Screen::Settings(menu)) => menu.handle(action),
            Some(Screen::Display(panel)) => panel.handle(action, snap, out),
            Some(Screen::Page(panel)) => panel.handle(action, snap, out),
            Some(Screen::Options(options)) => options.handle(action, snap, out),
            Some(Screen::QuickMenu(menu)) => menu.handle(action, snap, out),
            Some(Screen::Grid) => pattern::handle(action),
            Some(Screen::Game) if action == UiAction::Menu => Nav::Open(Target::QuickMenu),
            Some(Screen::Game) | None => Nav::Stay,
        };
        if let Some(menu) = self.quick_menu() {
            for intent in out.get(first_new..).unwrap_or(&[]) {
                menu.record(intent, snap);
            }
        }
        match nav {
            Nav::Stay => {}
            Nav::Back => self.back(snap),
            Nav::Open(target) => self.open(target),
        }
    }

    /// Closes the screen on top. The screen under it keeps its cursor, so
    /// the focus is on the row that opened the closed screen.
    ///
    /// A settings page closes only on back, in a frame of its own, so the
    /// snapshot still holds its rows: the quick menu under it forgets the
    /// changes on that page that the app took back.
    fn back(&mut self, snap: &Snapshot) {
        if self.stack.len() <= 1 {
            return;
        }
        let page = matches!(
            self.stack.last(),
            Some(Screen::Page(_) | Screen::Display(_))
        );
        self.stack.pop();
        if page && let Some(menu) = self.quick_menu() {
            menu.forget_taken_back(snap);
        }
    }

    fn open(&mut self, target: Target) {
        if self.stack.len() >= MAX_DEPTH {
            return;
        }
        let (layout, theme) = (&self.layout, &self.theme);
        let screen = match target {
            Target::Games(source) => Screen::Games(GameList::new(source, layout, theme)),
            Target::QuickMenu => Screen::QuickMenu(Box::new(QuickMenu::new(layout, theme))),
            Target::Page {
                page: GRID_PAGE, ..
            } => Screen::Grid,
            Target::Page {
                page: SETTINGS_PAGE,
                label,
                ..
            } => Screen::Settings(SettingsMenu::new(label, layout, theme)),
            Target::Page {
                page: DISPLAY_PAGE,
                label,
                ..
            } => Screen::Display(DisplayPanel::new(label, layout, theme)),
            Target::Page { page, level, label } => {
                Screen::Page(PagePanel::new(page, level, label, layout, theme))
            }
            Target::Options {
                page,
                key,
                level,
                label,
            } => Screen::Options(Options::new(page, key, level, label, layout, theme)),
        };
        self.stack.push(screen);
    }

    fn request(&self, snap: &Snapshot) -> Option<Intent> {
        match self.stack.last()? {
            Screen::Games(list) => list.request(snap),
            Screen::Display(panel) => panel.request(snap),
            Screen::Page(panel) => panel.request(snap),
            Screen::Options(options) => options.request(snap),
            Screen::QuickMenu(menu) => menu.request(snap),
            Screen::Home(_) | Screen::Settings(_) | Screen::Grid | Screen::Game => None,
        }
    }

    fn draw(&self, snap: &Snapshot, out: &mut UiFrame) {
        let (theme, layout) = (&self.theme, &self.layout);
        match self.stack.last() {
            Some(Screen::Home(home)) => {
                out.tree.set_screen("home");
                home.draw(snap, theme, layout, out);
            }
            Some(Screen::Games(list)) => {
                out.tree.set_screen("games");
                list.draw(snap, theme, layout, out);
            }
            Some(Screen::Settings(menu)) => {
                out.tree.set_screen("settings");
                menu.draw(theme, out);
            }
            Some(Screen::Display(panel)) => {
                out.tree.set_screen("display");
                panel.draw(snap, theme, out);
            }
            Some(Screen::Page(panel)) => {
                out.tree.set_screen(panel.screen_id());
                panel.draw(snap, theme, out);
            }
            Some(Screen::Options(options)) => {
                out.tree.set_screen("options");
                options.draw(snap, theme, out);
            }
            Some(Screen::QuickMenu(menu)) => {
                out.tree.set_screen("quick-menu");
                menu.draw(snap, theme, out);
            }
            Some(Screen::Grid) => {
                out.tree.set_screen("pattern/grid");
                pattern::draw(out);
            }
            Some(Screen::Game) | None => {
                out.tree.set_screen("game");
                if snap.confirm.is_some() {
                    chrome::background(out, theme);
                }
            }
        }
        if let Some(pending) = &snap.confirm {
            out.tree.clear();
            out.tree.set_screen("confirm");
            self.confirm.draw(pending, theme, out);
        } else if let Some(notice) = self.notices.current(snap).filter(|_| !self.in_game()) {
            out.tree.clear();
            out.tree.set_screen("notice");
            self.notices.draw(notice, theme, out);
        }
    }

    /// The quick menu, when it is open under the screen on top or is the
    /// screen on top.
    fn quick_menu(&mut self) -> Option<&mut QuickMenu> {
        self.stack.iter_mut().rev().find_map(|s| match s {
            Screen::QuickMenu(menu) => Some(&mut **menu),
            _ => None,
        })
    }

    #[cfg(test)]
    pub(super) fn stack_capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// The screen on top, for the scenes and the tests that start deep in a
    /// list.
    pub(super) fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }
}
