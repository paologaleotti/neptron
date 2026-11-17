//! Scripted gamepads: pads that a test plugs, presses and unplugs, read
//! through the same mapper, port rules, maps and notices as the SDL3
//! source.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use neptron_backend::InputSource;
use neptron_types::InputEvent;

use crate::generic_layout::{GENERIC_AXES, GenericLayout, JoystickCounts};
use crate::mapper::Mapper;
use crate::notice::{InputNotice, NoticeQueue};
use crate::pad_map::PAD_BUTTON_MAP;
use crate::pad_maps::{BuiltinMaps, PadLayout, PadMap};
use crate::port_table::PadId;
use crate::raw::{HatState, PadButton, RawInput};
use crate::roster::{PadInfo, PadRoster, PortRule, Roster};
use crate::scan::{Found, connect_found};
use crate::silent::PadActivity;

/// The raw buttons that a scripted joystick holds.
const JOY_BUTTONS: usize = 16;

/// Gamepads with no hardware, for tests of the frontend.
///
/// A plug or an unplug takes effect at the next `poll`, as a hotplug of
/// the SDL3 source does: first the unplugs, then the plugs of the poll as
/// one scan ([`Mapper::connect_scan`]), so the first `poll` settles the
/// ports. Each `poll` feeds the mapper the whole state of each pad, so
/// the events are those of a real pad with the same buttons down.
///
/// A pad is a gamepad with the standard layout ([`ScriptedPads::plug`])
/// or a plain joystick with no mapping
/// ([`ScriptedPads::plug_joystick`]), which gets the generic layout, or no
/// port when it is not a gamepad. The source gives the notices of the
/// SDL3 source for both.
///
/// Clones share the pads, so a test keeps one clone and gives the other
/// to the code under test.
#[derive(Debug, Clone)]
pub struct ScriptedPads {
    state: Arc<Mutex<State>>,
}

/// One pad of a [`ScriptedPads`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptedPad(usize);

impl ScriptedPads {
    /// No pad, with the port rule of the run and the roster of the run
    /// before.
    pub fn new(rule: PortRule, earlier: &Roster) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                mapper: Mapper::with_rule(rule, earlier),
                pads: Vec::new(),
                notices: NoticeQueue::default(),
            })),
        }
    }

    /// Adds a gamepad with the standard layout, plugged in at the next
    /// `poll`.
    pub fn plug(&self, info: PadInfo) -> ScriptedPad {
        self.add(info, PadLayout::Gamepad)
    }

    /// Adds a plain joystick with `counts` and no gamepad mapping, plugged
    /// in at the next `poll`.
    pub fn plug_joystick(&self, info: PadInfo, counts: JoystickCounts) -> ScriptedPad {
        self.add(info, PadLayout::Joystick(counts))
    }

    /// Plugs `pad` in again: the next `poll` sees it go away, when it was
    /// there, and come back as a new connection.
    pub fn replug(&self, pad: ScriptedPad) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0) {
            fake.plugged = true;
            fake.cycle = true;
        }
    }

    /// Unplugs `pad` at the next `poll`. Its controls go neutral.
    pub fn unplug(&self, pad: ScriptedPad) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0) {
            fake.plugged = false;
            fake.down = [false; PAD_BUTTON_MAP.len()];
            fake.joy = Joy::default();
        }
    }

    /// Sets a button of the standard layout of `pad`. The next `poll`
    /// reads it.
    pub fn set(&self, pad: ScriptedPad, button: PadButton, pressed: bool) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0) {
            let at = PAD_BUTTON_MAP.iter().position(|(b, _)| *b == button);
            if let Some(slot) = at.and_then(|i| fake.down.get_mut(i)) {
                *slot = pressed;
            }
        }
    }

    /// Sets raw button `index` of the joystick `pad`.
    pub fn set_joy_button(&self, pad: ScriptedPad, index: u8, pressed: bool) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0)
            && let Some(slot) = fake.joy.buttons.get_mut(usize::from(index))
        {
            *slot = pressed;
        }
    }

    /// Sets hat 0 of the joystick `pad`.
    pub fn set_joy_hat(&self, pad: ScriptedPad, hat: HatState) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0) {
            fake.joy.hat = hat;
        }
    }

    /// Sets raw axis `index` of the joystick `pad`.
    pub fn set_joy_axis(&self, pad: ScriptedPad, index: u8, value: i16) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0)
            && let Some(slot) = fake.joy.axes.get_mut(usize::from(index))
        {
            *slot = value;
        }
    }

    /// The device id of `pad` now, or `None` when it has no port.
    pub fn device(&self, pad: ScriptedPad) -> Option<u32> {
        let state = self.lock();
        match state.pads.get(pad.0)?.link {
            Link::Open(id, _) => state.mapper.device_of(id),
            Link::Waiting | Link::Refused => None,
        }
    }

    fn add(&self, info: PadInfo, layout: PadLayout) -> ScriptedPad {
        let mut state = self.lock();
        state.pads.push(Fake {
            info,
            layout,
            plugged: true,
            link: Link::Waiting,
            cycle: false,
            down: [false; PAD_BUTTON_MAP.len()],
            joy: Joy::default(),
        });
        ScriptedPad(state.pads.len() - 1)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A poisoned lock means a test thread panicked; the pads are still
        // usable.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl InputSource for ScriptedPads {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        let mut state = self.lock();
        let State {
            mapper,
            pads,
            notices,
        } = &mut *state;
        for fake in pads.iter_mut().filter(|f| !f.plugged || f.cycle) {
            if let Link::Open(id, _) = fake.link {
                let device = mapper.device_of(id);
                if let (Some(device), Ok(())) = (device, mapper.disconnect_gamepad(id, out)) {
                    notices.push(InputNotice::Disconnected { device });
                }
            }
            fake.link = Link::Waiting;
            fake.cycle = false;
        }
        let found = (0..pads.len())
            .filter(|i| pads[*i].plugged && pads[*i].link == Link::Waiting)
            .map(|i| Found {
                handle: i,
                info: pads[i].info.clone(),
                layout: pads[i].layout,
                what: format!("scripted pad {i}"),
            })
            .collect();
        let scan = connect_found(mapper, &BuiltinMaps, found, out, notices);
        for opened in scan.opened {
            if let Some(fake) = pads.get_mut(opened.handle) {
                fake.link = Link::Open(opened.id, opened.map);
            }
        }
        for refused in scan.refused {
            if let Some(fake) = pads.get_mut(refused) {
                fake.link = Link::Refused;
            }
        }
        for fake in pads.iter() {
            let Link::Open(id, map) = fake.link else {
                continue;
            };
            fake.feed(&map, &mut |input| {
                // The pad is open, so the mapper has it.
                let _ = mapper.apply_gamepad(id, input, out);
            });
        }
    }
}

impl PadRoster for ScriptedPads {
    fn roster(&self) -> Roster {
        self.lock().mapper.roster()
    }

    fn roster_changes(&self) -> u64 {
        self.lock().mapper.roster_changes()
    }

    fn take_notices(&self, out: &mut Vec<InputNotice>) {
        self.lock().notices.drain_into(out);
    }

    fn activity(&self) -> PadActivity {
        self.lock().mapper.activity()
    }
}

#[derive(Debug)]
struct State {
    mapper: Mapper,
    pads: Vec<Fake>,
    notices: NoticeQueue,
}

#[derive(Debug)]
struct Fake {
    info: PadInfo,
    layout: PadLayout,
    plugged: bool,
    link: Link,
    /// Set by a replug: the next poll disconnects the pad first.
    cycle: bool,
    /// The state of each button of `PAD_BUTTON_MAP`.
    down: [bool; PAD_BUTTON_MAP.len()],
    joy: Joy,
}

/// The raw state of a scripted joystick.
#[derive(Debug, Clone, Copy, Default)]
struct Joy {
    buttons: [bool; JOY_BUTTONS],
    hat: HatState,
    axes: [i16; GENERIC_AXES],
}

impl Fake {
    /// Gives `apply` the whole state of the pad as raw input, read as its
    /// backend reads it: the standard layout, or the indices of the generic
    /// table.
    fn feed(&self, map: &PadMap, apply: &mut dyn FnMut(RawInput)) {
        match map {
            PadMap::Generic(layout) => self.feed_joystick(layout, apply),
            PadMap::Known => {
                for ((button, _), pressed) in PAD_BUTTON_MAP.iter().zip(self.down) {
                    apply(RawInput::Button {
                        button: *button,
                        pressed,
                    });
                }
            }
        }
    }

    fn feed_joystick(&self, layout: &GenericLayout, apply: &mut dyn FnMut(RawInput)) {
        for index in 0..layout.buttons_read() {
            let pressed = self.joy.buttons.get(usize::from(index));
            apply(RawInput::JoyButton {
                index,
                pressed: pressed.copied().unwrap_or_default(),
            });
        }
        for index in 0..layout.hats_read() {
            apply(RawInput::JoyHat {
                index,
                hat: self.joy.hat,
            });
        }
        for index in 0..layout.axes_read() {
            let value = self.joy.axes.get(usize::from(index));
            apply(RawInput::JoyAxis {
                index,
                value: value.copied().unwrap_or_default(),
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    /// Not connected: unplugged, or plugged and not yet polled.
    Waiting,
    Open(PadId, PadMap),
    /// No port was free, or it is not a gamepad. Only an unplug and a plug
    /// try again, as with the SDL3 source.
    Refused,
}
