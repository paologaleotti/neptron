//! The intents of the UI, and the settings changes they ask for.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_settings::{
    DISPLAY_MONITOR, DISPLAY_OUTPUT, INPUT_MENU_HOLD_START_SECONDS, Level, Safety, Value,
};
use neptron_ui::Intent;
use neptron_ui::snapshot::NoticeKind;
use neptron_video::{ConfirmError, DisplayConfig};

use super::{Outcome, PendingChange, Shell, Stage};
use crate::pages::{self, Names};
use crate::relaunch::NextRun;

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Shell<D, S, I, C> {
    /// Carries out the intents of the last UI frame, in order.
    pub(super) fn carry_out(&mut self, now_ns: u64) {
        let mut intents = std::mem::take(&mut self.ui.intents);
        for intent in intents.drain(..) {
            self.intent(intent, now_ns);
        }
        self.ui.intents = intents;
    }

    fn intent(&mut self, intent: Intent, now_ns: u64) {
        match intent {
            Intent::LoadGames {
                source,
                first,
                count,
            } => {
                if !self.snap.games.covers(source, first, count) {
                    self.snap.games = self.library.window(source, first, count);
                }
            }
            Intent::LoadSettings { page } => {
                if !self.snap.settings.is_for(page) {
                    self.load_page(page);
                }
            }
            Intent::Launch { game } => self.launch(game),
            // Favourites are not kept in the MVP; the list stays empty.
            Intent::ToggleFavourite { .. } => {}
            Intent::SetValue { key, level, value } => {
                let names = Names {
                    presets: &self.presets,
                    detected: &self.detected,
                    styles: &[],
                };
                let found = pages::entry(&self.config, key)
                    .and_then(|e| Some((e.key, pages::value_of(e, value, names, &self.config)?)));
                if let (Some((key, value)), Some(level)) = (found, pages::settings_level(level)) {
                    self.change_setting(key, level, Some(value), now_ns);
                }
            }
            Intent::ResetValue { key, level } => {
                let key = pages::entry(&self.config, key).map(|e| e.key);
                if let (Some(key), Some(level)) = (key, pages::settings_level(level)) {
                    self.change_setting(key, level, None, now_ns);
                }
            }
            // The app adds no action rows of its own.
            Intent::Action(_) => {}
            Intent::SelectProfile(id) => {
                let profile = self.presets.profiles().get(id.0 as usize);
                if let Some(profile) = profile.map(|p| p.id.clone()) {
                    self.change_setting(
                        DISPLAY_MONITOR,
                        Level::Global,
                        Some(Value::Text(profile)),
                        now_ns,
                    );
                }
            }
            Intent::KeepMode => self.keep(now_ns),
            Intent::RevertMode => {
                let prompt = self.guard.pending();
                let step = prompt
                    .and_then(|p| self.with_engine(|guard, engine| guard.decline(p.id, engine)));
                if let Some(Ok(step)) = step {
                    self.on_guard(step, now_ns);
                }
            }
            Intent::Resume => {
                if let Stage::Game(game) = &mut self.stage {
                    game.session.resume();
                }
            }
            Intent::QuitGame => self.end_game(None),
            Intent::DismissNotice(id) => self.snap.notices.retain(|n| n.id != id),
        }
    }

    /// Starts a game: the run ends with a relaunch into it. A launch while
    /// a game runs is ignored.
    pub(super) fn launch(&mut self, game: neptron_ui::snapshot::GameId) {
        if !matches!(self.stage, Stage::List(_)) || self.outcome.is_some() {
            return;
        }
        if let Some(reference) = self.library.reference(game) {
            self.outcome = Some(Outcome::Relaunch(NextRun {
                game: Some(reference),
                notice: None,
            }));
        }
    }

    /// Ends the game and the run, with a relaunch to the game list.
    pub(super) fn end_game(&mut self, notice: Option<String>) {
        let Stage::Game(game) = std::mem::replace(&mut self.stage, Stage::Ended) else {
            return;
        };
        let report = game.session.quit();
        let notice = notice.or_else(|| {
            report
                .save_stats
                .last_error
                .map(|e| format!("The game was not saved: {e}."))
        });
        self.outcome = Some(Outcome::Relaunch(NextRun { game: None, notice }));
    }

    /// The answer Keep of the confirm dialog. A confirmation the guard
    /// refuses keeps the dialog; one after the deadline is dropped, and the
    /// deadline reverts.
    fn keep(&mut self, now_ns: u64) {
        let Some(prompt) = self.guard.pending() else {
            return;
        };
        let result = match &self.stage {
            Stage::List(list) => self.guard.confirm(prompt.id, now_ns, &list.engine),
            Stage::Game(game) => self.guard.confirm(prompt.id, now_ns, game.session.video()),
            Stage::Ended => return,
        };
        match result {
            Ok(step) => self.on_guard(step, now_ns),
            Err(ConfirmError::Expired { .. }) => {}
            Err(e) => self.log(format!("keep refused: {e}")),
        }
    }

    /// Sets (`Some`) or unsets (`None`) a setting at a level. A setting with
    /// no effect on the signal is written at once and applied live. A
    /// display setting goes to the guard: it is on screen at once and
    /// reaches its file only when the player keeps it.
    pub(super) fn change_setting(
        &mut self,
        key: &'static str,
        level: Level,
        value: Option<Value>,
        now_ns: u64,
    ) {
        let Some(entry) = self.config.schema().get(key).copied() else {
            return;
        };
        if key == DISPLAY_OUTPUT || self.config.file(level).is_none() {
            return;
        }
        let old = self.config.get(level, key).cloned();
        let applied = match value.clone() {
            Some(v) => self.config.set(level, key, v).map(|_| ()),
            None => {
                self.config.unset(level, key);
                Ok(())
            }
        };
        if let Err(e) = applied {
            self.notice(
                NoticeKind::Error,
                format!("The setting was not changed: {e}."),
            );
            return;
        }
        match entry.safety {
            Safety::None => {
                self.write_setting(level, key, value);
                self.apply_live(key);
            }
            Safety::Validated | Safety::ConfirmOrRevert => {
                self.pending.push(PendingChange {
                    level,
                    key,
                    old,
                    new: value,
                });
                self.propose(now_ns);
            }
        }
        self.reload_page();
    }

    /// Gives the display configuration of the settings in memory to the
    /// guard. The same configuration as on the engine needs no prompt: the
    /// waiting changes are written at once.
    fn propose(&mut self, now_ns: u64) {
        let Some(config) = self.display_config() else {
            self.notice(
                NoticeKind::Error,
                "The display setting does not map to a picture.".to_owned(),
            );
            return;
        };
        if self.guard.pending().is_none() && &config == self.guard.active() {
            for change in std::mem::take(&mut self.pending) {
                self.write_setting(change.level, change.key, change.new);
            }
            return;
        }
        if let Some(step) = self.with_engine(|guard, engine| guard.propose(config, now_ns, engine))
        {
            self.on_guard(step, now_ns);
        }
    }

    /// The display configuration that the settings in memory ask for: the
    /// global monitor, and the policy of the game, or the global policy in
    /// the game list.
    fn display_config(&self) -> Option<DisplayConfig> {
        let monitor = self
            .detected
            .monitor(&self.config.monitor(), &self.presets, &self.safe)
            .unwrap_or_else(|_| self.safe.monitor.clone());
        let settings = if self.in_game() {
            self.config.policy()
        } else {
            self.config.global_policy()
        };
        Some(DisplayConfig {
            monitor,
            settings: settings.ok()?,
        })
    }

    /// Applies a setting that does not change the signal.
    fn apply_live(&mut self, key: &str) {
        let scanlines = self.config.scanlines();
        let hold_ns = self.config.start_hold_ns();
        match &mut self.stage {
            Stage::List(list) if key == INPUT_MENU_HOLD_START_SECONDS => {
                list.hotkeys.set_start_hold_ns(hold_ns);
            }
            Stage::Game(game) if key == INPUT_MENU_HOLD_START_SECONDS => {
                game.session.set_start_hold_ns(hold_ns);
            }
            Stage::List(list) => {
                if let Ok(scanlines) = scanlines {
                    list.engine.set_scanlines(scanlines);
                }
            }
            Stage::Game(game) => {
                if let Ok(scanlines) = scanlines {
                    game.session.video_mut().set_scanlines(scanlines);
                }
            }
            Stage::Ended => {}
        }
    }
}
