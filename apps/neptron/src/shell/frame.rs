//! One frame of the loop, in the game list and in a game.
//!
//! Each frame reads the clock once and gives that time to every call of
//! the guard, the hotkeys and the navigation of the frame.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::Hotkey;
use neptron_types::{Frame, PixelFormat};
use neptron_ui::snapshot::NoticeKind;
use neptron_ui::{CpuCanvas, UI_HEIGHT, UI_WIDTH, UiAction};
use neptron_video::RefreshKind;

use super::guard_events::any_press;
use super::{MENU_SOURCE, Outcome, Shell, Stage, UNPACED_FRAME_NS};
use crate::outbox::Job;

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Shell<D, S, I, C> {
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
        self.poll_library();
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
        if self.offer_panel(any_press(&self.events), now_ns) {
            self.events.clear();
        }
        self.nav.update(&self.events, now_ns, &mut self.actions);
        self.update_ui(now_ns);
        if self.outcome.is_some() {
            return;
        }
        let shown = match &mut self.stage {
            Stage::List(list) => {
                list.engine.begin_frame(MENU_SOURCE);
                let presented = list.engine.present(None, Some(canvas_frame(&self.canvas)));
                presented.is_ok() && list.engine.wait_flip().is_ok()
            }
            _ => true,
        };
        if let Some(step) = self.with_engine(|guard, engine| guard.tick(now_ns, engine)) {
            self.on_guard(step, now_ns);
        }
        if !shown {
            (self.runtime.pace)(now_ns.saturating_add(UNPACED_FRAME_NS));
        }
    }

    fn game_frame(&mut self, now_ns: u64) {
        self.actions.clear();
        if std::mem::take(&mut self.menu_hotkey) {
            self.actions.push(UiAction::Menu);
        }
        let pressed = match &self.stage {
            Stage::Game(game) => any_press(game.session.input_events()),
            _ => false,
        };
        let offered = self.offer_panel(pressed, now_ns);
        if let Stage::Game(game) = &self.stage
            && !offered
        {
            let start = self.actions.len();
            self.nav
                .update(game.session.input_events(), now_ns, &mut self.actions);
            if !self.screens.shows_ui() {
                self.actions.truncate(start);
            }
        }
        self.update_ui(now_ns);
        let ui = self.screens.shows_ui().then(|| canvas_frame(&self.canvas));
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
                if report.ended.is_some() {
                    self.end_game(None);
                } else if report.blanked {
                    (self.runtime.pace)(now_ns.saturating_add(UNPACED_FRAME_NS));
                }
            }
            // One rule for a stalled display (`SessionError::DisplayStalled`),
            // a failed core and an ended session: the game ends, and the
            // run relaunches to the game list, which opens the display
            // again in a new process.
            Err(e) => self.end_game(Some(format!("The game stopped: {e}."))),
        }
    }

    /// Updates the snapshot, runs the UI on the actions of the frame,
    /// carries out its intents and draws the canvas.
    fn update_ui(&mut self, now_ns: u64) {
        self.refresh_view(now_ns);
        self.screens.update(&self.snap, &self.actions, &mut self.ui);
        self.carry_out(now_ns);
        if self.screens.shows_ui() {
            self.ui.render(&mut self.canvas);
        }
    }

    /// Takes a finished scan of the game list: the new systems, and the
    /// cache for the next start.
    fn poll_library(&mut self) {
        let Some(result) = self.library.poll() else {
            return;
        };
        match result {
            Ok(index) => {
                let job = Job::Cache(Box::new(index.clone()));
                self.outbox.send(job);
                self.snap.systems = self.library.systems();
                self.snap.games = Default::default();
            }
            Err(e) => self.notice(NoticeKind::Warning, format!("The game scan failed: {e}.")),
        }
    }

    /// A notice when the first picture of the game is filtered, cropped or
    /// small: the policy's notice, in the words of the video status.
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
        // A flat panel scales with the sharp filter by design: that is not
        // a notice, as the video status shows no alert for it.
        let panel = current.chosen.proposal.timing.target.kind == RefreshKind::Panel;
        let worth = current.chosen.notice.as_ref().is_some_and(|n| {
            (n.fitted && !panel) || n.cropped_lines > 0 || n.padded_fill_permille.is_some()
        });
        if !worth {
            return;
        }
        let mut status = neptron_video::VideoStatusText::default();
        if let Some(status) = game.session.video_status(&mut status) {
            let text = format!("Picture: {}.", status.result);
            self.notice(NoticeKind::Warning, text);
        }
    }
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
