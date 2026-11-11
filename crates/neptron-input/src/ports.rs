//! The RetroPad state of each port, merged from every device on the port.

use neptron_types::{Control, InputEvent};

use crate::device::{MAX_KEYBOARDS, MAX_PORTS, port_of};
use crate::state::ControlState;

/// The RetroPad state of every port, built from the events of every
/// source: gamepads, keyboards, the window keyboard, scripted input.
///
/// Several devices can play on one port (all keyboards play on port 0). A
/// button of a port is down while it is down on any of its devices. An
/// axis of a port has the value with the largest magnitude among its
/// devices, and the device that sent its events first wins a tie.
///
/// Events of a device id outside every range are ignored. The state needs
/// no allocation after `new`.
#[derive(Debug, Clone)]
pub struct PortStates {
    /// One entry for each device that holds a control, in the order of
    /// their first event. A device that becomes neutral leaves the table.
    devices: [Option<(u32, ControlState)>; TRACKED_DEVICES],
}

impl PortStates {
    /// All ports neutral.
    pub fn new() -> Self {
        Self {
            devices: [None; TRACKED_DEVICES],
        }
    }

    /// Applies `events`, oldest first.
    pub fn apply(&mut self, events: &[InputEvent]) {
        for event in events {
            self.apply_one(event);
        }
    }

    /// The value of `control` on `port`: 0 or 1 for a button, -32768 to
    /// 32767 for an axis.
    pub fn value(&self, port: u8, control: Control) -> i16 {
        let values = self
            .devices
            .iter()
            .flatten()
            .filter(|(device, _)| port_of(*device) == Some(port))
            .map(|(_, state)| state.get(control));
        if is_axis(control) {
            values.fold(0, |best, v| {
                if v.unsigned_abs() > best.unsigned_abs() {
                    v
                } else {
                    best
                }
            })
        } else {
            values.max().unwrap_or(0).min(1)
        }
    }

    /// True when the button `control` is down on `port`, or the axis
    /// `control` is off centre.
    pub fn is_held(&self, port: u8, control: Control) -> bool {
        self.value(port, control) != 0
    }

    fn apply_one(&mut self, event: &InputEvent) {
        if port_of(event.device).is_none() {
            return;
        }
        let existing = self
            .devices
            .iter()
            .position(|entry| entry.is_some_and(|(device, _)| device == event.device));
        let Some(index) = existing.or_else(|| {
            if event.value == 0 {
                return None;
            }
            // Each device id with a port has at most one entry, and the table
            // has one entry for each such id, so a new id finds a free one.
            self.devices.iter().position(Option::is_none)
        }) else {
            return;
        };
        let entry = &mut self.devices[index];
        let (_, state) = entry.get_or_insert((event.device, ControlState::default()));
        state.set(event.control, event.value);
        if state.is_neutral() {
            *entry = None;
        }
    }
}

impl Default for PortStates {
    fn default() -> Self {
        Self::new()
    }
}

/// The number of device ids that have a port: direct ids, gamepads and
/// keyboards.
const TRACKED_DEVICES: usize = 2 * MAX_PORTS as usize + MAX_KEYBOARDS as usize;

fn is_axis(control: Control) -> bool {
    matches!(
        control,
        Control::LeftX | Control::LeftY | Control::RightX | Control::RightY
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{GAMEPAD_DEVICE_BASE, KEYBOARD_DEVICE_BASE};

    fn ev(device: u32, control: Control, value: i16) -> InputEvent {
        InputEvent {
            device,
            control,
            value,
        }
    }

    #[test]
    fn every_device_with_a_port_fits_in_the_table() {
        let mut ports = PortStates::new();
        let ids = (0..u32::from(MAX_PORTS))
            .chain((0..u32::from(MAX_PORTS)).map(|p| GAMEPAD_DEVICE_BASE + p))
            .chain((0..u32::from(MAX_KEYBOARDS)).map(|k| KEYBOARD_DEVICE_BASE + k));
        let events: Vec<_> = ids.map(|id| ev(id, Control::L3, 1)).collect();
        assert_eq!(events.len(), TRACKED_DEVICES);
        ports.apply(&events);
        assert!(ports.devices.iter().all(Option::is_some));
        // Every keyboard plays on port 0.
        assert_eq!(ports.value(0, Control::L3), 1);
        assert_eq!(ports.value(7, Control::L3), 1);
    }

    #[test]
    fn a_neutral_device_leaves_the_table() {
        let mut ports = PortStates::new();
        ports.apply(&[ev(3, Control::A, 1), ev(3, Control::A, 0)]);
        assert!(ports.devices.iter().all(Option::is_none));
        ports.apply(&[ev(3, Control::A, 0)]);
        assert!(ports.devices.iter().all(Option::is_none));
    }

    #[test]
    fn ids_without_a_port_are_ignored() {
        let mut ports = PortStates::new();
        ports.apply(&[ev(8, Control::A, 1), ev(0x300, Control::A, 1)]);
        assert!(ports.devices.iter().all(Option::is_none));
    }
}
