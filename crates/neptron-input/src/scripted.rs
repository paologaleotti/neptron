//! Scripted gamepads: pads that a test plugs, presses and unplugs, read
//! through the same mapper and port rules as the SDL3 source.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use neptron_backend::InputSource;
use neptron_types::InputEvent;

use crate::mapper::Mapper;
use crate::pad_map::PAD_BUTTON_MAP;
use crate::port_table::PadId;
use crate::raw::{PadButton, RawInput};
use crate::roster::{PadInfo, PadRoster, PortRule, Roster};

/// Gamepads with no hardware, for tests of the frontend.
///
/// A plug or an unplug takes effect at the next `poll`, as a hotplug of
/// the SDL3 source does: first the unplugs, then the plugs of the poll as
/// one scan ([`Mapper::connect_scan`]), so the first `poll` settles the
/// ports. Each `poll`
/// feeds the mapper the whole state of each pad, so the events are those
/// of a real pad with the same buttons down.
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
            })),
        }
    }

    /// Adds a pad, plugged in at the next `poll`.
    pub fn plug(&self, info: PadInfo) -> ScriptedPad {
        let mut state = self.lock();
        state.pads.push(Fake {
            info,
            plugged: true,
            link: Link::Waiting,
            cycle: false,
            down: [false; PAD_BUTTON_MAP.len()],
        });
        ScriptedPad(state.pads.len() - 1)
    }

    /// Plugs `pad` in again: the next `poll` sees it go away, when it was
    /// there, and come back as a new connection.
    pub fn replug(&self, pad: ScriptedPad) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0) {
            fake.plugged = true;
            fake.cycle = true;
        }
    }

    /// Unplugs `pad` at the next `poll`. Its buttons go up.
    pub fn unplug(&self, pad: ScriptedPad) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0) {
            fake.plugged = false;
            fake.down = [false; PAD_BUTTON_MAP.len()];
        }
    }

    /// Sets a button of `pad`. The next `poll` reads it.
    pub fn set(&self, pad: ScriptedPad, button: PadButton, pressed: bool) {
        if let Some(fake) = self.lock().pads.get_mut(pad.0) {
            let at = PAD_BUTTON_MAP.iter().position(|(b, _)| *b == button);
            if let Some(slot) = at.and_then(|i| fake.down.get_mut(i)) {
                *slot = pressed;
            }
        }
    }

    /// The device id of `pad` now, or `None` when it has no port.
    pub fn device(&self, pad: ScriptedPad) -> Option<u32> {
        let state = self.lock();
        match state.pads.get(pad.0)?.link {
            Link::Open(id) => state.mapper.device_of(id),
            Link::Waiting | Link::Refused => None,
        }
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
        let State { mapper, pads } = &mut *state;
        for fake in pads.iter_mut().filter(|f| !f.plugged || f.cycle) {
            if let Link::Open(id) = fake.link {
                // The pad is open, so the mapper has it.
                let _ = mapper.disconnect_gamepad(id, out);
            }
            fake.link = Link::Waiting;
            fake.cycle = false;
        }
        let waiting = |f: &&mut Fake| f.plugged && f.link == Link::Waiting;
        let infos = pads
            .iter_mut()
            .filter(waiting)
            .map(|f| f.info.clone())
            .collect();
        let results = mapper.connect_scan(infos, out);
        for (fake, result) in pads.iter_mut().filter(waiting).zip(results) {
            fake.link = match result {
                Ok(id) => Link::Open(id),
                Err(_) => Link::Refused,
            };
        }
        for fake in pads.iter() {
            let Link::Open(id) = fake.link else {
                continue;
            };
            for ((button, _), pressed) in PAD_BUTTON_MAP.iter().zip(fake.down) {
                let input = RawInput::Button {
                    button: *button,
                    pressed,
                };
                // The pad is open, so the mapper has it.
                let _ = mapper.apply_gamepad(id, input, out);
            }
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
}

#[derive(Debug)]
struct State {
    mapper: Mapper,
    pads: Vec<Fake>,
}

#[derive(Debug)]
struct Fake {
    info: PadInfo,
    plugged: bool,
    link: Link,
    /// Set by a replug: the next poll disconnects the pad first.
    cycle: bool,
    /// The state of each button of `PAD_BUTTON_MAP`.
    down: [bool; PAD_BUTTON_MAP.len()],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    /// Not connected: unplugged, or plugged and not yet polled.
    Waiting,
    Open(PadId),
    /// No port was free. Only an unplug and a plug try again, as with the
    /// SDL3 source.
    Refused,
}
