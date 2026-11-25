//! The router: the screen stack, the overlays, and the order of work in one
//! frame.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::screensaver;
use crate::snapshot::{GameSource, Level, PageId, SettingKey, Snapshot};
use crate::theme::Theme;

use super::chrome;
use super::display::DisplayPanel;
use super::game_list::GameList;
use super::home::Home;
use super::options::Options;
use super::overlays::{Confirm, Notices};
use super::pad_mapping::PadMappingScreen;
use super::page::{Crumb, crumb};
use super::panel::PagePanel;
use super::pattern;
use super::quick_menu::QuickMenu;
use super::settings::SettingsMenu;
use super::{DISPLAY_PAGE, GRID_PAGE, SETTINGS_PAGE, SETTINGS_PANELS};
use crate::metrics::{Form, Metrics};

/// The label of the row that opens the Settings page, on the home screen
/// and in the quick menu.
const SETTINGS_LABEL: &str = "Settings";

/// The deepest screen stack. The longest path is home, game list, the
/// versions of a game, game, quick menu, Settings, display panel, option
/// list: eight screens.
pub const MAX_DEPTH: usize = 8;

/// The screens and their state. The app makes one at launch, keeps it, and
/// calls [`Screens::update`] once per frame.
///
/// The state is the stack of open screens with their cursors. Everything
/// else comes from the snapshot of the frame, so a screen is a function of
/// that state, the snapshot and the actions.
pub struct Screens {
    theme: Theme,
    metrics: Metrics,
    stack: Vec<Screen>,
    confirm: Confirm,
    notices: Notices,
    pad_mapping: PadMappingScreen,
    visible: bool,
    /// A panel that the app asked to open, and the row to focus.
    opening: Option<(PageId, Option<SettingKey>)>,
    /// A panel that the app asked to open and that could not open.
    open_failed: Option<PageId>,
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
        let metrics = Metrics::of(Form::Landscape, &theme);
        let mut stack = Vec::with_capacity(MAX_DEPTH);
        stack.push(Screen::Home(Home::new(&metrics, &theme)));
        Self {
            theme,
            metrics,
            stack,
            confirm: Confirm::new(),
            notices: Notices::new(),
            pad_mapping: PadMappingScreen::new(),
            visible: true,
            opening: None,
            open_failed: None,
        }
    }

    /// Runs one frame: follows the snapshot, applies `actions` in order,
    /// asks for missing data, and draws. `out` is cleared first; it then
    /// holds the scene, the widget tree and the intents of the frame.
    ///
    /// An action goes to the topmost of: the confirm dialog of a pending
    /// signal change, the guided mapping of a controller, the oldest
    /// notice (not while a game runs alone), the screen on top of the
    /// stack. While the snapshot shows the screensaver, no action goes
    /// anywhere, and the screensaver covers every screen.
    ///
    /// The UI has no clock. A held direction repeats only when the app
    /// sends the action again.
    pub fn update(&mut self, snap: &Snapshot, actions: &[UiAction], out: &mut UiFrame) {
        out.clear();
        let actions = if snap.screensaver.is_some() {
            &[]
        } else {
            actions
        };
        self.follow(snap);
        if self.open_requested(snap) {
            self.follow(snap);
        }
        self.confirm.begin_frame();
        self.pad_mapping.begin_frame();
        for &action in actions {
            let first_new = out.intents.len();
            self.handle(action, snap, &mut out.intents);
            self.follow(snap);
            // The snapshot of this frame shows the value from before a
            // change at the game level, and the change may open a confirm
            // dialog in the next frame. A later action of the frame, such
            // as a back that leaves the page, would act on the old value.
            let changed = out.intents.get(first_new..).is_some_and(|new| {
                new.iter().any(|i| {
                    matches!(
                        i,
                        Intent::SetValue {
                            level: Level::Game,
                            ..
                        }
                    )
                })
            });
            if changed {
                break;
            }
        }
        if let Some(request) = self.request(snap) {
            out.intents.push(request);
        }
        self.draw(snap, out);
    }

    /// False while a game runs with no menu, no confirm dialog and no
    /// guided mapping over it.
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

    /// Opens the settings panel `page` of [`SETTINGS_PANELS`] at the next
    /// [`Screens::update`], as the player would open it from the Settings
    /// page, with the focus on the row `focus` once its rows are in the
    /// snapshot. A row that is not there leaves the focus on the first
    /// row. The app calls it for an answer of the player, such as the
    /// second button of a notice. The rule:
    ///
    /// - The panel on show only moves its focus.
    /// - When a Settings page is open, the screens above it close (as by
    ///   back), and the panel opens from it: never a second Settings page.
    /// - Else, when the stack has no room for the Settings page and the
    ///   panel, the screens above the quick menu, or above the home
    ///   screen, close first (as by back). The deepest stack with no
    ///   Settings page has seven screens: home, a game list, the versions
    ///   of a game, the game, the quick menu, its video page and an option
    ///   list.
    /// - Then the Settings page and the panel open on top.
    ///
    /// When the panel cannot open in that frame (a game starts first, or
    /// no room is made), nothing more opens, and
    /// [`Screens::take_open_failure`] gives the page once.
    ///
    /// Returns false, and opens nothing, while a game runs with no menu,
    /// for a page that is no panel, and for the display panel, which has a
    /// screen of its own.
    pub fn open_panel(&mut self, page: PageId, focus: Option<SettingKey>) -> bool {
        let panel = SETTINGS_PANELS.iter().any(|p| p.page == page);
        if !panel || page == DISPLAY_PAGE || self.in_game() {
            return false;
        }
        self.opening = Some((page, focus));
        true
    }

    /// The panel of the last [`Screens::open_panel`] when it could not
    /// open, once.
    pub fn take_open_failure(&mut self) -> Option<PageId> {
        self.open_failed.take()
    }

    /// Carries out the request of [`Screens::open_panel`]. It runs in
    /// `update`, where the snapshot lets a closed page tell the quick menu
    /// what the app took back.
    /// True when a request was carried out.
    fn open_requested(&mut self, snap: &Snapshot) -> bool {
        let Some((page, focus)) = self.opening.take() else {
            return false;
        };
        let opened = self.open_panel_now(snap, page, focus);
        if !opened {
            self.open_failed = Some(page);
        }
        true
    }

    /// Opens the panel `page` by the rule of [`Screens::open_panel`].
    /// True only when the panel is on show at the end.
    fn open_panel_now(&mut self, snap: &Snapshot, page: PageId, focus: Option<SettingKey>) -> bool {
        if self.in_game() {
            return false;
        }
        if let Some(Screen::Page(panel)) = self.stack.last_mut()
            && panel.page_id() == page
        {
            panel.focus_on(focus);
            return true;
        }
        let Some(index) = SETTINGS_PANELS.iter().position(|p| p.page == page) else {
            return false;
        };
        let settings = self.stack.iter().any(|s| matches!(s, Screen::Settings(_)));
        loop {
            let done = if settings {
                matches!(self.stack.last(), Some(Screen::Settings(_)))
            } else {
                self.stack.len() + 2 <= MAX_DEPTH
            };
            if done {
                break;
            }
            let root = matches!(
                self.stack.last(),
                Some(Screen::Home(_) | Screen::QuickMenu(_) | Screen::Game) | None
            );
            if root && !settings {
                return false;
            }
            let depth = self.stack.len();
            self.back(snap);
            if self.stack.len() == depth {
                return false;
            }
        }
        if !settings {
            self.open(Target::Page {
                page: SETTINGS_PAGE,
                level: Level::Global,
                label: crumb(SETTINGS_LABEL),
            });
        }
        let Some(Screen::Settings(menu)) = self.stack.last_mut() else {
            return false;
        };
        menu.select(index);
        let label = SETTINGS_PANELS.get(index).map_or("", |p| p.label);
        self.open(Target::Page {
            page,
            level: Level::Global,
            label: crumb(label),
        });
        match self.stack.last_mut() {
            Some(Screen::Page(panel)) if panel.page_id() == page => {
                panel.focus_on(focus);
                true
            }
            _ => false,
        }
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
            Some(Screen::Games(list)) => list.sync(snap, &self.metrics, &self.theme),
            Some(Screen::Options(options)) => options.sync(snap),
            Some(Screen::Display(panel)) => panel.sync(snap, &self.metrics, &self.theme),
            Some(Screen::Page(panel)) => panel.sync(snap, &self.metrics, &self.theme),
            _ => {}
        }
        self.confirm.follow(snap.confirm.as_ref());
        self.pad_mapping.follow(snap.pad_mapping.as_ref());
        self.notices.follow(snap);
        self.visible = !self.in_game() || snap.confirm.is_some() || snap.pad_mapping.is_some();
    }

    fn in_game(&self) -> bool {
        matches!(self.stack.last(), Some(Screen::Game))
    }

    fn handle(&mut self, action: UiAction, snap: &Snapshot, out: &mut Vec<Intent>) {
        if let Some(pending) = &snap.confirm {
            self.confirm.handle(action, pending, out);
            return;
        }
        if let Some(mapping) = &snap.pad_mapping {
            self.pad_mapping.handle(action, mapping, out);
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
        let (layout, theme) = (&self.metrics, &self.theme);
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
        if let Some(frames) = snap.screensaver {
            out.tree.set_screen(screensaver::SCREENSAVER_SCREEN);
            screensaver::draw(frames, self.metrics.canvas, out);
            return;
        }
        let (theme, metrics) = (&self.theme, &self.metrics);
        let mut parts = [""; MAX_DEPTH];
        let path = self.path(snap, &mut parts);
        match self.stack.last() {
            Some(Screen::Home(home)) => {
                out.tree.set_screen("home");
                home.draw(snap, theme, metrics, out);
            }
            Some(Screen::Games(list)) => {
                out.tree.set_screen("games");
                list.draw(snap, theme, metrics, out);
            }
            Some(Screen::Settings(menu)) => {
                out.tree.set_screen("settings");
                menu.draw(theme, metrics, path, out);
            }
            Some(Screen::Display(panel)) => {
                out.tree.set_screen("display");
                panel.draw(snap, theme, metrics, path, out);
            }
            Some(Screen::Page(panel)) => {
                out.tree.set_screen(panel.screen_id());
                panel.draw(snap, theme, metrics, path, out);
            }
            Some(Screen::Options(options)) => {
                out.tree.set_screen("options");
                options.draw(snap, theme, metrics, path, out);
            }
            Some(Screen::QuickMenu(menu)) => {
                out.tree.set_screen("quick-menu");
                menu.draw(snap, theme, metrics, path, out);
            }
            Some(Screen::Grid) => {
                out.tree.set_screen("pattern/grid");
                pattern::draw(out, metrics);
            }
            Some(Screen::Game) | None => {
                out.tree.set_screen("game");
                if snap.confirm.is_some() {
                    chrome::background(out, theme, metrics);
                }
            }
        }
        if let Some(pending) = &snap.confirm {
            out.tree.clear();
            out.tree.set_screen("confirm");
            self.confirm.draw(pending, theme, metrics, out);
        } else if let Some(mapping) = &snap.pad_mapping {
            out.tree.clear();
            out.tree.set_screen(PadMappingScreen::screen_id(mapping));
            self.pad_mapping.draw(mapping, theme, metrics, out);
        } else if let Some(notice) = self.notices.current(snap).filter(|_| !self.in_game()) {
            out.tree.clear();
            out.tree.set_screen("notice");
            self.notices.draw(notice, theme, metrics, out);
        }
    }

    /// The breadcrumb of the screen on top, from its root: the labels of
    /// the screens above home, or above the game with the title of the game
    /// first. `parts` holds the text; the result is the part of it in use.
    fn path<'a>(
        &'a self,
        snap: &'a Snapshot,
        parts: &'a mut [&'a str; MAX_DEPTH],
    ) -> &'a [&'a str] {
        let mut count = 0;
        for screen in self.stack.iter().rev() {
            let (part, root) = match screen {
                Screen::Settings(menu) => (menu.label(), false),
                Screen::Display(panel) => (panel.label(), false),
                Screen::Page(panel) => (panel.label(), false),
                Screen::Options(options) => (options.label(), false),
                Screen::QuickMenu(menu) => (menu.label(snap), true),
                Screen::Home(_) | Screen::Games(_) | Screen::Grid | Screen::Game => break,
            };
            let Some(slot) = parts.get_mut(count) else {
                break;
            };
            *slot = part;
            count += 1;
            if root {
                break;
            }
        }
        let path = parts.get_mut(..count).unwrap_or_default();
        path.reverse();
        path
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
