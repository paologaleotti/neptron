//! The screensaver of the menus: the idle timer of the UI, fed with the
//! input of each frame, and its picture in the snapshot.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::PadRoster;
use neptron_types::InputEvent;
use neptron_ui::screensaver::{Idle, IdleStep};

use super::{Shell, Stage};

/// The screensaver of a run: the idle timer and the time setting.
#[derive(Debug, Clone, Copy)]
pub(super) struct Screensaver {
    idle: Idle,
    /// The time with no input before it starts; `None` with the setting
    /// off.
    timeout_ns: Option<u64>,
}

impl Screensaver {
    /// A screensaver whose timer starts at `now_ns`.
    pub(super) fn new(now_ns: u64, timeout_ns: Option<u64>) -> Self {
        Self {
            idle: Idle::new(now_ns),
            timeout_ns,
        }
    }

    /// Takes a new time setting. The timer keeps its start.
    pub(super) fn set_timeout(&mut self, timeout_ns: Option<u64>) {
        self.timeout_ns = timeout_ns;
    }
}

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// One frame of the screensaver, after the navigation of the frame and
    /// before the UI. It may show in the menus and in a paused game, and
    /// never under a dialog with a timeout: the confirm of a signal change
    /// or a guided mapping. The input that wakes it is consumed: the UI
    /// gets no action in this frame, and a control still held gives none
    /// until its release.
    pub(super) fn step_screensaver(&mut self, now_ns: u64) {
        let (input, menus) = match &self.stage {
            Stage::List(_) => (wakes(&self.events), true),
            Stage::Game(game) => (wakes(game.session.input_events()), self.screens.shows_ui()),
            Stage::Ended => (false, false),
        };
        let allowed = menus && self.guard.pending().is_none() && self.mapping.is_none();
        let saver = &mut self.screensaver;
        let step = saver.idle.step(now_ns, input, allowed, saver.timeout_ns);
        if step == IdleStep::Woken {
            self.actions.clear();
            self.nav.hold_back();
        }
        self.snap.screensaver = match step {
            IdleStep::Shows { elapsed_ns } => Some(elapsed_ns),
            IdleStep::Hidden | IdleStep::Woken => None,
        };
        if let Stage::Game(game) = &mut self.stage {
            game.session.hide_picture(self.snap.screensaver.is_some());
        }
    }
}

/// True when `events` hold a wake input: a press, or a stick out of its
/// dead zone. The input layer applies the dead zone, so an axis value of
/// 0 is a stick at rest, as a button value of 0 is a release.
fn wakes(events: &[InputEvent]) -> bool {
    events.iter().any(|e| e.value != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neptron_types::Control;

    fn event(control: Control, value: i16) -> InputEvent {
        InputEvent {
            device: 3,
            control,
            value,
        }
    }

    #[test]
    fn a_press_or_a_moved_stick_wakes_and_a_release_does_not() {
        assert!(wakes(&[event(Control::A, 1)]));
        assert!(wakes(&[event(Control::Up, 1)]));
        assert!(wakes(&[event(Control::LeftX, -9_000)]));
        assert!(!wakes(&[event(Control::A, 0), event(Control::LeftY, 0)]));
        assert!(!wakes(&[]));
    }
}
