//! The settings of one run: the three layers of the context, read at
//! start, and changed in memory as the frame loop writes them.

use neptron_settings::{
    AUDIO_OUTPUT, AUDIO_VOLUME, DISPLAY_MONITOR, DISPLAY_ORIENTATION, DISPLAY_OUTPUT,
    DISPLAY_PANEL_SCALE, DISPLAY_REFRESH_POLICY, DISPLAY_ROTATION, DISPLAY_SCANLINE_STRENGTH,
    DISPLAY_SCANLINES, DISPLAY_SCREENSAVER, DISPLAY_SCREENSAVER_MINUTES, DISPLAY_STYLE, DISPLAY_TV,
    DISPLAY_TV_WIDTH, DISPLAY_VERTICAL, INPUT_MENU_HOLD_START_SECONDS, Layer, Layers, Level,
    PICTURE_MODE_COVERS, Resolved, Schema, SetError, Storage, Target, Value, resolve_key,
};
use neptron_types::{Orientation, Rotation};
use neptron_video::{
    PolicySettings, ScanlineSettings, SettingsError, policy_settings, scanline_settings,
    with_display_choices,
};

use crate::outbox::FileTarget;

/// Nanoseconds in one minute.
const NANOS_PER_MINUTE: u64 = 60_000_000_000;

/// The game a run is for: the system and the game key (the ROM path below
/// the system folder without its extension, see
/// [`GameFiles::key`](crate::game_files::GameFiles::key)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameContext {
    /// The system id.
    pub system: String,
    /// The game key, which names the game's settings file.
    pub game: String,
    /// The game is an arcade board
    /// ([`runs_arcade_boards`](crate::library::runs_arcade_boards)): the
    /// policy of the game takes the arcade rules.
    pub arcade: bool,
}

/// One key that a write changed, with its value before and after, for the
/// write to the file and for a revert.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyChange {
    /// The key.
    pub key: &'static str,
    /// The value before, `None` when unset.
    pub old: Option<Value>,
    /// The value now, `None` when removed.
    pub new: Option<Value>,
}

/// The two picture rows as the app stores them: a picture mode and a
/// refresh choice, always written together at one level.
#[derive(Debug, Clone, PartialEq)]
pub struct PicturePair {
    /// The value of `display.style`.
    pub style: Value,
    /// The value of `display.refresh_policy`.
    pub refresh: Value,
}

/// What a write of the picture pair does with the expert keys that the
/// picture mode covers (`PICTURE_MODE_COVERS`) at its level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpertKeys {
    /// The player picked the picture mode: the keys go, so the mode the
    /// player picks is what the player gets.
    Remove,
    /// A pick of the refresh rate (also one that sets the mode to Auto
    /// too), a keep, or a removal of the pair: the keys stay, as the menu
    /// judged the pair with them.
    Keep,
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
        self.resolved_up_to(Level::Global, key)
    }

    /// `key` resolved from the defaults up to `level`: the value that a
    /// write at `level` acts with, with no layer above it. The system level
    /// needs a game context; without one it is the global level.
    pub fn resolved_up_to(&self, level: Level, key: &str) -> Option<Resolved<'_>> {
        let layers = match (level, &self.context) {
            (Level::Game, Some(_)) => return self.resolved(key),
            (Level::System, Some(_)) => {
                Layers::try_new(Some(&self.global), Some(&self.system), None)
            }
            (Level::Global, _) | (Level::System | Level::Game, None) => {
                Layers::try_new(Some(&self.global), None, None)
            }
        };
        // Each layer was made for its own level.
        resolve_key(&self.schema, &layers.unwrap_or_default(), key)
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

    /// Writes the picture pair at `level` in memory: both keys set
    /// (`Some`), or both removed (`None`), so a level that the app writes
    /// holds both keys or neither. A set with [`ExpertKeys::Remove`] also
    /// removes, at the same level, the expert keys that the picture mode
    /// covers (`PICTURE_MODE_COVERS`). Returns every key that changed,
    /// with its old and new value, for the same write and for a revert.
    ///
    /// # Errors
    ///
    /// [`SetError`] when the layer refuses a value. Nothing changed.
    pub fn write_pair(
        &mut self,
        level: Level,
        pair: Option<PicturePair>,
        expert: ExpertKeys,
    ) -> Result<Vec<KeyChange>, SetError> {
        let before = |c: &Self, key| c.get(level, key).cloned();
        let old_style = before(self, DISPLAY_STYLE);
        let old_refresh = before(self, DISPLAY_REFRESH_POLICY);
        let mut changes = Vec::new();
        match pair {
            Some(pair) => {
                self.set(level, DISPLAY_STYLE, pair.style.clone())?;
                if let Err(e) = self.set(level, DISPLAY_REFRESH_POLICY, pair.refresh.clone()) {
                    self.restore(level, DISPLAY_STYLE, old_style);
                    return Err(e);
                }
                changes.push(KeyChange {
                    key: DISPLAY_STYLE,
                    old: old_style,
                    new: Some(pair.style),
                });
                changes.push(KeyChange {
                    key: DISPLAY_REFRESH_POLICY,
                    old: old_refresh,
                    new: Some(pair.refresh),
                });
                if expert == ExpertKeys::Keep {
                    return Ok(changes);
                }
                changes.extend(PICTURE_MODE_COVERS.into_iter().filter_map(|covered| {
                    Some(KeyChange {
                        key: covered,
                        old: Some(self.unset(level, covered)?),
                        new: None,
                    })
                }));
            }
            None => {
                for (key, old) in [
                    (DISPLAY_STYLE, old_style),
                    (DISPLAY_REFRESH_POLICY, old_refresh),
                ] {
                    if old.is_some() {
                        self.unset(level, key);
                        changes.push(KeyChange {
                            key,
                            old,
                            new: None,
                        });
                    }
                }
            }
        }
        Ok(changes)
    }

    /// Whether `level` holds a key of the picture pair: a value of the
    /// picture mode or of the refresh choice.
    pub fn holds_picture(&self, level: Level) -> bool {
        self.get(level, DISPLAY_STYLE).is_some()
            || self.get(level, DISPLAY_REFRESH_POLICY).is_some()
    }

    /// Whether `level` holds exactly `pair`.
    pub fn holds_pair(&self, level: Level, pair: &PicturePair) -> bool {
        self.get(level, DISPLAY_STYLE) == Some(&pair.style)
            && self.get(level, DISPLAY_REFRESH_POLICY) == Some(&pair.refresh)
    }

    /// The picture pair that the context resolves to: the pair that the
    /// running game shows.
    pub fn resolved_pair(&self) -> Option<PicturePair> {
        Some(PicturePair {
            style: self.resolved(DISPLAY_STYLE)?.value,
            refresh: self.resolved(DISPLAY_REFRESH_POLICY)?.value,
        })
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

    /// The picture mode, the refresh rate, the TV width, the vertical
    /// games and the panel scale of the context, with the arcade rules for
    /// an arcade board.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn policy(&self) -> Result<PolicySettings, SettingsError> {
        let settings = policy_settings(
            &self.text(DISPLAY_STYLE),
            &self.text(DISPLAY_REFRESH_POLICY),
            None,
            &self.text(DISPLAY_TV_WIDTH),
        )?;
        let settings = with_display_choices(
            settings,
            &self.text(DISPLAY_VERTICAL),
            &self.text(DISPLAY_PANEL_SCALE),
        )?;
        Ok(PolicySettings {
            arcade: self.context.as_ref().is_some_and(|c| c.arcade),
            ..settings
        })
    }

    /// The picture mode, the refresh choice and the TV width of the global
    /// level, which the guard starts from. It has no game, so no arcade
    /// rules.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn global_policy(&self) -> Result<PolicySettings, SettingsError> {
        self.policy_up_to(Level::Global)
    }

    /// The policy settings resolved from the defaults up to `level`
    /// ([`Self::resolved_up_to`]): the values that a write at `level` acts
    /// with. The system and game levels of an arcade board keep its arcade
    /// rules.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn policy_up_to(&self, level: Level) -> Result<PolicySettings, SettingsError> {
        let text = |key| {
            self.resolved_up_to(level, key)
                .map(|r| r.value.to_string())
                .unwrap_or_default()
        };
        let settings = policy_settings(
            &text(DISPLAY_STYLE),
            &text(DISPLAY_REFRESH_POLICY),
            None,
            &text(DISPLAY_TV_WIDTH),
        )?;
        let settings = with_display_choices(
            settings,
            &text(DISPLAY_VERTICAL),
            &text(DISPLAY_PANEL_SCALE),
        )?;
        let arcade = level != Level::Global && self.context.as_ref().is_some_and(|c| c.arcade);
        Ok(PolicySettings { arcade, ..settings })
    }

    /// The scanline settings of the global level, which the Settings panel
    /// writes.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn global_scanlines(&self) -> Result<ScanlineSettings, SettingsError> {
        self.scanlines_up_to(Level::Global)
    }

    /// The scanline settings resolved from the defaults up to `level`.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] when the schema and the policy disagree.
    pub fn scanlines_up_to(&self, level: Level) -> Result<ScanlineSettings, SettingsError> {
        let mode = self
            .resolved_up_to(level, DISPLAY_SCANLINES)
            .map(|r| r.value.to_string())
            .unwrap_or_default();
        let strength = match self
            .resolved_up_to(level, DISPLAY_SCANLINE_STRENGTH)
            .map(|r| r.value)
        {
            Some(Value::Int(v)) => v,
            _ => 0,
        };
        scanline_settings(&mode, strength)
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

    /// The time with no input before the screensaver starts, in
    /// nanoseconds; `None` with the screensaver off.
    pub fn screensaver_timeout_ns(&self) -> Option<u64> {
        let on = self.resolved(DISPLAY_SCREENSAVER).map(|r| r.value);
        if on != Some(Value::Bool(true)) {
            return None;
        }
        let minutes: u64 = self.text(DISPLAY_SCREENSAVER_MINUTES).parse().ok()?;
        Some(minutes.saturating_mul(NANOS_PER_MINUTE))
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

    /// The orientation of the context: the screen orientation of the
    /// global level, and the game orientation of the game, `None` for
    /// Auto. The one place where the settings become an [`Orientation`].
    ///
    /// The layers hold only values of the choices of the schema, and each
    /// choice names a turn in degrees, so every value maps: the test
    /// `every_orientation_choice_is_a_turn` proves it. A value with no turn
    /// would give the screen at 0 and the game at Auto, the orientation of
    /// a display with no setting.
    pub fn orientation(&self) -> Orientation {
        let screen = turn_of(&self.text(DISPLAY_ORIENTATION)).unwrap_or(Rotation::R0);
        Orientation {
            screen,
            game_override: turn_of(&self.text(DISPLAY_ROTATION)),
        }
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

/// The turn of a choice of the orientation keys, in degrees; `None` for
/// `auto` and for a text that names no quarter turn.
fn turn_of(choice: &str) -> Option<Rotation> {
    choice.parse().ok().and_then(Rotation::from_degrees)
}

#[cfg(test)]
mod tests {
    use neptron_settings::Source;

    use super::*;

    fn text(v: &str) -> Value {
        Value::Text(v.to_owned())
    }

    fn pair(style: &str, refresh: &str) -> Option<PicturePair> {
        Some(PicturePair {
            style: text(style),
            refresh: text(refresh),
        })
    }

    #[test]
    fn every_orientation_choice_is_a_turn() {
        let screen: Vec<_> = neptron_settings::SCREEN_ORIENTATIONS
            .iter()
            .map(|c| turn_of(c))
            .collect();
        assert_eq!(screen, Rotation::ALL.map(Some));
        let game: Vec<_> = neptron_settings::GAME_ORIENTATIONS
            .iter()
            .map(|c| turn_of(c))
            .collect();
        assert_eq!(game[0], None, "auto");
        assert_eq!(game[1..], Rotation::ALL.map(Some));
    }

    #[test]
    fn the_orientation_reads_the_screen_and_the_game_override() {
        let context = GameContext {
            system: "snes".into(),
            game: "astro".into(),
            arcade: false,
        };
        let mut c = Config::load(&Storage::new("/nonexistent/config"), Some(context)).0;
        assert_eq!(c.orientation(), Orientation::NORMAL);
        c.set(Level::Global, DISPLAY_ORIENTATION, text("270"))
            .unwrap();
        c.set(Level::Game, DISPLAY_ROTATION, text("90")).unwrap();
        assert_eq!(
            c.orientation(),
            Orientation {
                screen: Rotation::R270,
                game_override: Some(Rotation::R90),
            }
        );
        c.set(Level::Game, DISPLAY_ROTATION, text("auto")).unwrap();
        assert_eq!(c.orientation().game_override, None);
    }

    #[test]
    fn a_pair_write_sets_both_keys_and_removes_the_covered_keys_only_when_it_succeeds() {
        let mut c = Config::load(&Storage::new("/nonexistent/config"), None).0;
        c.set(Level::Global, DISPLAY_VERTICAL, text("fit")).unwrap();
        c.set(Level::Global, DISPLAY_PANEL_SCALE, text("integer"))
            .unwrap();
        c.set(Level::Global, DISPLAY_REFRESH_POLICY, text("force_50"))
            .unwrap();
        // A value the layer refuses changes nothing: no key of the pair and
        // no expert key.
        assert!(
            c.write_pair(Level::Global, pair("speed", "never"), ExpertKeys::Remove)
                .is_err()
        );
        assert!(
            c.write_pair(Level::Global, pair("nine", "exact"), ExpertKeys::Remove)
                .is_err()
        );
        assert_eq!(c.get(Level::Global, DISPLAY_STYLE), None);
        assert_eq!(
            c.get(Level::Global, DISPLAY_REFRESH_POLICY),
            Some(&text("force_50"))
        );
        assert!(c.get(Level::Global, DISPLAY_VERTICAL).is_some());
        let changes = c
            .write_pair(Level::Global, pair("sharp", "exact"), ExpertKeys::Remove)
            .unwrap();
        let keys: Vec<&str> = changes.iter().map(|c| c.key).collect();
        assert_eq!(
            keys,
            [
                DISPLAY_STYLE,
                DISPLAY_REFRESH_POLICY,
                DISPLAY_VERTICAL,
                DISPLAY_PANEL_SCALE
            ]
        );
        assert_eq!(
            changes[1],
            KeyChange {
                key: DISPLAY_REFRESH_POLICY,
                old: Some(text("force_50")),
                new: Some(text("exact")),
            }
        );
        assert!(c.holds_pair(Level::Global, &pair("sharp", "exact").unwrap()));
        assert_eq!(c.get(Level::Global, DISPLAY_VERTICAL), None);
        assert_eq!(c.get(Level::Global, DISPLAY_PANEL_SCALE), None);
    }

    /// A write that is no pick of the picture mode keeps the expert keys
    /// of its level: the menu judged the pair with them.
    #[test]
    fn a_pair_write_that_keeps_the_expert_keys_changes_only_the_pair() {
        let mut c = Config::load(&Storage::new("/nonexistent/config"), None).0;
        c.set(Level::Global, DISPLAY_VERTICAL, text("fit")).unwrap();
        c.set(Level::Global, DISPLAY_PANEL_SCALE, text("integer"))
            .unwrap();
        let changes = c
            .write_pair(Level::Global, pair("auto", "force_60"), ExpertKeys::Keep)
            .unwrap();
        let keys: Vec<&str> = changes.iter().map(|c| c.key).collect();
        assert_eq!(keys, [DISPLAY_STYLE, DISPLAY_REFRESH_POLICY]);
        assert_eq!(c.get(Level::Global, DISPLAY_VERTICAL), Some(&text("fit")));
        assert_eq!(
            c.get(Level::Global, DISPLAY_PANEL_SCALE),
            Some(&text("integer"))
        );
    }

    /// A removal takes both keys and only them: an expert key of the level
    /// stays, and a level with one key (an old or hand-edited file) loses
    /// that key.
    #[test]
    fn a_pair_removal_takes_both_keys_and_nothing_else() {
        let mut c = Config::load(&Storage::new("/nonexistent/config"), None).0;
        c.set(Level::Global, DISPLAY_VERTICAL, text("fit")).unwrap();
        c.write_pair(Level::Global, pair("2x", "force_60"), ExpertKeys::Remove)
            .unwrap();
        c.set(Level::Global, DISPLAY_VERTICAL, text("fit")).unwrap();
        let changes = c
            .write_pair(Level::Global, None, ExpertKeys::Remove)
            .unwrap();
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert!(!c.holds_picture(Level::Global));
        assert_eq!(c.get(Level::Global, DISPLAY_VERTICAL), Some(&text("fit")));
        c.set(Level::Global, DISPLAY_REFRESH_POLICY, text("force_50"))
            .unwrap();
        assert!(c.holds_picture(Level::Global));
        let changes = c
            .write_pair(Level::Global, None, ExpertKeys::Remove)
            .unwrap();
        assert_eq!(
            changes,
            vec![KeyChange {
                key: DISPLAY_REFRESH_POLICY,
                old: Some(text("force_50")),
                new: None,
            }]
        );
        assert!(
            c.write_pair(Level::Global, None, ExpertKeys::Remove)
                .unwrap()
                .is_empty()
        );
    }

    /// Old files and hand-edited files can hold one key of the pair at a
    /// level: the keys resolve one by one, each from the nearest level
    /// that holds it.
    #[test]
    fn a_level_with_one_key_of_the_pair_resolves_each_key_alone() {
        let c = config(
            "[display]\nrefresh_policy = \"force_60\"\n",
            "[display]\nstyle = \"2x\"\n",
            "[display]\nrefresh_policy = \"force_50\"\n",
        );
        assert_eq!(c.resolved_pair(), pair("2x", "force_50"));
        let up_to = |level, key| {
            c.resolved_up_to(level, key)
                .map(|r| r.value.to_string())
                .unwrap_or_default()
        };
        assert_eq!(up_to(Level::System, DISPLAY_STYLE), "2x");
        assert_eq!(up_to(Level::System, DISPLAY_REFRESH_POLICY), "force_60");
        assert_eq!(up_to(Level::Global, DISPLAY_STYLE), "auto");
    }

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
            arcade: false,
        };
        let (config, issues) = Config::load(&Storage::new(&dir), Some(context));
        assert!(issues.is_empty(), "{issues:?}");
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
        config
    }

    #[test]
    fn an_arcade_board_takes_the_arcade_rules_and_the_global_level_does_not() {
        let mut c = config("", "", "");
        assert!(!c.policy().expect("policy").arcade, "a console game");
        if let Some(context) = c.context.as_mut() {
            context.arcade = true;
        }
        assert!(c.policy().expect("policy").arcade);
        assert!(!c.global_policy().expect("policy").arcade);
        let (list, _) = Config::load(&Storage::new(std::env::temp_dir().join("none")), None);
        assert!(!list.policy().expect("policy").arcade, "the game list");
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
    fn the_screensaver_time_is_in_nanoseconds_and_off_is_none() {
        let (c, _) = Config::load(&Storage::new("/nonexistent/config"), None);
        assert_eq!(c.screensaver_timeout_ns(), Some(5 * 60_000_000_000));
        let c = config("[display]\nscreensaver_minutes = \"30\"\n", "", "");
        assert_eq!(c.screensaver_timeout_ns(), Some(30 * 60_000_000_000));
        let c = config(
            "[display]\nscreensaver = false\nscreensaver_minutes = \"1\"\n",
            "",
            "",
        );
        assert_eq!(c.screensaver_timeout_ns(), None);
    }

    #[test]
    fn a_run_with_no_game_reads_the_global_level_only() {
        let (c, _) = Config::load(&Storage::new("/nonexistent/config"), None);
        assert_eq!(c.file(Level::Game), None);
        assert_eq!(c.file(Level::Global), Some(FileTarget::Global));
        assert_eq!(c.monitor(), "auto");
    }
}
