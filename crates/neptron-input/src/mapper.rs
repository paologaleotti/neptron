//! The pure mapper: raw device input to RetroPad events, with ports and
//! hotplug.

use neptron_types::InputEvent;

use crate::device::{
    MAX_KEYBOARDS, MAX_PORTS, gamepad_device, gamepad_port, keyboard_device, keyboard_slot,
};
use crate::generic_layout::dpad_axis_values;
use crate::key_map::key_control;
use crate::pad_map::{
    AxisTarget, axis_target, hat_values, pad_control, stick_value, trigger_value,
};
use crate::pad_maps::PadMap;
use crate::player_map::{Binding, half_axis_value, stick};
use crate::port_table::{PadId, PortTable, pad_index};
use crate::raw::{KeyAction, RawInput};
use crate::roster::{PadInfo, PortRule, Roster};
use crate::silent::{PadActivity, PadSignal};
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
/// Each gamepad keeps its map ([`PadMap`]): the standard layout of SDL3
/// reads [`RawInput::Button`], [`RawInput::Axis`] and [`RawInput::Hat`];
/// the generic layout and the map of the player read the raw indices of a
/// joystick ([`RawInput::JoyButton`], [`RawInput::JoyHat`],
/// [`RawInput::JoyAxis`]). [`Mapper::set_map`] changes the map of a
/// connected pad.
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
    /// The map of each connected gamepad, by its [`PadId`].
    maps: [PadMap; MAX_PORTS as usize],
    /// True once a gamepad sent a value that is not neutral, by its
    /// [`PadId`].
    heard: [bool; MAX_PORTS as usize],
    /// The plug number of each connected gamepad, by its [`PadId`].
    plugs: [u32; MAX_PORTS as usize],
    plug_count: u32,
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

    /// Connects a gamepad with the standard layout and returns its id.
    /// [`Mapper::device_of`] gives its device id.
    ///
    /// # Errors
    ///
    /// [`MapperError::NoFreePort`] when every port is in use or held.
    pub fn connect_gamepad(&mut self, pad: PadInfo) -> Result<PadId, MapperError> {
        self.connect_gamepad_with(pad, PadMap::Known)
    }

    /// Connects a gamepad that reads its input with `map`, and returns its
    /// id.
    ///
    /// # Errors
    ///
    /// [`MapperError::NoFreePort`] when every port is in use or held.
    pub fn connect_gamepad_with(
        &mut self,
        pad: PadInfo,
        map: PadMap,
    ) -> Result<PadId, MapperError> {
        let (id, _) = self.ports.connect(pad, map.kind())?;
        let index = pad_index(id);
        if let Some(state) = self.pads.get_mut(index) {
            *state = ControlState::default();
        }
        self.plug_count = self.plug_count.wrapping_add(1);
        if let Some(slot) = self.heard.get_mut(index) {
            *slot = false;
        }
        if let Some(slot) = self.maps.get_mut(index) {
            *slot = map;
        }
        if let Some(slot) = self.plugs.get_mut(index) {
            *slot = self.plug_count;
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
        let pads = pads.into_iter().map(|p| (p, PadMap::Known)).collect();
        self.connect_scan_with(pads, out)
    }

    /// [`Mapper::connect_scan`] for pads that each come with their map.
    pub fn connect_scan_with(
        &mut self,
        pads: Vec<(PadInfo, PadMap)>,
        out: &mut Vec<InputEvent>,
    ) -> Vec<Result<PadId, MapperError>> {
        let exact: Vec<bool> = pads
            .iter()
            .map(|(p, _)| self.ports.holds_exactly(p))
            .collect();
        let mut results = vec![Err(MapperError::NoFreePort { max: MAX_PORTS }); pads.len()];
        let order = (0..pads.len())
            .filter(|i| exact[*i])
            .chain((0..pads.len()).filter(|i| !exact[*i]));
        for i in order {
            let (info, map) = &pads[i];
            results[i] = self.connect_gamepad_with(info.clone(), *map);
        }
        if !self.settled {
            self.settle(out);
            for (result, (info, map)) in results.iter_mut().zip(&pads) {
                if matches!(result, Err(MapperError::NoFreePort { .. })) {
                    *result = self.connect_gamepad_with(info.clone(), *map);
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

    /// Gives the connected pad `pad` a new map: appends to `out` the
    /// release of every control that it holds, so the next input starts
    /// from neutral with the new map. The roster shows the new kind.
    ///
    /// # Errors
    ///
    /// [`MapperError::PadNotConnected`] when `pad` is not connected.
    pub fn set_map(
        &mut self,
        pad: PadId,
        map: PadMap,
        out: &mut Vec<InputEvent>,
    ) -> Result<(), MapperError> {
        self.release(pad, out)?;
        if let Some(slot) = self.maps.get_mut(pad_index(pad)) {
            *slot = map;
        }
        if self.ports.set_kind(pad, map.kind()) {
            self.changes = self.changes.wrapping_add(1);
        }
        Ok(())
    }

    /// The map of the connected pad `pad`.
    pub fn map_of(&self, pad: PadId) -> Option<PadMap> {
        self.ports.port(pad)?;
        self.maps.get(pad_index(pad)).copied()
    }

    /// Appends to `out` the release of every control that the connected
    /// pad `pad` holds. A backend calls it before it stops feeding the
    /// pad, for example while the guided mapping reads it.
    ///
    /// # Errors
    ///
    /// [`MapperError::PadNotConnected`] when `pad` is not connected.
    pub fn release(&mut self, pad: PadId, out: &mut Vec<InputEvent>) -> Result<(), MapperError> {
        let device = self
            .device_of(pad)
            .ok_or(MapperError::PadNotConnected { pad })?;
        if let Some(state) = self.state_mut(pad) {
            state.release_all(device, out);
        }
        Ok(())
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
        let index = pad_index(pad);
        let (Some(state), Some(pad_map), Some(heard)) = (
            self.pads.get_mut(index),
            self.maps.get(index),
            self.heard.get_mut(index),
        ) else {
            return Err(MapperError::PadNotConnected { pad });
        };
        let first = out.len();
        map(state, pad_map, device, input, out);
        if out
            .get(first..)
            .is_some_and(|new| new.iter().any(|e| e.value != 0))
        {
            *heard = true;
        }
        Ok(())
    }

    /// The connected pad of each port, for the silent pad rule. It
    /// allocates nothing.
    pub fn activity(&self) -> PadActivity {
        let mut activity = PadActivity::default();
        for (port, slot) in activity.ports.iter_mut().enumerate() {
            *slot = self.ports.playing(port).map(|(id, info)| {
                let index = pad_index(id);
                PadSignal {
                    plug: self.plugs.get(index).copied().unwrap_or_default(),
                    identity: info.identity,
                    path: info.path,
                    heard: self.heard.get(index).copied().unwrap_or_default(),
                }
            });
        }
        activity
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
        map(state, &PadMap::Known, device, input, out);
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

/// Maps one raw input into `state` with `pad_map`, and appends an event
/// for each control that changed. The raw indices of a joystick drive a
/// control only through a generic map or a map of the player.
fn map(
    state: &mut ControlState,
    pad_map: &PadMap,
    device: u32,
    input: RawInput,
    out: &mut Vec<InputEvent>,
) {
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
        RawInput::JoyButton { index, pressed } => match pad_map {
            PadMap::Generic(layout) => {
                if let Some(control) = layout.button(index) {
                    set(control, i16::from(pressed));
                }
            }
            PadMap::Player(player) => {
                for (control, binding) in player.bindings() {
                    if binding == Binding::Button(index) {
                        set(control, i16::from(pressed));
                    }
                }
            }
            PadMap::Known => {}
        },
        RawInput::JoyHat { index, hat } => match pad_map {
            PadMap::Generic(layout) => {
                if layout.hat_is_dpad(index) {
                    for (control, value) in hat_values(hat) {
                        set(control, value);
                    }
                }
            }
            PadMap::Player(player) => {
                for (control, binding) in player.bindings() {
                    if let Binding::Hat {
                        index: i,
                        direction,
                    } = binding
                        && i == index
                    {
                        set(control, i16::from(hat.has(direction.bit())));
                    }
                }
            }
            PadMap::Known => {}
        },
        RawInput::JoyAxis { index, value } => match pad_map {
            PadMap::Generic(layout) => {
                if let Some(role) = layout.axis(index) {
                    set(role.stick, stick_value(value));
                    if let Some(pair) = role.dpad {
                        for (control, digital) in dpad_axis_values(pair, value) {
                            set(control, digital);
                        }
                    }
                }
            }
            PadMap::Player(player) => {
                for (control, binding) in player.bindings() {
                    match binding {
                        Binding::HalfAxis {
                            index: i,
                            positive,
                            full,
                        } if i == index => set(control, half_axis_value(value, positive, full)),
                        Binding::Stick { index: i, inverted } if i == index => {
                            set(control, stick(value, inverted));
                        }
                        _ => {}
                    }
                }
            }
            PadMap::Known => {}
        },
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    connected: bool,
    state: ControlState,
}
