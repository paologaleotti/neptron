//! What the snapshot shows: the parts that follow the session, the engine
//! and the guard each frame, and the pages the UI asks for.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::PadRoster;
use neptron_settings::{DisplayClass, Level};
use neptron_types::{Modeline, MonitorKind, TvStandard};
use neptron_ui::screens::{
    AUDIO_PAGE, DISPLAY_PAGE, INPUT_PAGE, LIBRARY_PAGE, NETWORK_PAGE, QUICK_MENU_PAGE, SYSTEM_PAGE,
    VIDEO_PAGE,
};
use neptron_ui::snapshot::{
    ConfirmId, GameId, Notice, NoticeId, NoticeKind, PageId, PendingConfirm, SessionInfo,
    SettingsPage, VideoStatus,
};
use neptron_video::{
    AnswerRequest, Monitor, Phase, PictureChoices, REFERENCE_SOURCES, VideoStatusText,
};

use super::boot::profile_id;
use super::{Shell, Stage};
use crate::pages::{self, Names, Scope};

/// What the confirm dialog says.
pub const CONFIRM_TEXT: &str = "Keep the new picture?\nNo answer brings the old one back.";

/// The monitor line of the Display panel on a TV output.
pub const TV_MONITOR_NAME: &str = "Composite TV";

/// The notices the snapshot keeps. The oldest goes when a new one comes.
const NOTICES_MAX: usize = 8;

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// Updates the parts of the snapshot that change by themselves: the
    /// time left to confirm, the session, the video status and the mode.
    pub(super) fn refresh_view(&mut self, now_ns: u64) {
        match (self.guard.pending(), self.snap.confirm.as_mut()) {
            (Some(prompt), Some(confirm)) if confirm.id == ConfirmId(prompt.id.0) => {
                confirm.remaining_ns = prompt.deadline_ns.saturating_sub(now_ns);
            }
            (Some(prompt), _) => {
                self.snap.confirm = Some(PendingConfirm::new(
                    ConfirmId(prompt.id.0),
                    CONFIRM_TEXT,
                    prompt.deadline_ns.saturating_sub(now_ns),
                ));
            }
            (None, _) => self.snap.confirm = None,
        }
        if let Stage::Game(game) = &self.stage {
            let paused = game.session.paused();
            match &mut self.snap.session {
                Some(info) => info.paused = paused,
                None => {
                    let mut info = SessionInfo::new(GameId(0), self.library.title(&game.reference));
                    info.paused = paused;
                    self.snap.session = Some(info);
                }
            }
            let status = game.session.video_status(&mut self.status_text);
            copy_status(status, &mut self.snap.video);
        }
        self.refresh_mode();
    }

    /// The mode line of the display panel, from the read back of the mode
    /// on screen. It changes only with the mode or the safe mode.
    fn refresh_mode(&mut self) {
        let read_back = self.engine().and_then(|e| e.current()).map(|c| c.read_back);
        let safe = matches!(self.guard.phase(), Phase::Safe(_));
        if self.shown_mode == read_back && self.shown_safe == safe {
            return;
        }
        self.shown_mode = read_back;
        self.shown_safe = safe;
        let line = match self.engine().and_then(|e| e.current()) {
            Some(current) => format!(
                "mode set: {:?}; read back {:?}{}",
                current.chosen,
                current.read_back,
                if safe { " (safe mode)" } else { "" }
            ),
            None => "no mode on the wire".to_owned(),
        };
        self.log(line);
        let standard = match &self.guard.active().monitor.profile().kind {
            MonitorKind::Tv { standard } => Some(*standard),
            MonitorKind::Crt { .. } | MonitorKind::FlatPanel { .. } => None,
        };
        let mut text = read_back
            .map(|m| mode_line(&m, standard))
            .unwrap_or_default();
        if safe {
            text.push_str(" (safe mode)");
        }
        if let Some(display) = &mut self.snap.display {
            display.mode = text;
        }
        self.refresh_profile();
    }

    /// The active profile of the snapshot: the monitor the guard gave the
    /// engine.
    pub(super) fn refresh_profile(&mut self) {
        let monitor = &self.guard.active().monitor;
        let id = self.detected.monitor_setting(monitor);
        self.snap.active_profile = self
            .presets
            .profiles()
            .iter()
            .position(|p| p.id == id)
            .map(profile_id);
        // A monitor of its EDID is no preset: the panel shows its name. A
        // TV profile is built in: the panel names the kind of display, and
        // the mode line names the standard.
        let name = match monitor {
            _ if self.snap.active_profile.is_some() => String::new(),
            Monitor::Profile(profile) if matches!(profile.kind, MonitorKind::Tv { .. }) => {
                TV_MONITOR_NAME.to_owned()
            }
            Monitor::Edid(edid) => edid.name.clone().unwrap_or_else(|| edid.profile.id.clone()),
            Monitor::Profile(profile) => profile.id.clone(),
        };
        if let Some(display) = &mut self.snap.display {
            display.monitor = name;
        }
    }

    /// Puts the rows of `page` in the snapshot.
    pub(super) fn load_page(&mut self, page: PageId) {
        // Settings > Display writes the global level, the Video page of the
        // quick menu the game level: each judges with the values of its
        // level.
        let level = match page {
            VIDEO_PAGE => Level::Game,
            _ => Level::Global,
        };
        // The answers come from the cache of the engine; while they are in
        // work, the frame makes them a few decisions at a time and loads
        // the page again when they are there. Meanwhile the page shows its
        // last settled answers, and takes no pick of the picture rows.
        let (choices, judging) = match page {
            DISPLAY_PAGE | VIDEO_PAGE => {
                self.page_ask = self.picture_request(level);
                match self.picture_answers(level) {
                    Some(Some(choices)) => {
                        self.last_answers = Some((page, choices.clone()));
                        (Some(choices), false)
                    }
                    Some(None) => match &self.last_answers {
                        Some((shown, last)) if *shown == page => (Some(last.clone()), true),
                        _ => (None, true),
                    },
                    None => (None, false),
                }
            }
            _ => {
                self.page_ask = None;
                (None, false)
            }
        };
        self.keep_asks = self.keep_requests();
        let names = Names {
            presets: &self.presets,
            detected: &self.detected,
            choices: choices.as_ref(),
            judging,
            scope: self.scope_of(level),
            class: self.display_class(),
            outputs: &self.runtime.audio_outputs,
        };
        self.snap.settings = match page {
            DISPLAY_PAGE => {
                let mut page = pages::display_page(&self.config, names, self.in_game());
                page.rows.extend(pages::safe_mode_rows(self.guard.phase()));
                page
            }
            LIBRARY_PAGE => pages::library_page(self.library.scanning(), self.in_game()),
            NETWORK_PAGE => {
                self.network_builds += 1;
                pages::network_page(self.network_info.as_ref())
            }
            SYSTEM_PAGE => pages::system_page(
                self.in_game(),
                self.runtime.power,
                self.share_offer.as_ref(),
                match self.format_request {
                    super::FormatRequest::None | super::FormatRequest::Waiting => {
                        pages::FormatRow::Offered
                    }
                    super::FormatRequest::Failed => pages::FormatRow::NotSaved,
                    super::FormatRequest::Written => pages::FormatRow::Written,
                },
            ),
            AUDIO_PAGE => pages::audio_page(&self.config, names),
            INPUT_PAGE => pages::input_page(&self.config, names, &self.roster()),
            QUICK_MENU_PAGE => pages::quick_menu_page(&self.config, names),
            VIDEO_PAGE => pages::video_page(&self.config, names),
            other => match crate::pad_page::port_of_page(other) {
                Some(port) => self.pad_page(other, port),
                None => SettingsPage::new(other, "", Vec::new()),
            },
        };
    }

    /// The page of the controller on `port`, from the roster and the map
    /// that the source reads.
    fn pad_page(&self, page: PageId, port: u8) -> SettingsPage {
        let roster = self.roster();
        let entry = roster.get(port);
        let map = entry.and_then(|e| match &self.stage {
            Stage::List(list) => list.input.pad_map(e.pad.key()),
            Stage::Game(game) => game.session.input().pad_map(e.pad.key()),
            Stage::Ended => None,
        });
        let label = pages::port_label(port);
        let writable = self.pad_maps.locked().is_none();
        crate::pad_page::pad_page_rows(page, &label, entry, map, (self.in_game(), writable))
    }

    /// The class of the monitor that the guard gave the engine: the one
    /// whose path the run takes.
    pub(super) fn display_class(&self) -> DisplayClass {
        match self.guard.active().monitor.profile().kind {
            MonitorKind::FlatPanel { .. } => DisplayClass::FlatPanel,
            MonitorKind::Crt { .. } => DisplayClass::Crt,
            MonitorKind::Tv { .. } => DisplayClass::Tv,
        }
    }

    /// The request of the answers for the options of the picture mode and
    /// refresh rows of a page or a set that writes `level`, with the values
    /// resolved from the defaults up to that level. The game level is
    /// judged for the running game with the values of the run; the system
    /// level in a game for the running game with the values up to the
    /// system level; the global level, and every level in the game list,
    /// for every game (the running game, if any, then the reference
    /// sources, `REFERENCE_SOURCES`) with the global values.
    fn picture_request(&self, level: Level) -> Option<AnswerRequest> {
        let game = match &self.stage {
            Stage::List(_) => None,
            Stage::Game(game) => Some(game.session.source()),
            Stage::Ended => return None,
        };
        match (level, game) {
            (Level::Game, Some(source)) => {
                let engine = self.engine_ref()?;
                Some(engine.request_for(&source))
            }
            (Level::System, Some(source)) => Some(AnswerRequest {
                sources: vec![source],
                settings: self.config.policy_up_to(Level::System).ok()?,
                scanlines: self.config.scanlines_up_to(Level::System).ok()?,
            }),
            (Level::Global, _) | (Level::System | Level::Game, None) => Some(AnswerRequest {
                sources: game.into_iter().chain(REFERENCE_SOURCES).collect(),
                settings: self.config.policy_up_to(Level::Global).ok()?,
                scanlines: self.config.scanlines_up_to(Level::Global).ok()?,
            }),
        }
    }

    /// Whom a page or a set at `level` acts for: the game of the run at
    /// the game and system levels in a game, every game otherwise.
    pub(super) fn scope_of(&self, level: Level) -> Scope {
        match (level, &self.stage) {
            (Level::Game | Level::System, Stage::Game(_)) => Scope::ThisGame,
            (Level::Global, _) | (_, Stage::List(_) | Stage::Ended) => Scope::EveryGame,
        }
    }

    /// The engine of the run: the list's or the game's.
    pub(super) fn engine_ref(&self) -> Option<&neptron_video::VideoEngine<D>> {
        match &self.stage {
            Stage::List(list) => Some(&list.engine),
            Stage::Game(game) => Some(game.session.video()),
            Stage::Ended => None,
        }
    }

    /// The answers for a page that writes `level`: `Some(None)` while they
    /// are in work, `None` with no engine or no plan.
    pub(super) fn picture_answers(&self, level: Level) -> Option<Option<PictureChoices>> {
        let request = self.picture_request(level)?;
        match self.engine_ref()?.answers(&request) {
            None => Some(None),
            Some(Ok(choices)) => Some(Some(choices)),
            Some(Err(_)) => None,
        }
    }

    /// The answers for a set at `level`, finished at once: the gate of a
    /// set takes the answers of the loaded page from the cache, and makes
    /// them only for a level that no page shows (a keep, the protocol).
    /// `None` with no engine or no plan.
    pub(super) fn picture_choices(&self, level: Level) -> Option<PictureChoices> {
        let request = self.picture_request(level)?;
        self.engine_ref()?.answers_now(&request).ok()
    }

    /// Builds the loaded page again after a change of the settings.
    pub(super) fn reload_page(&mut self) {
        if let Some(page) = self.snap.settings.page {
            self.load_page(page);
        }
    }

    /// Adds a notice for the player, and the line to the log.
    pub(super) fn notice(&mut self, kind: NoticeKind, text: String) {
        self.push_notice(kind, text, None);
    }

    /// Adds a notice that asks the player to choose: `Cancel` or `choice`.
    /// Returns its id, which the answer names.
    pub(super) fn notice_with_choice(
        &mut self,
        kind: NoticeKind,
        text: String,
        choice: &str,
    ) -> NoticeId {
        self.push_notice(kind, text, Some(choice))
    }

    pub(super) fn push_notice(
        &mut self,
        kind: NoticeKind,
        text: String,
        choice: Option<&str>,
    ) -> NoticeId {
        self.log(text.clone());
        if self.snap.notices.len() == NOTICES_MAX {
            let dropped = self.snap.notices.remove(0);
            self.pad_notes.dropped(dropped.id);
        }
        let id = NoticeId(self.next_notice);
        self.next_notice = self.next_notice.wrapping_add(1);
        let notice = Notice::new(id, text, kind);
        self.snap.notices.push(match choice {
            Some(label) => notice.with_choice(label),
            None => notice,
        });
        id
    }
}

/// The mode line of the Display panel: the size with its scan and the
/// field rate, `720x240p 60.05 Hz`. On a TV the standard replaces the
/// width, which is always the 720 samples of the encoder:
/// `NTSC 240p 60.05 Hz`. Only a TV names a standard.
pub fn mode_line(mode: &Modeline, standard: Option<TvStandard>) -> String {
    let rate = mode.field_rate_mhz();
    let scan = if mode.interlace { "i" } else { "p" };
    let size = match standard {
        Some(standard) => format!("{} {}{scan}", crate::tv::name(standard), mode.vactive),
        None => format!("{}x{}{scan}", mode.hactive, mode.vactive),
    };
    format!("{size} {}.{:02} Hz", rate / 1000, rate % 1000 / 10)
}

/// Copies the status text into the snapshot. Equal text is not written,
/// and new text goes into the strings that are there, so a steady frame
/// allocates nothing.
fn copy_status(status: Option<&VideoStatusText>, video: &mut Option<VideoStatus>) {
    let Some(status) = status else {
        *video = None;
        return;
    };
    let shown = video.get_or_insert_with(VideoStatus::default);
    for (to, from) in [
        (&mut shown.game, &status.game),
        (&mut shown.mode, &status.mode),
        (&mut shown.result, &status.result),
    ] {
        if to != from {
            to.clear();
            to.push_str(from);
        }
    }
    shown.alert = status.alert;
}

#[cfg(test)]
mod tests {
    use neptron_types::{TV_525_240P, TV_525_480I, TV_625_288P};

    use super::*;

    #[test]
    fn only_a_tv_names_its_standard_on_the_mode_line() {
        assert_eq!(mode_line(&TV_525_240P, None), "720x240p 60.05 Hz");
        assert_eq!(
            mode_line(&TV_525_240P, Some(TvStandard::Ntsc)),
            "NTSC 240p 60.05 Hz"
        );
        assert_eq!(
            mode_line(&TV_525_480I, Some(TvStandard::Pal60)),
            "PAL 60 480i 59.94 Hz"
        );
        assert_eq!(
            mode_line(&TV_625_288P, Some(TvStandard::Pal)),
            "PAL 288p 50.08 Hz"
        );
    }
}
