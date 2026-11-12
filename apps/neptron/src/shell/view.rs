//! What the snapshot shows: the parts that follow the session, the engine
//! and the guard each frame, and the pages the UI asks for.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_ui::screens::{DISPLAY_PAGE, QUICK_MENU_PAGE};
use neptron_ui::snapshot::{
    ConfirmId, GameId, Notice, NoticeId, NoticeKind, PageId, PendingConfirm, SessionInfo,
    SettingsPage, VideoStatus,
};
use neptron_video::{Phase, VideoStatusText};

use super::boot::{game_name, profile_id};
use super::{Shell, Stage};
use crate::display::monitor_setting;
use crate::pages::{self, Names};

/// What the confirm dialog says.
const CONFIRM_TEXT: &str = "Keep the new picture?\nNo answer brings the old one back.";

/// The notices the snapshot keeps. The oldest goes when a new one comes.
const NOTICES_MAX: usize = 8;

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Shell<D, S, I, C> {
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
                    let mut info = SessionInfo::new(GameId(0), game_name(&game.reference.path));
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
        let mut text = read_back
            .map(|m| {
                let rate = m.field_rate_mhz();
                format!(
                    "{}x{}{} {}.{:02} Hz",
                    m.hactive,
                    m.vactive,
                    if m.interlace { "i" } else { "p" },
                    rate / 1000,
                    rate % 1000 / 10
                )
            })
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
        let id = monitor_setting(&self.guard.active().monitor);
        self.snap.active_profile = self
            .presets
            .profiles()
            .iter()
            .position(|p| p.id == id)
            .map(profile_id);
    }

    /// Puts the rows of `page` in the snapshot.
    pub(super) fn load_page(&mut self, page: PageId) {
        let names = Names {
            presets: &self.presets,
            detected: &self.detected,
        };
        self.snap.settings = match page {
            DISPLAY_PAGE => pages::display_page(&self.config, names),
            QUICK_MENU_PAGE => pages::quick_menu_page(&self.config, names),
            other => SettingsPage::new(other, "", Vec::new()),
        };
    }

    /// Builds the loaded page again after a change of the settings.
    pub(super) fn reload_page(&mut self) {
        if let Some(page) = self.snap.settings.page {
            self.load_page(page);
        }
    }

    /// Adds a notice for the player, and the line to the log.
    pub(super) fn notice(&mut self, kind: NoticeKind, text: String) {
        self.log(text.clone());
        if self.snap.notices.len() == NOTICES_MAX {
            self.snap.notices.remove(0);
        }
        let id = NoticeId(self.next_notice);
        self.next_notice = self.next_notice.wrapping_add(1);
        self.snap.notices.push(Notice::new(id, text, kind));
    }
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
