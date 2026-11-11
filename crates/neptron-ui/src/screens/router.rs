//! The router: the screen stack, the overlays, and the order of work in one
//! frame.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{GameSource, Level, PageId, SettingKey, Snapshot};
use crate::theme::Theme;

use super::chrome::Layout;
use super::display::DisplayPanel;
use super::game_list::GameList;
use super::home::Home;
use super::options::Options;

/// The deepest screen stack. The longest path is home, game list, game,
/// quick menu, display panel, option list: six screens.
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
    Display,
    /// The options of a choice row of a page, written at a level.
    Options {
        page: PageId,
        key: SettingKey,
        level: Level,
    },
    Grid,
}

pub(super) enum Screen {
    Home(Home),
    Games(GameList),
    Display(DisplayPanel),
    Options(Options),
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
            visible: true,
        }
    }

    /// Runs one frame: follows the snapshot, applies `actions` in order,
    /// asks for missing data, and draws. `out` is cleared first; it then
    /// holds the scene, the widget tree and the intents of the frame.
    ///
    /// The UI has no clock. A held direction repeats only when the app
    /// sends the action again.
    pub fn update(&mut self, snap: &Snapshot, actions: &[UiAction], out: &mut UiFrame) {
        out.clear();
        self.follow(snap);
        for &action in actions {
            self.handle(action, snap, &mut out.intents);
            self.follow(snap);
        }
        if let Some(request) = self.request(snap) {
            out.intents.push(request);
        }
        self.draw(snap, out);
    }

    /// False while a game runs with no menu over it. The app then shows the
    /// game alone, sends the UI no actions but the menu button, and keeps
    /// the core running. True otherwise: the core is paused.
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
            _ => {}
        }
        self.visible = !matches!(self.stack.last(), Some(Screen::Game));
    }

    fn handle(&mut self, action: UiAction, snap: &Snapshot, out: &mut Vec<Intent>) {
        let nav = match self.stack.last_mut() {
            Some(Screen::Home(home)) => home.handle(action, snap),
            Some(Screen::Games(list)) => list.handle(action, snap, out),
            Some(Screen::Display(panel)) => panel.handle(action, snap, out),
            Some(Screen::Options(options)) => options.handle(action, snap, out),
            Some(Screen::Game) | None => Nav::Stay,
        };
        match nav {
            Nav::Stay => {}
            Nav::Back => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                }
            }
            Nav::Open(target) => self.open(target),
        }
    }

    fn open(&mut self, target: Target) {
        if self.stack.len() >= MAX_DEPTH {
            return;
        }
        let (layout, theme) = (&self.layout, &self.theme);
        let screen = match target {
            Target::Games(source) => Screen::Games(GameList::new(source, layout, theme)),
            Target::Display => Screen::Display(DisplayPanel::new(layout, theme)),
            Target::Options { page, key, level } => {
                Screen::Options(Options::new(page, key, level, layout, theme))
            }
            Target::Grid => return,
        };
        self.stack.push(screen);
    }

    fn request(&self, snap: &Snapshot) -> Option<Intent> {
        match self.stack.last()? {
            Screen::Games(list) => list.request(snap),
            Screen::Display(panel) => panel.request(snap),
            Screen::Options(options) => options.request(snap),
            Screen::Home(_) | Screen::Game => None,
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
            Some(Screen::Display(panel)) => {
                out.tree.set_screen("display");
                panel.draw(snap, theme, out);
            }
            Some(Screen::Options(options)) => {
                out.tree.set_screen("options");
                options.draw(snap, theme, out);
            }
            Some(Screen::Game) | None => {
                out.tree.set_screen("game");
            }
        }
    }

    /// The screen on top, for the scenes and the tests that start deep in a
    /// list.
    pub(super) fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }
}
