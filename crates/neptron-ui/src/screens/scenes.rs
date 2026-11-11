//! The scenes of the screens: each one drives a [`Screens`] router with
//! fixture data and actions to the state it shows.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::registry::SceneDef;
use crate::snapshot::{
    ChoiceOption, DisplayInfo, GameEntry, GameId, GameSource, GameWindow, Level, MenuItem,
    ProfileEntry, ProfileId, Scopes, SettingKey, SettingValue, SettingsPage, Snapshot, SystemEntry,
    SystemId,
};
use crate::theme::Theme;

use super::game_list::WINDOW_ROWS;
use super::router::Screen;
use super::{DISPLAY_PAGE, GRID_PATTERN, Screens};

pub(super) const SCENES: &[SceneDef] = &[
    SceneDef {
        name: "screens/home",
        build: home,
    },
    SceneDef {
        name: "screens/home-empty",
        build: home_empty,
    },
    SceneDef {
        name: "screens/games-top",
        build: games_top,
    },
    SceneDef {
        name: "screens/games-middle",
        build: games_middle,
    },
    SceneDef {
        name: "screens/games-end",
        build: games_end,
    },
    SceneDef {
        name: "screens/games-one",
        build: games_one,
    },
    SceneDef {
        name: "screens/games-empty",
        build: games_empty,
    },
    SceneDef {
        name: "screens/display",
        build: display,
    },
    SceneDef {
        name: "screens/display-style",
        build: display_style,
    },
];

/// The settings of the fixtures.
pub(super) const OUTPUT: SettingKey = SettingKey(1);
pub(super) const MONITOR: SettingKey = SettingKey(2);
pub(super) const STYLE: SettingKey = SettingKey(3);
pub(super) const REFRESH: SettingKey = SettingKey(4);

/// The path from home to the display panel with the fixture systems.
pub(super) const TO_DISPLAY: [UiAction; 6] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
];

pub(super) const NES: SystemId = SystemId(1);
pub(super) const SNES: SystemId = SystemId(2);
pub(super) const MEGA_DRIVE: SystemId = SystemId(3);
/// The size of the long list of the fixtures.
pub(super) const LONG_LIST: usize = 10_000;
/// A title that is too long for a row, at the middle of the long list.
pub(super) const LONG_TITLE: &str =
    "The Legend of the Mystical Ninja: Ganbare Goemon, Special Collector's Edition";

const FIRST_WORDS: [&str; 24] = [
    "Astro", "Bit", "Castle", "Crystal", "Dragon", "Electric", "Final", "Galaxy", "Hyper", "Iron",
    "Jungle", "Kung-Fu", "Lunar", "Mega", "Neo", "Ocean", "Pixel", "Quantum", "Rocket", "Shadow",
    "Turbo", "Ultra", "Vector", "Wonder",
];
const SECOND_WORDS: [&str; 16] = [
    "Quest", "Racer", "Fighter", "Saga", "Patrol", "Blaster", "Legend", "Rally", "Warriors",
    "Panic", "Strike", "Odyssey", "Heroes", "Tactics", "Raiders", "Story",
];
const SUFFIXES: [&str; 5] = ["", "", " II", " 3", " Deluxe"];

/// The systems of the fixtures.
pub(super) fn systems() -> Vec<SystemEntry> {
    vec![
        SystemEntry::new(NES, "Nintendo Entertainment System", 312),
        SystemEntry::new(SNES, "Super Nintendo", LONG_LIST as u32),
        SystemEntry::new(MEGA_DRIVE, "Mega Drive", 1),
    ]
}

/// The title of game `index` of a fixture list: made up, sorted by its
/// first word, and the same on every run.
pub(super) fn title(index: usize) -> String {
    if index == LONG_LIST / 2 {
        return LONG_TITLE.to_owned();
    }
    let first = FIRST_WORDS[index * FIRST_WORDS.len() / LONG_LIST % FIRST_WORDS.len()];
    let second = SECOND_WORDS[index % SECOND_WORDS.len()];
    let suffix = SUFFIXES[index / SECOND_WORDS.len() % SUFFIXES.len()];
    format!("{first} {second}{suffix}")
}

/// A window of `count` rows from `first` of a fixture list of `total`
/// games.
pub(super) fn window(source: GameSource, first: usize, total: usize, count: usize) -> GameWindow {
    let system = match source {
        GameSource::System(id) => id,
        _ => NES,
    };
    let rows = (first..total.min(first + count))
        .map(|i| {
            let mut game = GameEntry::new(GameId(i as u64), system, title(i));
            game.favourite = i % 11 == 3;
            game
        })
        .collect();
    GameWindow::new(source, first, total, rows)
}

/// A snapshot with the fixture systems.
pub(super) fn with_systems() -> Snapshot {
    Snapshot {
        systems: systems(),
        ..Snapshot::default()
    }
}

fn home(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_systems();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[UiAction::Down], frame);
}

fn home_empty(frame: &mut UiFrame, theme: &Theme) {
    let mut screens = Screens::new(theme.clone());
    screens.update(&Snapshot::default(), &[], frame);
}

/// The game list of `system` at `cursor`, with the window the screen asks
/// for there.
fn games_at(frame: &mut UiFrame, theme: &Theme, system: usize, cursor: usize) {
    let mut snap = with_systems();
    let entry = &snap.systems[system];
    let (source, total) = (GameSource::System(entry.id), entry.game_count as usize);
    let mut screens = Screens::new(theme.clone());
    let mut path = vec![UiAction::Down; system];
    path.push(UiAction::Accept);
    screens.update(&snap, &path, frame);
    if let Some(Screen::Games(list)) = screens.top_mut() {
        list.select(cursor, total);
    }
    let first = cursor
        .saturating_sub(WINDOW_ROWS / 2)
        .min(total.saturating_sub(WINDOW_ROWS));
    snap.games = window(source, first, total, WINDOW_ROWS);
    screens.update(&snap, &[], frame);
}

fn games_top(frame: &mut UiFrame, theme: &Theme) {
    games_at(frame, theme, 1, 0);
}

fn games_middle(frame: &mut UiFrame, theme: &Theme) {
    games_at(frame, theme, 1, LONG_LIST / 2);
}

fn games_end(frame: &mut UiFrame, theme: &Theme) {
    games_at(frame, theme, 1, LONG_LIST - 1);
}

fn games_one(frame: &mut UiFrame, theme: &Theme) {
    games_at(frame, theme, 2, 0);
}

fn games_empty(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = with_systems();
    let mut screens = Screens::new(theme.clone());
    let favourites = [UiAction::Down; 3];
    screens.update(&snap, &favourites, frame);
    screens.update(&snap, &[UiAction::Accept], frame);
    snap.games = GameWindow::new(GameSource::Favourites, 0, 0, Vec::new());
    screens.update(&snap, &[], frame);
}

fn choice(index: usize, options: Vec<ChoiceOption>) -> SettingValue {
    SettingValue::Choice { index, options }
}

fn setting(
    key: SettingKey,
    label: &str,
    value: SettingValue,
    source: Level,
    scopes: Scopes,
) -> MenuItem {
    let mut item = MenuItem::new(key, label, value);
    item.source = source;
    item.scopes = scopes;
    item.live = true;
    item
}

/// The picture styles on a 31 kHz monitor that accepts 120 Hz, for a
/// 240-line game at 59.94 Hz.
pub(super) fn styles() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("Auto").with_detail("31.5 kHz  59.94 Hz"),
        ChoiceOption::new("Native").unavailable("Needs a 15 kHz monitor"),
        ChoiceOption::new("2x").with_detail("31.5 kHz  59.94 Hz"),
        ChoiceOption::new("3x").unavailable("Needs a 45 kHz monitor"),
        ChoiceOption::new("4x").unavailable("Needs a 60 kHz monitor"),
    ]
}

/// The rows of a settings page with the display settings, written at the
/// levels of `scopes`.
pub(super) fn display_rows(scopes: Scopes) -> Vec<MenuItem> {
    let global = Scopes {
        global: true,
        ..Scopes::NONE
    };
    let names = |names: &[&str]| names.iter().map(|n| ChoiceOption::new(*n)).collect();
    vec![
        setting(
            OUTPUT,
            "Output",
            choice(1, names(&["Auto", "VGA-1", "HDMI-A-1"])),
            Level::Global,
            global,
        ),
        setting(
            MONITOR,
            "Monitor",
            choice(2, names(&["Auto", "generic_15", "pc_31_120", "vesa_480"])),
            Level::Global,
            global,
        ),
        setting(
            STYLE,
            "Picture style",
            choice(0, styles()),
            Level::Default,
            scopes,
        ),
        setting(
            REFRESH,
            "Refresh",
            choice(
                0,
                vec![
                    ChoiceOption::new("Exact").with_detail("The rate of the game"),
                    ChoiceOption::new("Exact x2").with_detail("Twice the rate of the game"),
                    ChoiceOption::new("Force 50 Hz")
                        .unavailable("The monitor does not accept 50 Hz"),
                    ChoiceOption::new("Force 60 Hz").with_detail("Games made for 50 Hz run faster"),
                ],
            ),
            Level::Default,
            scopes,
        ),
        MenuItem::new(
            SettingKey(5),
            "Grid pattern",
            SettingValue::Action(GRID_PATTERN),
        ),
    ]
}

/// A snapshot with the fixture systems, the display, the monitor profiles
/// and the display page.
pub(super) fn with_display() -> Snapshot {
    let mut snap = with_systems();
    snap.display = Some(DisplayInfo::new("VGA-1", "640x480 59.94 Hz"));
    snap.profiles = vec![
        ProfileEntry::new(ProfileId(1), "generic_15"),
        ProfileEntry::new(ProfileId(2), "pc_31_120"),
        ProfileEntry::new(ProfileId(3), "vesa_480"),
    ];
    snap.active_profile = Some(ProfileId(2));
    snap.settings = SettingsPage::new(DISPLAY_PAGE, "Display", display_rows(Scopes::ALL));
    snap
}

fn display(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_display();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_DISPLAY, frame);
    screens.update(&snap, &[UiAction::Down, UiAction::Down], frame);
}

fn display_style(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_display();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_DISPLAY, frame);
    screens.update(
        &snap,
        &[UiAction::Down, UiAction::Down, UiAction::Accept],
        frame,
    );
    screens.update(&snap, &[UiAction::Down], frame);
}
