//! The port of each gamepad: plug order, the hold of a port in a game, and
//! the move down of the pads in the menus.

use crate::device::MAX_PORTS;
use crate::mapper::MapperError;
use crate::pad_maps::MapKind;
use crate::roster::{PadInfo, PortRule, Roster, RosterEntry};

/// A connected gamepad. It stays the same while the pad stays connected,
/// also when the pad moves to another port; a reconnect gives a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PadId(u8);

/// The ports of the gamepads and the rules that give them.
#[derive(Debug, Clone, Default)]
pub(crate) struct PortTable {
    rule: PortRule,
    seats: [Seat; PORTS],
    /// The port of each connected pad, by its id.
    pad_ports: [Option<usize>; PORTS],
}

impl PortTable {
    /// A table with no pad, that follows `rule`, with every pad of
    /// `earlier` held on its port: the next connects take them back.
    pub(crate) fn new(rule: PortRule, earlier: &Roster) -> Self {
        let mut table = Self {
            rule,
            ..Self::default()
        };
        for (seat, entry) in table.seats.iter_mut().zip(earlier.ports()) {
            if let Some(entry) = entry {
                *seat = Seat::Held(entry.pad.clone(), entry.map);
            }
        }
        table
    }

    /// Gives `pad` a port: a held port with the same identity and path,
    /// else a held port with the same identity, else the lowest free port.
    /// Returns the new id and the port. `map` is the kind of map that the
    /// roster shows for it.
    pub(crate) fn connect(
        &mut self,
        pad: PadInfo,
        map: MapKind,
    ) -> Result<(PadId, usize), MapperError> {
        let held = |exact: bool| {
            self.seats.iter().position(|seat| match seat {
                Seat::Held(old, _) => {
                    old.identity == pad.identity && (!exact || old.path == pad.path)
                }
                _ => false,
            })
        };
        let port = held(true)
            .or_else(|| held(false))
            .or_else(|| self.seats.iter().position(|s| matches!(s, Seat::Free)))
            .ok_or(MapperError::NoFreePort { max: MAX_PORTS })?;
        // A port is free or held, so fewer than PORTS pads are connected,
        // and one id is free.
        let index = self
            .pad_ports
            .iter()
            .position(Option::is_none)
            .ok_or(MapperError::NoFreePort { max: MAX_PORTS })?;
        let id =
            PadId(u8::try_from(index).map_err(|_| MapperError::NoFreePort { max: MAX_PORTS })?);
        self.seats[port] = Seat::Playing(id, pad, map);
        self.pad_ports[index] = Some(port);
        Ok((id, port))
    }

    /// True when a port is held for a pad with the identity and the path
    /// of `pad`.
    pub(crate) fn holds_exactly(&self, pad: &PadInfo) -> bool {
        self.seats.iter().any(|seat| {
            matches!(seat, Seat::Held(old, _) if old.identity == pad.identity && old.path == pad.path)
        })
    }

    /// Takes `pad` off its port, and returns the port it had. In a game the
    /// port is held for it. In the menus the port is freed and the pads
    /// above move down; `moved` gets each pad that moves and its old port.
    pub(crate) fn disconnect(
        &mut self,
        pad: PadId,
        moved: &mut dyn FnMut(PadId, usize),
    ) -> Option<usize> {
        let port = self.port(pad)?;
        self.pad_ports[pad_index(pad)] = None;
        let seat = std::mem::take(&mut self.seats[port]);
        match (self.rule, seat) {
            (PortRule::Game, Seat::Playing(_, info, map)) => {
                self.seats[port] = Seat::Held(info, map);
            }
            _ => self.compact(moved),
        }
        Some(port)
    }

    /// In the menus, frees every held port and moves the pads down. In a
    /// game it does nothing: held ports wait until the run ends.
    pub(crate) fn settle(&mut self, moved: &mut dyn FnMut(PadId, usize)) {
        if self.rule == PortRule::Game {
            return;
        }
        for seat in &mut self.seats {
            if matches!(seat, Seat::Held(..)) {
                *seat = Seat::Free;
            }
        }
        self.compact(moved);
    }

    /// The port of a connected pad.
    pub(crate) fn port(&self, pad: PadId) -> Option<usize> {
        self.pad_ports.get(pad_index(pad)).copied().flatten()
    }

    /// The pad playing on `port`.
    pub(crate) fn pad_on(&self, port: usize) -> Option<PadId> {
        match self.seats.get(port) {
            Some(Seat::Playing(id, ..)) => Some(*id),
            _ => None,
        }
    }

    /// The pad playing on `port`, with what it connected with.
    pub(crate) fn playing(&self, port: usize) -> Option<(PadId, &PadInfo)> {
        match self.seats.get(port) {
            Some(Seat::Playing(id, info, _)) => Some((*id, info)),
            _ => None,
        }
    }

    /// The pad on each port.
    pub(crate) fn roster(&self) -> Roster {
        let mut roster = Roster::new();
        for (port, seat) in (0u8..).zip(&self.seats) {
            let entry = match seat {
                Seat::Free => None,
                Seat::Playing(_, pad, map) => Some(RosterEntry {
                    pad: pad.clone(),
                    present: true,
                    map: *map,
                }),
                Seat::Held(pad, map) => Some(RosterEntry {
                    pad: pad.clone(),
                    present: false,
                    map: *map,
                }),
            };
            // `port` is below PORTS, the size of a roster.
            let _ = roster.set(port, entry);
        }
        roster
    }

    /// Moves every seat that is not free down over the free ones, in
    /// order, and tells `moved` of each pad that changes its port.
    fn compact(&mut self, moved: &mut dyn FnMut(PadId, usize)) {
        let mut to = 0;
        for from in 0..PORTS {
            if matches!(self.seats[from], Seat::Free) {
                continue;
            }
            if from != to {
                self.seats.swap(from, to);
                if let Seat::Playing(id, ..) = self.seats[to] {
                    self.pad_ports[pad_index(id)] = Some(to);
                    moved(id, from);
                }
            }
            to += 1;
        }
    }
}

/// The index of `pad` in a table by pad id: below `MAX_PORTS`.
pub(crate) fn pad_index(pad: PadId) -> usize {
    usize::from(pad.0)
}

const PORTS: usize = MAX_PORTS as usize;

#[derive(Debug, Clone, Default)]
enum Seat {
    #[default]
    Free,
    Playing(PadId, PadInfo, MapKind),
    /// In a game: the pad went away, and the port waits for it.
    Held(PadInfo, MapKind),
}
