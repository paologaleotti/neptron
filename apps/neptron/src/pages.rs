//! The settings pages of the UI: schema entries as menu rows, and a menu
//! value back as a schema value.

use neptron_audio::OutputDevice;
use neptron_input::{MapKind, PLAYER_PORTS, Roster, RosterEntry};
use neptron_settings::{
    AUDIO_OUTPUT, AUDIO_VOLUME, DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY,
    DISPLAY_SCANLINE_STRENGTH, DISPLAY_SCANLINES, DISPLAY_STYLE, DISPLAY_TV, DISPLAY_TV_WIDTH,
    DisplayClass, Entry, INPUT_MENU_HOLD_START_SECONDS, Kind, Level, Source, Value,
};
use neptron_types::TvStandard;
use neptron_ui::Value as UiValue;
use neptron_ui::font::has_glyph;
use neptron_ui::screens::{
    AUDIO_PAGE, DISPLAY_PAGE, GRID_PAGE, INPUT_PAGE, LIBRARY_PAGE, NETWORK_PAGE, QUICK_MENU_PAGE,
    QUIT_GAME, RESUME, SETTINGS_PAGE, SYSTEM_PAGE, VIDEO_PAGE,
};
use neptron_ui::snapshot::{
    ActionId, ChoiceOption, Level as UiLevel, MenuItem, PageId, Scopes, SettingKey, SettingValue,
    SettingsPage,
};
use neptron_video::{Phase, SafeReason};

use crate::config::Config;
use crate::display::{AUTO, Detected, FLAT};
use crate::network::NetworkInfo;
use crate::presets::Presets;
use crate::storage::FormatOffer;

/// The first setting key of an action row. Keys below it are positions in
/// the schema.
pub const ACTION_KEY_BASE: u32 = 0x0001_0000;

/// The action of the row that scans the games folder again. The UI hands
/// an action below its reserved numbers to the app.
pub const RESCAN_GAMES: ActionId = ActionId(1);

/// The label of the rescan row.
pub const RESCAN_LABEL: &str = "Rescan games";

/// The reason that the rescan row is disabled while a scan runs. The row
/// keeps its label.
pub const SCANNING_REASON: &str = "Scanning...";

/// The reason that the rescan row is disabled in a game: a game run lists
/// no games, and the return to the list checks the folders.
pub const RESCAN_IN_GAME: &str = "Not during a game";

/// The action of the `Reboot` row of the System panel.
pub const REBOOT: ActionId = ActionId(2);

/// The action of the `Shut down` row of the System panel.
pub const POWER_OFF: ActionId = ActionId(3);

/// The action of the `Format share` row of the System panel.
pub const FORMAT_SHARE: ActionId = ActionId(4);

/// The action of the `Keep this picture` row of the Display panel in safe
/// mode.
pub const KEEP_PICTURE: ActionId = ActionId(5);

/// The label of the row that keeps the picture on screen in safe mode.
pub const KEEP_PICTURE_LABEL: &str = "Keep this picture";

/// The action of the `Try my settings again` row of the Display panel in
/// safe mode after failed boots.
pub const TRY_SETTINGS: ActionId = ActionId(6);

/// The label of the row that ends safe mode after failed boots and applies
/// the settings again.
pub const TRY_SETTINGS_LABEL: &str = "Try my settings again";

/// The label of the row that asks for the format of the share partition.
pub const FORMAT_SHARE_LABEL: &str = "Format share";

/// The reason that the format row is disabled after the request file could
/// not be written in this run.
pub const FORMAT_NOT_SAVED: &str = "Not saved";

/// The reason that the format row is disabled after the request file was
/// written but the run could not end with the reboot: the next start
/// formats.
pub const FORMAT_NEXT_RESTART: &str = "At next restart";

/// The Info notice when the request file is written but the run takes no
/// reboot now.
pub const FORMAT_NEXT_RESTART_NOTICE: &str = "The format runs at the next restart.";

/// What became of the format request of this run, for the format row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatRow {
    /// No request, or one that waits for the writer: the row is as offered.
    Offered,
    /// The request file was not written.
    NotSaved,
    /// The request file is written, and the run did not reboot.
    Written,
}

/// The Warning notice when the request file could not be written.
pub const FORMAT_NOT_SAVED_NOTICE: &str =
    "The format request was not saved, so nothing is formatted.";

/// The label of the row that reboots the machine.
pub const REBOOT_LABEL: &str = "Reboot";

/// The label of the row that turns the machine off.
pub const SHUT_DOWN_LABEL: &str = "Shut down";

/// The reason that the rows of the System panel are disabled in a game:
/// the player quits the game first, so its quit writes the save RAM.
pub const POWER_IN_GAME: &str = "Not during a game";

/// The reason that the rows of the System panel are disabled in a run
/// that no launcher with power control started: the desktop window, or
/// the frontend alone on a bench.
pub const POWER_NO_LAUNCHER: &str = "Needs the launcher";

/// The reason that a row shows when its setting has no effect on the flat
/// panel that the run drives. It fits the value and source columns of a row.
pub const NOT_USED_ON_A_PANEL: &str = "Not used on a flat panel";

/// The reason that a row shows when its setting has no effect on the TV
/// that the run drives.
pub const NOT_USED_ON_A_TV: &str = "Not used on a TV";

/// The standard of the `auto` option of the TV standard.
pub const TV_AUTO_STANDARD: TvStandard = crate::display::TV_SAFE_STANDARD;

/// The reason that a TV standard the output does not offer cannot be
/// chosen.
pub const NOT_OFFERED: &str = "Not offered by this output";

/// The label of the address row with no port: no address, or none known.
pub const ADDRESS_LABEL: &str = "Address";

/// The text of the address row when the file is valid and no port has an
/// address.
pub const NO_ADDRESS: &str = "None (no cable or no network)";

/// The text of the address row when the address file is missing, cannot
/// be read, or is not valid.
pub const ADDRESS_NOT_KNOWN: &str = "Not known";

/// The text of the address row before the first read of the run ended.
pub const ADDRESS_READING: &str = "Reading...";

/// The name and the password of the published account of the network
/// share. They are public by design and must follow the user database of
/// the image (`os/board/common/rootfs-overlay/etc/ksmbd/ksmbdpwd.db`).
pub const SHARE_ACCOUNT: &str = "neptron";

/// The first key of the address rows of the Network panel: one row for
/// each port, after the action keys of the other rows.
const ADDRESS_KEY_BASE: u32 = ACTION_KEY_BASE + 0x100;

/// The rows of the display panel, written at the global level. The TV
/// rows show only on a TV. The expert keys that the picture mode covers
/// (vertical games, panel scale) have no row.
pub const DISPLAY_KEYS: [&str; 8] = [
    DISPLAY_OUTPUT,
    DISPLAY_MONITOR,
    DISPLAY_TV,
    DISPLAY_TV_WIDTH,
    DISPLAY_STYLE,
    DISPLAY_REFRESH_POLICY,
    DISPLAY_SCANLINES,
    DISPLAY_SCANLINE_STRENGTH,
];

/// The settings rows of the Input panel, after the rows of the players,
/// written at the global level.
pub const INPUT_KEYS: [&str; 1] = [INPUT_MENU_HOLD_START_SECONDS];

/// The text of a player row with no pad.
pub const NO_PAD: &str = "None";

/// What follows the name of a pad that went away in a game: its port waits
/// for it.
pub const AWAY_SUFFIX: &str = " (away)";

/// What follows the name of a present pad that uses the generic layout.
pub const GENERIC_SUFFIX: &str = " (generic layout)";

/// What follows the name of a present pad with a map of the player.
pub const YOURS_SUFFIX: &str = " (your layout)";

/// The label of the row of a pad on an extra port: it drives the menus,
/// and the core does not read it.
pub const EXTRA_PAD_LABEL: &str = "Extra pad";

/// The characters that a row of a settings list holds with a scroll bar:
/// the label, one space and the text. A pad name is cut to fit, so the row
/// is never clipped.
const ROW_MAX_CHARS: usize = 44;

/// The characters of a row that opens a page, with a scroll bar: the arrow
/// takes the room of one more.
const LINK_ROW_MAX_CHARS: usize = 43;

/// The rows of the audio panel, written at the global level.
pub const AUDIO_KEYS: [&str; 2] = [AUDIO_VOLUME, AUDIO_OUTPUT];

/// The settings rows of the first page of the quick menu, written at the
/// game level.
pub const QUICK_MENU_KEYS: [&str; 1] = [AUDIO_VOLUME];

/// The rows of the video page of the quick menu, written at the game level.
/// The TV width shows only on a TV.
pub const VIDEO_KEYS: [&str; 5] = [
    DISPLAY_TV_WIDTH,
    DISPLAY_STYLE,
    DISPLAY_REFRESH_POLICY,
    DISPLAY_SCANLINES,
    DISPLAY_SCANLINE_STRENGTH,
];

/// The detail of the `auto` option of the audio output.
const AUTO_OUTPUT_DETAIL: &str = "Automatic";
/// The detail of a stored output that the backend did not list.
const MISSING_OUTPUT_DETAIL: &str = "Not connected";

/// The choices of the setting rows that name things, from what the run
/// found.
#[derive(Debug, Clone, Copy)]
pub struct Names<'a> {
    /// The monitor presets.
    pub presets: &'a Presets,
    /// The detected outputs.
    pub detected: &'a Detected,
    /// What each picture style gives on the monitor now, from the engine's
    /// `style_rows`. A style with no entry shows as a plain option.
    pub styles: &'a [StyleOption],
    /// The class of the display that the run drives. A row whose setting
    /// does not apply to it is disabled.
    pub class: DisplayClass,
    /// The playback devices the audio backend listed at start, as menu
    /// options.
    pub outputs: &'a [OutputDevice],
}

/// What one picture style gives, for its option in the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleOption {
    /// The style, as the schema names it.
    pub key: &'static str,
    /// The line rate and the field rate of its mode, for example
    /// `15.73 kHz  59.94 Hz`; empty when nothing wins.
    pub detail: String,
    /// Why the player cannot choose it, `None` when the player can.
    pub unavailable: Option<String>,
}

/// Which level a page reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    /// The global level: the value every game inherits.
    Global,
    /// The game of the run: game, system, global, default.
    Game,
}

/// The rows of the display panel.
pub fn display_page(config: &Config, names: Names<'_>) -> SettingsPage {
    let mut rows: Vec<MenuItem> = DISPLAY_KEYS
        .iter()
        .filter_map(|key| row(config, names, key, View::Global))
        .collect();
    rows.push(page_row(0, "Grid pattern", GRID_PAGE));
    SettingsPage::new(DISPLAY_PAGE, "Display", rows)
}

/// The rows that end safe mode, at the end of the Display panel: `Keep
/// this picture` in every safe state, and `Try my settings again` after
/// failed boots. None outside safe mode.
pub fn safe_mode_rows(phase: Phase) -> Vec<MenuItem> {
    let Phase::Safe(reason) = phase else {
        return Vec::new();
    };
    let mut rows = vec![action(9, KEEP_PICTURE_LABEL, KEEP_PICTURE)];
    if reason == SafeReason::FailedBoots {
        rows.push(action(10, TRY_SETTINGS_LABEL, TRY_SETTINGS));
    }
    rows
}

/// The rows of the audio panel.
pub fn audio_page(config: &Config, names: Names<'_>) -> SettingsPage {
    let rows = AUDIO_KEYS
        .iter()
        .filter_map(|key| row(config, names, key, View::Global))
        .collect();
    SettingsPage::new(AUDIO_PAGE, "Audio", rows)
}

/// The rows of the Input panel: the pad of each player, each pad on an
/// extra port, the keyboard, and the input settings.
pub fn input_page(config: &Config, names: Names<'_>, roster: &Roster) -> SettingsPage {
    let mut rows = Vec::new();
    for (port, n) in (0..PLAYER_PORTS).zip(1u32..) {
        let label = port_label(port);
        rows.push(match roster.get(port) {
            Some(entry) => pad_row(PLAYER_ROW_FIRST + n - 1, port, entry, &label),
            None => text_row(PLAYER_ROW_FIRST + n - 1, &label, NO_PAD),
        });
    }
    let extra = (PLAYER_PORTS..neptron_input::MAX_PORTS)
        .filter_map(|port| roster.get(port).map(|entry| (port, entry)));
    for ((port, entry), n) in extra.zip(EXTRA_ROW_FIRST..) {
        rows.push(pad_row(n, port, entry, EXTRA_PAD_LABEL));
    }
    rows.push(text_row(28, "Keyboard", "Player 1"));
    rows.extend(
        INPUT_KEYS
            .iter()
            .filter_map(|key| row(config, names, key, View::Global)),
    );
    SettingsPage::new(INPUT_PAGE, "Input", rows)
}

/// The label of the row of the pad on `port`, and the title of its page:
/// `Player 1` to `Player 4`, or `Extra pad`.
pub fn port_label(port: u8) -> String {
    if port < PLAYER_PORTS {
        format!("Player {}", port + 1)
    } else {
        EXTRA_PAD_LABEL.to_owned()
    }
}

/// The row of the pad on `port`: a present pad opens its page, an away
/// pad is an information row.
fn pad_row(n: u32, port: u8, entry: &RosterEntry, label: &str) -> MenuItem {
    if !entry.present {
        return text_row(n, label, &pad_text(entry, label, ROW_MAX_CHARS));
    }
    MenuItem::new(
        SettingKey(ACTION_KEY_BASE + n),
        label,
        SettingValue::Link {
            page: crate::pad_page::pad_page(port),
            text: pad_text(entry, label, LINK_ROW_MAX_CHARS),
        },
    )
}

/// The key of the row of the Input panel that shows the pad on `port` of
/// `roster`, or `None` when the port has no pad.
pub fn input_row_key(roster: &Roster, port: u8) -> Option<SettingKey> {
    roster.get(port)?;
    let n = if port < PLAYER_PORTS {
        PLAYER_ROW_FIRST + u32::from(port)
    } else {
        let before = roster
            .ports()
            .iter()
            .take(usize::from(port))
            .skip(usize::from(PLAYER_PORTS))
            .flatten()
            .count();
        EXTRA_ROW_FIRST + u32::try_from(before).ok()?
    };
    Some(SettingKey(ACTION_KEY_BASE + n))
}

/// The row number of the row of player 1. The rows of players 2 to 4
/// follow.
const PLAYER_ROW_FIRST: u32 = 20;

/// The row number of the first extra pad row. The rows of the 4 extra
/// ports follow, before the keyboard row.
const EXTRA_ROW_FIRST: u32 = 24;

/// The text of the row of a pad: its name, with `(away)` when it went
/// away, `(generic layout)` for a present pad with the generic layout, or
/// `(your layout)` for a map of the player, cut so that the row with
/// `label` fits.
fn pad_text(entry: &RosterEntry, label: &str, max_chars: usize) -> String {
    let suffix = match (entry.present, entry.map) {
        (false, _) => AWAY_SUFFIX,
        (true, MapKind::Generic) => GENERIC_SUFFIX,
        (true, MapKind::Player) => YOURS_SUFFIX,
        (true, MapKind::Known) => "",
    };
    let room = max_chars
        .saturating_sub(label.chars().count() + 1)
        .saturating_sub(suffix.len());
    format!("{}{suffix}", fit_name(&entry.pad.name, room))
}

/// `name` in at most `room` characters: a character that the font does not
/// have shows as `?`, and a name that is too long ends with `...`.
pub(crate) fn fit_name(name: &str, room: usize) -> String {
    let shown: Vec<char> = name
        .chars()
        .map(|c| {
            if has_glyph(c) && !c.is_control() {
                c
            } else {
                '?'
            }
        })
        .collect();
    if shown.len() <= room {
        return shown.into_iter().collect();
    }
    let keep = room.saturating_sub(3);
    shown[..keep].iter().chain(['.'; 3].iter()).collect()
}

/// The rows of the first page of the quick menu: resume, the volume of the
/// game, the video page, the Settings page and quit.
pub fn quick_menu_page(config: &Config, names: Names<'_>) -> SettingsPage {
    let mut rows = vec![action(1, "Resume", RESUME)];
    rows.extend(
        QUICK_MENU_KEYS
            .iter()
            .filter_map(|key| row(config, names, key, View::Game)),
    );
    rows.push(page_row(2, "Video", VIDEO_PAGE));
    rows.push(page_row(5, "Settings", SETTINGS_PAGE));
    rows.push(action(3, "Quit game", QUIT_GAME));
    SettingsPage::new(QUICK_MENU_PAGE, "Quick menu", rows)
}

/// The row that scans the games folder again: disabled while a scan runs
/// and during a game.
pub fn rescan_row(scanning: bool, in_game: bool) -> MenuItem {
    match (scanning, in_game) {
        (_, true) => action(4, RESCAN_LABEL, RESCAN_GAMES).unavailable(RESCAN_IN_GAME),
        (true, false) => action(4, RESCAN_LABEL, RESCAN_GAMES).unavailable(SCANNING_REASON),
        (false, false) => action(4, RESCAN_LABEL, RESCAN_GAMES),
    }
}

/// The rows of the Library panel: the rescan of the games folder. The
/// panel is listed in a game too, where its row is disabled.
pub fn library_page(scanning: bool, in_game: bool) -> SettingsPage {
    SettingsPage::new(LIBRARY_PAGE, "Library", vec![rescan_row(scanning, in_game)])
}

/// The rows of the System panel: reboot and shut down, each behind a
/// confirm question, and the format of the share partition when the
/// storage script offers it (`offer`); with no offer that row is absent,
/// since no other state has a format that fixes it. With no power
/// control (`power`) every row is disabled; in a game too, and the panel
/// is still listed. After a request of this run that was not saved or that
/// waits for the next restart (`format`), the format row is disabled.
pub fn system_page(
    in_game: bool,
    power: bool,
    offer: Option<&FormatOffer>,
    format: FormatRow,
) -> SettingsPage {
    let reason = match (power, in_game) {
        (false, _) => Some(POWER_NO_LAUNCHER),
        (true, true) => Some(POWER_IN_GAME),
        (true, false) => None,
    };
    let rows = [
        action(6, REBOOT_LABEL, REBOOT).with_confirm(
            "Reboot NEPTRON?",
            "Settings and saves are written first. Then the machine starts again.",
            "Restarting...",
        ),
        action(7, SHUT_DOWN_LABEL, POWER_OFF).with_confirm(
            "Shut down NEPTRON?",
            "Settings and saves are written first. Then the machine turns off.",
            "Shutting down...",
        ),
    ];
    let format_row = offer.map(|offer| {
        action(8, FORMAT_SHARE_LABEL, FORMAT_SHARE).with_confirm(
            "Format the share partition?",
            format!(
                "NEPTRON finds no file system it can read on the partition for games ({}). Formatting it erases everything on it.",
                offer.size_text()
            ),
            "Formatting at the next start...",
        )
    });
    let format_row = format_row.map(|row| match (reason, format) {
        (None, FormatRow::NotSaved) => row.unavailable(FORMAT_NOT_SAVED),
        (None, FormatRow::Written) => row.unavailable(FORMAT_NEXT_RESTART),
        _ => row,
    });
    let rows = rows
        .into_iter()
        .chain(format_row)
        .map(|row| match reason {
            Some(reason) => row.unavailable(reason),
            None => row,
        })
        .collect();
    SettingsPage::new(SYSTEM_PAGE, "System", rows)
}

/// The information rows of the Network panel: one address row for each
/// port, the host name, the state of the share and, with the share on, its
/// mDNS name and how to reach it. With no read yet (`None`) or an unknown
/// status, the address row alone: the host name comes from the same file.
/// With no valid host name the name rows go, and the share paths use the
/// first address, or go too when there is none.
pub fn network_page(info: Option<&NetworkInfo>) -> SettingsPage {
    let rows = match info {
        None => vec![text_row(11, ADDRESS_LABEL, ADDRESS_READING)],
        Some(NetworkInfo::Unknown { .. }) => vec![text_row(11, ADDRESS_LABEL, ADDRESS_NOT_KNOWN)],
        Some(NetworkInfo::Known { status, share_on }) => {
            let mut rows: Vec<MenuItem> = status
                .addresses
                .iter()
                .zip(ADDRESS_KEY_BASE..)
                .map(|(a, key)| {
                    MenuItem::new(
                        SettingKey(key),
                        format!("{ADDRESS_LABEL} {}", a.port),
                        SettingValue::Text(a.ipv4.to_string()),
                    )
                })
                .collect();
            if rows.is_empty() {
                rows.push(text_row(11, ADDRESS_LABEL, NO_ADDRESS));
            }
            // mDNS runs only with the share on: the share script starts
            // `mdnsd` with the SMB server.
            let mdns = status
                .host_name
                .as_ref()
                .filter(|_| *share_on)
                .map(|name| format!("{name}.local"));
            let target = status
                .addresses
                .first()
                .map(|a| a.ipv4.to_string())
                .or_else(|| mdns.clone());
            if let Some(name) = &status.host_name {
                rows.push(text_row(12, "Host name", name));
            }
            if let Some(mdns) = &mdns {
                rows.push(text_row(13, "Network name", mdns));
            }
            rows.push(text_row(14, "Share", if *share_on { "On" } else { "Off" }));
            if *share_on {
                if let Some(target) = &target {
                    rows.push(text_row(15, "Windows", &format!("\\\\{target}\\share")));
                    rows.push(text_row(
                        16,
                        "macOS, Linux",
                        &format!("smb://{target}/share"),
                    ));
                }
                rows.push(text_row(17, "Share user", SHARE_ACCOUNT));
                rows.push(text_row(18, "Share password", SHARE_ACCOUNT));
            }
            rows
        }
    };
    SettingsPage::new(NETWORK_PAGE, "Network", rows)
}

/// The rows of the video page of the quick menu: the display settings of
/// the game.
pub fn video_page(config: &Config, names: Names<'_>) -> SettingsPage {
    let rows = VIDEO_KEYS
        .iter()
        .filter_map(|key| row(config, names, key, View::Game))
        .collect();
    SettingsPage::new(VIDEO_PAGE, "Video", rows)
}

/// The schema entry of a row key.
pub fn entry(config: &Config, key: SettingKey) -> Option<&Entry> {
    config.schema().entries().get(key.0 as usize)
}

/// The schema value of a menu value for `entry`. `None` for a value of
/// the wrong kind or out of the options.
pub fn value_of(entry: &Entry, value: UiValue, names: Names<'_>, config: &Config) -> Option<Value> {
    match (entry.kind, value) {
        (Kind::Bool { .. }, UiValue::Toggle(v)) => Some(Value::Bool(v)),
        (Kind::Int { .. }, UiValue::Number(n)) => Some(Value::Int(n)),
        (Kind::Choice { choices, .. }, UiValue::Choice(i)) => {
            choices.get(i).map(|c| Value::Text((*c).to_owned()))
        }
        (Kind::Name { .. } | Kind::Text { .. }, UiValue::Choice(i)) => {
            names_of(entry.key, names, config)
                .get(i)
                .cloned()
                .map(Value::Text)
        }
        _ => None,
    }
}

/// The UI level of a settings level.
pub fn ui_level(level: Level) -> UiLevel {
    match level {
        Level::Global => UiLevel::Global,
        Level::System => UiLevel::System,
        Level::Game => UiLevel::Game,
    }
}

/// The settings level of a UI level; `None` for the default level, which
/// is never written.
pub fn settings_level(level: UiLevel) -> Option<Level> {
    match level {
        UiLevel::Default => None,
        UiLevel::Global => Some(Level::Global),
        UiLevel::System => Some(Level::System),
        UiLevel::Game => Some(Level::Game),
    }
}

fn row(config: &Config, names: Names<'_>, key: &str, view: View) -> Option<MenuItem> {
    let position = config
        .schema()
        .entries()
        .iter()
        .position(|e| e.key == key)?;
    let entry = &config.schema().entries()[position];
    if !entry.shown(names.class) {
        return None;
    }
    let resolved = match view {
        View::Global => config.resolved_global(key)?,
        View::Game => config.resolved(key)?,
    };
    let value = match entry.kind {
        Kind::Bool { .. } => SettingValue::Toggle(resolved.value == Value::Bool(true)),
        Kind::Int { min, max, .. } => SettingValue::Number {
            value: match resolved.value {
                Value::Int(v) => v,
                _ => min,
            },
            min,
            max,
            step: if key == DISPLAY_SCANLINE_STRENGTH || key == AUDIO_VOLUME {
                5
            } else {
                1
            },
            unit: unit(key).to_owned(),
        },
        Kind::Choice { choices, .. } => SettingValue::Choice {
            index: choices
                .iter()
                .position(|c| resolved.value == Value::Text((*c).to_owned()))
                .unwrap_or(0),
            options: choices
                .iter()
                .map(|c| choice_option(key, c, names))
                .collect(),
        },
        Kind::Name { .. } | Kind::Text { .. } => {
            let options = names_of(key, names, config);
            SettingValue::Choice {
                index: options
                    .iter()
                    .position(|n| resolved.value == Value::Text(n.clone()))
                    .unwrap_or(0),
                options: options
                    .into_iter()
                    .map(|n| name_option(key, n, names))
                    .collect(),
            }
        }
    };
    let mut item = MenuItem::new(
        SettingKey(u32::try_from(position).ok()?),
        entry.label,
        value,
    );
    item.source = match resolved.source {
        Source::Game => UiLevel::Game,
        Source::System => UiLevel::System,
        Source::Global => UiLevel::Global,
        Source::Default => UiLevel::Default,
    };
    item.scopes = match (view, key) {
        // The connector is not an input of the guard, and a new connector
        // needs the display opened again, so the menu shows it and the
        // settings file changes it (open point in the README).
        (_, DISPLAY_OUTPUT) => Scopes::NONE,
        (View::Global, _) => Scopes {
            global: entry.levels.allows(Level::Global),
            ..Scopes::NONE
        },
        (View::Game, _) => Scopes {
            global: entry.levels.allows(Level::Global),
            system: entry.levels.allows(Level::System),
            game: entry.levels.allows(Level::Game),
        },
    };
    item.live = entry.live;
    if let Some(reason) = not_used(entry, names.class) {
        item = item.unavailable(reason);
    }
    Some(item)
}

/// The reason a row of `entry` is disabled on a display of `class`, or
/// `None` when the setting has an effect there.
pub fn not_used(entry: &Entry, class: DisplayClass) -> Option<&'static str> {
    if entry.applies_to.applies(class) {
        return None;
    }
    Some(match class {
        DisplayClass::Tv => NOT_USED_ON_A_TV,
        DisplayClass::Crt | DisplayClass::FlatPanel => NOT_USED_ON_A_PANEL,
    })
}

/// The options of a row that names a thing: `auto`, then what the run
/// found. A value in the settings that the run did not find stays an
/// option, so the row shows it.
fn names_of(key: &str, names: Names<'_>, config: &Config) -> Vec<String> {
    let mut options = vec![AUTO.to_owned()];
    match key {
        DISPLAY_MONITOR => {
            if names.detected.panel.is_some() {
                options.push(FLAT.to_owned());
            }
            options.extend(names.presets.profiles().iter().map(|p| p.id.clone()));
        }
        DISPLAY_OUTPUT => options.extend(names.detected.outputs.iter().cloned()),
        AUDIO_OUTPUT => options.extend(names.outputs.iter().map(|o| o.name.clone())),
        _ => {}
    }
    let current = config.text(key);
    if !options.contains(&current) {
        options.push(current);
    }
    options
}

/// The option of a row that names a thing: `auto` and `flat` of the
/// monitor row say what they drive, the panel's name and native mode. An
/// audio output shows its short label, with the device part of its raw
/// name as the detail.
fn name_option(key: &str, name: String, names: Names<'_>) -> ChoiceOption {
    if key == AUDIO_OUTPUT {
        return output_option(name, names.outputs);
    }
    let detail = match (key, name.as_str()) {
        (DISPLAY_MONITOR, AUTO) => Some(names.detected.auto_text()),
        (DISPLAY_MONITOR, FLAT) => names.detected.flat_text(),
        _ => None,
    };
    let option = ChoiceOption::new(name);
    match detail {
        Some(detail) => option.with_detail(detail),
        None => option,
    }
}

/// The option of an audio output: `auto`, a listed device with its label
/// and detail, or a stored device that the backend did not list, by its
/// name. A listed device shows the head of its label in the row.
fn output_option(name: String, outputs: &[OutputDevice]) -> ChoiceOption {
    if name == AUTO {
        return ChoiceOption::new(name).with_detail(AUTO_OUTPUT_DETAIL);
    }
    let Some(device) = outputs.iter().find(|o| o.name == name) else {
        return ChoiceOption::new(name).with_detail(MISSING_OUTPUT_DETAIL);
    };
    let mut option = ChoiceOption::new(device.label.clone());
    if !device.detail.is_empty() {
        option = option.with_detail(device.detail.clone());
    }
    match short_label(&device.label) {
        Some(short) => option.with_short(short),
        None => option,
    }
}

/// The head of a label of the form `head (where)`, for the value column of
/// a row: `HDMI/DP 1 (HDA Intel PCH)` gives `HDMI/DP 1`. `None` for a
/// label with no such tail.
fn short_label(label: &str) -> Option<&str> {
    let (head, _) = label.split_once(" (")?;
    (!head.is_empty()).then_some(head)
}

/// The option of choice `choice`: for a picture style, with what it gives
/// on the monitor now; for a TV standard, its name and rate.
fn choice_option(key: &str, choice: &str, names: Names<'_>) -> ChoiceOption {
    match key {
        DISPLAY_STYLE => style_option(choice, names.styles),
        DISPLAY_TV => tv_option(choice, names.detected),
        _ => ChoiceOption::new(choice),
    }
}

/// The option of a TV standard: its name with its rate, and `auto` with
/// the standard it gives as the detail. A standard that the output does
/// not offer cannot be chosen.
fn tv_option(choice: &str, detected: &Detected) -> ChoiceOption {
    let (option, standard) = match TvStandard::from_key(choice) {
        Some(standard) => (
            ChoiceOption::new(crate::tv::label(standard)).with_short(crate::tv::short(standard)),
            standard,
        ),
        None => (
            ChoiceOption::new(choice).with_detail(crate::tv::label(TV_AUTO_STANDARD)),
            TV_AUTO_STANDARD,
        ),
    };
    if detected.is_tv() && !detected.offers(standard) {
        option.unavailable(NOT_OFFERED)
    } else {
        option
    }
}

/// The option of a picture style, with what it gives on the monitor now.
fn style_option(choice: &str, styles: &[StyleOption]) -> ChoiceOption {
    let option = ChoiceOption::new(choice);
    match styles.iter().find(|s| s.key == choice) {
        Some(style) => {
            let option = option.with_detail(style.detail.clone());
            match &style.unavailable {
                Some(reason) => option.unavailable(reason.clone()),
                None => option,
            }
        }
        None => option,
    }
}

fn unit(key: &str) -> &'static str {
    match key {
        DISPLAY_SCANLINE_STRENGTH => "%",
        INPUT_MENU_HOLD_START_SECONDS => "s",
        AUDIO_VOLUME => "%",
        _ => "",
    }
}

/// A row that opens `page`. Its key is an action key, as for an action
/// row: it holds no setting.
fn page_row(n: u32, label: &str, page: PageId) -> MenuItem {
    MenuItem::new(
        SettingKey(ACTION_KEY_BASE + n),
        label,
        SettingValue::Page(page),
    )
}

/// An information row. Its key is an action key: it holds no setting.
fn text_row(n: u32, label: &str, text: &str) -> MenuItem {
    MenuItem::new(
        SettingKey(ACTION_KEY_BASE + n),
        label,
        SettingValue::Text(text.to_owned()),
    )
}

fn action(n: u32, label: &str, id: ActionId) -> MenuItem {
    MenuItem::new(
        SettingKey(ACTION_KEY_BASE + n),
        label,
        SettingValue::Action(id),
    )
}

#[cfg(test)]
mod tests {
    use neptron_settings::Storage;

    use super::*;

    fn config() -> Config {
        Config::load(&Storage::new("/nonexistent/config"), None).0
    }

    #[test]
    fn the_rescan_row_is_disabled_while_a_scan_runs_and_in_a_game() {
        let idle = rescan_row(false, false);
        assert_eq!(idle.label, RESCAN_LABEL);
        assert_eq!(idle.value, SettingValue::Action(RESCAN_GAMES));
        assert!(idle.is_enabled());
        let busy = rescan_row(true, false);
        assert_eq!(busy.label, RESCAN_LABEL, "the label stays");
        assert_eq!(busy.unavailable.as_deref(), Some(SCANNING_REASON));
        let page = library_page(false, false);
        assert!(page.is_for(LIBRARY_PAGE));
        assert_eq!(page.rows, [idle]);
        for scanning in [false, true] {
            let in_game = rescan_row(scanning, true);
            assert_eq!(in_game.unavailable.as_deref(), Some(RESCAN_IN_GAME));
        }
        const { assert!(RESCAN_GAMES.0 < neptron_ui::screens::RESERVED_ID_MIN) };
    }

    #[test]
    fn the_system_rows_ask_first_and_are_disabled_in_a_game_or_with_no_launcher() {
        let page = system_page(false, true, None, FormatRow::Offered);
        assert!(page.is_for(SYSTEM_PAGE));
        let rows: Vec<_> = page
            .rows
            .iter()
            .map(|r| (r.label.as_str(), &r.value, r.unavailable.as_deref()))
            .collect();
        assert_eq!(
            rows,
            [
                (REBOOT_LABEL, &SettingValue::Action(REBOOT), None),
                (SHUT_DOWN_LABEL, &SettingValue::Action(POWER_OFF), None),
            ]
        );
        let questions: Vec<_> = page
            .rows
            .iter()
            .map(|r| r.confirm.as_ref().map(|c| c.question.as_str()))
            .collect();
        assert_eq!(
            questions,
            [Some("Reboot NEPTRON?"), Some("Shut down NEPTRON?")]
        );
        for (in_game, power, reason) in [
            (true, true, POWER_IN_GAME),
            (false, false, POWER_NO_LAUNCHER),
            (true, false, POWER_NO_LAUNCHER),
        ] {
            let page = system_page(in_game, power, Some(&offer()), FormatRow::Offered);
            assert!(
                page.rows
                    .iter()
                    .all(|r| r.unavailable.as_deref() == Some(reason)),
                "{in_game} {power}"
            );
        }
        for id in [REBOOT, POWER_OFF, FORMAT_SHARE] {
            assert!(id.0 < neptron_ui::screens::RESERVED_ID_MIN);
            assert_ne!(id, RESCAN_GAMES);
        }
        assert_ne!(FORMAT_SHARE, REBOOT);
        assert_ne!(FORMAT_SHARE, POWER_OFF);
    }

    fn offer() -> FormatOffer {
        FormatOffer {
            device: "/dev/sda4".to_owned(),
            start: 10240,
            sectors: 110_546_944,
            kind: "7".to_owned(),
        }
    }

    #[test]
    fn the_format_row_is_there_only_with_an_offer_and_asks_first() {
        assert_eq!(
            system_page(false, true, None, FormatRow::Offered)
                .rows
                .len(),
            2
        );
        let page = system_page(false, true, Some(&offer()), FormatRow::Offered);
        let row = page.rows.last().expect("a row");
        assert_eq!(page.rows.len(), 3);
        assert_eq!(row.label, FORMAT_SHARE_LABEL);
        assert_eq!(row.value, SettingValue::Action(FORMAT_SHARE));
        assert!(row.is_enabled());
        let confirm = row.confirm.as_ref().expect("a question");
        assert_eq!(confirm.question, "Format the share partition?");
        assert_eq!(
            confirm.message,
            "NEPTRON finds no file system it can read on the partition for games (56.6 GB). Formatting it erases everything on it."
        );
        assert_eq!(confirm.working, "Formatting at the next start...");
        assert_eq!(
            system_page(true, true, Some(&offer()), FormatRow::Offered).rows[2]
                .unavailable
                .as_deref(),
            Some(POWER_IN_GAME)
        );
        assert_eq!(
            system_page(false, false, Some(&offer()), FormatRow::Offered).rows[2]
                .unavailable
                .as_deref(),
            Some(POWER_NO_LAUNCHER)
        );
        for (format, reason) in [
            (FormatRow::NotSaved, FORMAT_NOT_SAVED),
            (FormatRow::Written, FORMAT_NEXT_RESTART),
        ] {
            let page = system_page(false, true, Some(&offer()), format);
            assert_eq!(page.rows[2].unavailable.as_deref(), Some(reason));
            assert!(page.rows[..2].iter().all(|r| r.is_enabled()), "{format:?}");
            let no_power = system_page(false, false, Some(&offer()), format);
            assert_eq!(
                no_power.rows[2].unavailable.as_deref(),
                Some(POWER_NO_LAUNCHER),
                "the reason of the whole panel comes first"
            );
        }
    }

    #[test]
    fn the_display_audio_and_input_pages_hold_every_global_setting() {
        let config = config();
        let globals: Vec<_> = config
            .schema()
            .entries()
            .iter()
            .filter(|e| e.levels.allows(Level::Global) && !e.expert)
            .map(|e| e.key)
            .collect();
        let shown: Vec<_> = DISPLAY_KEYS
            .iter()
            .chain(&AUDIO_KEYS)
            .chain(&INPUT_KEYS)
            .copied()
            .collect();
        assert_eq!(shown.len(), globals.len());
        for key in globals {
            assert!(shown.contains(&key), "{key} has no row");
        }
    }

    #[test]
    fn the_audio_page_shows_the_volume_and_the_outputs_with_auto_first() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let outputs = neptron_audio::alsa_outputs(&neptron_audio::AlsaText {
            cards: " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n".to_owned(),
            pcm: "00-00: ALC221 Analog : ALC221 Analog : playback 1\n00-03: HDMI 0 : HDMI 0 : playback 1\n".to_owned(),
            elds: vec![(0, "monitor_present 1\neld_valid 1\nmonitor_name DELL U2724D\n".to_owned())],
        });
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            outputs: &outputs,
            class: DisplayClass::Crt,
        };
        let page = audio_page(&config, names);
        assert!(page.is_for(AUDIO_PAGE));
        assert_eq!(page.rows.len(), 2);
        assert_eq!(
            page.rows[0].value,
            SettingValue::Number {
                value: 80,
                min: 0,
                max: 100,
                step: 5,
                unit: "%".to_owned(),
            }
        );
        assert!(page.rows[0].live);
        assert!(page.rows[0].scopes.global && !page.rows[0].scopes.game);
        let SettingValue::Choice { index, options } = &page.rows[1].value else {
            panic!("a choice");
        };
        assert_eq!(*index, 0);
        let shown: Vec<_> = options
            .iter()
            .map(|o| (o.label.as_str(), o.detail.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                ("auto", "Automatic"),
                ("Analog (HDA Intel PCH)", "ALC221 Analog"),
                ("DELL U2724D (HDMI/DP)", "Monitor connected"),
            ]
        );
        let shorts: Vec<_> = options.iter().map(|o| o.value_text()).collect();
        assert_eq!(shorts, ["auto", "Analog", "DELL U2724D"]);
        assert!(!page.rows[1].live);
        let output = config.schema().get(AUDIO_OUTPUT).expect("output");
        assert_eq!(
            value_of(output, UiValue::Choice(1), names, &config),
            Some(Value::Text("hw:CARD=PCH,DEV=0".to_owned()))
        );
        assert_eq!(value_of(output, UiValue::Choice(3), names, &config), None);
    }

    #[test]
    fn a_short_label_is_the_head_before_the_place() {
        assert_eq!(short_label("HDMI/DP 1 (HDA Intel PCH)"), Some("HDMI/DP 1"));
        assert_eq!(short_label("Analog"), None);
        assert_eq!(short_label(" (HDA Intel PCH)"), None);
    }

    #[test]
    fn a_stored_output_that_is_not_connected_stays_an_option() {
        let mut config = config();
        config
            .set(
                Level::Global,
                AUDIO_OUTPUT,
                Value::Text("hw:CARD=Device,DEV=0".to_owned()),
            )
            .expect("set");
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            outputs: &[],
            class: DisplayClass::Crt,
        };
        let page = audio_page(&config, names);
        let SettingValue::Choice { index, options } = &page.rows[1].value else {
            panic!("a choice");
        };
        assert_eq!(*index, 1);
        assert_eq!(options[1].label, "hw:CARD=Device,DEV=0");
        assert_eq!(options[1].detail, "Not connected");
    }

    #[test]
    fn the_display_page_holds_its_global_settings_and_the_grid() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            class: DisplayClass::Crt,
            outputs: &[],
        };
        let page = display_page(&config, names);
        assert!(page.is_for(DISPLAY_PAGE));
        assert_eq!(
            page.rows.len(),
            DISPLAY_KEYS.len() - 2 + 1,
            "no TV row on a CRT"
        );
        assert_eq!(page.rows[0].scopes, Scopes::NONE, "the output is read-only");
        assert!(page.rows[1].scopes.global);
        assert!(
            !page.rows[2].scopes.game,
            "the panel writes the global level"
        );
    }

    #[test]
    fn menu_values_map_back_to_schema_values() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            class: DisplayClass::Crt,
            outputs: &[],
        };
        let style = config.schema().get(DISPLAY_STYLE).expect("style");
        assert_eq!(
            value_of(style, UiValue::Choice(2), names, &config),
            Some(Value::Text("2x".to_owned()))
        );
        assert_eq!(value_of(style, UiValue::Choice(99), names, &config), None);
        assert_eq!(value_of(style, UiValue::Number(1), names, &config), None);
        let monitor = config.schema().get(DISPLAY_MONITOR).expect("monitor");
        assert_eq!(
            value_of(monitor, UiValue::Choice(0), names, &config),
            Some(Value::Text(AUTO.to_owned()))
        );
        let strength = config
            .schema()
            .get(DISPLAY_SCANLINE_STRENGTH)
            .expect("strength");
        assert_eq!(
            value_of(strength, UiValue::Number(50), names, &config),
            Some(Value::Int(50))
        );
    }

    #[test]
    fn picture_styles_show_their_rates_and_grey_out_what_the_monitor_cannot_show() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let styles = [
            StyleOption {
                key: "auto",
                detail: "15.73 kHz  59.94 Hz".to_owned(),
                unavailable: None,
            },
            StyleOption {
                key: "2x",
                detail: String::new(),
                unavailable: Some("Needs another monitor".to_owned()),
            },
        ];
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &styles,
            class: DisplayClass::Crt,
            outputs: &[],
        };
        let page = video_page(&config, names);
        let style = page
            .rows
            .iter()
            .find(|r| r.label == "Picture mode")
            .expect("the style row");
        let SettingValue::Choice { options, .. } = &style.value else {
            panic!("a choice");
        };
        assert_eq!(options[0].detail, "15.73 kHz  59.94 Hz");
        assert!(options[0].is_enabled());
        assert!(!options[2].is_enabled(), "2x");
        assert_eq!(options[1].detail, "", "a style with no row has no detail");
    }

    #[test]
    fn the_monitor_row_names_the_panel_and_its_native_mode() {
        let config = config();
        let presets = Presets::default();
        let edid = include_bytes!("../tests/fixtures/edid/panel_1080p.bin");
        let platform = neptron_platform::Platform {
            gpus: Vec::new(),
            connectors: vec![neptron_platform::Connector {
                card: "card0".to_owned(),
                name: "HDMI-A-1".to_owned(),
                kind: "HDMI-A".to_owned(),
                status: neptron_platform::ConnectorStatus::Connected,
                edid: Some(edid.to_vec()),
            }],
        };
        let detected = Detected::of(&platform, "HDMI-A-1", None);
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            class: DisplayClass::Crt,
            outputs: &[],
        };
        let page = display_page(&config, names);
        let monitor = page
            .rows
            .iter()
            .find(|r| r.label == "Monitor profile")
            .expect("the monitor row");
        let SettingValue::Choice { options, index } = &monitor.value else {
            panic!("a choice");
        };
        assert_eq!(*index, 0);
        assert_eq!(options[0].label, AUTO);
        assert_eq!(options[0].detail, "NPT FHD 24  1920x1080 60 Hz");
        assert_eq!(options[1].label, FLAT);
        assert_eq!(options.len(), 2, "no preset in this test");
    }

    #[test]
    fn rows_with_no_effect_on_the_display_are_disabled_with_the_reason() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        for class in [DisplayClass::Crt, DisplayClass::FlatPanel, DisplayClass::Tv] {
            let names = Names {
                presets: &presets,
                detected: &detected,
                styles: &[],
                class,
                outputs: &[],
            };
            for page in [
                display_page(&config, names),
                quick_menu_page(&config, names),
                video_page(&config, names),
            ] {
                for row in page.rows.iter().filter(|r| r.key.0 < ACTION_KEY_BASE) {
                    let entry = entry(&config, row.key).expect("a setting");
                    let expected = not_used(entry, class).map(str::to_owned);
                    assert_eq!(row.unavailable, expected, "{} {class:?}", entry.key);
                    assert!(entry.shown(class), "{} {class:?}", entry.key);
                }
            }
        }
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            class: DisplayClass::FlatPanel,
            outputs: &[],
        };
        let page = video_page(&config, names);
        let disabled: Vec<_> = page
            .rows
            .iter()
            .filter(|r| !r.is_enabled())
            .map(|r| r.label.as_str())
            .collect();
        assert_eq!(
            disabled,
            ["Refresh policy"],
            "the picture mode applies to a panel, the refresh policy does not"
        );
    }

    #[test]
    fn every_action_or_page_row_has_its_own_key() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            class: DisplayClass::Crt,
            outputs: &[],
        };
        let mut keys: Vec<_> = [
            quick_menu_page(&config, names),
            display_page(&config, names),
            library_page(false, false),
            system_page(false, true, Some(&offer()), FormatRow::Offered),
        ]
        .iter()
        .flat_map(|page| page.rows.iter().map(|r| r.key))
        .filter(|k| k.0 >= ACTION_KEY_BASE)
        .collect();
        let count = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), count, "{keys:?}");
    }

    #[test]
    fn the_quick_menu_has_its_actions_around_the_game_settings() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            class: DisplayClass::Crt,
            outputs: &[],
        };
        let page = quick_menu_page(&config, names);
        let rows: Vec<_> = page
            .rows
            .iter()
            .map(|r| (r.label.as_str(), &r.value))
            .collect();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0], ("Resume", &SettingValue::Action(RESUME)));
        assert_eq!(rows[1].0, "Volume");
        assert!(page.rows[1].scopes.game, "the volume of the game");
        assert_eq!(rows[2], ("Video", &SettingValue::Page(VIDEO_PAGE)));
        assert_eq!(rows[3], ("Settings", &SettingValue::Page(SETTINGS_PAGE)));
        assert_eq!(rows[4], ("Quit game", &SettingValue::Action(QUIT_GAME)));
        let keys: Vec<_> = page.rows.iter().map(|r| r.key).collect();
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), keys.len(), "one key per row");
        let video = video_page(&config, names);
        assert!(video.is_for(VIDEO_PAGE));
        assert_eq!(
            video.rows.len(),
            VIDEO_KEYS.len() - 1,
            "no TV width on a CRT"
        );
        assert!(video.rows.iter().all(|r| r.scopes.game));
    }

    #[test]
    fn on_a_tv_the_display_page_shows_the_standard_and_the_width() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
            styles: &[],
            class: DisplayClass::Tv,
            outputs: &[],
        };
        let page = display_page(&config, names);
        let labels: Vec<_> = page.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Output",
                "Monitor profile",
                "TV standard",
                "TV picture width",
                "Picture mode",
                "Refresh policy",
                "Scanlines",
                "Scanline strength",
                "Grid pattern",
            ]
        );
        assert_eq!(page.rows[1].unavailable.as_deref(), Some(NOT_USED_ON_A_TV));
        assert_eq!(
            page.rows[5].unavailable.as_deref(),
            Some(NOT_USED_ON_A_TV),
            "the standard fixes the rate"
        );
        let SettingValue::Choice { index, options } = &page.rows[2].value else {
            panic!("a choice");
        };
        assert_eq!(*index, 0);
        let shown: Vec<_> = options
            .iter()
            .map(|o| (o.label.as_str(), o.detail.as_str()))
            .collect();
        assert_eq!(shown[0], ("auto", "NTSC (60 Hz)"));
        assert_eq!(shown[1], ("NTSC (60 Hz)", ""));
        assert_eq!(shown[4], ("PAL 60 (60 Hz)", ""));
        assert_eq!(
            options[4].value_text(),
            "PAL 60",
            "the short form in the row"
        );
        assert_eq!(shown[6], ("PAL (50 Hz)", ""));
        assert_eq!(options.len(), neptron_settings::TV_STANDARDS.len());
        assert!(page.rows[2].scopes.global && !page.rows[2].scopes.game);
        let tv = config.schema().get(DISPLAY_TV).expect("tv");
        assert_eq!(
            value_of(tv, UiValue::Choice(6), names, &config),
            Some(Value::Text("pal".to_owned()))
        );
        let SettingValue::Choice { options, .. } = &page.rows[3].value else {
            panic!("a choice");
        };
        let widths: Vec<_> = options.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(widths, ["aspect", "integer", "fill"]);
        let video = video_page(&config, names);
        assert_eq!(video.rows[0].label, "TV picture width");
        assert!(video.rows[0].scopes.game, "the width of one game");

        let detected = Detected::default().with_tv(Some(vec![TvStandard::Ntsc, TvStandard::Pal]));
        let names = Names {
            detected: &detected,
            ..names
        };
        let page = display_page(&config, names);
        let SettingValue::Choice { options, .. } = &page.rows[2].value else {
            panic!("a choice");
        };
        let enabled: Vec<_> = options
            .iter()
            .filter(|o| o.is_enabled())
            .map(|o| o.label.as_str())
            .collect();
        assert_eq!(enabled, ["auto", "NTSC (60 Hz)", "PAL (50 Hz)"]);
    }

    fn network(addresses: &[(&str, [u8; 4])], share_on: bool) -> NetworkInfo {
        named(Some("neptron"), addresses, share_on)
    }

    fn named(host: Option<&str>, addresses: &[(&str, [u8; 4])], share_on: bool) -> NetworkInfo {
        NetworkInfo::Known {
            status: neptron_platform::NetworkStatus {
                host_name: host.map(str::to_owned),
                addresses: addresses
                    .iter()
                    .map(|&(port, ipv4)| neptron_platform::PortAddress {
                        port: port.to_owned(),
                        ipv4: std::net::Ipv4Addr::from(ipv4),
                    })
                    .collect(),
            },
            share_on,
        }
    }

    fn texts(page: &SettingsPage) -> Vec<(&str, &str)> {
        page.rows
            .iter()
            .map(|r| match &r.value {
                SettingValue::Text(text) => (r.label.as_str(), text.as_str()),
                other => panic!("not an information row: {other:?}"),
            })
            .collect()
    }

    fn names_for<'a>(presets: &'a Presets, detected: &'a Detected) -> Names<'a> {
        Names {
            presets,
            detected,
            styles: &[],
            class: DisplayClass::Crt,
            outputs: &[],
        }
    }

    fn pad_entry(name: &str, present: bool) -> RosterEntry {
        RosterEntry {
            pad: neptron_input::PadInfo {
                identity: neptron_input::DeviceIdentity(1),
                path: neptron_input::DeviceIdentity(2),
                name: name.to_owned(),
            },
            present,
            map: neptron_input::MapKind::Known,
        }
    }

    /// The label and the text or value of each row of the Input panel.
    fn input_rows(roster: &Roster) -> Vec<(String, String)> {
        let (presets, detected) = (Presets::default(), Detected::default());
        let page = input_page(&config(), names_for(&presets, &detected), roster);
        assert!(page.is_for(INPUT_PAGE));
        page.rows
            .iter()
            .map(|r| {
                let text = match &r.value {
                    SettingValue::Text(text) | SettingValue::Link { text, .. } => text.clone(),
                    SettingValue::Number { value, unit, .. } => format!("{value} {unit}"),
                    other => panic!("an unexpected row: {other:?}"),
                };
                (r.label.clone(), text)
            })
            .collect()
    }

    fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
        rows.iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    #[test]
    fn the_input_panel_with_no_pad_has_its_four_players() {
        assert_eq!(
            input_rows(&Roster::new()),
            pairs(&[
                ("Player 1", "None"),
                ("Player 2", "None"),
                ("Player 3", "None"),
                ("Player 4", "None"),
                ("Keyboard", "Player 1"),
                ("Hold Start for menu", "0 s"),
            ])
        );
    }

    #[test]
    fn the_input_panel_shows_each_pad_an_away_pad_and_the_extra_pads() {
        let mut roster = Roster::new();
        let pads = [
            (0, pad_entry("Xbox 360 Controller", true)),
            (1, pad_entry("PS4 Controller", false)),
            (3, pad_entry("Pad \u{e9}\u{1}\u{fffd}\u{263a}", true)),
            (5, pad_entry("Extra One", true)),
            (7, pad_entry("Extra Two", true)),
        ];
        for (port, pad) in pads {
            roster.set(port, Some(pad)).expect("a port");
        }
        assert_eq!(
            input_rows(&roster),
            pairs(&[
                ("Player 1", "Xbox 360 Controller"),
                ("Player 2", "PS4 Controller (away)"),
                ("Player 3", "None"),
                ("Player 4", "Pad \u{e9}???"),
                ("Extra pad", "Extra One"),
                ("Extra pad", "Extra Two"),
                ("Keyboard", "Player 1"),
                ("Hold Start for menu", "0 s"),
            ])
        );
    }

    fn generic_entry(name: &str, present: bool) -> RosterEntry {
        RosterEntry {
            map: MapKind::Generic,
            ..pad_entry(name, present)
        }
    }

    #[test]
    fn a_pad_with_a_map_of_the_player_says_so_and_still_fits() {
        let mut roster = Roster::new();
        let yours = |name: &str, present| RosterEntry {
            map: MapKind::Player,
            ..pad_entry(name, present)
        };
        roster
            .set(0, Some(yours("USB Joystick", true)))
            .expect("a port");
        roster
            .set(1, Some(yours("USB Joystick", false)))
            .expect("a port");
        roster
            .set(
                2,
                Some(yours("A Very Long Gamepad Name From A Vendor", true)),
            )
            .expect("a port");
        let rows = input_rows(&roster);
        assert_eq!(
            rows[..3],
            pairs(&[
                ("Player 1", "USB Joystick (your layout)"),
                ("Player 2", "USB Joystick (away)"),
                ("Player 3", "A Very Long Gamep... (your layout)"),
            ])
        );
        for (label, text) in &rows[..3] {
            assert!(
                label.chars().count() + 1 + text.chars().count() <= ROW_MAX_CHARS,
                "{label} {text}"
            );
        }
    }

    #[test]
    fn a_pad_with_the_generic_layout_says_so_and_still_fits() {
        let mut roster = Roster::new();
        roster
            .set(0, Some(pad_entry("8BitDo Pad", true)))
            .expect("a port");
        roster
            .set(1, Some(generic_entry("USB Joystick", true)))
            .expect("a port");
        roster
            .set(2, Some(generic_entry("USB Joystick", false)))
            .expect("a port");
        roster
            .set(
                3,
                Some(generic_entry(
                    "A Very Long Gamepad Name From A Vendor",
                    true,
                )),
            )
            .expect("a port");
        roster
            .set(5, Some(generic_entry("Generic USB Joystick", true)))
            .expect("a port");
        let rows = input_rows(&roster);
        assert_eq!(
            rows[..5],
            pairs(&[
                ("Player 1", "8BitDo Pad"),
                ("Player 2", "USB Joystick (generic layout)"),
                ("Player 3", "USB Joystick (away)"),
                ("Player 4", "A Very Long Ga... (generic layout)"),
                ("Extra pad", "Generic USB J... (generic layout)"),
            ])
        );
        for (label, text) in &rows[..5] {
            assert!(
                label.chars().count() + 1 + text.chars().count() <= ROW_MAX_CHARS,
                "{label} {text}"
            );
        }
    }

    #[test]
    fn the_row_key_of_a_port_is_the_key_of_its_row_on_the_panel() {
        let mut roster = Roster::new();
        for port in [0, 2, 5, 7] {
            roster
                .set(port, Some(pad_entry("Pad", true)))
                .expect("a port");
        }
        let (presets, detected) = (Presets::default(), Detected::default());
        let page = input_page(&config(), names_for(&presets, &detected), &roster);
        let key_of = |label: &str, nth: usize| {
            page.rows
                .iter()
                .filter(|r| r.label == label)
                .nth(nth)
                .map(|r| r.key)
        };
        assert_eq!(input_row_key(&roster, 0), key_of("Player 1", 0));
        assert_eq!(input_row_key(&roster, 2), key_of("Player 3", 0));
        assert_eq!(input_row_key(&roster, 5), key_of("Extra pad", 0));
        assert_eq!(input_row_key(&roster, 7), key_of("Extra pad", 1));
        assert_eq!(input_row_key(&roster, 1), None, "no pad");
        assert_eq!(input_row_key(&roster, 9), None, "no port");
    }

    #[test]
    fn a_present_pad_opens_the_page_of_its_port_and_an_away_pad_does_not() {
        let mut roster = Roster::new();
        roster
            .set(0, Some(pad_entry("Pad A", true)))
            .expect("a port");
        roster
            .set(1, Some(pad_entry("Pad B", false)))
            .expect("a port");
        roster
            .set(5, Some(pad_entry("Pad F", true)))
            .expect("a port");
        let (presets, detected) = (Presets::default(), Detected::default());
        let page = input_page(&config(), names_for(&presets, &detected), &roster);
        let pages: Vec<_> = page
            .rows
            .iter()
            .map(|r| match &r.value {
                SettingValue::Link { page, .. } => Some(*page),
                _ => None,
            })
            .collect();
        assert_eq!(
            pages[..5],
            [
                Some(crate::pad_page::pad_page(0)),
                None,
                None,
                None,
                Some(crate::pad_page::pad_page(5)),
            ]
        );
        assert_eq!(port_label(5), "Extra pad");
        assert_eq!(port_label(3), "Player 4");
    }

    #[test]
    fn a_long_pad_name_is_cut_so_the_row_fits() {
        let long = "A Very Long Gamepad Name From A Vendor Database";
        let mut roster = Roster::new();
        roster.set(0, Some(pad_entry(long, true))).expect("a port");
        roster.set(1, Some(pad_entry(long, false))).expect("a port");
        roster.set(4, Some(pad_entry(long, true))).expect("a port");
        let rows = input_rows(&roster);
        assert_eq!(rows[0].1, "A Very Long Gamepad Name From A...");
        assert_eq!(rows[1].1, "A Very Long Gamepad Name ... (away)");
        assert_eq!(rows[4].1, "A Very Long Gamepad Name From ...");
        for (label, text) in [&rows[0], &rows[4]] {
            assert!(
                label.chars().count() + 1 + text.chars().count() <= LINK_ROW_MAX_CHARS,
                "the arrow of a present pad takes room: {label} {text}"
            );
        }
        for (label, text) in &rows[..5] {
            assert!(
                label.chars().count() + 1 + text.chars().count() <= ROW_MAX_CHARS,
                "{label} {text}"
            );
        }
        // A name that fits is kept whole.
        assert_eq!(fit_name("Pad", 3), "Pad");
        assert_eq!(fit_name("Pads", 3), "...");
    }

    #[test]
    fn the_network_rows_of_one_port_with_the_share_on() {
        let page = network_page(Some(&network(&[("eth0", [192, 168, 0, 112])], true)));
        assert!(page.is_for(NETWORK_PAGE));
        assert_eq!(
            texts(&page),
            [
                ("Address eth0", "192.168.0.112"),
                ("Host name", "neptron"),
                ("Network name", "neptron.local"),
                ("Share", "On"),
                ("Windows", "\\\\192.168.0.112\\share"),
                ("macOS, Linux", "smb://192.168.0.112/share"),
                ("Share user", "neptron"),
                ("Share password", "neptron"),
            ]
        );
    }

    #[test]
    fn several_ports_give_one_row_each_and_the_share_uses_the_first() {
        let info = network(
            &[
                ("enx001122334455", [10, 0, 0, 5]),
                ("eth0", [192, 168, 0, 2]),
            ],
            true,
        );
        let page = network_page(Some(&info));
        let rows = texts(&page);
        assert_eq!(rows[0], ("Address enx001122334455", "10.0.0.5"));
        assert_eq!(rows[1], ("Address eth0", "192.168.0.2"));
        assert!(rows.contains(&("macOS, Linux", "smb://10.0.0.5/share")));
        let keys: std::collections::BTreeSet<_> = page.rows.iter().map(|r| r.key).collect();
        assert_eq!(keys.len(), page.rows.len(), "every key differs");
        let labels: std::collections::BTreeSet<_> = page.rows.iter().map(|r| &r.label).collect();
        assert_eq!(labels.len(), page.rows.len(), "every label differs");
        assert!(page.rows.iter().all(|r| r.key.0 >= ACTION_KEY_BASE));
    }

    #[test]
    fn with_no_address_the_share_uses_the_network_name() {
        let page = network_page(Some(&network(&[], true)));
        let rows = texts(&page);
        assert_eq!(rows[0], (ADDRESS_LABEL, NO_ADDRESS));
        assert!(rows.contains(&("Windows", "\\\\neptron.local\\share")));
        assert!(rows.contains(&("macOS, Linux", "smb://neptron.local/share")));
    }

    #[test]
    fn with_the_share_off_the_panel_does_not_say_how_to_reach_it() {
        let page = network_page(Some(&network(&[("eth0", [192, 168, 0, 112])], false)));
        assert_eq!(
            texts(&page),
            [
                ("Address eth0", "192.168.0.112"),
                ("Host name", "neptron"),
                ("Share", "Off"),
            ],
            "with the share off mdnsd does not run, so no network name"
        );
    }

    #[test]
    fn an_unknown_status_or_no_read_gives_the_address_row_alone() {
        let unknown = NetworkInfo::Unknown {
            reason: "cannot read /run/neptron/network.toml".to_owned(),
        };
        assert_eq!(
            texts(&network_page(Some(&unknown))),
            [(ADDRESS_LABEL, ADDRESS_NOT_KNOWN)]
        );
        assert_eq!(
            texts(&network_page(None)),
            [(ADDRESS_LABEL, ADDRESS_READING)]
        );
    }

    #[test]
    fn with_no_valid_host_name_the_name_rows_go_and_the_address_stays() {
        let page = network_page(Some(&named(None, &[("eth0", [192, 168, 0, 112])], true)));
        assert_eq!(
            texts(&page),
            [
                ("Address eth0", "192.168.0.112"),
                ("Share", "On"),
                ("Windows", "\\\\192.168.0.112\\share"),
                ("macOS, Linux", "smb://192.168.0.112/share"),
                ("Share user", "neptron"),
                ("Share password", "neptron"),
            ]
        );
        let page = network_page(Some(&named(None, &[], true)));
        assert_eq!(
            texts(&page),
            [
                (ADDRESS_LABEL, NO_ADDRESS),
                ("Share", "On"),
                ("Share user", "neptron"),
                ("Share password", "neptron"),
            ],
            "no address and no name: no share path"
        );
    }

    #[test]
    fn a_host_name_with_dots_is_shown_as_it_is() {
        let page = network_page(Some(&named(Some("neptron.lan"), &[], true)));
        let rows = texts(&page);
        assert!(rows.contains(&("Host name", "neptron.lan")));
        assert!(rows.contains(&("Network name", "neptron.lan.local")));
    }
}
