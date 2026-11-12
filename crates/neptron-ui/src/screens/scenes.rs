//! The scenes of the screens: each one drives a [`Screens`] router with
//! fixture data and actions to the state it shows.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::registry::SceneDef;
use crate::snapshot::{
    ChoiceOption, ConfirmId, DisplayInfo, GameEntry, GameId, GameSource, GameWindow, Level,
    MenuItem, Notice, NoticeId, NoticeKind, PendingConfirm, ProfileEntry, ProfileId, Scopes,
    SettingKey, SettingValue, SettingsPage, Snapshot, SystemEntry, SystemId,
};
use crate::snapshot::{SessionInfo, VideoStatus};
use crate::theme::Theme;

use super::game_list::WINDOW_ROWS;
use super::router::Screen;
use super::{
    DISPLAY_PAGE, GRID_PATTERN, OPEN_DISPLAY, QUICK_MENU_PAGE, QUIT_GAME, RESUME, Screens,
};

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
    SceneDef {
        name: "screens/confirm-10s",
        build: confirm_10s,
    },
    SceneDef {
        name: "screens/confirm-3s",
        build: confirm_3s,
    },
    SceneDef {
        name: "screens/notice",
        build: notice,
    },
    SceneDef {
        name: "screens/notice-warning",
        build: notice_warning,
    },
    SceneDef {
        name: "screens/quick-menu",
        build: quick_menu,
    },
    SceneDef {
        name: "screens/quick-menu-keep",
        build: quick_menu_keep,
    },
    SceneDef {
        name: "screens/quick-menu-video",
        build: quick_menu_video,
    },
    SceneDef {
        name: "screens/quick-menu-video-filtered",
        build: quick_menu_video_filtered,
    },
    SceneDef {
        name: "screens/quick-menu-video-long",
        build: quick_menu_video_long,
    },
    SceneDef {
        name: "screens/display-video",
        build: display_video,
    },
    SceneDef {
        name: "pattern/grid",
        build: grid,
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

/// The change that the confirm scenes wait for.
pub(super) const CHANGE: &str = "Picture style 2x\n640x480, 31.5 kHz, 59.94 Hz";

/// The display panel after the player chose the 2x picture style, with a
/// confirmation that has `remaining_ns` left.
fn confirm_at(frame: &mut UiFrame, theme: &Theme, remaining_ns: u64) {
    let mut snap = with_display();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_DISPLAY, frame);
    let choose_2x = [
        UiAction::Down,
        UiAction::Down,
        UiAction::Accept,
        UiAction::Down,
        UiAction::Down,
        UiAction::Accept,
    ];
    screens.update(&snap, &choose_2x, frame);
    snap.confirm = Some(PendingConfirm::new(ConfirmId(1), CHANGE, remaining_ns));
    screens.update(&snap, &[], frame);
}

fn confirm_10s(frame: &mut UiFrame, theme: &Theme) {
    confirm_at(frame, theme, 10_000_000_000);
}

fn confirm_3s(frame: &mut UiFrame, theme: &Theme) {
    confirm_at(frame, theme, 2_400_000_000);
}

fn notice(frame: &mut UiFrame, theme: &Theme) {
    notice_over_list(
        frame,
        theme,
        "Astro Racer did not start: the core for this system is missing.",
        NoticeKind::Error,
    );
}

fn notice_warning(frame: &mut UiFrame, theme: &Theme) {
    notice_over_list(
        frame,
        theme,
        "The monitor cannot show 50 Hz. Astro Racer runs at 60 Hz, a little faster.",
        NoticeKind::Warning,
    );
}

/// A notice over the NES list, after a launch from it.
fn notice_over_list(frame: &mut UiFrame, theme: &Theme, text: &str, kind: NoticeKind) {
    let mut snap = with_systems();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[UiAction::Accept], frame);
    let nes = GameSource::System(NES);
    snap.games = window(nes, 0, 312, WINDOW_ROWS);
    screens.update(&snap, &[UiAction::Down, UiAction::Accept], frame);
    snap.notices = vec![Notice::new(NoticeId(1), text, kind)];
    screens.update(&snap, &[], frame);
}

/// The rows of the quick menu page: the display settings of the game, and
/// the rows the UI carries out itself.
pub(super) fn quick_rows() -> Vec<MenuItem> {
    let mut rows = display_rows(Scopes::ALL);
    rows.retain(|r| r.key == STYLE || r.key == REFRESH);
    if let Some(style) = rows.iter_mut().find(|r| r.key == STYLE) {
        style.source = Level::System;
    }
    let action = |key, label, id| MenuItem::new(SettingKey(key), label, SettingValue::Action(id));
    rows.insert(0, action(10, "Resume", RESUME));
    rows.push(action(11, "Display", OPEN_DISPLAY));
    rows.push(action(12, "Quit game", QUIT_GAME));
    rows
}

/// A snapshot of a running game with the quick menu page.
pub(super) fn in_game() -> Snapshot {
    let mut snap = with_display();
    snap.session = Some(SessionInfo::new(GameId(1), "Astro Racer"));
    snap.settings = SettingsPage::new(QUICK_MENU_PAGE, "Quick menu", quick_rows());
    snap
}

fn quick_menu(frame: &mut UiFrame, theme: &Theme) {
    let snap = in_game();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[], frame);
    screens.update(&snap, &[UiAction::Menu, UiAction::Down], frame);
}

fn quick_menu_keep(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = in_game();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[], frame);
    let force_60 = [
        UiAction::Menu,
        UiAction::Down,
        UiAction::Down,
        UiAction::Accept,
        UiAction::Down,
        UiAction::Down,
        UiAction::Down,
        UiAction::Accept,
    ];
    screens.update(&snap, &force_60, frame);
    if let Some(refresh) = snap.settings.rows.iter_mut().find(|r| r.key == REFRESH) {
        refresh.value = SettingValue::Choice {
            index: 3,
            options: match &refresh.value {
                SettingValue::Choice { options, .. } => options.clone(),
                _ => Vec::new(),
            },
        };
        refresh.source = Level::Game;
    }
    screens.update(&snap, &[UiAction::Back], frame);
}

/// A game at its exact rate on a 2x mode.
pub(super) fn exact_status() -> VideoStatus {
    VideoStatus::new(
        "256x224             60.099 Hz",
        "768x448p  30.05 kHz 60.099 Hz  2x",
        "exact",
    )
}

/// A game that the mode cannot show at an integer scale.
pub(super) fn filtered_status() -> VideoStatus {
    VideoStatus::new(
        "320x224             59.923 Hz",
        "640x480p  31.47 kHz 59.923 Hz  2x",
        "filtered",
    )
    .with_alert(true)
}

/// The longest status of the fixtures: a mode line that fills its column
/// and a result that needs two lines.
pub(super) fn long_status() -> VideoStatus {
    VideoStatus::new(
        "256x240             60.099 Hz",
        "1920x238p 15.73 kHz 59.940 Hz  native",
        "cropped 2 lines, speed changed (60.099 to 59.940 Hz)",
    )
    .with_alert(true)
}

/// A game slowed to the rate of a 31 kHz mode that the fixture monitor
/// `pc_31_120` shows.
pub(super) fn slowed_status() -> VideoStatus {
    VideoStatus::new(
        "320x240             60.099 Hz",
        "640x480p  31.47 kHz 59.940 Hz  2x",
        "speed changed (60.099 to 59.940 Hz)",
    )
    .with_alert(true)
}

/// A snapshot of a running game with the quick menu page and `status`.
pub(super) fn in_game_with(status: VideoStatus) -> Snapshot {
    let mut snap = in_game();
    snap.video = Some(status);
    snap
}

fn quick_menu_with(frame: &mut UiFrame, theme: &Theme, status: VideoStatus) {
    let snap = in_game_with(status);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[], frame);
    screens.update(&snap, &[UiAction::Menu], frame);
}

fn quick_menu_video(frame: &mut UiFrame, theme: &Theme) {
    quick_menu_with(frame, theme, exact_status());
}

fn quick_menu_video_filtered(frame: &mut UiFrame, theme: &Theme) {
    quick_menu_with(frame, theme, filtered_status());
}

fn quick_menu_video_long(frame: &mut UiFrame, theme: &Theme) {
    quick_menu_with(frame, theme, long_status());
}

/// The display panel opened from the quick menu, with the video block of
/// a 31 kHz mode.
fn display_video(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = in_game_with(slowed_status());
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[], frame);
    let to_display = [
        UiAction::Menu,
        UiAction::Down,
        UiAction::Down,
        UiAction::Down,
        UiAction::Accept,
    ];
    screens.update(&snap, &to_display, frame);
    snap.settings = with_display().settings;
    screens.update(&snap, &[UiAction::Down, UiAction::Down], frame);
}

fn grid(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_display();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_DISPLAY, frame);
    let to_grid = [
        UiAction::Down,
        UiAction::Down,
        UiAction::Down,
        UiAction::Down,
        UiAction::Accept,
    ];
    screens.update(&snap, &to_grid, frame);
}
