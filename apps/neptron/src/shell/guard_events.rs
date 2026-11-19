//! The guard's events: the confirm dialog, the settings writes that wait
//! for a confirmation, the notices, the record and the blind recovery.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::PadRoster;
use neptron_settings::{
    DISPLAY_MONITOR, DISPLAY_REFRESH_POLICY, DISPLAY_STYLE, DISPLAY_TV, Level, Value,
};
use neptron_ui::snapshot::NoticeKind;
use neptron_video::{
    DisplayConfig, Guard, GuardEvent, GuardRecord, GuardStep, RevertReason, SafeReason, VideoEngine,
};

use super::{Shell, Stage};
use crate::display::{tv_setting, tv_standard};
use crate::outbox::Job;
use crate::record;

/// The length of the audio cue of the blind recovery, in milliseconds.
const CUE_MS: u32 = 200;
/// The pitch of the cue, in hertz.
const CUE_HZ: u32 = 880;
/// The amplitude of the cue: a quarter of full scale.
const CUE_LEVEL: i16 = 8_192;

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// Calls `f` with the guard and the engine of the stage. `None` after
    /// the game ended.
    pub(super) fn with_engine<R>(
        &mut self,
        f: impl FnOnce(&mut Guard, &mut VideoEngine<D>) -> R,
    ) -> Option<R> {
        let engine = match &mut self.stage {
            Stage::List(list) => &mut list.engine,
            Stage::Game(game) => game.session.video_mut(),
            Stage::Ended => return None,
        };
        Some(f(&mut self.guard, engine))
    }

    /// Sends `record` to the writer thread, with its last known good as
    /// the entry of this place and the entries of the other places as
    /// they are.
    pub(super) fn save_record(&mut self, record: GuardRecord) {
        self.places = record::merge(&self.places, &self.here, record.last_known_good.as_ref());
        self.outbox.send(Job::Record {
            record,
            places: self.places.clone(),
        });
    }

    /// Acts on one step of the guard.
    pub(super) fn on_guard(&mut self, step: GuardStep, now_ns: u64) {
        if let Some(record) = step.save {
            self.save_record(record);
        }
        let Some(event) = step.event else {
            return;
        };
        self.log(format!("guard: {event:?}"));
        match event {
            GuardEvent::PromptOpened(_) => {
                self.nav.hold_back();
                self.refresh_view(now_ns);
            }
            GuardEvent::Confirmed => {
                let changes = std::mem::take(&mut self.pending);
                for change in &changes {
                    self.write_setting(change.level, change.key, change.new.clone());
                }
                self.snap.confirm = None;
            }
            GuardEvent::Reverted {
                reason,
                rewrite,
                changed,
            } => {
                self.roll_back();
                if let Some(config) = rewrite {
                    self.rewrite_global(&config);
                }
                self.snap.confirm = None;
                if let Some(text) = reverted_text(reason).filter(|_| changed) {
                    self.notice(NoticeKind::Info, text.to_owned());
                }
            }
            GuardEvent::SafeMode(reason) => {
                self.roll_back();
                self.snap.confirm = None;
                if reason == SafeReason::Recovery {
                    self.play_cue();
                }
                self.notice(NoticeKind::Warning, safe_text(reason).to_owned());
            }
            GuardEvent::GoodBoot => {}
        }
        self.refresh_profile();
        self.reload_page();
    }

    /// The blind recovery hotkey: the safe default, in every screen and
    /// state, also with a dialog open.
    pub(super) fn recover(&mut self, now_ns: u64) {
        if let Some(step) = self.with_engine(|guard, engine| guard.recover(engine)) {
            self.on_guard(step, now_ns);
        }
    }

    /// Puts the safe default on the engine for this run, with `notice`.
    /// The guard records safe mode, so the next start that can write its
    /// record starts in it too, until the user confirms a configuration.
    pub(super) fn safe_for_this_run(&mut self, notice: String) {
        if let Some(step) = self.with_engine(|guard, engine| guard.recover(engine)) {
            if let Some(record) = step.save {
                self.save_record(record);
            }
            self.roll_back();
            self.snap.confirm = None;
            self.notice(NoticeKind::Warning, notice);
        }
    }

    /// Takes back every display change that waited for a confirmation, the
    /// newest first.
    fn roll_back(&mut self) {
        while let Some(change) = self.pending.pop() {
            self.config.restore(change.level, change.key, change.old);
        }
    }

    /// Writes the configuration of a reverted boot prompt to the global
    /// settings, so the next boot does not show the change again. It is the
    /// revert target the guard gives, never the active configuration.
    ///
    /// A TV profile is written as `display.tv`, and `display.monitor`
    /// stays: the monitor setting has no effect on a TV, and the next
    /// output may be no TV. A value of `display.tv` that already gives the
    /// standard, such as `auto` for NTSC, stays too.
    pub(super) fn rewrite_global(&mut self, config: &DisplayConfig) {
        let tv = tv_setting(&config.monitor);
        if let Some(key) = tv {
            let current = tv_standard(&self.config.tv()).map(|s| s.key());
            if current != Some(key) {
                let value = Value::Text(key.to_owned());
                if self
                    .config
                    .set(Level::Global, DISPLAY_TV, value.clone())
                    .is_ok()
                {
                    self.write_setting(Level::Global, DISPLAY_TV, Some(value));
                }
            }
        }
        let values = [
            (
                DISPLAY_MONITOR,
                self.detected.monitor_setting(&config.monitor),
            ),
            (DISPLAY_STYLE, config.settings.style.key().to_owned()),
            (
                DISPLAY_REFRESH_POLICY,
                config.settings.refresh.key().to_owned(),
            ),
        ];
        for (key, value) in values {
            if key == DISPLAY_MONITOR && tv.is_some() {
                continue;
            }
            let value = Value::Text(value);
            if self.config.set(Level::Global, key, value.clone()).is_ok() {
                self.write_setting(Level::Global, key, Some(value));
            }
        }
    }

    /// Sends one setting to its file.
    pub(super) fn write_setting(&mut self, level: Level, key: &'static str, value: Option<Value>) {
        self.write_settings(level, vec![(key, value)]);
    }

    /// Writes `changes` to the file of `level` in one write.
    pub(super) fn write_settings(
        &mut self,
        level: Level,
        changes: Vec<(&'static str, Option<Value>)>,
    ) {
        if let Some(target) = self.config.file(level) {
            self.outbox.send(Job::Settings { target, changes });
        }
    }

    /// The audio cue of the blind recovery. In the game list the app owns
    /// the sink; in a game the session writes the cue to its sink.
    fn play_cue(&mut self) {
        let rate = match &self.stage {
            Stage::List(list) => list.audio.rate_hz(),
            Stage::Game(game) => match game.session.audio_sink() {
                Some(sink) => sink.rate_hz(),
                None => return,
            },
            Stage::Ended => return,
        };
        let frames = rate * CUE_MS / 1_000;
        let half_period = (rate / CUE_HZ / 2).max(1);
        let cue: Vec<i16> = (0..frames)
            .flat_map(|n| {
                let level = if (n / half_period).is_multiple_of(2) {
                    CUE_LEVEL
                } else {
                    -CUE_LEVEL
                };
                [level, level]
            })
            .collect();
        // The sink takes what fits; a cue cut short by a full queue still
        // sounds.
        let _ = match &mut self.stage {
            Stage::List(list) => list.audio.write(&cue),
            Stage::Game(game) => game.session.write_cue(&cue),
            Stage::Ended => 0,
        };
    }
}

/// The notice of a revert. A superseded change has none: the setting that
/// replaced it is what the user asked for.
fn reverted_text(reason: RevertReason) -> Option<&'static str> {
    match reason {
        RevertReason::Timeout => Some("Display change undone: not kept in time."),
        RevertReason::Declined => Some("Display change undone."),
        RevertReason::NoPicture => Some("Display change undone: no picture."),
        RevertReason::Superseded => None,
        RevertReason::ClockWentBack => Some("Display change undone: the clock went back."),
    }
}

/// The notice of safe mode for `reason`.
pub(super) fn safe_text(reason: SafeReason) -> &'static str {
    match reason {
        SafeReason::FailedBoots => {
            "Safe mode: the last starts gave no picture. Keep a picture in Settings, Display to leave it."
        }
        SafeReason::Recovery => {
            "Safe mode: the picture was reset. Keep a picture in Settings, Display to leave it."
        }
        SafeReason::NoValidMode => "Safe mode: the display settings gave no picture.",
        SafeReason::NothingConfirmed => "Safe mode: no display setting was kept.",
    }
}
