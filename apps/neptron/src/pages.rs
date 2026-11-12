//! The settings pages of the UI: schema entries as menu rows, and a menu
//! value back as a schema value.

use neptron_settings::{
    DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY, DISPLAY_SCANLINE_STRENGTH,
    DISPLAY_SCANLINES, DISPLAY_STYLE, Entry, INPUT_MENU_HOLD_START_SECONDS, Kind, Level, Source,
    Value,
};
use neptron_ui::Value as UiValue;
use neptron_ui::screens::{
    DISPLAY_PAGE, GRID_PATTERN, OPEN_DISPLAY, QUICK_MENU_PAGE, QUIT_GAME, RESUME,
};
use neptron_ui::snapshot::{
    ActionId, ChoiceOption, Level as UiLevel, MenuItem, Scopes, SettingKey, SettingValue,
    SettingsPage,
};

use crate::config::Config;
use crate::display::{AUTO, Detected};
use crate::presets::Presets;

/// The first setting key of an action row. Keys below it are positions in
/// the schema.
pub const ACTION_KEY_BASE: u32 = 0x0001_0000;

/// The rows of the display panel, written at the global level. The
/// display panel is the one settings page that home reaches, so it also
/// holds the global settings that are not about the picture.
pub const DISPLAY_KEYS: [&str; 7] = [
    DISPLAY_OUTPUT,
    DISPLAY_MONITOR,
    DISPLAY_STYLE,
    DISPLAY_REFRESH_POLICY,
    DISPLAY_SCANLINES,
    DISPLAY_SCANLINE_STRENGTH,
    INPUT_MENU_HOLD_START_SECONDS,
];

/// The settings rows of the quick menu, written at the game level.
pub const QUICK_MENU_KEYS: [&str; 4] = [
    DISPLAY_STYLE,
    DISPLAY_REFRESH_POLICY,
    DISPLAY_SCANLINES,
    DISPLAY_SCANLINE_STRENGTH,
];

/// The choices of the setting rows that name things, from what the run
/// found.
#[derive(Debug, Clone, Copy)]
pub struct Names<'a> {
    /// The monitor presets.
    pub presets: &'a Presets,
    /// The detected outputs.
    pub detected: &'a Detected,
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
    rows.push(action(0, "Grid pattern", GRID_PATTERN));
    SettingsPage::new(DISPLAY_PAGE, "Display", rows)
}

/// The rows of the quick menu.
pub fn quick_menu_page(config: &Config, names: Names<'_>) -> SettingsPage {
    let mut rows = vec![action(1, "Resume", RESUME)];
    rows.extend(
        QUICK_MENU_KEYS
            .iter()
            .filter_map(|key| row(config, names, key, View::Game)),
    );
    rows.push(action(2, "Display", OPEN_DISPLAY));
    rows.push(action(3, "Quit game", QUIT_GAME));
    SettingsPage::new(QUICK_MENU_PAGE, "Quick menu", rows)
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
        (Kind::Name { .. }, UiValue::Choice(i)) => names_of(entry.key, names, config)
            .get(i)
            .cloned()
            .map(Value::Text),
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
            step: if key == DISPLAY_SCANLINE_STRENGTH {
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
            options: choices.iter().map(|c| ChoiceOption::new(*c)).collect(),
        },
        Kind::Name { .. } => {
            let options = names_of(key, names, config);
            SettingValue::Choice {
                index: options
                    .iter()
                    .position(|n| resolved.value == Value::Text(n.clone()))
                    .unwrap_or(0),
                options: options.into_iter().map(ChoiceOption::new).collect(),
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
    Some(item)
}

/// The options of a row that names a thing: `auto`, then what the run
/// found. A value in the settings that the run did not find stays an
/// option, so the row shows it.
fn names_of(key: &str, names: Names<'_>, config: &Config) -> Vec<String> {
    let mut options = vec![AUTO.to_owned()];
    match key {
        DISPLAY_MONITOR => options.extend(names.presets.profiles().iter().map(|p| p.id.clone())),
        DISPLAY_OUTPUT => options.extend(names.detected.outputs.iter().cloned()),
        _ => {}
    }
    let current = config.text(key);
    if !options.contains(&current) {
        options.push(current);
    }
    options
}

fn unit(key: &str) -> &'static str {
    match key {
        DISPLAY_SCANLINE_STRENGTH => "%",
        INPUT_MENU_HOLD_START_SECONDS => "s",
        _ => "",
    }
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
    fn the_display_page_holds_every_global_setting_and_the_grid() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
        };
        let page = display_page(&config, names);
        assert!(page.is_for(DISPLAY_PAGE));
        assert_eq!(page.rows.len(), DISPLAY_KEYS.len() + 1);
        let globals = config
            .schema()
            .entries()
            .iter()
            .filter(|e| e.levels.allows(Level::Global))
            .count();
        assert_eq!(DISPLAY_KEYS.len(), globals, "every setting has a row");
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
    fn the_quick_menu_has_its_actions_around_the_game_settings() {
        let config = config();
        let presets = Presets::default();
        let detected = Detected::default();
        let names = Names {
            presets: &presets,
            detected: &detected,
        };
        let page = quick_menu_page(&config, names);
        assert_eq!(page.rows.len(), QUICK_MENU_KEYS.len() + 3);
        assert_eq!(page.rows[0].value, SettingValue::Action(RESUME));
        assert!(page.rows[1].scopes.game);
    }
}
