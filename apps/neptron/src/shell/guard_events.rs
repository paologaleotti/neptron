//! The guard's events: the confirm dialog, the settings writes that wait
//! for a confirmation, the notices, the record and the blind recovery.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_settings::{DISPLAY_MONITOR, DISPLAY_REFRESH_POLICY, DISPLAY_STYLE, Level, Value};
use neptron_types::{Control, InputEvent};
use neptron_ui::snapshot::NoticeKind;
use neptron_video::{
    DisplayConfig, Guard, GuardEvent, GuardStep, Phase, RevertReason, SafeReason, VideoEngine,
};

use super::{Shell, Stage};
use crate::display::{AUTO, SAFE_START_PROFILE};
use crate::outbox::Job;

/// The length of the audio cue of the blind recovery, in milliseconds.
const CUE_MS: u32 = 200;
/// The pitch of the cue, in hertz.
const CUE_HZ: u32 = 880;
/// The amplitude of the cue: a quarter of full scale.
const CUE_LEVEL: i16 = 8_192;

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Shell<D, S, I, C> {
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

    /// Acts on one step of the guard.
    pub(super) fn on_guard(&mut self, step: GuardStep, now_ns: u64) {
        if matches!(step.event, Some(GuardEvent::Confirmed)) {
            self.record_output = Some(self.runtime.output.clone());
        }
        if let Some(record) = step.save {
            self.outbox.send(Job::Record {
                record,
                output: self.record_output.clone(),
            });
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
            GuardEvent::Reverted { reason, rewrite } => {
                self.roll_back();
                if let Some(config) = rewrite {
                    self.rewrite_global(&config);
                }
                self.snap.confirm = None;
                self.notice(NoticeKind::Info, reverted_text(reason).to_owned());
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

    /// The way back to a flat panel that the 15 kHz safe start leaves
    /// dark. It proposes `display.monitor = auto` (the panel) under a new
    /// prompt with a full window when all hold:
    ///
    /// - `pressed`: a fresh press of a button ([`any_press`]);
    /// - the guard is in safe mode, not after a blind recovery, which
    ///   always ends in the safe start;
    /// - nothing is confirmed on this output;
    /// - `display.monitor` is `auto` and gives a panel: an explicit preset
    ///   is never overridden;
    /// - no offer came yet in this run, and the run has had fewer than
    ///   [`PANEL_WINDOWS_PER_RUN`] panel windows (the boot prompt counts).
    ///
    /// Returns whether it did, so that press does not also move the menu.
    /// After the one offer every press goes to the menu.
    pub(super) fn offer_panel(&mut self, pressed: bool, now_ns: u64) -> bool {
        let safe = matches!(self.guard.phase(), Phase::Safe(r) if r != SafeReason::Recovery);
        let lost = safe
            && self.guard.record().last_known_good.is_none()
            && self.detected.auto_is_panel()
            && self.config.monitor() == AUTO;
        let left = !self.panel_offered && self.panel_windows < PANEL_WINDOWS_PER_RUN;
        if !pressed || !lost || !left {
            return false;
        }
        self.panel_offered = true;
        self.panel_windows = self.panel_windows.saturating_add(1);
        self.change_setting(
            DISPLAY_MONITOR,
            Level::Global,
            Some(Value::Text(AUTO.to_owned())),
            now_ns,
        );
        self.guard.pending().is_some()
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
                self.outbox.send(Job::Record {
                    record,
                    output: self.record_output.clone(),
                });
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
    /// When `auto` gives a flat panel and the revert target is the 15 kHz
    /// safe start, `auto` stays: the next boot asks for the panel again,
    /// for one confirm window at most, and the failed boots still lead to
    /// safe mode.
    fn rewrite_global(&mut self, config: &DisplayConfig) {
        let panel_left = self.detected.auto_is_panel()
            && self.config.monitor() == AUTO
            && self.detected.monitor_setting(&config.monitor) == SAFE_START_PROFILE;
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
            if key == DISPLAY_MONITOR && panel_left {
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
        if let Some(target) = self.config.file(level) {
            self.outbox.send(Job::Settings {
                target,
                changes: vec![(key, value)],
            });
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

fn reverted_text(reason: RevertReason) -> &'static str {
    match reason {
        RevertReason::Timeout => "The display change went back: it was not kept in time.",
        RevertReason::Declined => "The display change went back.",
        RevertReason::NoPicture => "The display change went back: it gave no picture.",
        RevertReason::Superseded => "The display change went back: a new setting replaced it.",
        RevertReason::ClockWentBack => "The display change went back: the clock went back.",
    }
}

/// The panel windows a run can open at most: the boot prompt of a panel
/// and one offer after it.
pub const PANEL_WINDOWS_PER_RUN: u8 = 2;

/// The value of a fresh press; a key repeat is 2.
const PRESS_VALUE: i16 = 1;

/// Whether `events` hold a fresh press of a button: not a release, not a
/// key repeat, not a stick move.
pub(super) fn any_press(events: &[InputEvent]) -> bool {
    events.iter().any(|e| {
        e.value == PRESS_VALUE
            && !matches!(
                e.control,
                Control::LeftX | Control::LeftY | Control::RightX | Control::RightY
            )
    })
}

fn safe_text(reason: SafeReason) -> &'static str {
    match reason {
        SafeReason::FailedBoots => "Safe mode: the last starts gave no confirmed picture.",
        SafeReason::Recovery => {
            "Safe mode: the picture was reset. Keep a display setting to leave it."
        }
        SafeReason::NoValidMode => "Safe mode: the display settings gave no picture.",
        SafeReason::NothingConfirmed => "Safe mode: no display setting was kept.",
    }
}
