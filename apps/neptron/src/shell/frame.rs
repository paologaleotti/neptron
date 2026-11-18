//! One frame of the loop, in the game list and in a game.
//!
//! Each frame reads the clock once and gives that time to every call of
//! the guard, the hotkeys and the navigation of the frame.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::Hotkey;
use neptron_input::PadRoster;
use neptron_session::EndReason;
use neptron_types::{Control, Frame, PixelFormat};
use neptron_ui::screens::{INPUT_PAGE, NETWORK_PAGE, NETWORK_SCREEN};
use neptron_ui::snapshot::NoticeKind;
use neptron_ui::{CpuCanvas, UI_HEIGHT, UI_WIDTH, UiAction};

use super::intents::EndNotice;
use super::picture::{picture_text, vertical_interlaced};
use super::{MENU_SOURCE, Outcome, Shell, Stage, UNPACED_FRAME_NS};

use crate::library::{ScanKind, Scanned, changes_text};
use crate::network::{NETWORK_REFRESH_NS, NetworkInfo};
use crate::notices::{VERTICAL_INTERLACED, VERTICAL_INTERLACED_TEXT};
use crate::outbox::Job;
use crate::pad_notes::{NoteKind, SET_UP_CONTROLLER, silent_text};
use crate::start_failure;

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// Runs one frame. Returns how the run ends, when it ends with this
    /// frame.
    pub fn step(&mut self) -> Option<Outcome> {
        let now_ns = self.clock.now_ns();
        self.now_ns = now_ns;
        self.frame.set(self.frame.get() + 1);
        self.outbox.flush();
        while let Some(message) = self.outbox.failure() {
            self.notice(NoticeKind::Error, message);
        }
        self.poll_format_request();
        self.poll_library();
        self.poll_set_check();
        self.poll_network(now_ns);
        self.poll_pads(now_ns);
        match self.stage {
            Stage::List(_) => self.list_frame(now_ns),
            Stage::Game(_) => self.game_frame(now_ns),
            Stage::Ended => {}
        }
        if self
            .runtime
            .quit
            .as_ref()
            .is_some_and(|q| q.load(std::sync::atomic::Ordering::Relaxed))
        {
            return Some(Outcome::Quit);
        }
        self.outcome.take()
    }

    fn list_frame(&mut self, now_ns: u64) {
        self.actions.clear();
        self.events.clear();
        let mut recover = false;
        if let Stage::List(list) = &mut self.stage {
            list.input.poll(&mut self.events);
            list.ports.apply(&self.events);
            list.fired.clear();
            list.hotkeys.update(&list.ports, now_ns, &mut list.fired);
            recover = list
                .fired
                .iter()
                .any(|h| matches!(h, Hotkey::BlindRecovery { .. }));
        }
        if recover {
            self.recover(now_ns);
        }
        self.poll_mapping(now_ns);
        self.nav.update(&self.events, now_ns, &mut self.actions);
        self.update_ui(now_ns);
        // A reboot or a power-off keeps its last frame on screen until the
        // machine stops, so that frame must say what happens: it shows,
        // through the same present in the mode on screen. Any other end
        // shows nothing more.
        let last = matches!(self.outcome, Some(Outcome::Reboot | Outcome::PowerOff));
        if self.outcome.is_some() && !last {
            return;
        }
        let shown = match &mut self.stage {
            Stage::List(list) => {
                list.engine.begin_frame(MENU_SOURCE);
                let presented = list
                    .engine
                    .present(None, Some(canvas_frame(self.canvas.canvas())));
                presented.is_ok() && list.engine.wait_flip().is_ok()
            }
            _ => true,
        };
        if last {
            return;
        }
        if let Some(step) = self.with_engine(|guard, engine| guard.tick(now_ns, engine)) {
            self.on_guard(step, now_ns);
        }
        if !shown {
            (self.runtime.pace)(now_ns.saturating_add(UNPACED_FRAME_NS));
        }
    }

    fn game_frame(&mut self, now_ns: u64) {
        self.actions.clear();
        // The session has paused for the menu hotkey. A confirm dialog on
        // screen takes every action and would drop the menu, and the game
        // would stay paused with no menu: the menu waits for the dialog to
        // close.
        if self.snap.confirm.is_none() && std::mem::take(&mut self.menu_hotkey) {
            self.actions.push(UiAction::Menu);
        }
        if let Stage::Game(game) = &self.stage {
            let start = self.actions.len();
            self.nav
                .update(game.session.input_events(), now_ns, &mut self.actions);
            if !self.screens.shows_ui() {
                self.actions.truncate(start);
            }
        }
        self.update_ui(now_ns);
        let ui = self
            .screens
            .shows_ui()
            .then(|| canvas_frame(self.canvas.canvas()));
        let Stage::Game(game) = &mut self.stage else {
            return;
        };
        let report = game.session.run_frame(ui);
        if let Some(step) = self.with_engine(|guard, engine| guard.tick(now_ns, engine)) {
            self.on_guard(step, now_ns);
        }
        match report {
            Ok(report) => {
                self.menu_hotkey |= report.menu_opened;
                if report.blind_recovery.is_some() {
                    self.recover(now_ns);
                }
                self.notice_picture();
                if let Some(reason) = report.ended {
                    self.end_by_core(reason);
                } else if report.blanked {
                    (self.runtime.pace)(now_ns.saturating_add(UNPACED_FRAME_NS));
                }
            }
            // One rule for a stalled display (`SessionError::DisplayStalled`),
            // a failed core and an ended session: the game ends, and the
            // run relaunches to the game list, which opens the display
            // again in a new process.
            Err(e) => self.end_game(EndNotice::First(format!("The game stopped: {e}."))),
        }
    }

    /// Ends the game that the core asked to stop, always with a notice
    /// and the words of the core: S2 for a game that did not start, with
    /// the error lines of the launch in the log, and S3 for a game that
    /// ran.
    fn end_by_core(&mut self, reason: EndReason) {
        let Stage::Game(game) = &self.stage else {
            return;
        };
        let name = super::boot::game_name(&game.reference.path);
        let session = &game.session;
        let (notice, lines) = match reason {
            EndReason::FailedStart => {
                let errors = session.start_errors();
                // A core that loads in silence can log its reason in the
                // frame in which it stops.
                let said = session.last_message().or(session.last_error());
                let text = start_failure::stopped_notice(&name, start_failure::first(errors, said));
                (EndNotice::First(text), start_failure::core_log(errors))
            }
            EndReason::CoreShutdown => {
                let said = session.last_message().or(session.last_error());
                let text = start_failure::ended_notice(&name, said);
                (EndNotice::AfterSave(text), Vec::new())
            }
        };
        for line in lines {
            self.log(line);
        }
        if let EndNotice::First(text) | EndNotice::AfterSave(text) = &notice {
            self.log(text.clone());
        }
        self.end_game(notice);
    }

    /// Updates the snapshot, runs the UI on the actions of the frame,
    /// carries out its intents and draws the canvas.
    fn update_ui(&mut self, now_ns: u64) {
        self.refresh_view(now_ns);
        self.screens.update(&self.snap, &self.actions, &mut self.ui);
        if let Some(page) = self.screens.take_open_failure() {
            let label = neptron_ui::screens::SETTINGS_PANELS
                .iter()
                .find(|p| p.page == page)
                .map_or("The panel", |p| p.label);
            self.notice(
                NoticeKind::Info,
                format!("{label} did not open here. Open it from Settings."),
            );
        }
        self.carry_out(now_ns);
        if self.screens.shows_ui() {
            self.canvas.paint(&self.ui.scene);
        }
    }

    /// Takes a finished scan of the game list: the new systems, the game
    /// list on show updated in place, the cache for the next start, and a
    /// notice of what changed.
    fn poll_library(&mut self) {
        let Some(result) = self.library.poll() else {
            return;
        };
        match result {
            Ok(scanned) => {
                let job = Job::Cache {
                    index: self.library.shared_index(),
                    full_scan: scanned.kind != ScanKind::Check,
                };
                self.outbox.send(job);
                self.snap.systems = self.library.systems();
                // The rows of the same list around the old ones, so the
                // screen keeps its place, and the ids let it keep its game
                // when it moved by up to the margin.
                if let Some(source) = self.snap.games.source {
                    let first = self.snap.games.first.saturating_sub(MOVED_GAME_MARGIN_ROWS);
                    let count = self.snap.games.rows.len() + 2 * MOVED_GAME_MARGIN_ROWS;
                    self.snap.games = self.library.window(source, first, count);
                }
                if let Some(text) = scan_notice(&scanned) {
                    self.notice(NoticeKind::Info, text);
                }
            }
            Err(e) => self.notice(NoticeKind::Warning, format!("The game scan failed: {e}.")),
        }
        self.reload_page();
    }

    /// Asks for a read of the network status when the Network panel comes
    /// on screen and every `NETWORK_REFRESH_NS` while it stays there, and
    /// takes a changed status: the panel shows it with no need to leave it.
    /// The snapshot keeps the page after the panel closes, so the screen on
    /// show of the last frame decides. The frame thread never waits for the
    /// reader.
    /// Builds the Input page again when a pad came, went or moved while
    /// the page is in the snapshot, so the open panel shows it; takes the
    /// notices of the input source; and follows the silent pad rule, which
    /// tells only in the game list. The events of the last frame give the
    /// presses. With no change and no notice, nothing allocates.
    fn poll_pads(&mut self, now_ns: u64) {
        let (changes, activity, presses, check) = match &self.stage {
            Stage::List(list) => {
                list.input.take_notices(&mut self.input_notices);
                let presses = button_presses(&self.events);
                (
                    list.input.roster_changes(),
                    list.input.activity(),
                    presses,
                    true,
                )
            }
            Stage::Game(game) => {
                let input = game.session.input();
                input.take_notices(&mut self.input_notices);
                let presses = button_presses(game.session.input_events());
                (input.roster_changes(), input.activity(), presses, false)
            }
            Stage::Ended => return,
        };
        if changes != self.pad_changes {
            self.pad_changes = changes;
            let page = self.snap.settings.page;
            let pad_page = page.and_then(crate::pad_page::port_of_page).is_some();
            if self.snap.settings.is_for(INPUT_PAGE) || pad_page {
                self.reload_page();
            }
        }
        if !self.input_notices.is_empty() {
            self.take_input_notices();
        }
        if let Some(pad) = self.pad_notes.silent(&activity, presses, now_ns, check) {
            let roster = self.roster();
            let name = roster.get(pad.port).map_or("", |e| e.pad.name.as_str());
            self.notice(NoticeKind::Info, silent_text(name));
        }
        if self.pad_notes.take_changed() {
            self.outbox.send(Job::PadNotes {
                text: self.pad_notes.to_text(),
            });
        }
    }

    /// Logs each notice of the input source, and shows the ones a player
    /// needs. A pad is told once its player answers its notice, so a
    /// notice that waits in a game for the menu, when the run ends first,
    /// shows in the next run. The notice of a pad with the generic layout
    /// has the second button that opens the Input panel on its row.
    fn take_input_notices(&mut self) {
        let mut notices = std::mem::take(&mut self.input_notices);
        for notice in &notices {
            self.log(format!("input: {notice}"));
            let Some(player) = self.pad_notes.player_notice(notice) else {
                continue;
            };
            let choice = (player.kind == NoteKind::Generic).then_some(SET_UP_CONTROLLER);
            let id = self.push_notice(NoticeKind::Info, player.text.clone(), choice);
            self.pad_notes.show(id, &player);
        }
        notices.clear();
        self.input_notices = notices;
    }

    fn poll_network(&mut self, now_ns: u64) {
        let Some(watch) = &self.network else {
            return;
        };
        let shown = self.ui.tree.screen() == NETWORK_SCREEN;
        let opened = shown && !self.network_shown;
        self.network_shown = shown;
        if opened || (shown && now_ns >= self.network_next_ns) {
            watch.request();
            self.network_requests += 1;
            self.network_next_ns = now_ns.saturating_add(NETWORK_REFRESH_NS);
        }
        let taken = watch.poll();
        // A status taken while the panel was closed is not on the page yet.
        if opened && taken.is_none() && self.snap.settings.is_for(NETWORK_PAGE) {
            self.load_page(NETWORK_PAGE);
        }
        let Some(info) = taken else {
            return;
        };
        if let NetworkInfo::Unknown { reason } = &info {
            self.log(format!("network: {reason}"));
        }
        self.network_info = Some(info);
        if shown {
            self.load_page(NETWORK_PAGE);
        }
    }

    /// A notice when the first picture of the game is not exact: the
    /// reasons of the policy (soft, small, speed, double refresh) in the
    /// texts of the app, or the words of the video status for a fit or a
    /// crop with no reason of its own. The first time a vertical game shows
    /// interlaced on a 15 kHz screen, also the one-time notice that says
    /// how to change it; the writer thread records it as shown.
    fn notice_picture(&mut self) {
        if self.picture_noticed {
            return;
        }
        let Stage::Game(game) = &self.stage else {
            return;
        };
        let Some(current) = game.session.video().current() else {
            return;
        };
        self.picture_noticed = true;
        let Some(notice) = current.chosen.notice.as_ref() else {
            return;
        };
        let kind = current.chosen.proposal.timing.target.kind;
        let mut status = neptron_video::VideoStatusText::default();
        let words = game
            .session
            .video_status(&mut status)
            .map_or("", |s| s.result.as_str());
        let text = picture_text(kind, notice, words);
        let vertical = self.config.policy().map(|p| p.vertical);
        let first_vertical = vertical.is_ok_and(|v| {
            vertical_interlaced(
                &current.source,
                &current.read_back,
                &self.guard.active().monitor.profile().kind,
                v,
            )
        });
        if let Some(text) = text {
            self.notice(NoticeKind::Warning, text);
        }
        if first_vertical && self.shown_notices.insert(VERTICAL_INTERLACED) {
            self.notice(NoticeKind::Info, VERTICAL_INTERLACED_TEXT.to_owned());
            self.outbox.send(Job::Notices {
                text: self.shown_notices.to_text(),
            });
        }
    }
}

/// The notice after a scan: what changed, after a scan the player asked
/// for always, and after the scan of a start only when an index from the
/// cache changed. The first scan of a library adds every game, which
/// tells nothing.
fn scan_notice(scanned: &Scanned) -> Option<String> {
    let text = changes_text(&scanned.changes);
    match scanned.kind {
        ScanKind::Rescan => Some(text.unwrap_or_else(|| NO_CHANGE.to_owned())),
        ScanKind::Boot | ScanKind::Check if scanned.first => None,
        ScanKind::Boot | ScanKind::Check => text,
    }
}

/// The rows that the game list on show gets before and after its old rows
/// when a scan ends. A focused game that moves by more rows (more games
/// added or removed above it) loses the focus, and the cursor stays at its
/// index.
const MOVED_GAME_MARGIN_ROWS: usize = 64;

/// The notice of a rescan that found nothing new.
const NO_CHANGE: &str = "No games added or removed.";

/// The button presses among `events`: a value that is not 0 on a control
/// that is not a stick axis.
fn button_presses(events: &[neptron_types::InputEvent]) -> u32 {
    let count = events
        .iter()
        .filter(|e| e.value != 0)
        .filter(|e| {
            !matches!(
                e.control,
                Control::LeftX | Control::LeftY | Control::RightX | Control::RightY
            )
        })
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The UI canvas as the `ui` frame of a scene.
fn canvas_frame(canvas: &CpuCanvas) -> Frame<'_> {
    Frame {
        data: canvas.as_bytes(),
        width: UI_WIDTH as u16,
        height: UI_HEIGHT as u16,
        pitch: canvas.pitch(),
        format: PixelFormat::Xrgb8888,
    }
}

#[cfg(test)]
mod tests {
    /// A TV shows no `speed` word for a change the audio rate control
    /// absorbs: the status of the video crate and the audio crate must use
    /// one bound. The app depends on both, and neither may depend on the
    /// other.
    #[test]
    fn the_tv_speed_bound_is_the_audio_rate_control_bound() {
        assert_eq!(
            neptron_video::TV_SPEED_SILENT_PPM,
            neptron_audio::MAX_RATE_DEVIATION_PPM
        );
    }
}
