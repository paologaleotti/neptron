//! The roster: which gamepad is on which port, as the frontend reads it.

use crate::device::{DeviceIdentity, MAX_PORTS};
use crate::notice::InputNotice;
use crate::pad_maps::MapKind;
use crate::silent::PadActivity;

/// The ports that the core reads: ports 0 to 3, players 1 to 4. A pad on
/// a higher port drives the menus and the hotkeys only.
pub const PLAYER_PORTS: u8 = 4;

/// What a backend knows of a gamepad when it connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadInfo {
    /// The model: the SDL GUID (vendor, product, version). Two pads of one
    /// model have the same identity. Everything that is kept per pad is
    /// kept by it.
    pub identity: DeviceIdentity,
    /// Where the pad is plugged: a hash of the device path of the backend,
    /// the event node or, with the HIDAPI drivers of SDL3, the hidraw
    /// node. The kernel keeps the node while the pad stays plugged, so the
    /// path tells two pads of one model apart in one boot. A replug can
    /// change it.
    pub path: DeviceIdentity,
    /// The name to show, as the backend gives it.
    pub name: String,
}

impl PadInfo {
    /// The pad of a backend from its identity, its path and the bytes of
    /// its name. A name that is not UTF-8 is read with a lossy conversion:
    /// a device can report any bytes, and a name must never stop a run.
    pub fn from_device(identity: DeviceIdentity, path: DeviceIdentity, name: &[u8]) -> Self {
        Self {
            identity,
            path,
            name: String::from_utf8_lossy(name).into_owned(),
        }
    }
}

/// The rule of the ports for one run of the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortRule {
    /// No game runs. A pad that goes away frees its port, and the pads on
    /// higher ports move down, so the ports in use are always 0 to n-1.
    #[default]
    Menus,
    /// A game runs. A pad that goes away keeps its port until the run
    /// ends, and nothing moves.
    Game,
}

/// The pad of one port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    /// The pad.
    pub pad: PadInfo,
    /// False when the pad went away and the port waits for it.
    pub present: bool,
    /// The kind of map the pad uses. A pad of the run before that has not
    /// come back keeps the kind it had.
    pub map: MapKind,
}

/// The pad on each port, from port 0. The relaunch file carries it to the
/// next run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Roster {
    ports: [Option<RosterEntry>; MAX_PORTS as usize],
}

/// A source that gives gamepads their ports: what the frontend reads of it
/// beyond its events.
pub trait PadRoster {
    /// The pad on each port now. It allocates the names, so read it when
    /// a page is built or a run ends, not in every frame.
    fn roster(&self) -> Roster;

    /// A number that changes each time the roster changes. It allocates
    /// nothing, so the frontend can compare it in every frame.
    fn roster_changes(&self) -> u64;

    /// Moves the notices of the source since the last call to `out`. It
    /// allocates nothing when there is none, so the frontend can call it
    /// in every frame. A source with no notice gives none.
    fn take_notices(&self, _out: &mut Vec<InputNotice>) {}

    /// The connected pads, for the silent pad rule. It allocates nothing.
    /// A source with no pad gives none.
    fn activity(&self) -> PadActivity {
        PadActivity::default()
    }
}

impl Roster {
    /// No pad on any port.
    pub fn new() -> Self {
        Self::default()
    }

    /// The entry of each port, from port 0. The slice has `MAX_PORTS`
    /// entries.
    pub fn ports(&self) -> &[Option<RosterEntry>] {
        &self.ports
    }

    /// The entry of `port`, or `None` for a free port or a port outside
    /// the range.
    pub fn get(&self, port: u8) -> Option<&RosterEntry> {
        self.ports.get(usize::from(port)).and_then(Option::as_ref)
    }

    /// Sets the entry of `port`.
    ///
    /// # Errors
    ///
    /// [`RosterError::NoSuchPort`] for a port at or above `MAX_PORTS`.
    pub fn set(&mut self, port: u8, entry: Option<RosterEntry>) -> Result<(), RosterError> {
        let slot = self
            .ports
            .get_mut(usize::from(port))
            .ok_or(RosterError::NoSuchPort { port })?;
        *slot = entry;
        Ok(())
    }

    /// True when no port has a pad.
    pub fn is_empty(&self) -> bool {
        self.ports.iter().all(Option::is_none)
    }
}

/// An error of [`Roster`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RosterError {
    /// The port is outside the range.
    #[error("port {port} does not exist; the ports are 0 to {}", MAX_PORTS - 1)]
    NoSuchPort {
        /// The port given.
        port: u8,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: u8) -> RosterEntry {
        RosterEntry {
            pad: PadInfo {
                identity: DeviceIdentity(u64::from(n)),
                path: DeviceIdentity(0),
                name: format!("Pad {n}"),
            },
            present: true,
            map: MapKind::Known,
        }
    }

    #[test]
    fn a_roster_holds_one_entry_per_port() {
        let mut roster = Roster::new();
        assert!(roster.is_empty());
        assert_eq!(roster.ports().len(), usize::from(MAX_PORTS));
        roster.set(7, Some(entry(1))).unwrap();
        assert_eq!(roster.get(7), Some(&entry(1)));
        assert!(!roster.is_empty());
        assert_eq!(
            roster.set(8, Some(entry(2))),
            Err(RosterError::NoSuchPort { port: 8 })
        );
        assert_eq!(roster.get(8), None);
        assert_eq!(
            RosterError::NoSuchPort { port: 8 }.to_string(),
            "port 8 does not exist; the ports are 0 to 7"
        );
    }
}
