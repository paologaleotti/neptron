//! The intents of the UI, and the settings changes they ask for.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::PadRoster;
use neptron_session::{ExitReport, Flush};
use neptron_settings::{
    AUDIO_VOLUME, DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY, DISPLAY_STYLE,
    INPUT_MENU_HOLD_START_SECONDS, Level, Safety, Value,
};
use neptron_ui::Intent;
use neptron_ui::screens::INPUT_PAGE;
use neptron_ui::snapshot::NoticeKind;
use neptron_video::{DisplayConfig, Phase};

use super::{Outcome, PendingChange, PictureNotice, Shell, Stage, proposal_hash};
use crate::library::{CheckStart, ScanKind, ScanStart};
use crate::outbox::Job;
use crate::pad_notes::NoteKind;
use crate::pages::{self, Gate, Names};
use crate::power::{self, PowerRequest};
use crate::relaunch::{NextRun, PadRecord};

/// The button of the notice that starts an arcade game with no set check.
pub const START_UNCHECKED: &str = "Start";

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// Carries out the intents of the last UI frame, in order.
    pub(super) fn carry_out(&mut self, now_ns: u64) {
        let mut intents = std::mem::take(&mut self.ui.intents);
        for intent in intents.drain(..) {
            self.intent(intent, now_ns);
        }
        // A refused move belongs to the keep of this batch only.
        self.refused_keeps.clear();
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
                    choices: None,
                    scope: pages::Scope::of_level(Level::Game),
                    class: self.display_class(),
                    outputs: &self.runtime.audio_outputs,
                };
                let found = pages::entry(&self.config, key)
                    .and_then(|e| Some((e.key, pages::value_of(e, value, names, &self.config)?)));
                if let (Some((key, value)), Some(level)) = (found, pages::settings_level(level)) {
                    self.set_value(key, level, value, now_ns);
                }
            }
            Intent::ResetValue { key, level } => {
                let key = pages::entry(&self.config, key).map(|e| e.key);
                if let (Some(key), Some(level)) = (key, pages::settings_level(level)) {
                    self.reset_value(key, level, now_ns);
                }
            }
            Intent::Action(pages::RESCAN_GAMES) => self.rescan_games(),
            Intent::Action(pages::REBOOT) => self.request_power(PowerRequest::Reboot),
            Intent::Action(pages::POWER_OFF) => self.request_power(PowerRequest::PowerOff),
            Intent::Action(pages::FORMAT_SHARE) => self.request_share_format(),
            Intent::Action(pages::KEEP_PICTURE) => self.keep_picture(now_ns),
            Intent::Action(pages::TRY_SETTINGS) => self.try_settings(now_ns),
            Intent::Action(id) => match crate::pad_page::pad_action(id) {
                Some(crate::pad_page::PadAction::MapButtons(port)) => {
                    let key = self.roster().get(port).map(|e| e.pad.key());
                    if !key.is_some_and(|key| self.start_mapping(key)) {
                        self.log(format!("no guided mapping for port {port}"));
                    }
                }
                Some(crate::pad_page::PadAction::Reset(port)) => self.reset_layout(port),
                None => {}
            },
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
            Intent::QuitGame => self.end_game(EndNotice::None),
            Intent::DismissNotice(id) => {
                self.snap.notices.retain(|n| n.id != id);
                self.pad_notes.answered(id);
                self.unchecked_launch.take_if(|(notice, _)| *notice == id);
            }
            Intent::KeepPadMapping => self.keep_mapping(),
            Intent::CancelPadMapping => {
                if self.mapping.is_some() {
                    self.log("the guided mapping is cancelled; nothing changed".to_owned());
                }
                self.end_mapping();
            }
            Intent::AcceptNotice(id) => {
                self.snap.notices.retain(|n| n.id != id);
                if let Some((NoteKind::Generic, pad)) = self.pad_notes.answered(id) {
                    self.set_up_pad(pad);
                }
                if let Some((_, game)) = self.unchecked_launch.take_if(|(notice, _)| *notice == id)
                {
                    self.log("an arcade game starts with no set check".to_owned());
                    self.relaunch_into(game);
                }
            }
        }
    }

    /// The answer "Set up this controller" of the notice of a pad with the
    /// generic layout: in the game list the guided mapping of that pad
    /// starts. Else, or when it cannot start, the Input panel opens with
    /// the focus on the row of that pad, or on its first row when the pad
    /// has no port now.
    fn set_up_pad(&mut self, (identity, path): crate::pad_notes::PadKey) {
        if self.start_mapping(neptron_input::PadKey { identity, path }) {
            return;
        }
        let roster = self.roster();
        let port = (0u8..).zip(roster.ports()).find_map(|(port, entry)| {
            entry
                .as_ref()
                .filter(|e| e.present && e.pad.identity == identity && e.pad.path == path)
                .map(|_| port)
        });
        let key = port.and_then(|port| pages::input_row_key(&roster, port));
        if !self.screens.open_panel(INPUT_PAGE, key) {
            self.log("the Input panel cannot open over the screen on show".to_owned());
        }
    }

    /// Starts a game. An arcade game first gets its set check on its own
    /// thread, which [`Self::poll_set_check`] takes; any other game ends
    /// the run with a relaunch into it. A launch while a game runs or a
    /// check runs is ignored.
    pub(super) fn launch(&mut self, game: neptron_ui::snapshot::GameId) {
        if !matches!(self.stage, Stage::List(_)) || self.outcome.is_some() {
            return;
        }
        let started = self
            .library
            .start_check(game, &self.paths.roms(), &self.paths.bios());
        match started {
            Ok(CheckStart::NotArcade) => self.relaunch_into(game),
            Ok(CheckStart::Started) => self.log("set check of an arcade game".to_owned()),
            Ok(CheckStart::Running) => {}
            Err(e) => self.notice(NoticeKind::Warning, format!("The game did not start: {e}.")),
        }
    }

    /// Takes a finished set check: the game starts, or a notice says why
    /// not. With no arcade index, the notice asks the player whether to
    /// start the game unchecked. The details of the check go to the log.
    pub(super) fn poll_set_check(&mut self) {
        let Some((game, result)) = self.library.poll_check() else {
            return;
        };
        let verdict = match result {
            Ok(verdict) => verdict,
            Err(e) => {
                self.notice(
                    NoticeKind::Warning,
                    format!("The files of the game could not be checked: {e}."),
                );
                return;
            }
        };
        for line in verdict.details() {
            self.log(format!("set check: {line}"));
        }
        let core = self
            .library
            .game(game)
            .map_or_else(String::new, |f| f.core.name.clone());
        match verdict.message(&core) {
            None => self.relaunch_into(game),
            Some(text) if verdict.stops() => self.notice(NoticeKind::Warning, text),
            Some(text) => {
                let id = self.notice_with_choice(NoticeKind::Warning, text, START_UNCHECKED);
                self.unchecked_launch = Some((id, game));
            }
        }
    }

    /// Ends the run with a relaunch into `game`, from the game list.
    fn relaunch_into(&mut self, game: neptron_ui::snapshot::GameId) {
        if !matches!(self.stage, Stage::List(_)) || self.outcome.is_some() {
            return;
        }
        if let Some(reference) = self.library.reference(game) {
            self.outcome = Some(Outcome::Relaunch(NextRun {
                pads: PadRecord::list(&self.roster()),
                game: Some(reference),
                notice: None,
            }));
        }
    }

    /// Scans the games folder again, reading every folder, on the scan
    /// thread. The game list stays usable, and the rescan row says that a
    /// scan runs. Ignored during a game and while a scan runs.
    ///
    /// The `Rescan games` row calls it through its intent. It is public for
    /// the tier 2 tests, and for a later `library rescan` command of the
    /// control protocol.
    pub fn rescan_games(&mut self) {
        if !matches!(self.stage, Stage::List(_)) {
            return;
        }
        let started = self.library.start_scan(
            self.paths.roms(),
            ScanKind::Rescan,
            std::time::SystemTime::now(),
        );
        match started {
            Ok(ScanStart::Started) => self.log("rescan of the games folder".to_owned()),
            Ok(ScanStart::Running) => self.log("rescan asked while a scan runs".to_owned()),
            Err(e) => self.notice(
                NoticeKind::Warning,
                format!("The game scan did not start: {e}."),
            ),
        }
        self.reload_page();
    }

    /// Ends the run for a reboot or a power-off, as a quit: the caller
    /// writes the settings and joins the threads, and the launcher asks
    /// init. Ignored when [`power::takes_request`] says no: during a game,
    /// with no power control, while a display change waits for its
    /// confirmation, and after the run has an outcome. The end of the run
    /// is clean, so it counts no failed boot.
    ///
    /// The rows of the System panel call it through their intents. It is
    /// public for the tier 2 tests.
    pub fn request_power(&mut self, request: PowerRequest) {
        if !power::takes_request(self.run_state()) {
            return;
        }
        let outcome = request.outcome();
        self.log(format!("{outcome:?} from the System panel"));
        self.outcome = Some(outcome);
    }

    /// Asks the storage script for the format of the share partition that
    /// it offered at this boot: the request file goes to the writer thread,
    /// and the run ends with a reboot, as for [`Self::request_power`], only
    /// once the writer has written it ([`Self::poll_format_request`]). The
    /// frontend never formats; the script checks the disk again at the next
    /// boot. Ignored with no offer, while a request waits or after one
    /// failed, and when the run takes no reboot.
    ///
    /// The `Format share` row calls it through its intent. It is
    /// public for the tier 2 tests.
    pub fn request_share_format(&mut self) {
        let Some(offer) = self.share_offer.clone() else {
            return;
        };
        if self.format_request != super::FormatRequest::None
            || !power::takes_request(self.run_state())
        {
            return;
        }
        self.log(format!(
            "format of the share partition {} asked from the System panel",
            offer.device
        ));
        self.outbox.send(Job::ShareFormatRequest {
            text: offer.request_text(),
        });
        self.format_request = super::FormatRequest::Waiting;
    }

    /// Takes the result of a waiting format request: a written request file
    /// ends the run with a reboot; a failed write ends nothing, shows a
    /// Warning notice, and disables the row for this run. It never waits.
    pub(super) fn poll_format_request(&mut self) {
        if self.format_request != super::FormatRequest::Waiting {
            return;
        }
        match self.outbox.request_result() {
            None => {}
            Some(Ok(())) => {
                self.log("the format request is written".to_owned());
                self.request_power(PowerRequest::Reboot);
                if self.outcome.is_some() {
                    self.format_request = super::FormatRequest::None;
                } else {
                    // A display change opened its confirmation while the
                    // writer wrote: the run takes no reboot now, and the
                    // request waits on the state partition.
                    self.format_request = super::FormatRequest::Written;
                    self.log("the format request waits for the next start".to_owned());
                    self.notice(
                        NoticeKind::Info,
                        pages::FORMAT_NEXT_RESTART_NOTICE.to_owned(),
                    );
                    self.reload_page();
                }
            }
            Some(Err(e)) => {
                self.format_request = super::FormatRequest::Failed;
                self.log(format!("the format request was not written: {e}"));
                self.notice(
                    NoticeKind::Warning,
                    pages::FORMAT_NOT_SAVED_NOTICE.to_owned(),
                );
                self.reload_page();
            }
        }
    }

    /// What the run is doing, for a power request.
    fn run_state(&self) -> power::RunState {
        power::RunState {
            in_list: matches!(self.stage, Stage::List(_)),
            control: self.runtime.power,
            pending: self.guard.pending().is_some(),
            ended: self.outcome.is_some(),
        }
    }

    /// Ends the game and the run, with a relaunch to the game list and the
    /// notice that [`pick_notice`] takes.
    pub(super) fn end_game(&mut self, notice: EndNotice) {
        let pads = PadRecord::list(&self.roster());
        let Stage::Game(game) = std::mem::replace(&mut self.stage, Stage::Ended) else {
            return;
        };
        let report = game.session.quit();
        let picked = pick_notice(notice, lost_save(&report));
        if let Some(text) = picked.logged {
            self.log(format!("a notice the game list does not show: {text}"));
        }
        self.outcome = Some(Outcome::Relaunch(NextRun {
            game: None,
            notice: picked.shown,
            pads,
        }));
    }

    /// The answer Keep of the confirm dialog. Every confirmation that the
    /// guard refuses goes to the log, and the dialog stays: for example a
    /// Keep while the change shows no valid picture. The UI shows no Keep
    /// at 0 s, so a refusal for the deadline only comes from a fault.
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
            Err(e) => self.log(format!("keep refused: {e}")),
        }
    }

    /// The row `Keep this picture` in safe mode: the picture on screen
    /// becomes the kept configuration, and the global settings name it, so
    /// the next boot starts on it with no prompt.
    fn keep_picture(&mut self, now_ns: u64) {
        let result = match &self.stage {
            Stage::List(list) => self.guard.keep_shown(&list.engine),
            Stage::Game(game) => self.guard.keep_shown(game.session.video()),
            Stage::Ended => return,
        };
        match result {
            Ok(step) => {
                self.on_guard(step, now_ns);
                let kept = self.guard.active().clone();
                self.rewrite_global(&kept);
                self.reload_page();
            }
            Err(e) => self.log(format!("keep refused: {e}")),
        }
    }

    /// The row `Try my settings again` after failed boots: safe mode ends,
    /// and the configuration of the settings applies, under a prompt when
    /// its monitor is not known.
    fn try_settings(&mut self, now_ns: u64) {
        let Some(config) = self.display_config() else {
            self.notice(
                NoticeKind::Error,
                "The display setting does not map to a picture.".to_owned(),
            );
            return;
        };
        let trusted = self.detected.trusts(&self.config.monitor());
        if let Some(step) =
            self.with_engine(|guard, engine| guard.retry_settings(config, trusted, now_ns, engine))
        {
            self.on_guard(step, now_ns);
        }
        self.reload_page();
    }

    /// Sets (`Some`) or unsets (`None`) a setting at a level. A setting with
    /// no effect on the signal is written at once and applied live. A
    /// picture style, refresh policy or TV width on a confirmed monitor is
    /// written at once and goes to the guard with no prompt: it only
    /// changes which modes the validator gave for that monitor the policy
    /// picks. Any other display setting goes to the guard under a prompt:
    /// it is on screen at once and reaches its file only when the player
    /// keeps it.
    pub(super) fn change_setting(
        &mut self,
        key: &'static str,
        level: Level,
        value: Option<Value>,
        now_ns: u64,
    ) {
        self.change_setting_with(key, level, value, None, now_ns);
    }

    /// A set from the menus or the keep prompt, by the gate of `level`. A
    /// keep that moves a game-level pick to another level and that the gate
    /// refuses leaves the pick at the game level.
    fn set_value(&mut self, key: &'static str, level: Level, value: Value, now_ns: u64) {
        // A keep moves the whole change of the visit: a pick that also set
        // the other row to Auto at the game level moves with that reset,
        // where the level gives the pick at all.
        let moves_reset =
            self.is_keep(key, level, &value) && self.game_resets.iter().any(|(k, _)| *k == key);
        let gate = match self.gate(key, level, &value) {
            Gate::Set { note } if moves_reset => match pages::other_row(key) {
                Some((other, _)) => Gate::SetWithReset { other, note },
                None => Gate::Set { note },
            },
            gate => gate,
        };
        match gate {
            Gate::Set { note } => {
                self.change_setting(key, level, Some(value), now_ns);
                if let Some(note) = note {
                    self.notice(NoticeKind::Info, note);
                }
            }
            Gate::SetWithReset { other, note } => {
                let auto = pages::other_row(key).map(|(_, auto)| Value::Text(auto.to_owned()));
                self.change_setting_with(key, level, Some(value), Some((other, auto)), now_ns);
                if let Some(note) = note {
                    self.notice(NoticeKind::Info, note);
                }
            }
            Gate::Refused(reason) if self.is_keep(key, level, &value) => {
                self.refused_keeps.push(key);
                self.log(format!(
                    "{key} = {value}: not kept at {}: {reason}",
                    level.name()
                ));
                self.notice(
                    NoticeKind::Info,
                    format!(
                        "Not kept {}: {}. The choice stays for this game.",
                        pages::level_phrase(level),
                        pages::lower_first(&reason)
                    ),
                );
            }
            Gate::Refused(reason) => self.refuse_value(key, &value, &reason),
        }
    }

    /// A set at `level` of the value that the game level holds: the keep
    /// prompt moves it there.
    fn is_keep(&self, key: &str, level: Level, value: &Value) -> bool {
        level != Level::Game && self.config.get(Level::Game, key) == Some(value)
    }

    /// A reset from the menus or the keep prompt. A key whose move the gate
    /// refused in this batch keeps its game-level value. A pick of this
    /// run that also set the other picture row at the game level loses both
    /// keys there, in one write: no key of the pair stays behind alone.
    fn reset_value(&mut self, key: &'static str, level: Level, now_ns: u64) {
        if level == Level::Game && self.refused_keeps.contains(&key) {
            return;
        }
        let pair = (level == Level::Game)
            .then(|| {
                self.game_resets
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, o)| *o)
            })
            .flatten();
        self.game_resets.retain(|(k, _)| *k != key);
        self.change_setting_with(key, level, None, pair.map(|other| (other, None)), now_ns);
    }

    /// [`Self::change_setting`], and with `reset` the other picture row
    /// (a key, with its new value or `None` to remove it) at the same
    /// level, in the same write and the same revert. A set of the other row
    /// to Auto gets a notice that names it.
    pub(super) fn change_setting_with(
        &mut self,
        key: &'static str,
        level: Level,
        value: Option<Value>,
        reset: Option<(&'static str, Option<Value>)>,
        now_ns: u64,
    ) {
        let Some(entry) = self.config.schema().get(key).copied() else {
            return;
        };
        if key == DISPLAY_OUTPUT || self.config.file(level).is_none() {
            return;
        }
        let old = self.config.get(level, key).cloned();
        let reset =
            reset.filter(|(other, _)| pages::other_row(key).is_some_and(|(o, _)| o == *other));
        let side = match self
            .config
            .set_clearing(level, key, value.clone(), reset.clone())
        {
            Ok(side) => side,
            Err(e) => {
                self.notice(
                    NoticeKind::Error,
                    format!("The setting was not changed: {e}."),
                );
                return;
            }
        };
        self.note_picture_change(key);
        if level == Level::Game && value.is_some() {
            self.game_resets.retain(|(k, _)| *k != key);
            if let Some((other, Some(_))) = &reset {
                self.game_resets.push((key, other));
            }
        }
        if let Some((other, Some(_))) = &reset {
            self.notice(
                NoticeKind::Info,
                match level {
                    Level::Game => format!("{} is set to Auto too.", pages::row_label(other)),
                    Level::System | Level::Global => format!(
                        "{} is set to Auto too, {}.",
                        pages::row_label(other),
                        pages::level_phrase(level)
                    ),
                },
            );
        }
        match entry.safety {
            Safety::None => {
                self.write_setting(level, key, value);
                self.apply_live(key);
            }
            Safety::Validated if self.guard.phase() == Phase::Confirmed => {
                let mut changes = vec![(key, value)];
                changes.extend(side.into_iter().map(|c| (c.key, c.new)));
                self.write_settings(level, changes);
                self.follow(now_ns);
            }
            Safety::Validated | Safety::ConfirmOrRevert => {
                self.pending.push(PendingChange {
                    level,
                    key,
                    old,
                    new: value,
                });
                self.pending.extend(side.into_iter().map(|c| PendingChange {
                    level,
                    key: c.key,
                    old: c.old,
                    new: c.new,
                }));
                self.propose(now_ns);
            }
        }
        self.reload_page();
    }

    /// What a set of `value` for `key` at `level` does, by the rule that
    /// builds its option in the menu of that level (`pages::gate`). No
    /// input, from the menus or the protocol, sets a value that the rule
    /// refuses.
    pub(super) fn gate(&self, key: &str, level: Level, value: &Value) -> Gate {
        if pages::other_row(key).is_none() {
            return Gate::Set { note: None };
        }
        let choices = self.picture_choices(level);
        pages::gate(
            key,
            &value.to_string(),
            choices.as_ref(),
            self.scope_of(level),
        )
    }

    /// A short notice for a value that the display does not give, and the
    /// line in the log.
    fn refuse_value(&mut self, key: &str, value: &Value, reason: &str) {
        self.log(format!("{key} = {value}: not offered: {reason}"));
        self.notice(
            NoticeKind::Info,
            format!("Not changed: {}.", pages::lower_first(reason)),
        );
    }

    /// A change of the picture mode or the refresh choice in a game: the
    /// picture notice shows again for the first new picture.
    fn note_picture_change(&mut self, key: &str) {
        if key != DISPLAY_STYLE && key != DISPLAY_REFRESH_POLICY {
            return;
        }
        if let Stage::Game(game) = &self.stage
            && let Some(current) = game.session.video().current()
        {
            self.picture_notice =
                PictureNotice::after_change(proposal_hash(&current.chosen.proposal));
        }
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
        // In safe mode the configuration on screen still waits for a Keep:
        // keeping it ends safe mode.
        if self.guard.phase() == Phase::Confirmed && &config == self.guard.active() {
            let changes = std::mem::take(&mut self.pending);
            self.write_pending(changes);
            return;
        }
        if let Some(step) = self.with_engine(|guard, engine| guard.propose(config, now_ns, engine))
        {
            self.on_guard(step, now_ns);
        }
    }

    /// Gives the policy settings of the settings in memory to the guard,
    /// with no prompt.
    fn follow(&mut self, now_ns: u64) {
        let Some(config) = self.display_config() else {
            self.notice(
                NoticeKind::Error,
                "The display setting does not map to a picture.".to_owned(),
            );
            return;
        };
        if let Some(step) =
            self.with_engine(|guard, engine| guard.follow_settings(config.settings, engine))
        {
            self.on_guard(step, now_ns);
        }
    }

    /// The display configuration that the settings in memory ask for: the
    /// global monitor (on a TV, the profile of the global standard), and
    /// the policy of the game, or the global policy in the game list.
    fn display_config(&self) -> Option<DisplayConfig> {
        let monitor = self
            .detected
            .wanted(&self.config.monitor(), &self.config.tv(), &self.presets)
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
        let volume = self.config.volume_percent();
        match &mut self.stage {
            // The game list plays no game audio: the next game resolves
            // the volume at its launch.
            Stage::List(_) if key == AUDIO_VOLUME => {}
            Stage::Game(game) if key == AUDIO_VOLUME => game.session.set_volume_percent(volume),
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

/// The notice of the end of a game, before the notice of a lost save is
/// weighed against it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EndNotice {
    /// The player quit: no notice of its own.
    None,
    /// A notice that comes before a lost save: the game did not start
    /// (S2), so it had no play to save.
    First(String),
    /// A notice that comes after a lost save: the core ended a game that
    /// ran (S3), or the session failed (a stalled display, for example
    /// after an hour of play). The lost save is worth more to the player.
    AfterSave(String),
}

/// The notices after a game: the one the game list shows, and the one
/// that lost, which goes to the log only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Picked {
    /// The notice of the game list.
    pub(super) shown: Option<String>,
    /// The notice that lost to it, for the log.
    pub(super) logged: Option<String>,
}

/// The one notice of the game list after a game, the end notice or the
/// notice of a lost save in the order of [`EndNotice`], and the other one
/// for the log, so no notice is lost in silence.
pub(super) fn pick_notice(end: EndNotice, lost_save: Option<String>) -> Picked {
    match end {
        EndNotice::None => Picked {
            shown: lost_save,
            logged: None,
        },
        EndNotice::First(text) => Picked {
            shown: Some(text),
            logged: lost_save,
        },
        EndNotice::AfterSave(text) => match lost_save {
            Some(lost) => Picked {
                shown: Some(lost),
                logged: Some(text),
            },
            None => Picked {
                shown: Some(text),
                logged: None,
            },
        },
    }
}

/// The notice of a save RAM that the save file does not hold at the end of
/// the game. Only the last flush decides: a failed write in the game that a
/// later write repaired lost nothing.
pub(super) fn lost_save(report: &ExitReport) -> Option<String> {
    if !matches!(report.save, Flush::Failed | Flush::WriteFailed) {
        return None;
    }
    Some(match &report.save_stats.last_error {
        Some(e) => format!("The game was not saved: {e}."),
        None => "The game was not saved: the last write did not finish.".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use neptron_session::{Counters, Flush, SaveError, SaveStats};

    use super::*;

    fn report(save: Flush, last_error: Option<SaveError>) -> ExitReport {
        ExitReport {
            save,
            save_stats: SaveStats {
                last_error,
                ..SaveStats::default()
            },
            counters: Counters::default(),
        }
    }

    fn copy_error() -> Option<SaveError> {
        Some(SaveError::WriterStopped {
            path: "game.srm".into(),
        })
    }

    #[test]
    fn a_failure_that_a_later_write_repaired_is_not_a_lost_save() {
        assert_eq!(lost_save(&report(Flush::Written, copy_error())), None);
        assert_eq!(lost_save(&report(Flush::Unchanged, copy_error())), None);
        assert_eq!(lost_save(&report(Flush::NoSaveRam, None)), None);
    }

    fn picked(shown: Option<&str>, logged: Option<&str>) -> Picked {
        Picked {
            shown: shown.map(str::to_owned),
            logged: logged.map(str::to_owned),
        }
    }

    const LOST: &str = "The game was not saved: disk full.";

    #[test]
    fn a_failed_start_is_shown_and_a_lost_save_goes_to_the_log() {
        assert_eq!(
            pick_notice(EndNotice::First("S2".to_owned()), Some(LOST.to_owned())),
            picked(Some("S2"), Some(LOST))
        );
        assert_eq!(
            pick_notice(EndNotice::First("S2".to_owned()), None),
            picked(Some("S2"), None)
        );
    }

    #[test]
    fn a_lost_save_is_shown_before_a_stop_and_the_stop_goes_to_the_log() {
        assert_eq!(
            pick_notice(EndNotice::AfterSave("S3".to_owned()), Some(LOST.to_owned())),
            picked(Some(LOST), Some("S3"))
        );
        assert_eq!(
            pick_notice(EndNotice::AfterSave("S3".to_owned()), None),
            picked(Some("S3"), None)
        );
    }

    #[test]
    fn a_lost_save_is_shown_before_a_session_error_after_play() {
        let stalled = "The game stopped: no flip completed for 10 frames.";
        assert_eq!(
            pick_notice(
                EndNotice::AfterSave(stalled.to_owned()),
                Some(LOST.to_owned())
            ),
            picked(Some(LOST), Some(stalled))
        );
        assert_eq!(
            pick_notice(EndNotice::AfterSave(stalled.to_owned()), None),
            picked(Some(stalled), None)
        );
    }

    #[test]
    fn a_quit_shows_only_a_lost_save() {
        assert_eq!(
            pick_notice(EndNotice::None, Some(LOST.to_owned())),
            picked(Some(LOST), None)
        );
        assert_eq!(pick_notice(EndNotice::None, None), picked(None, None));
    }

    #[test]
    fn a_failed_last_flush_is_a_lost_save() {
        for flush in [Flush::Failed, Flush::WriteFailed] {
            let text = lost_save(&report(flush, copy_error())).expect("a notice");
            assert!(text.starts_with("The game was not saved: "), "{text}");
        }
    }
}
