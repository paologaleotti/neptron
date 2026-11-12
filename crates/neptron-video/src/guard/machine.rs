use neptron_backend::Display;

use super::{
    CONFIRM_WINDOW_NS, DisplayConfig, GOOD_BOOT_PICTURE_NS, GuardRecord, Health,
    NO_PICTURE_LIMIT_NS, PENDING_LIMIT_NS, SAFE_MODE_FAILED_BOOTS,
};
use crate::{PolicySettings, VideoEngine};

/// Confirm-or-revert, last known good, safe mode and blind recovery.
///
/// The app makes one guard per boot with [`Guard::boot`], calls
/// [`Guard::tick`] after each present, and passes the user's display
/// changes, the answers of the confirm dialog and the blind recovery
/// hotkey. Every method that changes the display gives the new inputs to
/// the engine; none calls the display. Each returns a [`GuardStep`]: an
/// event for the app, and a record to write when it changed.
#[derive(Debug, Clone)]
pub struct Guard {
    state: State,
    /// The configuration the engine has.
    active: DisplayConfig,
    /// The safe default for the detected output.
    safe: DisplayConfig,
    record: GuardRecord,
    /// The boot reached a confirmed picture.
    boot_good: bool,
    last_prompt: u32,
    /// The latest time any call gave. The guard never uses an earlier one.
    latest_ns: u64,
    /// Since when the engine shows a picture, with no break.
    picture_since_ns: Option<u64>,
    /// Since when the engine shows no valid picture, with no break.
    no_picture_since_ns: Option<u64>,
}

/// Where the guard is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The configuration on the engine is the confirmed one.
    Confirmed,
    /// A change waits for the user's confirmation.
    Pending(Prompt),
    /// The safe default is on the engine.
    Safe(SafeReason),
}

/// A display change that waits for the user's confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prompt {
    /// The confirmation. Each change gets a new id, also one that replaces
    /// a pending change.
    pub id: PromptId,
    /// The change reverts at this time, in nanoseconds.
    pub deadline_ns: u64,
}

/// The number of a confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PromptId(pub u32);

/// Why the safe default is on the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafeReason {
    /// [`SAFE_MODE_FAILED_BOOTS`] boots in a row reached no confirmed
    /// picture.
    FailedBoots,
    /// The user held the blind recovery combination, in this boot or in an
    /// earlier one that confirmed nothing since.
    Recovery,
    /// The confirmed configuration gave no valid picture for
    /// [`NO_PICTURE_LIMIT_NS`].
    NoValidMode,
    /// Nothing was ever confirmed, and the first change reverted.
    NothingConfirmed,
}

/// Why a pending change went back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RevertReason {
    /// [`CONFIRM_WINDOW_NS`] passed with no confirmation.
    Timeout,
    /// The user chose to revert.
    Declined,
    /// The change gave no valid picture for [`NO_PICTURE_LIMIT_NS`].
    NoPicture,
    /// A settings change that needs no confirmation came while the change
    /// was pending, or the user chose the confirmed configuration again.
    Superseded,
    /// The time of a call was earlier than the time of a call before it,
    /// so the guard cannot measure the confirm window any more.
    ClockWentBack,
}

/// What the app must act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardEvent {
    /// Show the confirm dialog for this prompt. Stop every key repeat, and
    /// send no action for a key that is held now.
    PromptOpened(Prompt),
    /// The user kept the change. Write the changed settings now: the
    /// settings files hold a display change only once it is confirmed.
    Confirmed,
    /// The pending change went back. Close the dialog.
    Reverted {
        /// Why.
        reason: RevertReason,
        /// The change came from the settings files at boot, and they still
        /// hold it: write this configuration, the revert target, to the
        /// global settings, or every boot shows the change again. It is
        /// captured at the revert, so a later `follow_settings` does not
        /// change it. `None`: write no setting.
        rewrite: Option<DisplayConfig>,
    },
    /// The safe default is on the engine. Close any dialog. For
    /// [`SafeReason::Recovery`], play the audio cue.
    SafeMode(SafeReason),
    /// The boot reached a confirmed picture.
    GoodBoot,
}

/// What one call of the guard asks of the app.
#[must_use = "the app must act on the event and write the record"]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuardStep {
    /// What happened, if anything.
    pub event: Option<GuardEvent>,
    /// The record changed: write it (temp file, fsync, rename) before the
    /// next boot can start. Not on the frame thread.
    pub save: Option<GuardRecord>,
}

/// Why the guard did not take a confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfirmError {
    /// No change waits for a confirmation.
    #[error("no change waits for a confirmation (asked for {asked:?})")]
    NoPrompt {
        /// The confirmation the app sent.
        asked: PromptId,
    },
    /// The confirmation is for another change.
    #[error("confirmation {asked:?} is not the open one, {open:?}")]
    WrongPrompt {
        /// The confirmation the app sent.
        asked: PromptId,
        /// The confirmation that waits.
        open: PromptId,
    },
    /// The confirmation came at or after the deadline. The next tick
    /// reverts.
    #[error("confirmation at {now_ns} ns, after the deadline of {deadline_ns} ns")]
    Expired {
        /// The deadline of the change.
        deadline_ns: u64,
        /// When the confirmation came.
        now_ns: u64,
    },
    /// The engine shows no valid picture now, so the user cannot see what
    /// they keep.
    #[error("no valid picture is on the display: {health:?}")]
    NoPicture {
        /// What the engine shows.
        health: Health,
    },
    /// No tick has seen a valid picture of the change since it was given
    /// to the engine: the picture on the display can still be the old
    /// mode.
    #[error("no frame has shown the change yet")]
    NotShown,
}

#[derive(Debug, Clone)]
enum State {
    Confirmed,
    Pending {
        prompt: Prompt,
        fallback: Fallback,
        /// When the first of the changes with no confirmation came.
        since_ns: u64,
        /// The first of them came from the settings files at boot.
        from_settings: bool,
    },
    Safe(SafeReason),
}

/// Changes with no confirmation, one after the other.
#[derive(Debug, Clone, Copy)]
struct Chain {
    since_ns: u64,
    from_settings: bool,
}

/// What a revert restores.
#[derive(Debug, Clone)]
enum Fallback {
    Confirmed(DisplayConfig),
    Safe(SafeReason),
}

impl Guard {
    /// Starts the guard at boot, before the first frame, and gives the
    /// chosen configuration to `engine`.
    ///
    /// `record` is what the last boot saved. `wanted` is the configuration
    /// of the user settings. `safe` is the safe default for the detected
    /// output. The boot counts as failed until it reaches a confirmed
    /// picture, so the returned step always saves the record with one more
    /// failed boot; the app writes it before it sets the first mode.
    ///
    /// - [`SAFE_MODE_FAILED_BOOTS`] failed boots before this one, or a
    ///   record in safe mode: the safe default.
    /// - `wanted` has the last known good monitor: `wanted`, confirmed.
    /// - Otherwise `wanted` waits for a confirmation, as a user change
    ///   does. It reverts to the last known good monitor, or to the safe
    ///   default when there is none.
    pub fn boot<D: Display>(
        record: GuardRecord,
        wanted: DisplayConfig,
        safe: DisplayConfig,
        now_ns: u64,
        engine: &mut VideoEngine<D>,
    ) -> (Self, GuardStep) {
        let failed_before = record.failed_boots;
        let mut guard = Self {
            state: State::Confirmed,
            active: wanted.clone(),
            safe,
            record,
            boot_good: false,
            last_prompt: 0,
            latest_ns: now_ns,
            picture_since_ns: None,
            no_picture_since_ns: None,
        };
        guard.record.failed_boots = failed_before.saturating_add(1);
        let event = if failed_before >= SAFE_MODE_FAILED_BOOTS {
            Some(guard.enter_safe(SafeReason::FailedBoots, engine))
        } else if guard.record.safe_mode {
            Some(guard.enter_safe(SafeReason::Recovery, engine))
        } else if guard.record.last_known_good.as_ref() == Some(&wanted.monitor) {
            guard.apply(engine);
            None
        } else {
            let fallback = match guard.record.last_known_good.clone() {
                Some(monitor) => Fallback::Confirmed(DisplayConfig {
                    monitor,
                    settings: wanted.settings,
                }),
                None => Fallback::Safe(SafeReason::NothingConfirmed),
            };
            let chain = Chain {
                since_ns: now_ns,
                from_settings: true,
            };
            Some(guard.open_prompt(wanted, fallback, chain, now_ns, engine))
        };
        let save = Some(guard.record.clone());
        (guard, GuardStep { event, save })
    }

    /// Where the guard is.
    pub fn phase(&self) -> Phase {
        match &self.state {
            State::Confirmed => Phase::Confirmed,
            State::Pending { prompt, .. } => Phase::Pending(*prompt),
            State::Safe(reason) => Phase::Safe(*reason),
        }
    }

    /// The change that waits for a confirmation.
    pub fn pending(&self) -> Option<Prompt> {
        match &self.state {
            State::Pending { prompt, .. } => Some(*prompt),
            _ => None,
        }
    }

    /// The configuration on the engine.
    pub fn active(&self) -> &DisplayConfig {
        &self.active
    }

    /// The safe default for the detected output.
    pub fn safe(&self) -> &DisplayConfig {
        &self.safe
    }

    /// The record as it is now.
    pub fn record(&self) -> &GuardRecord {
        &self.record
    }

    /// The boot reached a confirmed picture.
    pub fn boot_good(&self) -> bool {
        self.boot_good
    }

    /// A display change that the user asks for: a monitor profile, a
    /// picture style, a refresh policy. It applies at once and reverts at
    /// `now_ns` plus [`CONFIRM_WINDOW_NS`] unless the user confirms it.
    ///
    /// A change while another one is pending replaces it with a new prompt
    /// and a new deadline, but no later than [`PENDING_LIMIT_NS`] after the
    /// first change with no confirmation; a revert still restores the
    /// configuration that was confirmed before both. The confirmed configuration itself ends
    /// the pending change with no prompt.
    pub fn propose<D: Display>(
        &mut self,
        config: DisplayConfig,
        now_ns: u64,
        engine: &mut VideoEngine<D>,
    ) -> GuardStep {
        let (now_ns, _) = self.clock(now_ns);
        let new_chain = Chain {
            since_ns: now_ns,
            from_settings: false,
        };
        let (fallback, chain) = match &self.state {
            State::Confirmed => (Fallback::Confirmed(self.active.clone()), new_chain),
            State::Pending {
                fallback,
                since_ns,
                from_settings,
                ..
            } => (
                fallback.clone(),
                Chain {
                    since_ns: *since_ns,
                    from_settings: *from_settings,
                },
            ),
            State::Safe(reason) => (Fallback::Safe(*reason), new_chain),
        };
        if let Fallback::Confirmed(confirmed) = &fallback
            && *confirmed == config
        {
            if matches!(self.state, State::Confirmed) {
                return GuardStep::default();
            }
            return self.revert(RevertReason::Superseded, engine);
        }
        GuardStep::event(self.open_prompt(config, fallback, chain, now_ns, engine))
    }

    /// The user keeps the pending change `id`. Its monitor becomes the last
    /// known good, the boot counts as good, and safe mode ends.
    ///
    /// # Errors
    ///
    /// [`ConfirmError`] when no change waits, `id` is not the open one,
    /// `now_ns` is at or after the deadline, the engine shows no valid
    /// picture, or no [`Guard::tick`] has seen a valid picture of the
    /// change since it was given to the engine. The guard does not change.
    pub fn confirm<D: Display>(
        &mut self,
        id: PromptId,
        now_ns: u64,
        engine: &VideoEngine<D>,
    ) -> Result<GuardStep, ConfirmError> {
        let prompt = self.open_prompt_id(id)?;
        let (now_ns, _) = self.clock(now_ns);
        if now_ns >= prompt.deadline_ns {
            return Err(ConfirmError::Expired {
                deadline_ns: prompt.deadline_ns,
                now_ns,
            });
        }
        let health = Health::of(engine);
        if !health.picture {
            return Err(ConfirmError::NoPicture { health });
        }
        if self.picture_since_ns.is_none() {
            return Err(ConfirmError::NotShown);
        }
        self.state = State::Confirmed;
        self.boot_good = true;
        self.record = GuardRecord {
            last_known_good: Some(self.active.monitor.clone()),
            failed_boots: 0,
            safe_mode: false,
        };
        Ok(GuardStep {
            event: Some(GuardEvent::Confirmed),
            save: Some(self.record.clone()),
        })
    }

    /// The user reverts the pending change `id` at once.
    ///
    /// # Errors
    ///
    /// [`ConfirmError::NoPrompt`] or [`ConfirmError::WrongPrompt`]. The
    /// guard does not change.
    pub fn decline<D: Display>(
        &mut self,
        id: PromptId,
        engine: &mut VideoEngine<D>,
    ) -> Result<GuardStep, ConfirmError> {
        self.open_prompt_id(id)?;
        Ok(self.revert(RevertReason::Declined, engine))
    }

    /// A settings change that needs no confirmation, such as the resolved
    /// settings of a game at launch. A pending change reverts first. In
    /// safe mode the safe default's settings stay.
    pub fn follow_settings<D: Display>(
        &mut self,
        settings: PolicySettings,
        engine: &mut VideoEngine<D>,
    ) -> GuardStep {
        let step = if matches!(self.state, State::Pending { .. }) {
            self.revert(RevertReason::Superseded, engine)
        } else {
            GuardStep::default()
        };
        if matches!(self.state, State::Confirmed) && self.active.settings != settings {
            self.active.settings = settings;
            engine.set_settings(settings);
            self.picture_since_ns = None;
        }
        step
    }

    /// The blind recovery: from any state, the safe default goes to the
    /// engine, a pending change ends with no revert, and the next boots
    /// start in safe mode until the user confirms a configuration.
    pub fn recover<D: Display>(&mut self, engine: &mut VideoEngine<D>) -> GuardStep {
        let before = self.record.clone();
        let event = self.enter_safe(SafeReason::Recovery, engine);
        self.finish(event, &before)
    }

    /// Takes the state of the engine after the present of a frame, at
    /// `now_ns`. Call it once per frame, also when the present failed.
    ///
    /// - A pending change reverts at its deadline, when it gives no valid
    ///   picture for [`NO_PICTURE_LIMIT_NS`], or when `now_ns` is earlier
    ///   than a time the guard has seen.
    /// - The confirmed configuration that gives no valid picture for
    ///   [`NO_PICTURE_LIMIT_NS`] gives way to the safe default.
    /// - The confirmed configuration that shows a picture for
    ///   [`GOOD_BOOT_PICTURE_NS`] makes the boot good.
    pub fn tick<D: Display>(&mut self, now_ns: u64, engine: &mut VideoEngine<D>) -> GuardStep {
        let (now_ns, went_back) = self.clock(now_ns);
        let health = Health::of(engine);
        let no_picture_ns = watch(&mut self.no_picture_since_ns, health.no_picture(), now_ns);
        // A picture counts only when the engine reports the mode on the
        // display as the winner of a decision on the inputs it has now
        // (`Odren::settled`). Every change of the guard ends in `retry`,
        // `set_monitor` or `set_settings`, and each raises the engine's
        // input generation or sets a decision due. The engine records the
        // generation that placed its mode only at a modeset whose read back
        // holds and at a scale-only change; a failed modeset, a decision
        // with no winner, a blocked source and a restore leave the old
        // generation, and a blank leaves no mode. So the old mode never
        // counts as a picture of the change, also when it still passes the
        // new inputs.
        let shown = health.picture && health.decided;
        let picture_ns = watch(&mut self.picture_since_ns, shown, now_ns);
        let no_picture = no_picture_ns.is_some_and(|ns| ns >= NO_PICTURE_LIMIT_NS);
        match &self.state {
            State::Pending { .. } if went_back => self.revert(RevertReason::ClockWentBack, engine),
            State::Pending { prompt, .. } if now_ns >= prompt.deadline_ns => {
                self.revert(RevertReason::Timeout, engine)
            }
            State::Pending { .. } if no_picture => self.revert(RevertReason::NoPicture, engine),
            State::Confirmed if no_picture => {
                GuardStep::event(self.enter_safe(SafeReason::NoValidMode, engine))
            }
            State::Confirmed
                if !self.boot_good && picture_ns.is_some_and(|ns| ns >= GOOD_BOOT_PICTURE_NS) =>
            {
                let before = self.record.clone();
                self.boot_good = true;
                self.record.failed_boots = 0;
                self.finish(GuardEvent::GoodBoot, &before)
            }
            _ => GuardStep::default(),
        }
    }

    /// The time to use for `now_ns`: never earlier than a time already
    /// seen, so a clock that goes back cannot stretch a deadline or start
    /// a watch again. Also says whether it went back.
    fn clock(&mut self, now_ns: u64) -> (u64, bool) {
        let went_back = now_ns < self.latest_ns;
        self.latest_ns = self.latest_ns.max(now_ns);
        (self.latest_ns, went_back)
    }

    fn open_prompt_id(&self, id: PromptId) -> Result<Prompt, ConfirmError> {
        let State::Pending { prompt, .. } = &self.state else {
            return Err(ConfirmError::NoPrompt { asked: id });
        };
        if prompt.id != id {
            return Err(ConfirmError::WrongPrompt {
                asked: id,
                open: prompt.id,
            });
        }
        Ok(*prompt)
    }

    fn open_prompt<D: Display>(
        &mut self,
        target: DisplayConfig,
        fallback: Fallback,
        chain: Chain,
        now_ns: u64,
        engine: &mut VideoEngine<D>,
    ) -> GuardEvent {
        self.last_prompt = self.last_prompt.wrapping_add(1);
        let window_end_ns = now_ns.saturating_add(CONFIRM_WINDOW_NS);
        let prompt = Prompt {
            id: PromptId(self.last_prompt),
            deadline_ns: window_end_ns.min(chain.since_ns.saturating_add(PENDING_LIMIT_NS)),
        };
        self.state = State::Pending {
            prompt,
            fallback,
            since_ns: chain.since_ns,
            from_settings: chain.from_settings,
        };
        self.active = target;
        self.apply(engine);
        GuardEvent::PromptOpened(prompt)
    }

    /// Ends a pending change with what it replaced. Outside a pending
    /// change it does nothing.
    fn revert<D: Display>(
        &mut self,
        reason: RevertReason,
        engine: &mut VideoEngine<D>,
    ) -> GuardStep {
        let State::Pending {
            fallback,
            from_settings,
            ..
        } = &self.state
        else {
            return GuardStep::default();
        };
        let from_settings = *from_settings;
        let before = self.record.clone();
        match fallback.clone() {
            Fallback::Confirmed(config) => {
                self.state = State::Confirmed;
                self.active = config;
                self.apply(engine);
            }
            Fallback::Safe(safe_reason) => {
                self.enter_safe(safe_reason, engine);
            }
        }
        let event = GuardEvent::Reverted {
            reason,
            rewrite: from_settings.then(|| self.active.clone()),
        };
        self.finish(event, &before)
    }

    /// Gives the safe default to the engine, also when it is there
    /// already: `apply_to` retries, so a blocked or refused state gets a
    /// new set of attempts.
    fn enter_safe<D: Display>(
        &mut self,
        reason: SafeReason,
        engine: &mut VideoEngine<D>,
    ) -> GuardEvent {
        if matches!(reason, SafeReason::FailedBoots | SafeReason::Recovery) {
            self.record.safe_mode = true;
        }
        self.state = State::Safe(reason);
        self.active = self.safe.clone();
        self.apply(engine);
        GuardEvent::SafeMode(reason)
    }

    /// A picture must be seen again for the new inputs. The no-picture
    /// watch goes on: only a valid picture stops it, so a loop of changes
    /// with no picture cannot hold off the guard.
    fn apply<D: Display>(&mut self, engine: &mut VideoEngine<D>) {
        self.active.apply_to(engine);
        self.picture_since_ns = None;
    }

    fn finish(&self, event: GuardEvent, before: &GuardRecord) -> GuardStep {
        GuardStep {
            event: Some(event),
            save: (self.record != *before).then(|| self.record.clone()),
        }
    }
}

impl GuardStep {
    fn event(event: GuardEvent) -> Self {
        Self {
            event: Some(event),
            save: None,
        }
    }
}

/// How long `on` has held without a break at `now_ns`, or `None` when it
/// is off.
fn watch(since_ns: &mut Option<u64>, on: bool, now_ns: u64) -> Option<u64> {
    if !on {
        *since_ns = None;
        return None;
    }
    Some(now_ns.saturating_sub(*since_ns.get_or_insert(now_ns)))
}
