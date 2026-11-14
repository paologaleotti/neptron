//! What the snapshot shows: the parts that follow the session, the engine
//! and the guard each frame, and the pages the UI asks for.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_settings::DisplayClass;
use neptron_types::{Modeline, MonitorKind, TvStandard};
use neptron_ui::screens::{
    AUDIO_PAGE, DISPLAY_PAGE, LIBRARY_PAGE, QUICK_MENU_PAGE, SYSTEM_PAGE, VIDEO_PAGE,
};
use neptron_ui::snapshot::{
    ConfirmId, GameId, Notice, NoticeId, NoticeKind, PageId, PendingConfirm, SessionInfo,
    SettingsPage, VideoStatus,
};
use neptron_video::{Monitor, Phase, VideoStatusText};

use super::boot::{game_name, profile_id};
use super::{MENU_SOURCE, Shell, Stage};
use crate::pages::{self, Names, StyleOption};

/// What the confirm dialog says.
const CONFIRM_TEXT: &str = "Keep the new picture?\nNo answer brings the old one back.";

/// The monitor line of the Display panel on a TV output.
pub const TV_MONITOR_NAME: &str = "Composite TV";

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
        let styles = self.style_options();
        let names = Names {
            presets: &self.presets,
            detected: &self.detected,
            styles: &styles,
            class: self.display_class(),
            outputs: &self.runtime.audio_outputs,
        };
        self.snap.settings = match page {
            DISPLAY_PAGE => pages::display_page(&self.config, names),
            LIBRARY_PAGE => pages::library_page(self.library.scanning(), self.in_game()),
            SYSTEM_PAGE => pages::system_page(self.in_game(), self.runtime.power),
            AUDIO_PAGE => pages::audio_page(&self.config, names),
            QUICK_MENU_PAGE => pages::quick_menu_page(&self.config, names),
            VIDEO_PAGE => pages::video_page(&self.config, names),
            other => SettingsPage::new(other, "", Vec::new()),
        };
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

    /// What each picture style gives for the source on screen, from the
    /// same policy, caps and validator as the run (`style_rows`): the game
    /// in a game, the menu source in the list.
    fn style_options(&self) -> Vec<StyleOption> {
        let (engine, source) = match &self.stage {
            Stage::List(list) => (&list.engine, MENU_SOURCE),
            Stage::Game(game) => (game.session.video(), game.session.source()),
            Stage::Ended => return Vec::new(),
        };
        let Ok(rows) = engine.style_rows(&source) else {
            return Vec::new();
        };
        rows.iter()
            .map(|row| {
                let mode = row
                    .winner
                    .and_then(|i| row.plan.considered.get(i))
                    .and_then(|c| c.outcome.as_ref().ok())
                    .map(|p| p.mode);
                let detail = mode
                    .map(|m| {
                        let line = m.line_rate_mhz();
                        let field = m.field_rate_mhz();
                        format!(
                            "{}.{:02} kHz  {}.{:02} Hz",
                            line / 1_000_000,
                            line % 1_000_000 / 10_000,
                            field / 1_000,
                            field % 1_000 / 10
                        )
                    })
                    .unwrap_or_default();
                StyleOption {
                    key: row.style.key(),
                    detail,
                    unavailable: (!row.available).then(|| "Needs another monitor".to_owned()),
                }
            })
            .collect()
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
