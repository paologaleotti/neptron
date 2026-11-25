//! One frame of the loop, in the game list and in a game.
//!
//! Each frame reads the clock once and gives that time to every call of
//! the guard, the hotkeys and the navigation of the frame.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::Hotkey;
use neptron_input::PadRoster;
use neptron_session::{EndReason, PacingChange};
use neptron_types::{Control, Frame, PixelFormat};
use neptron_ui::screens::{INPUT_PAGE, KEEP_SCREEN, NETWORK_PAGE, NETWORK_SCREEN};
use neptron_ui::snapshot::NoticeKind;
use neptron_ui::{CpuCanvas, Form, SceneCanvas, UiAction};

use super::intents::EndNotice;
use super::picture::{declined_text, picture_text, scanline_text, vertical_interlaced};
use super::{
    MENU_SOURCE, Outcome, PICTURE_DECISIONS_PER_FRAME, Shell, Stage, UNPACED_FRAME_NS,
    proposal_hash,
};

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
        self.step_picture_answers();
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

    /// Makes a few decisions of the picture answers that the page on show
    /// or the keep prompt waits for, and loads the page again when they are
    /// there. The frame thread spends at most `PICTURE_DECISIONS_PER_FRAME`
    /// decisions on them, and only while a menu shows: in the game list,
    /// or in a paused game. A game that runs makes no answer. The keep
    /// prompt goes first while it shows, else the page. In a paused game
    /// the snapshot says which levels of the keep prompt wait.
    fn step_picture_answers(&mut self) {
        let paused = matches!(&self.stage, Stage::Game(game) if game.session.paused());
        if !paused && !matches!(self.stage, Stage::List(_)) {
            return;
        }
        let keeps = if paused {
            self.keep_asks.as_slice()
        } else {
            &[]
        };
        if self.page_ask.is_none() && keeps.is_empty() {
            return;
        }
        let prompt = self.ui.tree.screen() == KEEP_SCREEN;
        let page = self.page_ask.iter();
        let keep = keeps.iter().map(|(_, request)| request);
        let ready = self.engine_ref().is_some_and(|engine| {
            if prompt {
                engine.step_answers(keep.chain(page), PICTURE_DECISIONS_PER_FRAME)
            } else {
                engine.step_answers(page.chain(keep), PICTURE_DECISIONS_PER_FRAME)
            }
        });
        if paused {
            let waits = self.keep_waits();
            if let Some(info) = &mut self.snap.session {
                info.keep_waits = waits;
            }
        }
        if ready && self.page_ask.is_some() {
            self.reload_page();
        }
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
        self.step_screensaver(now_ns);
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
            // A direction held in the game would repeat in the menu and
            // can reach `Quit game`, which quits with no question.
            self.nav.hold_back();
        }
        if let Stage::Game(game) = &self.stage {
            let start = self.actions.len();
            self.nav
                .update(game.session.input_events(), now_ns, &mut self.actions);
            if !self.screens.shows_ui() {
                self.actions.truncate(start);
            }
        }
        self.step_screensaver(now_ns);
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
                if let Some(change) = report.pacing {
                    self.log(pacing_line(change).to_owned());
                }
                self.menu_hotkey |= report.menu_opened;
                if report.blind_recovery.is_some() {
                    self.recover(now_ns);
                }
                self.notice_picture();
                self.notice_speed(now_ns);
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
            Err(e) => self.end_game(EndNotice::AfterSave(format!("The game stopped: {e}."))),
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
                let mut lines = start_failure::core_log(errors);
                lines.extend(start_failure::lost_lines(session.start_dropped_logs()));
                (EndNotice::First(text), lines)
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
        self.sync_form();
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
        // An intent of this frame turned the screen: the frame draws the UI
        // again in the new form, so the scene it presents has one form.
        if self.sync_form() {
            self.screens.update(&self.snap, &[], &mut self.ui);
            self.carry_out(now_ns);
        }
        if self.screens.shows_ui() {
            self.canvas.paint(&self.ui.scene);
        }
    }

    /// Gives the UI the form of the engine of the stage, and the canvas of
    /// that form. The one path from the engine's form to the UI: at boot
    /// and in every frame. True when the form changed.
    pub(super) fn sync_form(&mut self) -> bool {
        let Some(form) = self.engine_ref().map(|e| ui_form(e.form())) else {
            return false;
        };
        if form == self.screens.form() {
            return false;
        }
        self.screens.set_form(form);
        let size = form.size();
        let next = match self.spare_canvas.take() {
            Some(canvas) if canvas.canvas().size() == size => canvas,
            _ => SceneCanvas::with_size(size),
        };
        self.spare_canvas = Some(std::mem::replace(&mut self.canvas, next));
        self.log(format!("ui: the {form:?} form"));
        true
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

    /// A notice when the first picture of the game, or the first new
    /// picture after a change of the picture mode or the refresh choice,
    /// is not exact: the reasons of the policy in the texts of the app, or
    /// the words of the video status for a fit or a crop with no reason of
    /// its own. Also, from the same `offer` as the menus: a stored mode or
    /// refresh choice that this display does not give, with what shows in
    /// its place, and scanlines that are on but not drawn, with why. The
    /// first time a vertical game shows interlaced on a 15 kHz screen, also
    /// the one-time notice that says how to change it; the writer thread
    /// records it as shown.
    fn notice_picture(&mut self) {
        let Stage::Game(game) = &self.stage else {
            return;
        };
        let Some(current) = game.session.video().current() else {
            return;
        };
        let settled = game.session.video().settled();
        let (next, show) = self
            .picture_notice
            .step(proposal_hash(&current.chosen.proposal), settled);
        self.picture_notice = next;
        if !show {
            return;
        }
        let kind = current.chosen.proposal.timing.target.kind;
        let mut status = neptron_video::VideoStatusText::default();
        let (words, shown) = game
            .session
            .video_status(&mut status)
            .map_or(("", ""), |s| (s.result.as_str(), s.mode.trim_end()));
        let mut texts: Vec<String> = Vec::new();
        let answer = game.session.video().offer(&game.session.source()).ok();
        let policy = self.config.policy().ok();
        if let (Some(Err(declined)), Some(policy)) = (&answer, policy) {
            texts.push(declined_text(policy, declined.reason, shown));
        }
        if let Some(text) = current
            .chosen
            .notice
            .as_ref()
            .and_then(|notice| picture_text(kind, notice, words))
        {
            texts.push(text);
        }
        let scanlines = self.config.scanlines().ok();
        let outcome = match &answer {
            Some(Ok(offered)) => Some(offered.scanlines),
            Some(Err(declined)) => declined.scanlines,
            None => None,
        };
        if let (Some(outcome), Some(scanlines)) = (outcome, scanlines)
            && let Some(text) = scanline_text(scanlines, outcome)
        {
            texts.push(text.to_owned());
        }
        let vertical = policy.map(|p| p.read_vertical());
        let first_vertical = vertical.is_some_and(|v| {
            vertical_interlaced(
                &current.source,
                &current.read_back,
                &self.guard.active().monitor.profile().kind,
                v,
            )
        });
        if !texts.is_empty() {
            self.notice(NoticeKind::Warning, texts.join(" "));
        }
        if first_vertical && self.shown_notices.insert(VERTICAL_INTERLACED) {
            self.notice(NoticeKind::Info, VERTICAL_INTERLACED_TEXT.to_owned());
            self.outbox.send(Job::Notices {
                text: self.shown_notices.to_text(),
            });
        }
    }

    /// A notice when the game runs slower than its rate, and when its
    /// sound turns off below half speed (`SpeedNotices::step`). Only the
    /// frame that shows a notice allocates its text.
    fn notice_speed(&mut self, now_ns: u64) {
        let Stage::Game(game) = &self.stage else {
            return;
        };
        let slow = game.session.slow_speed_percent();
        let sound_off = game.session.sound_too_slow();
        if let Some(notice) = self.speed_notices.step(slow, sound_off, now_ns) {
            self.notice(NoticeKind::Warning, notice.text());
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

/// The form of the UI for the form of the video contract. The two crates
/// may not depend on each other, so the app joins them.
pub(super) fn ui_form(form: neptron_types::Form) -> Form {
    match form {
        neptron_types::Form::Landscape => Form::Landscape,
        neptron_types::Form::Portrait => Form::Portrait,
    }
}

/// The UI canvas as the `ui` frame of a scene, of the size of the canvas.
fn canvas_frame(canvas: &CpuCanvas) -> Frame<'_> {
    let size = canvas.size();
    // A canvas has the size of a form, which fits a u16; a side of 0 would
    // only show no UI.
    let side = |pixels: u32| u16::try_from(pixels).unwrap_or(0);
    Frame {
        data: canvas.as_bytes(),
        width: side(size.width()),
        height: side(size.height()),
        pitch: canvas.pitch(),
        format: PixelFormat::Xrgb8888,
    }
}

/// The log line of a change of the pacing of the frame loop.
fn pacing_line(change: PacingChange) -> &'static str {
    match change {
        PacingChange::Late => {
            "late pacing: a frame of the game did not fit in its flip interval, and the next frame starts at once"
        }
        PacingChange::Wait => {
            "wait pacing: the frames of the game fit in their flip interval again"
        }
    }
}

#[cfg(test)]
mod tests {
    use neptron_ui::{CpuCanvas, Form, Size};

    use super::{canvas_frame, ui_form};

    /// The frame of a canvas has the size of the canvas, in both forms.
    #[test]
    fn the_canvas_frame_has_the_size_of_the_canvas() {
        for form in [Form::Landscape, Form::Portrait] {
            let canvas = CpuCanvas::with_size(form.size());
            let frame = canvas_frame(&canvas);
            let size = (u32::from(frame.width), u32::from(frame.height));
            assert_eq!(size, (form.size().width(), form.size().height()));
            assert_eq!(frame.pitch * usize::from(frame.height), frame.data.len());
        }
    }

    /// The UI and the video contract each name the forms; the app joins
    /// them, so each form has one canvas size on both sides.
    #[test]
    fn each_form_has_the_same_canvas_size_in_the_ui_and_the_contract() {
        let pairs = [
            (Form::Landscape, neptron_types::Form::Landscape),
            (Form::Portrait, neptron_types::Form::Portrait),
        ];
        for (ui, contract) in pairs {
            let (w, h) = contract.canvas();
            assert_eq!(Size::new(u32::from(w), u32::from(h)), Ok(ui.size()));
            assert_eq!(ui_form(contract), ui);
        }
    }

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
