//! The pure mapper: raw device input to RetroPad events, with ports and
//! hotplug.

use neptron_types::InputEvent;

use crate::device::{
    DeviceIdentity, MAX_KEYBOARDS, MAX_PORTS, gamepad_device, gamepad_port, keyboard_device,
    keyboard_slot,
};
use crate::key_map::key_control;
use crate::pad_map::{
    AxisTarget, axis_target, hat_values, pad_control, stick_value, trigger_value,
};
use crate::raw::{KeyAction, RawInput};
use crate::state::ControlState;

/// Turns raw input of connected devices into RetroPad events, and gives
/// each device its port and its device id.
///
/// Port rules for gamepads:
///
/// - A new gamepad takes its last port when that port is free. The last
///   port is the one that a device with the same identity had before.
/// - Otherwise it takes the lowest free port.
/// - A disconnect frees the port at once, and the release of every control
///   that the device held comes out as events.
///
/// Keyboards take the lowest free keyboard slot and all play on port 0.
///
/// An event comes out only when a control changes value, so a backend can
/// feed the full state of a device every frame. The mapper makes no
/// allocation of its own: it only appends to the caller's buffer.
#[derive(Debug, Clone, Default)]
pub struct Mapper {
    pads: [Slot; MAX_PORTS as usize],
    keyboards: [Slot; MAX_KEYBOARDS as usize],
}

/// An error of [`Mapper`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MapperError {
    /// Every gamepad port is in use.
    #[error("every one of the {max} gamepad ports is in use")]
    NoFreePort {
        /// The number of ports.
        max: u8,
    },
    /// Every keyboard slot is in use.
    #[error("every one of the {max} keyboard slots is in use")]
    NoFreeKeyboardSlot {
        /// The number of slots.
        max: u8,
    },
    /// The device id belongs to no connected device of this mapper.
    #[error("device {device:#x} is not connected")]
    NotConnected {
        /// The device id.
        device: u32,
    },
}

impl Mapper {
    /// A mapper with no device.
    pub fn new() -> Self {
        Self::default()
    }

    /// Connects a gamepad and returns its device id, which holds its port.
    ///
    /// # Errors
    ///
    /// [`MapperError::NoFreePort`] when every port is in use.
    pub fn connect_gamepad(&mut self, identity: DeviceIdentity) -> Result<u32, MapperError> {
        let free = |slot: &Slot| !slot.connected;
        let port = self
            .pads
            .iter()
            .position(|slot| free(slot) && slot.last == Some(identity))
            .or_else(|| self.pads.iter().position(free))
            .ok_or(MapperError::NoFreePort { max: MAX_PORTS })?;
        self.pads[port] = Slot {
            connected: true,
            last: Some(identity),
            state: ControlState::default(),
        };
        Ok(gamepad_device(port))
    }

    /// Connects a keyboard and returns its device id.
    ///
    /// # Errors
    ///
    /// [`MapperError::NoFreeKeyboardSlot`] when every slot is in use.
    pub fn connect_keyboard(&mut self) -> Result<u32, MapperError> {
        let slot = self
            .keyboards
            .iter()
            .position(|slot| !slot.connected)
            .ok_or(MapperError::NoFreeKeyboardSlot { max: MAX_KEYBOARDS })?;
        self.keyboards[slot] = Slot {
            connected: true,
            last: None,
            state: ControlState::default(),
        };
        Ok(keyboard_device(slot))
    }

    /// Disconnects `device`: appends to `out` the release of every control
    /// that it held, and frees its port or slot.
    ///
    /// # Errors
    ///
    /// [`MapperError::NotConnected`] when `device` is not connected.
    pub fn disconnect(
        &mut self,
        device: u32,
        out: &mut Vec<InputEvent>,
    ) -> Result<(), MapperError> {
        let slot = self
            .connected_mut(device)
            .ok_or(MapperError::NotConnected { device })?;
        slot.state.release_all(device, out);
        slot.connected = false;
        Ok(())
    }

    /// Applies one raw input of `device`, and appends to `out` an event for
    /// each control that changed.
    ///
    /// # Errors
    ///
    /// [`MapperError::NotConnected`] when `device` is not connected.
    pub fn apply(
        &mut self,
        device: u32,
        input: RawInput,
        out: &mut Vec<InputEvent>,
    ) -> Result<(), MapperError> {
        let state = &mut self
            .connected_mut(device)
            .ok_or(MapperError::NotConnected { device })?
            .state;
        let mut set = |control, value| {
            if state.set(control, value) {
                out.push(InputEvent {
                    device,
                    control,
                    value,
                });
            }
        };
        match input {
            RawInput::Button { button, pressed } => {
                if let Some(control) = pad_control(button) {
                    set(control, i16::from(pressed));
                }
            }
            RawInput::Axis { axis, value } => match axis_target(axis) {
                AxisTarget::Stick(control) => set(control, stick_value(value)),
                AxisTarget::Trigger(control) => set(control, trigger_value(value)),
            },
            RawInput::Hat(hat) => {
                for (control, value) in hat_values(hat) {
                    set(control, value);
                }
            }
            RawInput::Key { code, action } => {
                let value = match action {
                    KeyAction::Press => 1,
                    KeyAction::Release => 0,
                    KeyAction::Repeat => return Ok(()),
                };
                if let Some(control) = key_control(code) {
                    set(control, value);
                }
            }
        }
        Ok(())
    }

    /// True when `device` is connected.
    pub fn is_connected(&self, device: u32) -> bool {
        let slot = match (gamepad_port(device), keyboard_slot(device)) {
            (Some(port), _) => self.pads.get(port),
            (_, Some(slot)) => self.keyboards.get(slot),
            _ => None,
        };
        slot.is_some_and(|slot| slot.connected)
    }

    /// The slot of a connected device.
    fn connected_mut(&mut self, device: u32) -> Option<&mut Slot> {
        let slot = match (gamepad_port(device), keyboard_slot(device)) {
            (Some(port), _) => self.pads.get_mut(port),
            (_, Some(slot)) => self.keyboards.get_mut(slot),
            _ => None,
        };
        slot.filter(|slot| slot.connected)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    connected: bool,
    /// The identity of the last gamepad on this port. It stays after a
    /// disconnect, so that the same device can get the port back.
    last: Option<DeviceIdentity>,
    state: ControlState,
}
