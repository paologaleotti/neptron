//! The value of every RetroPad control of one device.

use neptron_types::{Control, InputEvent};

/// The number of RetroPad controls.
pub(crate) const CONTROL_COUNT: usize = 20;

/// Every RetroPad control, in the order of the libretro button ids, then
/// the four axes.
pub(crate) const ALL_CONTROLS: [Control; CONTROL_COUNT] = [
    Control::B,
    Control::Y,
    Control::Select,
    Control::Start,
    Control::Up,
    Control::Down,
    Control::Left,
    Control::Right,
    Control::A,
    Control::X,
    Control::L,
    Control::R,
    Control::L2,
    Control::R2,
    Control::L3,
    Control::R3,
    Control::LeftX,
    Control::LeftY,
    Control::RightX,
    Control::RightY,
];

/// The values of all controls of one device. A new state is neutral: no
/// button down, every axis centred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ControlState([i16; CONTROL_COUNT]);

impl ControlState {
    /// The value of `control`.
    pub(crate) fn get(&self, control: Control) -> i16 {
        self.0[index(control)]
    }

    /// Sets `control` to `value`. True when the value changed.
    pub(crate) fn set(&mut self, control: Control, value: i16) -> bool {
        let slot = &mut self.0[index(control)];
        let changed = *slot != value;
        *slot = value;
        changed
    }

    /// True when no button is down and every axis is centred.
    pub(crate) fn is_neutral(&self) -> bool {
        self.0.iter().all(|value| *value == 0)
    }

    /// Appends to `out` an event that sets each control of `device` that is
    /// not neutral back to 0, and makes the state neutral.
    pub(crate) fn release_all(&mut self, device: u32, out: &mut Vec<InputEvent>) {
        for control in ALL_CONTROLS {
            if self.set(control, 0) {
                out.push(InputEvent {
                    device,
                    control,
                    value: 0,
                });
            }
        }
    }
}

/// The position of `control` in [`ALL_CONTROLS`].
fn index(control: Control) -> usize {
    match control {
        Control::B => 0,
        Control::Y => 1,
        Control::Select => 2,
        Control::Start => 3,
        Control::Up => 4,
        Control::Down => 5,
        Control::Left => 6,
        Control::Right => 7,
        Control::A => 8,
        Control::X => 9,
        Control::L => 10,
        Control::R => 11,
        Control::L2 => 12,
        Control::R2 => 13,
        Control::L3 => 14,
        Control::R3 => 15,
        Control::LeftX => 16,
        Control::LeftY => 17,
        Control::RightX => 18,
        Control::RightY => 19,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_has_its_own_index() {
        for (i, control) in ALL_CONTROLS.iter().enumerate() {
            assert_eq!(index(*control), i, "{control:?}");
        }
    }

    #[test]
    fn set_reports_a_change_only() {
        let mut state = ControlState::default();
        assert!(state.is_neutral());
        assert!(state.set(Control::A, 1));
        assert!(!state.set(Control::A, 1));
        assert_eq!(state.get(Control::A), 1);
        assert!(!state.is_neutral());
        assert!(state.set(Control::A, 0));
        assert!(state.is_neutral());
    }

    #[test]
    fn release_all_sets_each_held_control_to_0_in_order() {
        let mut state = ControlState::default();
        state.set(Control::Start, 1);
        state.set(Control::B, 1);
        state.set(Control::LeftX, -20_000);
        let mut out = Vec::new();
        state.release_all(0x100, &mut out);
        let released: Vec<_> = out.iter().map(|e| (e.device, e.control, e.value)).collect();
        assert_eq!(
            released,
            [
                (0x100, Control::B, 0),
                (0x100, Control::Start, 0),
                (0x100, Control::LeftX, 0),
            ]
        );
        assert!(state.is_neutral());
        out.clear();
        state.release_all(0x100, &mut out);
        assert!(out.is_empty());
    }
}
