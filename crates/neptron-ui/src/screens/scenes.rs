//! The scenes of the screens: each one drives a [`Screens`] router with
//! fixture data and actions to the state it shows.

use crate::frame::UiFrame;
use crate::input::UiAction;
use crate::registry::SceneDef;
use crate::snapshot::{
    ActionId, MappingId, MappingStage, PadMapping, PageId, SessionInfo, VideoStatus,
};
use crate::snapshot::{
    ChoiceOption, ConfirmId, DisplayInfo, GameEntry, GameId, GameMarks, GameSource, GameWindow,
    Level, MenuItem, Notice, NoticeId, NoticeKind, PendingConfirm, ProfileEntry, ProfileId, Scopes,
    SettingKey, SettingValue, SettingsPage, Snapshot, SystemEntry, SystemId,
};
use crate::theme::Theme;

use super::game_list::WINDOW_ROWS;
use super::router::Screen;
use super::{
    AUDIO_PAGE, DISPLAY_PAGE, GRID_PAGE, INPUT_PAGE, LIBRARY_PAGE, NETWORK_PAGE, QUICK_MENU_PAGE,
    QUIT_GAME, RESUME, SETTINGS_PAGE, SYSTEM_PAGE, Screens, VIDEO_PAGE,
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
        name: "screens/settings",
        build: settings,
    },
    SceneDef {
        name: "screens/breadcrumb-long",
        build: breadcrumb_long,
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
        name: "screens/games-arcade",
        build: games_arcade,
    },
    SceneDef {
        name: "screens/games-versions",
        build: games_versions,
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
        name: "screens/display-safe-mode",
        build: display_safe_mode,
    },
    SceneDef {
        name: "screens/display-tv",
        build: display_tv,
    },
    SceneDef {
        name: "screens/display-tv-standard",
        build: display_tv_standard,
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
        name: "screens/confirm-0s",
        build: confirm_0s,
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
        name: "screens/notice-choice",
        build: notice_choice,
    },
    SceneDef {
        name: "screens/notice-generic-pad",
        build: notice_generic_pad,
    },
    SceneDef {
        name: "screens/notice-set-files",
        build: notice_set_files,
    },
    SceneDef {
        name: "screens/notice-wrong-set",
        build: notice_wrong_set,
    },
    SceneDef {
        name: "screens/quick-menu",
        build: quick_menu,
    },
    SceneDef {
        name: "screens/quick-menu-panel",
        build: quick_menu_panel,
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
    SceneDef {
        name: "screens/audio",
        build: audio,
    },
    SceneDef {
        name: "screens/audio-output",
        build: audio_output,
    },
    SceneDef {
        name: "screens/quick-menu-volume",
        build: quick_menu_volume,
    },
    SceneDef {
        name: "screens/library-scanning",
        build: library_scanning,
    },
    SceneDef {
        name: "screens/network",
        build: network,
    },
    SceneDef {
        name: "screens/network-two-ports",
        build: network_two_ports,
    },
    SceneDef {
        name: "screens/network-no-address",
        build: network_no_address,
    },
    SceneDef {
        name: "screens/network-unknown",
        build: network_unknown,
    },
    SceneDef {
        name: "screens/network-share-off",
        build: network_share_off,
    },
    SceneDef {
        name: "screens/input",
        build: input,
    },
    SceneDef {
        name: "screens/input-empty",
        build: input_empty,
    },
    SceneDef {
        name: "screens/pad-page",
        build: pad_page,
    },
    SceneDef {
        name: "screens/pad-mapping",
        build: pad_mapping,
    },
    SceneDef {
        name: "screens/pad-mapping-note",
        build: pad_mapping_note,
    },
    SceneDef {
        name: "screens/pad-mapping-review",
        build: pad_mapping_review,
    },
    SceneDef {
        name: "screens/pad-mapping-incomplete",
        build: pad_mapping_incomplete,
    },
    SceneDef {
        name: "screens/system",
        build: system,
    },
    SceneDef {
        name: "screens/system-shutdown-confirm",
        build: system_shutdown_confirm,
    },
    SceneDef {
        name: "screens/system-shutting-down",
        build: system_shutting_down,
    },
    SceneDef {
        name: "screens/system-format",
        build: system_format,
    },
    SceneDef {
        name: "screens/system-format-confirm",
        build: system_format_confirm,
    },
];

/// The settings of the fixtures.
pub(super) const OUTPUT: SettingKey = SettingKey(1);
pub(super) const MONITOR: SettingKey = SettingKey(2);
pub(super) const STYLE: SettingKey = SettingKey(3);
pub(super) const REFRESH: SettingKey = SettingKey(4);
pub(super) const VOLUME: SettingKey = SettingKey(6);
pub(super) const AUDIO_OUTPUT: SettingKey = SettingKey(7);

/// The path from home to the Settings page with the fixture systems.
pub(super) const TO_SETTINGS: [UiAction; 6] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
];

/// The path from home to the display panel with the fixture systems.
pub(super) const TO_DISPLAY: [UiAction; 7] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
    UiAction::Accept,
];

/// The path from home to the audio panel with the fixture systems.
pub(super) const TO_AUDIO: [UiAction; 8] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
    UiAction::Down,
    UiAction::Accept,
];

/// The path from home to the Input panel with the fixture systems.
pub(super) const TO_INPUT: [UiAction; 9] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
];

/// The path from home to the Library panel with the fixture systems.
pub(super) const TO_LIBRARY: [UiAction; 10] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
];

/// The path from home to the Network panel with the fixture systems.
pub(super) const TO_NETWORK: [UiAction; 11] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
];

/// The path from home to the System panel with the fixture systems.
pub(super) const TO_SYSTEM: [UiAction; 12] = [
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
];

/// The action of the fixture `Reboot` row.
pub(super) const REBOOT: crate::snapshot::ActionId = crate::snapshot::ActionId(2);
/// The action of the fixture `Shut down` row.
pub(super) const SHUT_DOWN: crate::snapshot::ActionId = crate::snapshot::ActionId(3);

/// The rows of the System panel as the app gives them, enabled or
/// disabled for `reason`.
pub(super) fn system_rows(reason: Option<&str>) -> Vec<MenuItem> {
    let rows = [
        (
            10,
            "Reboot",
            REBOOT,
            "Reboot NEPTRON?",
            "Settings and saves are written first. Then the machine starts again.",
            "Restarting...",
        ),
        (
            11,
            "Shut down",
            SHUT_DOWN,
            "Shut down NEPTRON?",
            "Settings and saves are written first. Then the machine turns off.",
            "Shutting down...",
        ),
    ];
    rows.into_iter()
        .map(|(key, label, action, question, message, working)| {
            let row = MenuItem::new(SettingKey(key), label, SettingValue::Action(action))
                .with_confirm(question, message, working);
            match reason {
                Some(reason) => row.unavailable(reason),
                None => row,
            }
        })
        .collect()
}

/// The fixture systems with the System panel in the snapshot.
pub(super) fn with_system(reason: Option<&str>) -> Snapshot {
    let mut snap = with_systems();
    snap.settings = SettingsPage::new(SYSTEM_PAGE, "System", system_rows(reason));
    snap
}

/// The path from a running game to the video page of the quick menu.
pub(super) const TO_VIDEO: [UiAction; 4] = [
    UiAction::Menu,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
];

/// The path from a running game to the display panel, through the
/// Settings page of the quick menu.
pub(super) const TO_GAME_DISPLAY: [UiAction; 6] = [
    UiAction::Menu,
    UiAction::Down,
    UiAction::Down,
    UiAction::Down,
    UiAction::Accept,
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

/// The Settings page, with the focus on the audio panel.
fn settings(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_systems();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_SETTINGS, frame);
    screens.update(&snap, &[UiAction::Down], frame);
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

/// The fixture arcade system, first on the home screen.
pub(super) const ARCADE: SystemId = SystemId(9);
/// The game of the arcade fixture with other versions.
pub(super) const STAR_PATROL: GameId = GameId(901);

/// The rows of the arcade fixture: a game with two other versions and a
/// vertical screen, one that does not work, one with a name that is not a
/// set, and one with two marks.
pub(super) fn arcade_rows() -> Vec<GameEntry> {
    let marks = |vertical, not_working, unknown_set| GameMarks {
        vertical,
        not_working,
        unknown_set,
    };
    let game = |id, title: &str| GameEntry::new(GameId(id), ARCADE, title);
    vec![
        game(900, "Alpine Racer"),
        game(STAR_PATROL.0, "Star Patrol - Mission One")
            .with_versions(2)
            .with_marks(marks(true, false, false)),
        game(902, "Wreck Race").with_marks(marks(false, true, false)),
        game(903, "mystery").with_marks(marks(false, false, true)),
        game(904, "Zed Blaster").with_marks(marks(true, true, false)),
    ]
}

/// The fixture systems with the arcade system first.
pub(super) fn with_arcade() -> Snapshot {
    let mut snap = with_systems();
    snap.systems.insert(
        0,
        SystemEntry::new(ARCADE, "Arcade", arcade_rows().len() as u32),
    );
    snap
}

/// The arcade list, with the focus on the game with other versions.
fn games_arcade(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = with_arcade();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[UiAction::Accept], frame);
    let rows = arcade_rows();
    snap.games = GameWindow::new(GameSource::System(ARCADE), 0, rows.len(), rows);
    screens.update(&snap, &[UiAction::Down], frame);
}

/// The other versions of the arcade game, opened from the list.
fn games_versions(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = with_arcade();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[UiAction::Accept], frame);
    let rows = arcade_rows();
    snap.games = GameWindow::new(GameSource::System(ARCADE), 0, rows.len(), rows);
    screens.update(&snap, &[UiAction::Down, UiAction::Accept], frame);
    let versions = ["World", "Japan", "USA, bootleg"]
        .iter()
        .enumerate()
        .map(|(i, region)| {
            let id = if i == 0 {
                STAR_PATROL
            } else {
                GameId(910 + i as u64)
            };
            GameEntry::new(id, ARCADE, format!("Star Patrol - Mission One ({region})")).with_marks(
                GameMarks {
                    vertical: true,
                    ..GameMarks::default()
                },
            )
        })
        .collect::<Vec<_>>();
    snap.games = GameWindow::new(
        GameSource::Versions(STAR_PATROL),
        0,
        versions.len(),
        versions,
    );
    screens.update(&snap, &[UiAction::Down], frame);
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
        MenuItem::new(SettingKey(5), "Grid pattern", SettingValue::Page(GRID_PAGE)),
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

/// The actions of the fixture rows that end safe mode.
const KEEP_PICTURE: crate::snapshot::ActionId = crate::snapshot::ActionId(5);
const TRY_SETTINGS: crate::snapshot::ActionId = crate::snapshot::ActionId(6);

/// The display panel in safe mode after failed boots, as the app gives it:
/// the rows that end safe mode at its end, with the focus on the first.
fn display_safe_mode(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = with_display();
    snap.display = Some(DisplayInfo::new("VGA-1", "320x240 59.94 Hz (safe mode)"));
    snap.active_profile = Some(ProfileId(1));
    snap.settings.rows.extend([
        MenuItem::new(
            SettingKey(0x0001_0009),
            "Keep this picture",
            SettingValue::Action(KEEP_PICTURE),
        ),
        MenuItem::new(
            SettingKey(0x0001_000A),
            "Try my settings again",
            SettingValue::Action(TRY_SETTINGS),
        ),
    ]);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_DISPLAY, frame);
    screens.update(&snap, &[UiAction::Down; 32], frame);
    screens.update(&snap, &[UiAction::Up], frame);
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

/// The rows of the display page on the composite output of a Raspberry
/// Pi: the TV standard and the TV picture width show, and the monitor
/// profile has no effect.
fn display_tv_rows() -> Vec<MenuItem> {
    let global = Scopes {
        global: true,
        ..Scopes::NONE
    };
    let names = |names: &[&str]| names.iter().map(|n| ChoiceOption::new(*n)).collect();
    let mut standards = vec![ChoiceOption::new("auto").with_detail("NTSC (60 Hz)")];
    standards.extend(
        [
            ("NTSC (60 Hz)", "NTSC"),
            ("NTSC-J (60 Hz)", "NTSC-J"),
            ("NTSC 4.43 (60 Hz)", "NTSC 4.43"),
            ("PAL 60 (60 Hz)", "PAL 60"),
            ("PAL-M (60 Hz)", "PAL-M"),
            ("PAL (50 Hz)", "PAL"),
            ("PAL-N (50 Hz)", "PAL-N"),
            ("SECAM (50 Hz)", "SECAM"),
            ("Mono (60 Hz)", "Mono 60"),
            ("Mono (50 Hz)", "Mono 50"),
        ]
        .map(|(label, short)| ChoiceOption::new(label).with_short(short)),
    );
    vec![
        setting(
            OUTPUT,
            "Output",
            choice(0, names(&["auto", "HDMI-A-1", "Composite-1"])),
            Level::Default,
            Scopes::NONE,
        ),
        setting(
            MONITOR,
            "Monitor profile",
            choice(0, names(&["auto", "generic_15", "ntsc", "pal"])),
            Level::Default,
            global,
        )
        .unavailable("Not used on a TV"),
        setting(
            TV_STANDARD,
            "TV standard",
            choice(4, standards),
            Level::Global,
            global,
        ),
        setting(
            TV_WIDTH,
            "TV picture width",
            choice(0, names(&["aspect", "integer", "fill"])),
            Level::Default,
            global,
        ),
        setting(
            STYLE,
            "Picture style",
            choice(
                0,
                vec![
                    ChoiceOption::new("auto").with_detail("15.73 kHz  60.05 Hz"),
                    ChoiceOption::new("native").with_detail("15.73 kHz  60.05 Hz"),
                    ChoiceOption::new("2x").unavailable("Needs another monitor"),
                    ChoiceOption::new("3x").unavailable("Needs another monitor"),
                    ChoiceOption::new("4x").unavailable("Needs another monitor"),
                ],
            ),
            Level::Default,
            global,
        ),
        MenuItem::new(SettingKey(5), "Grid pattern", SettingValue::Page(GRID_PAGE)),
    ]
}

/// The key of the TV standard row of the TV scenes.
const TV_STANDARD: SettingKey = SettingKey(30);
/// The key of the TV picture width row of the TV scenes.
const TV_WIDTH: SettingKey = SettingKey(31);

/// A snapshot of the display panel on the composite output of a
/// Raspberry Pi, in PAL 60 240p.
fn with_display_tv() -> Snapshot {
    let mut snap = with_systems();
    snap.display =
        Some(DisplayInfo::new("Composite-1", "PAL 60 240p 60.05 Hz").monitor("Composite TV"));
    snap.settings = SettingsPage::new(DISPLAY_PAGE, "Display", display_tv_rows());
    snap
}

/// The display panel on a TV, with the focus on the TV standard.
fn display_tv(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_display_tv();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_DISPLAY, frame);
    screens.update(&snap, &[UiAction::Down, UiAction::Down], frame);
}

/// The option list of the TV standard, with the focus on the standard in
/// use, PAL 60.
fn display_tv_standard(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_display_tv();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_DISPLAY, frame);
    screens.update(
        &snap,
        &[UiAction::Down, UiAction::Down, UiAction::Accept],
        frame,
    );
    screens.update(&snap, &[], frame);
}

/// The Library panel with the app's rescan row while a scan runs: a
/// disabled action row with the focus and its reason.
fn library_scanning(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = with_systems();
    let scanning = MenuItem::new(
        SettingKey(9),
        "Rescan games",
        SettingValue::Action(crate::snapshot::ActionId(1)),
    )
    .unavailable("Scanning...");
    snap.settings = SettingsPage::new(LIBRARY_PAGE, "Library", vec![scanning]);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_LIBRARY, frame);
    screens.update(&snap, &[], frame);
}

/// The System panel with the focus on `Shut down`.
fn system(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_system(None);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_SYSTEM, frame);
    screens.update(&snap, &[UiAction::Down], frame);
}

/// The confirm question of `Shut down`, with the focus on `Cancel`.
fn system_shutdown_confirm(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_system(None);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_SYSTEM, frame);
    screens.update(&snap, &[UiAction::Down, UiAction::Accept], frame);
}

/// The last frame after the confirm of `Shut down`: the box that says
/// what happens, with no buttons.
fn system_shutting_down(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_system(None);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_SYSTEM, frame);
    screens.update(
        &snap,
        &[
            UiAction::Down,
            UiAction::Accept,
            UiAction::Right,
            UiAction::Accept,
        ],
        frame,
    );
}

/// The action of the fixture `Format share` row.
const FORMAT_SHARE: crate::snapshot::ActionId = crate::snapshot::ActionId(4);

/// The System panel as the app gives it when the storage script offers a
/// format of the share partition: the third row.
fn with_format_offer() -> Snapshot {
    let mut snap = with_system(None);
    snap.settings.rows.push(
        MenuItem::new(
            SettingKey(12),
            "Format share",
            SettingValue::Action(FORMAT_SHARE),
        )
        .with_confirm(
            "Format the share partition?",
            "NEPTRON finds no file system it can read on the partition for games (56.6 GB). Formatting it erases everything on it.",
            "Formatting at the next start...",
        ),
    );
    snap
}

/// The System panel with the format row, with the focus on it.
fn system_format(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_format_offer();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_SYSTEM, frame);
    screens.update(&snap, &[UiAction::Down, UiAction::Down], frame);
}

/// The confirm question of the format row, with the focus on `Cancel`.
fn system_format_confirm(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_format_offer();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_SYSTEM, frame);
    screens.update(
        &snap,
        &[UiAction::Down, UiAction::Down, UiAction::Accept],
        frame,
    );
}

/// The rows of the Input panel as the app gives them: the text of each
/// player row, then one row for each extra pad, the keyboard and the Start
/// hold.
pub(super) fn input_rows(players: [&str; 4], extra: &[&str]) -> Vec<MenuItem> {
    let text = |key: u32, label: String, text: &str| {
        MenuItem::new(SettingKey(key), label, SettingValue::Text(text.to_owned()))
    };
    // A present pad opens its page; no pad and an away pad are text.
    let pad = |key: u32, label: String, name: &str| {
        if name == "None" || name.ends_with("(away)") {
            return text(key, label, name);
        }
        MenuItem::new(
            SettingKey(key),
            label,
            SettingValue::Link {
                page: PageId(PAD_PAGE_FIRST + key),
                text: name.to_owned(),
            },
        )
    };
    let mut rows: Vec<MenuItem> = players
        .iter()
        .zip(1..)
        .map(|(name, n)| pad(40 + n, format!("Player {n}"), name))
        .collect();
    rows.extend(
        extra
            .iter()
            .zip(45..)
            .map(|(name, key)| pad(key, "Extra pad".to_owned(), name)),
    );
    rows.push(text(49, "Keyboard".to_owned(), "Player 1"));
    rows.push(setting(
        SettingKey(50),
        "Hold Start for menu",
        SettingValue::Number {
            value: 0,
            min: 0,
            max: 10,
            step: 1,
            unit: "s".to_owned(),
        },
        Level::Default,
        Scopes {
            global: true,
            system: false,
            game: false,
        },
    ));
    rows
}

/// The Input panel with `rows`, the focus on the first row.
fn input_with(frame: &mut UiFrame, theme: &Theme, rows: Vec<MenuItem>) {
    let mut snap = with_systems();
    snap.settings = SettingsPage::new(INPUT_PAGE, "Input", rows);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_INPUT, frame);
    screens.update(&snap, &[], frame);
}

/// Four players, player 1 with a layout of the player, player 2 away in a
/// game, player 3 with the generic layout, and a fifth pad with the
/// generic layout. A present pad opens its page, so its row has the arrow.
/// The longest names the app keeps: 34 characters on a row with the
/// arrow, 35 with ` (away)` (28 before it), and 17 before
/// ` (generic layout)` or 18 before ` (your layout)`.
fn input(frame: &mut UiFrame, theme: &Theme) {
    let rows = input_rows(
        [
            "Arcade Stick (your layout)",
            "Nintendo Switch Pro Contr... (away)",
            "USB Joystick (generic layout)",
            "8BitDo SN30 Pro+ Bluetooth Gamep...",
        ],
        &["Generic USB J... (generic layout)"],
    );
    input_with(frame, theme, rows);
}

/// The first page id of the page of a controller in the fixtures.
const PAD_PAGE_FIRST: u32 = 100;

/// The page of the controller of player 1, as the app gives it.
pub(super) fn pad_page_rows() -> Vec<MenuItem> {
    let text = |key: u32, label: &str, text: &str| {
        MenuItem::new(SettingKey(key), label, SettingValue::Text(text.to_owned()))
    };
    let action = |key: u32, label: &str| {
        MenuItem::new(SettingKey(key), label, SettingValue::Action(ActionId(key)))
    };
    let mut rows = vec![
        text(60, "Controller", "Arcade Stick"),
        text(61, "Layout", "Yours"),
        action(62, "Map buttons"),
        action(63, "Reset to default").with_confirm(
            "Reset this controller?",
            "Your layout of this model is removed. The default layout comes back.",
            "Resetting...",
        ),
    ];
    let bindings = [
        ("Up", "Hat 0 up"),
        ("Down", "Hat 0 down"),
        ("Left", "Hat 0 left"),
        ("Right", "Hat 0 right"),
        ("A", "Button 2"),
        ("B", "Button 1"),
        ("Start", "Button 9"),
        ("Select", "Button 8"),
        ("L2", "Axis 4 + (from its end)"),
        ("Left stick X", "Axis 0"),
        ("Left stick Y", "Axis 1 inverted"),
        ("R3", "None"),
    ];
    rows.extend(
        bindings
            .iter()
            .zip(64..)
            .map(|((label, binding), key)| text(key, label, binding)),
    );
    rows
}

/// The page of a controller, opened from its row on the Input panel.
fn pad_page(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = with_systems();
    snap.settings = SettingsPage::new(
        INPUT_PAGE,
        "Input",
        input_rows(["Arcade Stick (your layout)", "None", "None", "None"], &[]),
    );
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_INPUT, frame);
    screens.update(&snap, &[UiAction::Accept], frame);
    snap.settings = SettingsPage::new(PageId(PAD_PAGE_FIRST + 41), "Player 1", pad_page_rows());
    screens.update(&snap, &[], frame);
    screens.update(&snap, &[UiAction::Down, UiAction::Down], frame);
}

/// The guided mapping over the page of the controller, at `stage`.
fn pad_mapping_at(frame: &mut UiFrame, theme: &Theme, stage: MappingStage, remaining_ns: u64) {
    let mut snap = with_systems();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[], frame);
    snap.pad_mapping = Some(PadMapping::new(
        MappingId(1),
        "Arcade Stick",
        stage,
        remaining_ns,
    ));
    screens.update(&snap, &[], frame);
}

/// The third step, with 12 s left.
fn pad_mapping(frame: &mut UiFrame, theme: &Theme) {
    let stage = MappingStage::Prompt {
        prompt: "Press Left".to_owned(),
        step: 3,
        steps: 20,
        note: String::new(),
    };
    pad_mapping_at(frame, theme, stage, 11_200_000_000);
}

/// A press that the mapping refused, at the longest note the app gives.
fn pad_mapping_note(frame: &mut UiFrame, theme: &Theme) {
    let stage = MappingStage::Prompt {
        prompt: "Press L2 (left trigger)".to_owned(),
        step: 13,
        steps: 20,
        note: "That is Select already. Press another one.".to_owned(),
    };
    pad_mapping_at(frame, theme, stage, 15_000_000_000);
}

/// The review of a map that can be kept, with the focus on Cancel.
fn pad_mapping_review(frame: &mut UiFrame, theme: &Theme) {
    let stage = MappingStage::Review {
        message: "16 of 20 controls are set. Skipped: L3, R3, Right stick X, Right stick Y."
            .to_owned(),
        can_keep: true,
    };
    pad_mapping_at(frame, theme, stage, 15_000_000_000);
}

/// The review of a map that lacks what the menus need, with its warning
/// about the quick menu.
fn pad_mapping_incomplete(frame: &mut UiFrame, theme: &Theme) {
    let stage = MappingStage::Review {
        message:
            "This layout has no Left and no B. A controller needs the four directions, A and B."
                .to_owned(),
        can_keep: false,
    };
    pad_mapping_at(frame, theme, stage, 9_000_000_000);
}

/// No pad: the four player rows stay, so the panel has one shape.
fn input_empty(frame: &mut UiFrame, theme: &Theme) {
    input_with(frame, theme, input_rows(["None"; 4], &[]));
}

/// The rows of the Network panel as the app gives them for `addresses`
/// (port and address), with the share on or off. No address gives the row
/// of a valid file with no lease. The network name is announced only with
/// the share on.
pub(super) fn network_rows(addresses: &[(&str, &str)], share_on: bool) -> Vec<MenuItem> {
    let text = |key: u32, label: String, text: &str| {
        MenuItem::new(SettingKey(key), label, SettingValue::Text(text.to_owned()))
    };
    let mut rows: Vec<MenuItem> = addresses
        .iter()
        .zip(20..)
        .map(|(&(port, ipv4), key)| text(key, format!("Address {port}"), ipv4))
        .collect();
    if rows.is_empty() {
        rows.push(text(
            20,
            "Address".to_owned(),
            "None (no cable or no network)",
        ));
    }
    let target = addresses.first().map_or("neptron.local", |&(_, ipv4)| ipv4);
    rows.push(text(30, "Host name".to_owned(), "neptron"));
    if share_on {
        rows.push(text(31, "Network name".to_owned(), "neptron.local"));
    }
    rows.push(text(
        32,
        "Share".to_owned(),
        if share_on { "On" } else { "Off" },
    ));
    if share_on {
        rows.push(text(
            33,
            "Windows".to_owned(),
            &format!("\\\\{target}\\share"),
        ));
        rows.push(text(
            34,
            "macOS, Linux".to_owned(),
            &format!("smb://{target}/share"),
        ));
        rows.push(text(35, "Share user".to_owned(), "neptron"));
        rows.push(text(36, "Share password".to_owned(), "neptron"));
    }
    rows
}

/// The Network panel with `rows`, the focus on the first row.
fn network_with(frame: &mut UiFrame, theme: &Theme, rows: Vec<MenuItem>) {
    let mut snap = with_systems();
    snap.settings = SettingsPage::new(NETWORK_PAGE, "Network", rows);
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_NETWORK, frame);
    screens.update(&snap, &[], frame);
}

/// One port with an address, the share on.
fn network(frame: &mut UiFrame, theme: &Theme) {
    network_with(
        frame,
        theme,
        network_rows(&[("eth0", "192.168.0.112")], true),
    );
}

/// Two ports with addresses: the longest port name and the longest
/// address, so the share lines are the longest they get.
fn network_two_ports(frame: &mut UiFrame, theme: &Theme) {
    let rows = network_rows(
        &[
            ("enx001122334455", "255.255.255.255"),
            ("eth0", "192.168.0.112"),
        ],
        true,
    );
    network_with(frame, theme, rows);
}

/// A valid file with no address: no cable, or no lease yet.
fn network_no_address(frame: &mut UiFrame, theme: &Theme) {
    network_with(frame, theme, network_rows(&[], true));
}

/// No file, a damaged file, or a file of another version.
fn network_unknown(frame: &mut UiFrame, theme: &Theme) {
    let row = MenuItem::new(
        SettingKey(20),
        "Address",
        SettingValue::Text("Not known".to_owned()),
    );
    network_with(frame, theme, vec![row]);
}

/// One address, the share switched off.
fn network_share_off(frame: &mut UiFrame, theme: &Theme) {
    network_with(
        frame,
        theme,
        network_rows(&[("eth0", "192.168.0.112")], false),
    );
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

/// No time left: the dialog shows Revert alone.
fn confirm_0s(frame: &mut UiFrame, theme: &Theme) {
    confirm_at(frame, theme, 0);
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

/// A notice that asks the player to choose: an arcade game that cannot be
/// checked. `Cancel` has the focus.
fn notice_choice(frame: &mut UiFrame, theme: &Theme) {
    let notice = Notice::new(
        NoticeId(1),
        "NEPTRON cannot find its list of arcade games, so it cannot check this game.",
        NoticeKind::Warning,
    )
    .with_choice("Start");
    notice_over_list_with(frame, theme, notice);
}

/// A pad with no gamepad mapping came: the notice of the generic layout,
/// with the button that opens its row of the Input panel.
fn notice_generic_pad(frame: &mut UiFrame, theme: &Theme) {
    let notice = Notice::new(
        NoticeId(1),
        "USB Joystick has no known layout, so it uses a generic one. Some buttons can be in other places.",
        NoticeKind::Info,
    )
    .with_choice("Set up this controller");
    notice_over_list_with(frame, theme, notice);
}

/// The set check of an arcade game found a few files missing or wrong:
/// the message names two, at the longest the app gives, and the sets the
/// image needs.
fn notice_set_files(frame: &mut UiFrame, theme: &Theme) {
    notice_over_list(
        frame,
        theme,
        "1234 files of this game are missing or differ: abcdefghijklmnopqrstuvwx, abcdefghijklmnopqrstuvwx and 1232 more. This image needs sets for FinalBurn Neo 1.0.0.03 (commit d7450cf). The log lists every file.",
        NoticeKind::Warning,
    );
}

/// The set check of an arcade game found most files wrong: a set for
/// another emulator or version.
fn notice_wrong_set(frame: &mut UiFrame, theme: &Theme) {
    notice_over_list(
        frame,
        theme,
        "Most files of this game do not match. The set is for another emulator or another version. This image needs sets for FinalBurn Neo 1.0.0.03 (commit d7450cf).",
        NoticeKind::Warning,
    );
}

/// A notice over the NES list, after a launch from it.
fn notice_over_list(frame: &mut UiFrame, theme: &Theme, text: &str, kind: NoticeKind) {
    notice_over_list_with(frame, theme, Notice::new(NoticeId(1), text, kind));
}

fn notice_over_list_with(frame: &mut UiFrame, theme: &Theme, notice: Notice) {
    let mut snap = with_systems();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[UiAction::Accept], frame);
    let nes = GameSource::System(NES);
    snap.games = window(nes, 0, 312, WINDOW_ROWS);
    screens.update(&snap, &[UiAction::Down, UiAction::Accept], frame);
    snap.notices = vec![notice];
    screens.update(&snap, &[], frame);
}

/// The row of the fixture quick menu that opens the video page.
pub(super) const VIDEO_ROW: SettingKey = SettingKey(13);
/// The row of the fixture quick menu that opens the Settings page.
pub(super) const SETTINGS_ROW: SettingKey = SettingKey(14);

/// The rows of the first page of the quick menu: resume, the volume of the
/// game, the video page, the Settings page and quit.
pub(super) fn quick_rows() -> Vec<MenuItem> {
    let action = |key, label, id| MenuItem::new(SettingKey(key), label, SettingValue::Action(id));
    let page = |key, label, id| MenuItem::new(key, label, SettingValue::Page(id));
    vec![
        action(10, "Resume", RESUME),
        volume_row(80, Level::Default, Scopes::ALL),
        page(VIDEO_ROW, "Video", VIDEO_PAGE),
        page(SETTINGS_ROW, "Settings", SETTINGS_PAGE),
        action(12, "Quit game", QUIT_GAME),
    ]
}

/// The rows of the video page of the quick menu: the display settings of
/// the game, with the picture style set for its system.
pub(super) fn video_rows() -> Vec<MenuItem> {
    let mut rows = display_rows(Scopes::ALL);
    rows.retain(|r| r.key == STYLE || r.key == REFRESH);
    if let Some(style) = rows.iter_mut().find(|r| r.key == STYLE) {
        style.source = Level::System;
    }
    rows
}

/// The video page of the quick menu.
pub(super) fn video_page() -> SettingsPage {
    SettingsPage::new(VIDEO_PAGE, "Video", video_rows())
}

/// A snapshot of a running game with the first page of the quick menu.
pub(super) fn in_game() -> Snapshot {
    let mut snap = with_display();
    snap.session = Some(SessionInfo::new(GameId(1), "Astro Racer"));
    snap.settings = SettingsPage::new(QUICK_MENU_PAGE, "Quick menu", quick_rows());
    snap
}

/// The first page of the quick menu, with the focus on the video page.
fn quick_menu(frame: &mut UiFrame, theme: &Theme) {
    let snap = in_game();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[], frame);
    screens.update(
        &snap,
        &[UiAction::Menu, UiAction::Down, UiAction::Down],
        frame,
    );
}

/// The video page of the quick menu, opened over the game of `snap`. The
/// scene then puts the rows of the video page in `snap`, as the app does.
fn video_page_of(frame: &mut UiFrame, theme: &Theme, snap: &mut Snapshot) -> Screens {
    let mut screens = Screens::new(theme.clone());
    screens.update(snap, &[], frame);
    screens.update(snap, &TO_VIDEO, frame);
    snap.settings = video_page();
    screens.update(snap, &[], frame);
    screens
}

/// A game with a title too long for the title band, on the video page: the
/// breadcrumb cuts the title and keeps the page.
fn breadcrumb_long(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = in_game();
    snap.session = Some(SessionInfo::new(GameId(1), LONG_TITLE));
    let mut screens = video_page_of(frame, theme, &mut snap);
    screens.update(&snap, &[UiAction::Down], frame);
}

/// The video page of the quick menu on a flat panel: the picture style and
/// the refresh policy have no effect there, so both rows are disabled with
/// the reason.
fn quick_menu_panel(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = in_game();
    let mut screens = video_page_of(frame, theme, &mut snap);
    for row in &mut snap.settings.rows {
        if row.key == STYLE || row.key == REFRESH {
            *row = row.clone().unavailable("Not used on a flat panel");
        }
    }
    screens.update(&snap, &[UiAction::Down], frame);
}

/// The keep prompt after a change of the refresh policy on the video page.
fn quick_menu_keep(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = in_game();
    let mut screens = video_page_of(frame, theme, &mut snap);
    let force_60 = [
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
    snap.settings = in_game().settings;
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
        "[!] filtered",
    )
    .with_alert(true)
}

/// The longest status of the fixtures: a mode line that fills its column
/// and a result that needs two lines.
pub(super) fn long_status() -> VideoStatus {
    VideoStatus::new(
        "256x240             60.099 Hz",
        "1920x238p 15.73 kHz 59.940 Hz  native",
        "[!] cropped 2 lines, speed (60.099 to 59.940 Hz)",
    )
    .with_alert(true)
}

/// A game slowed to the rate of a 31 kHz mode that the fixture monitor
/// `pc_31_120` shows.
pub(super) fn slowed_status() -> VideoStatus {
    VideoStatus::new(
        "320x240             60.099 Hz",
        "640x480p  31.47 kHz 59.940 Hz  2x",
        "[!] speed (60.099 to 59.940 Hz)",
    )
    .with_alert(true)
}

/// A snapshot of a running game with the quick menu page and `status`.
pub(super) fn in_game_with(status: VideoStatus) -> Snapshot {
    let mut snap = in_game();
    snap.video = Some(status);
    snap
}

/// The video page of the quick menu with the video block of `status`.
fn quick_menu_with(frame: &mut UiFrame, theme: &Theme, status: VideoStatus) {
    let mut snap = in_game_with(status);
    video_page_of(frame, theme, &mut snap);
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
    screens.update(&snap, &TO_GAME_DISPLAY, frame);
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

/// The volume row at `percent`, from `source`, written at the levels of
/// `scopes`.
pub(super) fn volume_row(percent: i64, source: Level, scopes: Scopes) -> MenuItem {
    setting(
        VOLUME,
        "Volume",
        SettingValue::Number {
            value: percent,
            min: 0,
            max: 100,
            step: 5,
            unit: "%".to_owned(),
        },
        source,
        scopes,
    )
}

/// The rows of the audio page: the volume, and the output device of the
/// bench PC: one HDA card with three HDMI/DP outputs whose monitor
/// `/proc` cannot place, and the first of them chosen.
pub(super) fn audio_rows() -> Vec<MenuItem> {
    let global = Scopes {
        global: true,
        ..Scopes::NONE
    };
    let mut output = setting(
        AUDIO_OUTPUT,
        "Audio output",
        choice(
            2,
            vec![
                ChoiceOption::new("auto").with_detail("Automatic"),
                ChoiceOption::new("Analog (HDA Intel PCH)")
                    .with_detail("ALC221 Analog")
                    .with_short("Analog"),
                ChoiceOption::new("HDMI/DP 1 (HDA Intel PCH)").with_short("HDMI/DP 1"),
                ChoiceOption::new("HDMI/DP 2 (HDA Intel PCH)").with_short("HDMI/DP 2"),
                ChoiceOption::new("HDMI/DP 3 (HDA Intel PCH)").with_short("HDMI/DP 3"),
            ],
        ),
        Level::Global,
        global,
    );
    output.live = false;
    vec![volume_row(75, Level::Global, global), output]
}

/// A snapshot with the fixture systems and the audio page.
pub(super) fn with_audio() -> Snapshot {
    let mut snap = with_systems();
    snap.settings = SettingsPage::new(AUDIO_PAGE, "Audio", audio_rows());
    snap
}

fn audio(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_audio();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_AUDIO, frame);
    screens.update(&snap, &[], frame);
}

fn audio_output(frame: &mut UiFrame, theme: &Theme) {
    let snap = with_audio();
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &TO_AUDIO, frame);
    screens.update(&snap, &[UiAction::Down, UiAction::Accept], frame);
    screens.update(&snap, &[UiAction::Down], frame);
}

/// The quick menu with the volume of the game, set for its system.
fn quick_menu_volume(frame: &mut UiFrame, theme: &Theme) {
    let mut snap = in_game();
    if let Some(volume) = snap.settings.rows.iter_mut().find(|r| r.key == VOLUME) {
        *volume = volume_row(60, Level::System, Scopes::ALL);
    }
    let mut screens = Screens::new(theme.clone());
    screens.update(&snap, &[], frame);
    screens.update(&snap, &[UiAction::Menu, UiAction::Down], frame);
}
