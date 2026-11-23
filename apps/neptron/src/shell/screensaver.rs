//! The screensaver of the menus: the idle timer of the UI, fed with the
//! input of each frame, and its picture in the snapshot.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::PadRoster;
use neptron_types::{Control, InputEvent};
use neptron_ui::screensaver::{Idle, IdleStep};

use super::{Shell, Stage};

/// The stick axes that [`Sticks`] follows at once: four axes on each of
/// eight devices.
const STICK_AXES_MAX: usize = 32;

/// The screensaver of a run: the idle timer and the time setting.
#[derive(Debug, Clone, Copy)]
pub(super) struct Screensaver {
    idle: Idle,
    /// The time with no input before it starts; `None` with the setting
    /// off.
    timeout_ns: Option<u64>,
    sticks: Sticks,
}

/// The stick axes out of their dead zone now. A stick that drifts or
/// jitters outside the dead zone sends an event at each change; only its
/// move out of the dead zone is a wake input, so a worn stick does not
/// hold the screensaver off for good.
#[derive(Debug, Clone, Copy)]
struct Sticks {
    out: [(u32, Control); STICK_AXES_MAX],
    len: usize,
}

impl Screensaver {
    /// A screensaver whose timer starts at `now_ns`.
    pub(super) fn new(now_ns: u64, timeout_ns: Option<u64>) -> Self {
        Self {
            idle: Idle::new(now_ns),
            timeout_ns,
            sticks: Sticks::new(),
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
    /// until its release. An action of the frame counts as input too: a
    /// held direction repeats with no new event.
    pub(super) fn step_screensaver(&mut self, now_ns: u64) {
        let saver = &mut self.screensaver;
        let (events, menus) = match &self.stage {
            Stage::List(_) => (saver.sticks.wakes(&self.events), true),
            Stage::Game(game) => (
                saver.sticks.wakes(game.session.input_events()),
                game.session.paused() && self.screens.shows_ui(),
            ),
            Stage::Ended => (false, false),
        };
        let input = events || !self.actions.is_empty();
        let allowed = menus && self.guard.pending().is_none() && self.mapping.is_none();
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

impl Sticks {
    fn new() -> Self {
        Self {
            out: [(0, Control::LeftX); STICK_AXES_MAX],
            len: 0,
        }
    }

    /// True when `events` hold a wake input: a button press, or a stick
    /// axis that leaves its dead zone. The input layer applies the dead
    /// zone, so an axis value of 0 is a stick at rest, as a button value of
    /// 0 is a release. With the table full, every axis event out of the
    /// dead zone wakes.
    fn wakes(&mut self, events: &[InputEvent]) -> bool {
        let mut wake = false;
        for event in events {
            let axis = matches!(
                event.control,
                Control::LeftX | Control::LeftY | Control::RightX | Control::RightY
            );
            if !axis {
                wake |= event.value != 0;
                continue;
            }
            let key = (event.device, event.control);
            let at = self.out[..self.len].iter().position(|k| *k == key);
            match (event.value != 0, at) {
                (true, None) => {
                    wake = true;
                    if let Some(slot) = self.out.get_mut(self.len) {
                        *slot = key;
                        self.len += 1;
                    }
                }
                (false, Some(i)) => {
                    self.len -= 1;
                    self.out.swap(i, self.len);
                }
                (true, Some(_)) | (false, None) => {}
            }
        }
        wake
    }
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
        let mut sticks = Sticks::new();
        assert!(sticks.wakes(&[event(Control::A, 1)]));
        assert!(sticks.wakes(&[event(Control::Up, 1)]));
        assert!(sticks.wakes(&[event(Control::LeftX, -9_000)]));
        assert!(!sticks.wakes(&[event(Control::A, 0), event(Control::LeftY, 0)]));
        assert!(!sticks.wakes(&[]));
    }

    #[test]
    fn a_stick_that_rests_outside_its_dead_zone_wakes_once() {
        let mut sticks = Sticks::new();
        assert!(sticks.wakes(&[event(Control::RightY, 4_100)]));
        for value in [4_000, 4_300, 3_900, -4_000, 4_100] {
            assert!(!sticks.wakes(&[event(Control::RightY, value)]), "{value}");
        }
        assert!(!sticks.wakes(&[event(Control::RightY, 0)]), "back at rest");
        assert!(
            sticks.wakes(&[event(Control::RightY, 20_000)]),
            "a real push"
        );
        let other_pad = InputEvent {
            device: 4,
            ..event(Control::RightY, 9_000)
        };
        assert!(
            sticks.wakes(&[other_pad]),
            "another device has its own axis"
        );
    }

    #[test]
    fn a_full_table_still_wakes_on_a_stick_out_of_its_dead_zone() {
        let mut sticks = Sticks::new();
        for device in 0..u32::try_from(STICK_AXES_MAX).expect("a small count") {
            let push = InputEvent {
                device,
                control: Control::LeftX,
                value: 9_000,
            };
            assert!(sticks.wakes(&[push]));
        }
        let more = InputEvent {
            device: 99,
            control: Control::LeftX,
            value: 9_000,
        };
        assert!(sticks.wakes(&[more]));
        assert!(sticks.wakes(&[more]), "with no slot, each event wakes");
    }
}
