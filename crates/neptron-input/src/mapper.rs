//! The pure mapper: raw device input to RetroPad events, with ports and
//! hotplug.

use neptron_types::InputEvent;

use crate::device::{
    MAX_KEYBOARDS, MAX_PORTS, gamepad_device, gamepad_port, keyboard_device, keyboard_slot,
};
use crate::key_map::key_control;
use crate::pad_map::{
    AxisTarget, axis_target, hat_values, pad_control, stick_value, trigger_value,
};
use crate::port_table::{PadId, PortTable, pad_index};
use crate::raw::{KeyAction, RawInput};
use crate::roster::{PadInfo, PortRule, Roster};
use crate::state::ControlState;

/// Turns raw input of connected devices into RetroPad events, and gives
/// each device its port and its device id.
///
/// A gamepad gets its port from the port table, by the rule of the run
/// ([`PortRule`]):
///
/// - A new gamepad takes a held port with the same identity and path, else
///   a held port with the same identity, else the lowest free port.
/// - In the menus a disconnect frees the port, and the pads on higher ports
///   move down. A pad that moves gets the release of every control that it
///   held, on its old device id, and starts neutral on the new one.
/// - In a game a disconnect holds the port for the pad, and nothing moves.
///
/// A disconnect gives the release of every control that the device held.
/// The device id of a gamepad holds its port, so a backend keeps the
/// [`PadId`] of each pad and reads its device id with
/// [`Mapper::device_of`].
///
/// Keyboards take the lowest free keyboard slot and all play on port 0.
///
/// An event comes out only when a control changes value, so a backend can
/// feed the full state of a device every frame. The mapper makes no
/// allocation of its own: it only appends to the caller's buffer.
#[derive(Debug, Clone, Default)]
pub struct Mapper {
    ports: PortTable,
    /// The state of each connected gamepad, by its [`PadId`].
    pads: [ControlState; MAX_PORTS as usize],
    keyboards: [Slot; MAX_KEYBOARDS as usize],
    changes: u64,
    /// The first scan has settled the ports.
    settled: bool,
}

/// An error of [`Mapper`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MapperError {
    /// Every gamepad port is in use or held.
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
    /// The pad is not connected.
    #[error("pad {pad:?} is not connected")]
    PadNotConnected {
        /// The pad.
        pad: PadId,
    },
}

impl Mapper {
    /// A mapper with no device, with the rule of the menus.
    pub fn new() -> Self {
        Self::default()
    }

    /// A mapper with no device, that follows `rule`, with the pads of
    /// `earlier` (the roster of the run before) held on their ports.
    pub fn with_rule(rule: PortRule, earlier: &Roster) -> Self {
        Self {
            ports: PortTable::new(rule, earlier),
            ..Self::default()
        }
    }

    /// Connects a gamepad and returns its id. [`Mapper::device_of`] gives
    /// its device id.
    ///
    /// # Errors
    ///
    /// [`MapperError::NoFreePort`] when every port is in use or held.
    pub fn connect_gamepad(&mut self, pad: PadInfo) -> Result<PadId, MapperError> {
        let (id, _) = self.ports.connect(pad)?;
        if let Some(state) = self.state_mut(id) {
            *state = ControlState::default();
        }
        self.changes = self.changes.wrapping_add(1);
        Ok(id)
    }

    /// Connects the pads that one scan of a backend found, and returns the
    /// result of each, in the order of `pads`.
    ///
    /// A pad whose identity and path match a held port connects first, so
    /// a pad of the same model at a new path cannot take the port that is
    /// held for a pad later in the scan. The first call of a run is the
    /// first scan: after its connects it settles the ports, then it tries
    /// again each pad that found every port in use or held, so a pad of
    /// this run never loses to a held port of the run before in the menus.
    /// It allocates the results; call it at a hotplug, not every frame.
    pub fn connect_scan(
        &mut self,
        pads: Vec<PadInfo>,
        out: &mut Vec<InputEvent>,
    ) -> Vec<Result<PadId, MapperError>> {
        let exact: Vec<bool> = pads.iter().map(|p| self.ports.holds_exactly(p)).collect();
        let mut results = vec![Err(MapperError::NoFreePort { max: MAX_PORTS }); pads.len()];
        let order = (0..pads.len())
            .filter(|i| exact[*i])
            .chain((0..pads.len()).filter(|i| !exact[*i]));
        for i in order {
            results[i] = self.connect_gamepad(pads[i].clone());
        }
        if !self.settled {
            self.settle(out);
            for (result, pad) in results.iter_mut().zip(&pads) {
                if matches!(result, Err(MapperError::NoFreePort { .. })) {
                    *result = self.connect_gamepad(pad.clone());
                }
            }
        }
        results
    }

    /// Disconnects a gamepad: appends to `out` the release of every control
    /// that it held, then applies the rule of the run to its port. In the
    /// menus `out` also gets the release of each pad that moves down.
    ///
    /// # Errors
    ///
    /// [`MapperError::PadNotConnected`] when `pad` is not connected.
    pub fn disconnect_gamepad(
        &mut self,
        pad: PadId,
        out: &mut Vec<InputEvent>,
    ) -> Result<(), MapperError> {
        let port = self
            .ports
            .port(pad)
            .ok_or(MapperError::PadNotConnected { pad })?;
        if let Some(state) = self.state_mut(pad) {
            state.release_all(gamepad_device(port), out);
        }
        let pads = &mut self.pads;
        self.ports.disconnect(pad, &mut |moved, from| {
            if let Some(state) = pads.get_mut(pad_index(moved)) {
                state.release_all(gamepad_device(from), out);
            }
        });
        self.changes = self.changes.wrapping_add(1);
        Ok(())
    }

    /// Ends the start of a run: once the backend has connected the pads it
    /// found, the menus free every held port that no pad took, and the
    /// pads move down. In a game it does nothing. The first
    /// [`Mapper::connect_scan`] calls it.
    pub fn settle(&mut self, out: &mut Vec<InputEvent>) {
        self.settled = true;
        let pads = &mut self.pads;
        self.ports.settle(&mut |moved, from| {
            if let Some(state) = pads.get_mut(pad_index(moved)) {
                state.release_all(gamepad_device(from), out);
            }
        });
        self.changes = self.changes.wrapping_add(1);
    }

    /// The device id of a connected gamepad. It holds the port, so it
    /// changes when the pad moves.
    pub fn device_of(&self, pad: PadId) -> Option<u32> {
        self.ports.port(pad).map(gamepad_device)
    }

    /// Applies one raw input of a gamepad, and appends to `out` an event
    /// for each control that changed.
    ///
    /// # Errors
    ///
    /// [`MapperError::PadNotConnected`] when `pad` is not connected.
    pub fn apply_gamepad(
        &mut self,
        pad: PadId,
        input: RawInput,
        out: &mut Vec<InputEvent>,
    ) -> Result<(), MapperError> {
        let device = self
            .device_of(pad)
            .ok_or(MapperError::PadNotConnected { pad })?;
        let state = self
            .state_mut(pad)
            .ok_or(MapperError::PadNotConnected { pad })?;
        map(state, device, input, out);
        Ok(())
    }

    /// The pad on each port.
    pub fn roster(&self) -> Roster {
        self.ports.roster()
    }

    /// A number that changes with each connect, disconnect and settle.
    pub fn roster_changes(&self) -> u64 {
        self.changes
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
            state: ControlState::default(),
        };
        Ok(keyboard_device(slot))
    }

    /// Disconnects `device`, a keyboard or a gamepad by its device id:
    /// appends to `out` the release of every control that it held, and
    /// frees its slot, or applies the port rule ([`Mapper::disconnect_gamepad`]).
    ///
    /// # Errors
    ///
    /// [`MapperError::NotConnected`] when `device` is not connected.
    pub fn disconnect(
        &mut self,
        device: u32,
        out: &mut Vec<InputEvent>,
    ) -> Result<(), MapperError> {
        if let Some(pad) = self.pad_of(device) {
            return self.disconnect_gamepad(pad, out);
        }
        let slot = self
            .keyboard_mut(device)
            .ok_or(MapperError::NotConnected { device })?;
        slot.state.release_all(device, out);
        slot.connected = false;
        Ok(())
    }

    /// Applies one raw input of `device`, a keyboard or a gamepad by its
    /// device id, and appends to `out` an event for each control that
    /// changed.
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
        if let Some(pad) = self.pad_of(device) {
            return self.apply_gamepad(pad, input, out);
        }
        let state = &mut self
            .keyboard_mut(device)
            .ok_or(MapperError::NotConnected { device })?
            .state;
        map(state, device, input, out);
        Ok(())
    }

    /// True when `device` is connected.
    pub fn is_connected(&self, device: u32) -> bool {
        self.pad_of(device).is_some()
            || keyboard_slot(device)
                .and_then(|slot| self.keyboards.get(slot))
                .is_some_and(|slot| slot.connected)
    }

    /// The gamepad that has the device id `device` now.
    fn pad_of(&self, device: u32) -> Option<PadId> {
        gamepad_port(device).and_then(|port| self.ports.pad_on(port))
    }

    fn state_mut(&mut self, pad: PadId) -> Option<&mut ControlState> {
        self.pads.get_mut(pad_index(pad))
    }

    /// The slot of a connected keyboard.
    fn keyboard_mut(&mut self, device: u32) -> Option<&mut Slot> {
        keyboard_slot(device)
            .and_then(|slot| self.keyboards.get_mut(slot))
            .filter(|slot| slot.connected)
    }
}

/// Maps one raw input into `state`, and appends an event for each control
/// that changed.
fn map(state: &mut ControlState, device: u32, input: RawInput, out: &mut Vec<InputEvent>) {
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
                KeyAction::Repeat => return,
            };
            if let Some(control) = key_control(code) {
                set(control, value);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    connected: bool,
    state: ControlState,
}
