//! Navigation tests: sequences of actions against the intents and the
//! widget tree they give. No test looks at pixels.

use crate::frame::UiFrame;
use crate::input::UiAction::{self, Accept, Back, Down, Left, Menu, Right, Up};
use crate::intent::Intent;
use crate::snapshot::{GameId, GameSource, GameWindow, SessionInfo, Snapshot};
use crate::theme::Theme;

use super::DISPLAY_PAGE;
use super::Screens;
use super::game_list::{PREFETCH_ROWS, WINDOW_ROWS};
use super::scenes::{
    LONG_LIST, MEGA_DRIVE, SNES, STYLE, TO_DISPLAY, window, with_display, with_systems,
};
use crate::intent::Value;
use crate::snapshot::{Level, SettingsPage};

pub(super) struct Run {
    pub screens: Screens,
    pub frame: UiFrame,
}

impl Run {
    pub fn new() -> Self {
        Self {
            screens: Screens::new(Theme::default()),
            frame: UiFrame::new(),
        }
    }

    pub fn step(&mut self, snap: &Snapshot, actions: &[UiAction]) -> &UiFrame {
        self.screens.update(snap, actions, &mut self.frame);
        &self.frame
    }

    pub fn screen(&self) -> &'static str {
        self.frame.tree.screen()
    }

    pub fn focused(&self) -> Option<&str> {
        self.frame.tree.focused().map(|r| r.label)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.frame.tree.rows().map(|r| r.label).collect()
    }

    pub fn intents(&self) -> &[Intent] {
        &self.frame.intents
    }

    pub fn loads(&self) -> Vec<(usize, usize)> {
        self.frame
            .intents
            .iter()
            .filter_map(|i| match *i {
                Intent::LoadGames { first, count, .. } => Some((first, count)),
                _ => None,
            })
            .collect()
    }
}

const SNES_LIST: GameSource = GameSource::System(SNES);

/// A run on the game list of the Super Nintendo, with its first window.
fn snes_list() -> (Run, Snapshot) {
    let mut run = Run::new();
    let mut snap = with_systems();
    run.step(&snap, &[Down, Accept]);
    snap.games = window(SNES_LIST, 0, LONG_LIST, WINDOW_ROWS);
    run.step(&snap, &[]);
    (run, snap)
}

#[test]
fn home_lists_the_systems_then_favourites_recents_and_display() {
    let mut run = Run::new();
    run.step(&with_systems(), &[]);
    assert_eq!(run.screen(), "home");
    let rows: Vec<_> = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::List)
        .map(|r| (r.label, r.value))
        .collect();
    assert_eq!(
        rows,
        [
            ("Nintendo Entertainment System", Some("312 games")),
            ("Super Nintendo", Some("10000 games")),
            ("Mega Drive", Some("1 game")),
            ("Favourites", None),
            ("Recent games", None),
            ("Display", None),
        ]
    );
    assert_eq!(run.focused(), Some("Nintendo Entertainment System"));
    assert!(run.intents().is_empty());
}

#[test]
fn home_with_no_systems_says_where_the_games_go() {
    let mut run = Run::new();
    run.step(&Snapshot::default(), &[Down, Up, Back, Menu]);
    assert_eq!(run.screen(), "home");
    assert_eq!(run.focused(), Some("Display"));
    let text = run.labels().join(" ");
    for folder in ["roms/nes", "roms/snes", "roms/megadrive", ".zip"] {
        assert!(text.contains(folder), "{folder} is missing from {text:?}");
    }
    assert!(
        run.frame.tree.rows().all(|r| !r.clipped),
        "{:?}",
        run.labels()
    );
}

#[test]
fn a_system_opens_its_game_list_and_back_returns_home() {
    let mut run = Run::new();
    let snap = with_systems();
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.screen(), "games");
    assert_eq!(
        run.intents(),
        [Intent::LoadGames {
            source: SNES_LIST,
            first: 0,
            count: WINDOW_ROWS
        }]
    );
    run.step(&snap, &[Back]);
    assert_eq!(run.screen(), "home");
    assert_eq!(run.focused(), Some("Super Nintendo"));
    run.step(&snap, &[Back, Back]);
    assert_eq!(run.screen(), "home");
}

#[test]
fn favourites_and_recents_open_their_lists() {
    let mut run = Run::new();
    let snap = with_systems();
    run.step(&snap, &[Down, Down, Down, Accept]);
    assert_eq!(run.loads(), [(0, WINDOW_ROWS)]);
    assert!(matches!(
        run.intents()[0],
        Intent::LoadGames {
            source: GameSource::Favourites,
            ..
        }
    ));
    run.step(&snap, &[Back, Down, Accept]);
    assert!(matches!(
        run.intents()[0],
        Intent::LoadGames {
            source: GameSource::Recents,
            ..
        }
    ));
}

#[test]
fn the_game_list_asks_for_nothing_while_the_window_covers_the_need() {
    let (mut run, snap) = snes_list();
    assert!(run.loads().is_empty());
    assert_eq!(run.focused(), Some(super::scenes::title(0).as_str()));
    let visible = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::List)
        .count();
    // The need is the visible rows plus the margin; the window of 128
    // rows covers it until the top row passes 128 - visible - margin.
    let quiet = WINDOW_ROWS - visible - PREFETCH_ROWS;
    for step in 0..quiet + visible - 1 {
        run.step(&snap, &[Down]);
        assert!(run.loads().is_empty(), "step {step}");
    }
    run.step(&snap, &[Down]);
    let top = quiet + 1;
    let first = top + visible / 2 - WINDOW_ROWS / 2;
    assert_eq!(run.loads(), [(first, WINDOW_ROWS)]);
}

#[test]
fn the_game_list_asks_once_per_frame_and_never_shows_a_blank_row() {
    let (mut run, mut snap) = snes_list();
    for _ in 0..200 {
        run.step(&snap, &[Right, Right, Down, Right]);
        let loads = run.loads();
        assert!(loads.len() <= 1, "{loads:?}");
        assert!(
            run.labels().iter().all(|l| *l != "..."),
            "a blank row at {:?}",
            run.focused()
        );
        if let Some(&(first, count)) = loads.first() {
            snap.games = window(SNES_LIST, first, LONG_LIST, count);
        }
    }
}

#[test]
fn the_end_of_a_long_list_asks_for_a_full_window() {
    let (mut run, snap) = snes_list();
    if let Some(super::router::Screen::Games(list)) = run.screens.top_mut() {
        list.select(LONG_LIST - 1, LONG_LIST);
    }
    run.step(&snap, &[]);
    assert_eq!(run.loads(), [(LONG_LIST - WINDOW_ROWS, WINDOW_ROWS)]);
    run.step(&snap, &[Down, Right]);
    assert_eq!(run.focused(), Some("..."));
}

#[test]
fn the_cursor_survives_a_window_of_a_different_list() {
    let (mut run, mut snap) = snes_list();
    run.step(&snap, &[Down, Down, Down]);
    snap.games = GameWindow::new(GameSource::Recents, 0, 0, Vec::new());
    run.step(&snap, &[Down]);
    assert_eq!(run.loads(), [(0, WINDOW_ROWS)]);
    snap.games = window(SNES_LIST, 0, LONG_LIST, WINDOW_ROWS);
    run.step(&snap, &[]);
    assert_eq!(run.focused(), Some(super::scenes::title(3).as_str()));
}

#[test]
fn accept_launches_and_menu_marks_a_favourite() {
    let (mut run, snap) = snes_list();
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.intents(), [Intent::Launch { game: GameId(1) }]);
    run.step(&snap, &[Menu, Left]);
    assert_eq!(run.intents(), [Intent::ToggleFavourite { game: GameId(1) }]);
}

#[test]
fn the_ui_hides_while_a_game_runs_and_returns_to_the_list() {
    let (mut run, mut snap) = snes_list();
    run.step(&snap, &[Down, Down, Accept]);
    snap.session = Some(SessionInfo::new(GameId(2), "Astro Saga"));
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "game");
    assert!(!run.screens.shows_ui());
    assert!(run.frame.scene.commands().is_empty());
    assert!(run.intents().is_empty());
    run.step(&snap, &[Down, Accept, Back]);
    assert_eq!(run.screen(), "game");
    snap.session = None;
    run.step(&snap, &[]);
    assert!(run.screens.shows_ui());
    assert_eq!(run.screen(), "games");
    assert_eq!(run.focused(), Some(super::scenes::title(2).as_str()));
}

#[test]
fn a_list_with_one_game_and_an_empty_list() {
    let mut run = Run::new();
    let mut snap = with_systems();
    run.step(&snap, &[Down, Down, Accept]);
    snap.games = window(GameSource::System(MEGA_DRIVE), 0, 1, WINDOW_ROWS);
    run.step(&snap, &[Down, Down, Right, Up]);
    assert_eq!(run.focused(), Some(super::scenes::title(0).as_str()));
    assert!(run.loads().is_empty());

    snap.games = window(GameSource::System(MEGA_DRIVE), 0, 0, WINDOW_ROWS);
    run.step(&snap, &[Accept, Menu]);
    assert!(run.intents().is_empty());
    assert_eq!(run.focused(), None);
    assert!(run.labels().iter().any(|l| l.contains("no games")));
}

fn field<'a>(run: &'a Run, name: &str) -> Option<&'a str> {
    run.frame
        .tree
        .rows()
        .find(|r| r.label == name)
        .and_then(|r| r.value)
}

#[test]
fn the_display_panel_asks_for_its_page_and_shows_the_mode() {
    let mut run = Run::new();
    let mut snap = with_display();
    let page = core::mem::take(&mut snap.settings);
    run.step(&snap, &TO_DISPLAY);
    assert_eq!(run.screen(), "display");
    assert_eq!(run.intents(), [Intent::LoadSettings { page: DISPLAY_PAGE }]);
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.intents(), [Intent::LoadSettings { page: DISPLAY_PAGE }]);
    snap.settings = page;
    run.step(&snap, &[]);
    assert!(run.intents().is_empty());
    assert_eq!(field(&run, "Output"), Some("VGA-1"));
    assert_eq!(field(&run, "Mode"), Some("640x480 59.94 Hz"));
    assert_eq!(field(&run, "Monitor"), Some("pc_31_120"));
    assert_eq!(run.focused(), Some("Output"));
    run.step(&snap, &[Back]);
    assert_eq!(run.screen(), "home");
}

#[test]
fn the_display_panel_never_changes_a_choice_in_place() {
    let mut run = Run::new();
    let snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down]);
    assert_eq!(run.focused(), Some("Picture style"));
    for action in [Left, Right, Left, Right] {
        run.step(&snap, &[action]);
        assert!(run.intents().is_empty(), "{action:?}");
        assert_eq!(run.screen(), "display");
    }
}

#[test]
fn a_picture_style_applies_only_from_its_option_list() {
    let mut run = Run::new();
    let snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down, Accept]);
    assert_eq!(run.screen(), "options");
    assert_eq!(run.focused(), Some("Auto"));
    assert_eq!(field(&run, "In use"), Some("Auto"));
    let rows: Vec<_> = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::List)
        .map(|r| (r.label, r.value))
        .collect();
    assert_eq!(
        rows,
        [
            ("Auto", Some("31.5 kHz  59.94 Hz")),
            ("Native", Some("Needs a 15 kHz monitor")),
            ("2x", Some("31.5 kHz  59.94 Hz")),
            ("3x", Some("Needs a 45 kHz monitor")),
            ("4x", Some("Needs a 60 kHz monitor")),
        ]
    );

    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.screen(), "options");
    assert!(run.intents().is_empty());
    assert!(
        run.labels()
            .contains(&"Not available. Needs a 15 kHz monitor")
    );

    run.step(&snap, &[Down, Accept]);
    assert_eq!(
        run.intents(),
        [Intent::SetValue {
            key: STYLE,
            level: Level::Global,
            value: Value::Choice(2)
        }]
    );
    assert_eq!(run.screen(), "display");
}

#[test]
fn the_option_in_use_or_back_closes_the_list_with_no_change() {
    let mut run = Run::new();
    let snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down, Accept]);
    run.step(&snap, &[Accept]);
    assert!(run.intents().is_empty());
    assert_eq!(run.screen(), "display");
    run.step(&snap, &[Accept, Down, Down, Back]);
    assert!(run.intents().is_empty());
    assert_eq!(run.screen(), "display");
}

#[test]
fn the_option_list_waits_for_its_page() {
    let mut run = Run::new();
    let mut snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down, Accept]);
    snap.settings = SettingsPage::default();
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.intents(), [Intent::LoadSettings { page: DISPLAY_PAGE }]);
    assert_eq!(run.screen(), "options");
    snap = with_display();
    run.step(&snap, &[]);
    assert!(run.intents().is_empty());
    assert_eq!(run.focused(), Some("Auto"));
}
