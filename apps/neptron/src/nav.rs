//! The navigation of the menus: RetroPad buttons to UI actions, with key
//! repeat for the directions.

use neptron_types::{Control, InputEvent};
use neptron_ui::UiAction;

/// The time a direction is held before it repeats, in nanoseconds. It is
/// below the 60 quiet frames that the confirm dialog waits for at 120 Hz
/// (500 ms), as OP-048 asks.
pub const REPEAT_DELAY_NS: u64 = 400_000_000;

/// The time between two repeats of a held direction, in nanoseconds.
pub const REPEAT_INTERVAL_NS: u64 = 66_000_000;

/// The controls the navigator follows at once. A press beyond it is
/// ignored until a control is released.
const HELD_MAX: usize = 32;

/// The menu action of a RetroPad control: the D-pad moves, A accepts, B
/// goes back and X is the menu button. Start and Select are left out:
/// together they are the quick menu and the blind recovery hold.
pub fn action_of(control: Control) -> Option<UiAction> {
    match control {
        Control::Up => Some(UiAction::Up),
        Control::Down => Some(UiAction::Down),
        Control::Left => Some(UiAction::Left),
        Control::Right => Some(UiAction::Right),
        Control::A => Some(UiAction::Accept),
        Control::B => Some(UiAction::Back),
        Control::X => Some(UiAction::Menu),
        _ => None,
    }
}

/// Turns the input events of each frame into UI actions.
///
/// A press gives its action once. A held direction repeats after
/// [`REPEAT_DELAY_NS`], then every [`REPEAT_INTERVAL_NS`], at most once per
/// frame. [`Navigator::hold_back`] stops every repeat and ignores the
/// controls held at that moment until their release (OP-048): a key held
/// when a confirmation appears never reaches it.
#[derive(Debug, Clone, Default)]
pub struct Navigator {
    held: Vec<Held>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Held {
    device: u32,
    control: Control,
    next_repeat_ns: u64,
    ignored: bool,
}

impl Navigator {
    /// A navigator with no control held.
    pub fn new() -> Self {
        Self {
            held: Vec::with_capacity(HELD_MAX),
        }
    }

    /// Takes the events of one frame at `now_ns` and appends the actions.
    pub fn update(&mut self, events: &[InputEvent], now_ns: u64, out: &mut Vec<UiAction>) {
        for event in events {
            let Some(action) = action_of(event.control) else {
                continue;
            };
            let at = self
                .held
                .iter()
                .position(|h| h.device == event.device && h.control == event.control);
            match (event.value != 0, at) {
                (true, None) if self.held.len() < HELD_MAX => {
                    self.held.push(Held {
                        device: event.device,
                        control: event.control,
                        next_repeat_ns: now_ns.saturating_add(REPEAT_DELAY_NS),
                        ignored: false,
                    });
                    out.push(action);
                }
                (false, Some(i)) => {
                    self.held.swap_remove(i);
                }
                _ => {}
            }
        }
        for held in self.held.iter_mut().filter(|h| !h.ignored) {
            let repeats = matches!(
                held.control,
                Control::Up | Control::Down | Control::Left | Control::Right
            );
            if repeats && now_ns >= held.next_repeat_ns {
                held.next_repeat_ns = now_ns.saturating_add(REPEAT_INTERVAL_NS);
                if let Some(action) = action_of(held.control) {
                    out.push(action);
                }
            }
        }
    }

    /// Stops every repeat and ignores every control held now until it is
    /// released.
    pub fn hold_back(&mut self) {
        for held in &mut self.held {
            held.ignored = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn event(control: Control, value: i16) -> InputEvent {
        InputEvent {
            device: 0,
            control,
            value,
        }
    }

    fn step(nav: &mut Navigator, events: &[InputEvent], now_ns: u64) -> Vec<UiAction> {
        let mut out = Vec::new();
        nav.update(events, now_ns, &mut out);
        out
    }

    #[test]
    fn buttons_map_to_actions() {
        let mut nav = Navigator::new();
        let presses = [
            event(Control::A, 1),
            event(Control::B, 1),
            event(Control::X, 1),
            event(Control::Start, 1),
            event(Control::Select, 1),
        ];
        assert_eq!(
            step(&mut nav, &presses, 0),
            [UiAction::Accept, UiAction::Back, UiAction::Menu]
        );
        assert!(
            step(&mut nav, &[], 2_000 * MS).is_empty(),
            "buttons do not repeat"
        );
    }

    #[test]
    fn a_held_direction_repeats_after_the_delay() {
        let mut nav = Navigator::new();
        assert_eq!(
            step(&mut nav, &[event(Control::Down, 1)], 0),
            [UiAction::Down]
        );
        assert!(step(&mut nav, &[], 399 * MS).is_empty());
        assert_eq!(step(&mut nav, &[], 400 * MS), [UiAction::Down]);
        assert!(step(&mut nav, &[], 465 * MS).is_empty());
        assert_eq!(step(&mut nav, &[], 466 * MS), [UiAction::Down]);
        assert!(step(&mut nav, &[event(Control::Down, 0)], 600 * MS).is_empty());
        assert!(step(&mut nav, &[], 2_000 * MS).is_empty());
    }

    #[test]
    fn a_held_key_is_ignored_after_a_hold_back_until_its_release() {
        let mut nav = Navigator::new();
        let _ = step(&mut nav, &[event(Control::Right, 1)], 0);
        nav.hold_back();
        assert!(step(&mut nav, &[], 1_000 * MS).is_empty(), "no repeat");
        assert!(
            step(&mut nav, &[event(Control::Right, 1)], 1_100 * MS).is_empty(),
            "a press event of a held key gives nothing"
        );
        let _ = step(&mut nav, &[event(Control::Right, 0)], 1_200 * MS);
        assert_eq!(
            step(&mut nav, &[event(Control::Right, 1)], 1_300 * MS),
            [UiAction::Right],
            "a new press after the release counts"
        );
    }

    #[test]
    fn two_devices_hold_their_own_controls() {
        let mut nav = Navigator::new();
        let other = InputEvent {
            device: 1,
            ..event(Control::Up, 1)
        };
        assert_eq!(
            step(&mut nav, &[event(Control::Up, 1), other], 0),
            [UiAction::Up, UiAction::Up]
        );
        let _ = step(&mut nav, &[event(Control::Up, 0)], 10 * MS);
        assert_eq!(
            step(&mut nav, &[], 400 * MS),
            [UiAction::Up],
            "device 1 still holds"
        );
    }
}
