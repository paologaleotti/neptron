//! The settings of one run: the three layers of the context, read at
//! start, and changed in memory as the frame loop writes them.

use neptron_settings::{
    AUDIO_OUTPUT, AUDIO_VOLUME, DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY,
    DISPLAY_SCANLINE_STRENGTH, DISPLAY_SCANLINES, DISPLAY_STYLE, DISPLAY_TV, DISPLAY_TV_WIDTH,
    INPUT_MENU_HOLD_START_SECONDS, Layer, Layers, Level, Resolved, Schema, SetError, Storage,
    Target, Value, resolve_key,
};
use neptron_video::{
    PolicySettings, ScanlineSettings, SettingsError, policy_settings, scanline_settings,
};

use crate::outbox::FileTarget;

/// The game a run is for: the system and the game name (the ROM file name
/// without its extension).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameContext {
    /// The system id.
    pub system: String,
    /// The game name.
    pub game: String,
}

/// The schema and the layers of the context.
#[derive(Debug, Clone)]
pub struct Config {
    schema: Schema,
    global: Layer,
    system: Layer,
    game: Layer,
    context: Option<GameContext>,
}

/// One problem found while the settings were read. The setting falls back
/// to the level below; the frontend shows the problem as a notice.
pub type ConfigIssue = String;

impl Config {
    /// Reads the global layer and, for a game, its system and game layers.
    /// A file that cannot be read or parsed gives an empty layer and an
    /// issue: bad input never stops the boot.
    pub fn load(storage: &Storage, context: Option<GameContext>) -> (Self, Vec<ConfigIssue>) {
        let schema = Schema::builtin();
        let mut issues = Vec::new();
        let mut read = |target: Target<'_>| -> Layer {
            match storage.load(&schema, target) {
                Ok(loaded) => {
                    for issue in &loaded.parsed.issues {
                        issues.push(format!("{}: {issue:?}", loaded.path.display()));
                    }
                    loaded.parsed.layer
                }
                Err(e) => {
                    issues.push(e.to_string());
                    Layer::new(target.level())
                }
            }
        };
        let global = read(Target::Global);
        let (system, game) = match &context {
            Some(c) => (
                read(Target::System { system: &c.system }),
                read(Target::Game {
                    system: &c.system,
                    game: &c.game,
                }),
            ),
            None => (Layer::new(Level::System), Layer::new(Level::Game)),
        };
        (
            Self {
                schema,
                global,
                system,
                game,
                context,
            },
            issues,
        )
    }

    /// The schema.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The game of the run, if any.
    pub fn context(&self) -> Option<&GameContext> {
        self.context.as_ref()
    }

    /// `key` resolved for the context: game, system, global, default.
    pub fn resolved(&self, key: &str) -> Option<Resolved<'_>> {
        resolve_key(&self.schema, &self.layers(), key)
    }

    /// `key` resolved at the global level only: the value every game
    /// inherits.
    pub fn resolved_global(&self, key: &str) -> Option<Resolved<'_>> {
        let layers = Layers::try_new(Some(&self.global), None, None).unwrap_or_default();
        resolve_key(&self.schema, &layers, key)
    }

    /// The value of `key` at `level`, if that layer sets it.
    pub fn get(&self, level: Level, key: &str) -> Option<&Value> {
        self.layer(level).get(key)
    }

    /// Sets `key` at `level` in memory and returns the value it replaces.
    ///
    /// # Errors
    ///
    /// [`SetError`] for an unknown key, a level the key does not allow, or
    /// a value outside the schema. Nothing changes.
    pub fn set(
        &mut self,
        level: Level,
        key: &str,
        value: Value,
    ) -> Result<Option<Value>, SetError> {
        let schema = &self.schema;
        match level {
            Level::Global => self.global.set(schema, key, value),
            Level::System => self.system.set(schema, key, value),
            Level::Game => self.game.set(schema, key, value),
        }
    }

    /// Removes `key` at `level` in memory and returns the old value.
    pub fn unset(&mut self, level: Level, key: &str) -> Option<Value> {
        self.layer_mut(level).remove(key)
    }

    /// Puts `old` back for `key` at `level`, as before a change.
    pub fn restore(&mut self, level: Level, key: &str, old: Option<Value>) {
        match old {
            // The value was in the layer before, so it passes the checks.
            Some(value) => {
                let _ = self.set(level, key, value);
            }
            None => {
                let _ = self.unset(level, key);
            }
        }
    }

    /// The settings file of `level` in the context. `None` for a system or
    /// game level with no game.
    pub fn file(&self, level: Level) -> Option<FileTarget> {
        match (level, &self.context) {
            (Level::Global, _) => Some(FileTarget::Global),
            (Level::System, Some(c)) => Some(FileTarget::System(c.system.clone())),
            (Level::Game, Some(c)) => Some(FileTarget::Game {
                system: c.system.clone(),
                game: c.game.clone(),
            }),
            _ => None,
        }
    }

    /// The picture style, the refresh policy and the TV width of the
    /// context.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn policy(&self) -> Result<PolicySettings, SettingsError> {
        policy_settings(
            &self.text(DISPLAY_STYLE),
            &self.text(DISPLAY_REFRESH_POLICY),
            None,
            &self.text(DISPLAY_TV_WIDTH),
        )
    }

    /// The picture style, the refresh policy and the TV width of the global
    /// level, which the guard starts from.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn global_policy(&self) -> Result<PolicySettings, SettingsError> {
        let text = |key| {
            self.resolved_global(key)
                .map(|r| r.value.to_string())
                .unwrap_or_default()
        };
        policy_settings(
            &text(DISPLAY_STYLE),
            &text(DISPLAY_REFRESH_POLICY),
            None,
            &text(DISPLAY_TV_WIDTH),
        )
    }

    /// The scanline settings of the context.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn scanlines(&self) -> Result<ScanlineSettings, SettingsError> {
        scanline_settings(
            &self.text(DISPLAY_SCANLINES),
            self.int(DISPLAY_SCANLINE_STRENGTH),
        )
    }

    /// The Start hold of the quick menu, in nanoseconds; 0 is off.
    pub fn start_hold_ns(&self) -> u64 {
        u64::try_from(self.int(INPUT_MENU_HOLD_START_SECONDS))
            .unwrap_or(0)
            .saturating_mul(1_000_000_000)
    }

    /// The volume of the game, in percent: 100 is the core's own level.
    pub fn volume_percent(&self) -> u32 {
        u32::try_from(self.int(AUDIO_VOLUME)).unwrap_or(0)
    }

    /// The audio output setting: `auto` or a device name as the audio
    /// backend lists it.
    pub fn audio_output(&self) -> String {
        self.text(AUDIO_OUTPUT)
    }

    /// The monitor setting: `auto` or a profile id.
    pub fn monitor(&self) -> String {
        self.text(DISPLAY_MONITOR)
    }

    /// The TV standard setting: `auto` (NTSC) or the key of a standard.
    pub fn tv(&self) -> String {
        self.text(DISPLAY_TV)
    }

    /// The output setting: `auto` or a connector name.
    pub fn output(&self) -> String {
        self.text(DISPLAY_OUTPUT)
    }

    /// The text of `key` in the context.
    pub fn text(&self, key: &str) -> String {
        self.resolved(key)
            .map(|r| r.value.to_string())
            .unwrap_or_default()
    }

    fn int(&self, key: &str) -> i64 {
        match self.resolved(key).map(|r| r.value) {
            Some(Value::Int(v)) => v,
            _ => 0,
        }
    }

    fn layers(&self) -> Layers<'_> {
        let (system, game) = match self.context {
            Some(_) => (Some(&self.system), Some(&self.game)),
            None => (None, None),
        };
        // Each layer was made for its own level.
        Layers::try_new(Some(&self.global), system, game).unwrap_or_default()
    }

    fn layer(&self, level: Level) -> &Layer {
        match level {
            Level::Global => &self.global,
            Level::System => &self.system,
            Level::Game => &self.game,
        }
    }

    fn layer_mut(&mut self, level: Level) -> &mut Layer {
        match level {
            Level::Global => &mut self.global,
            Level::System => &mut self.system,
            Level::Game => &mut self.game,
        }
    }
}

#[cfg(test)]
mod tests {
    use neptron_settings::Source;

    use super::*;

    fn config(global: &str, system: &str, game: &str) -> Config {
        let dir = std::env::temp_dir().join(format!(
            "neptron-config-{}-{}",
            std::process::id(),
            global.len() * 31 + system.len() * 7 + game.len()
        ));
        std::fs::create_dir_all(dir.join("systems")).expect("dir");
        std::fs::create_dir_all(dir.join("games").join("nes")).expect("dir");
        std::fs::write(dir.join("global.toml"), global).expect("write");
        std::fs::write(dir.join("systems").join("nes.toml"), system).expect("write");
        std::fs::write(dir.join("games").join("nes").join("Game.toml"), game).expect("write");
        let context = GameContext {
            system: "nes".to_owned(),
            game: "Game".to_owned(),
        };
        let (config, issues) = Config::load(&Storage::new(&dir), Some(context));
        assert!(issues.is_empty(), "{issues:?}");
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
        config
    }

    #[test]
    fn the_game_level_wins_and_the_guard_starts_from_the_global_level() {
        let c = config(
            "[display]\nstyle = \"2x\"\n",
            "[display]\nstyle = \"3x\"\n",
            "[display]\nstyle = \"native\"\nscanline_strength = 40\n",
        );
        assert_eq!(c.policy().expect("policy").style.key(), "native");
        assert_eq!(c.global_policy().expect("policy").style.key(), "2x");
        assert_eq!(
            c.resolved(DISPLAY_SCANLINE_STRENGTH).expect("key").source,
            Source::Game
        );
    }

    #[test]
    fn a_change_in_memory_can_be_restored() {
        let mut c = config("", "", "");
        let old = c
            .set(Level::System, DISPLAY_STYLE, Value::Text("4x".to_owned()))
            .expect("set");
        assert_eq!(c.text(DISPLAY_STYLE), "4x");
        c.restore(Level::System, DISPLAY_STYLE, old);
        assert_eq!(c.text(DISPLAY_STYLE), "auto");
        assert!(
            c.set(Level::Game, DISPLAY_MONITOR, Value::Text("x".to_owned()))
                .is_err()
        );
    }

    #[test]
    fn the_tv_width_of_the_game_wins_and_the_standard_is_global() {
        let c = config(
            "[display]\ntv = \"pal\"\ntv_width = \"fill\"\n",
            "[display]\ntv_width = \"integer\"\n",
            "",
        );
        assert_eq!(c.policy().expect("policy").tv_width.key(), "integer");
        assert_eq!(c.global_policy().expect("policy").tv_width.key(), "fill");
        assert_eq!(c.tv(), "pal");
        let (none, _) = Config::load(&Storage::new("/nonexistent/config"), None);
        assert_eq!(none.tv(), "auto");
        assert_eq!(none.policy().expect("policy").tv_width.key(), "aspect");
    }

    #[test]
    fn the_start_hold_is_in_nanoseconds() {
        let c = config("[input]\nmenu_hold_start_seconds = 2\n", "", "");
        assert_eq!(c.start_hold_ns(), 2_000_000_000);
    }

    #[test]
    fn a_run_with_no_game_reads_the_global_level_only() {
        let (c, _) = Config::load(&Storage::new("/nonexistent/config"), None);
        assert_eq!(c.file(Level::Game), None);
        assert_eq!(c.file(Level::Global), Some(FileTarget::Global));
        assert_eq!(c.monitor(), "auto");
    }
}
