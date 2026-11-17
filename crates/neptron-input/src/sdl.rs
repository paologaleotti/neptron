//! SDL3 gamepads, read with no event pump and no video subsystem.

use std::cell::RefCell;
use std::path::Path;

use neptron_backend::InputSource;
use neptron_types::InputEvent;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::joystick::{Joystick, JoystickId};
use sdl3::{GamepadSubsystem, JoystickSubsystem};

use crate::device::DeviceIdentity;
use crate::ffi;
use crate::generic_layout::JoystickCounts;
use crate::mapper::Mapper;
use crate::mapping_file::{read_mapping_file, sdl_add_order};
use crate::notice::{InputNotice, NoticeQueue};
use crate::pad_maps::{PadLayout, PadMap, PadMaps, PlayerMaps};
use crate::port_table::PadId;
use crate::raw::{HatState, PadAxis, PadButton, RawInput};
use crate::raw_state::RawReads;
use crate::roster::{PadInfo, PadRoster, PortRule, Roster};
use crate::scan::{Found, connect_found, list_failed};
use crate::silent::PadActivity;

/// The number of polls between two scans for new gamepads. At 60 frames a
/// second a new gamepad is found within half a second.
pub const SCAN_INTERVAL_POLLS: u32 = 30;

/// The gamepads and joysticks that SDL3 lists, mapped to the RetroPad,
/// with hotplug.
///
/// The source turns off SDL's joystick and gamepad events and reads the
/// state of each pad itself: each `poll` calls `SDL_UpdateGamepads`,
/// which reads every joystick, and then the state of every button and
/// axis. No event pump is needed, so the source works on the device,
/// where no video subsystem exists, and in a window build, where the
/// window owns the one event pump of the process.
///
/// A joystick with a gamepad mapping (from a mapping file or
/// the database of SDL3) opens as a gamepad with the standard layout. Any
/// other joystick opens as a plain joystick with the generic layout, or
/// is ignored, with a notice, when it is not a gamepad. A map of the
/// player comes first, and reads the raw indices of the joystick of the
/// pad, a gamepad too. The source keeps the joystick of every pad open,
/// so a gamepad can be read by raw index. A pad that finds
/// every port in use is ignored until it reconnects. The ports follow the
/// rule of the run (see [`Mapper`]); each scan connects the pads it found
/// together, and the first one settles the ports.
///
/// Make the source and poll it on one thread. With the `sdl3` crate, every
/// `sdl3::init` of the process runs on the thread of the first one.
pub struct SdlGamepads {
    // Declared first, so that the pads close before the subsystems.
    open: Vec<OpenPad>,
    ignored: Vec<JoystickId>,
    mapper: Mapper,
    maps: PlayerMaps,
    /// Set by new maps: the next poll gives each open pad its map again.
    remap: bool,
    polls_to_scan: u32,
    // The frontend reads the notices through a shared reference
    // (`PadRoster`), while a game session owns the source.
    notices: RefCell<NoticeQueue>,
    gamepads: GamepadSubsystem,
    joysticks: JoystickSubsystem,
}

/// An error of [`SdlGamepads::open`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SdlInputError {
    /// An SDL3 call failed.
    #[error("SDL3 {step} failed: {message}")]
    Sdl {
        /// The step that failed.
        step: &'static str,
        /// The SDL error text.
        message: String,
    },
}

impl SdlGamepads {
    /// Starts the SDL3 joystick and gamepad subsystems, and adds the good
    /// lines of the mapping files at `mappings` to SDL3. The files are in
    /// priority order: for one GUID, a line of an earlier file wins over a
    /// line of a later file, and both win over the built-in line of SDL3.
    /// The first `poll` finds the pads that are already connected. The
    /// ports follow `rule`, and start with the pads of `earlier` (the
    /// roster of the run before) held on their ports.
    ///
    /// A missing or damaged mapping file gives notices and never stops
    /// the source.
    ///
    /// # Errors
    ///
    /// [`SdlInputError::Sdl`] when SDL3 cannot start or cannot start a
    /// subsystem.
    pub fn open(
        rule: PortRule,
        earlier: &Roster,
        mappings: &[&Path],
    ) -> Result<Self, SdlInputError> {
        let sdl = sdl3::init().map_err(|e| sdl_error("init", &e))?;
        let joysticks = sdl.joystick().map_err(|e| sdl_error("joystick init", &e))?;
        let gamepads = sdl.gamepad().map_err(|e| sdl_error("gamepad init", &e))?;
        // Nobody pumps events for this source, and a window build pumps them
        // elsewhere. With the events off, SDL queues none, and only
        // `SDL_UpdateGamepads` reads the devices.
        joysticks.set_joystick_events_enabled(false);
        gamepads.set_events_processing_state(false);
        let mut notices = NoticeQueue::default();
        for path in sdl_add_order(mappings) {
            add_mappings(&gamepads, path, &mut notices);
        }
        Ok(Self {
            open: Vec::with_capacity(usize::from(crate::MAX_PORTS)),
            ignored: Vec::new(),
            mapper: Mapper::with_rule(rule, earlier),
            maps: PlayerMaps::new(),
            remap: false,
            polls_to_scan: 0,
            notices: RefCell::new(notices),
            gamepads,
            joysticks,
        })
    }

    /// Moves the notices since the last call to `out`.
    pub fn take_notices(&self, out: &mut Vec<InputNotice>) {
        self.notices.borrow_mut().drain_into(out);
    }

    fn remove_disconnected(&mut self, out: &mut Vec<InputEvent>) {
        let mut i = 0;
        while let Some(pad) = self.open.get(i) {
            if pad.device.connected() {
                i += 1;
                continue;
            }
            let gone = self.open.remove(i);
            let device = self.mapper.device_of(gone.id);
            let notice = match (device, self.mapper.disconnect_gamepad(gone.id, out)) {
                (Some(device), Ok(())) => Some(InputNotice::Disconnected { device }),
                (_, Err(e)) => Some(lost_track(&e)),
                (None, Ok(())) => None,
            };
            if let Some(notice) = notice {
                self.notices.get_mut().push(notice);
            }
        }
    }

    fn scan(&mut self, out: &mut Vec<InputEvent>) {
        let ids = match self.joysticks.joysticks() {
            Ok(ids) => ids,
            Err(e) => {
                list_failed(
                    &mut self.mapper,
                    "the SDL3 joystick list",
                    e.to_string(),
                    out,
                    self.notices.get_mut(),
                );
                return;
            }
        };
        self.ignored.retain(|id| ids.contains(id));
        let mut found = Vec::new();
        for id in ids {
            let known = self.open.iter().any(|pad| pad.joystick == id);
            if !known
                && !self.ignored.contains(&id)
                && let Some(pad) = self.open_pad(id)
            {
                found.push(pad);
            }
        }
        let scan = connect_found(
            &mut self.mapper,
            &self.maps,
            found,
            out,
            self.notices.get_mut(),
        );
        self.ignored
            .extend(scan.refused.into_iter().map(|(joystick, ..)| joystick));
        for opened in scan.opened {
            let (joystick, pending, layout, info) = opened.handle;
            match Device::with_map(pending, opened.map) {
                Some(device) => self.open.push(OpenPad {
                    device,
                    joystick,
                    id: opened.id,
                    layout,
                    info,
                }),
                None => self.refuse_map(joystick, opened.id, out),
            }
        }
    }

    /// Gives each open pad the map of the maps now. A pad whose map
    /// changes is read with its new map from the next read on. A pad that
    /// only the map of the player could read is dropped, as a pad that is
    /// not a gamepad.
    fn remap(&mut self, out: &mut Vec<InputEvent>) {
        for pad in std::mem::take(&mut self.open) {
            let old = self.mapper.map_of(pad.id);
            match self.maps.map(pad.info.identity, pad.layout) {
                Ok(map) if Some(map) == old => self.open.push(pad),
                Ok(map) => match Device::with_map(pad.device.into_pending(), map) {
                    Some(device) => {
                        if let Err(e) = self.mapper.set_map(pad.id, map, out) {
                            self.notices.get_mut().push(lost_track(&e));
                        }
                        self.open.push(OpenPad { device, ..pad });
                    }
                    None => self.refuse_map(pad.joystick, pad.id, out),
                },
                Err(reason) => {
                    if let Err(e) = self.mapper.disconnect_gamepad(pad.id, out) {
                        self.notices.get_mut().push(lost_track(&e));
                    }
                    self.ignored.push(pad.joystick);
                    self.notices.get_mut().push(InputNotice::NotAGamepad {
                        pad: pad.info,
                        reason,
                    });
                }
            }
        }
    }

    /// Opens the joystick `id` as a gamepad when SDL3 has a mapping for
    /// it, else as a plain joystick, and reads what the port rules need of
    /// it. A joystick that cannot be opened is ignored with a notice.
    fn open_pad(
        &mut self,
        id: JoystickId,
    ) -> Option<Found<(JoystickId, Pending, PadLayout, PadInfo)>> {
        let guid = self.gamepads.guid_for_id(id);
        let what = format!("SDL joystick {} with GUID {}", id.raw(), guid.string());
        // The name of the gamepad API is the device name when the mapping
        // has none, which can be any bytes, and the crate unwraps it as
        // UTF-8. The joystick API reads it with a lossy conversion.
        let joystick = match self.joysticks.open(id) {
            Ok(joystick) => joystick,
            Err(e) => {
                self.ignore(id, what, e.to_string());
                return None;
            }
        };
        let name = joystick.name();
        // The gamepad call of the `sdl3` crate unwraps the path as UTF-8.
        // The joystick call of `ffi` reads it lossy, for a plain joystick
        // and a gamepad alike. A joystick with no path gets the empty one.
        let path = ffi::joystick_path(id).unwrap_or_default();
        let info = PadInfo::from_device(
            DeviceIdentity::from_parts(&[&guid.raw().data]),
            DeviceIdentity::from_parts(&[path.as_bytes()]),
            name.as_bytes(),
        );
        let (device, layout) = if self.gamepads.is_gamepad(id) {
            match self.gamepads.open(id) {
                // SDL counts the opens of a joystick: the source keeps both
                // open, so a map of the player can read the raw indices.
                Ok(pad) => (Pending::Gamepad(pad, joystick), PadLayout::Gamepad),
                Err(e) => {
                    self.ignore(id, what, e.to_string());
                    return None;
                }
            }
        } else {
            let counts = JoystickCounts {
                buttons: joystick.num_buttons(),
                hats: joystick.num_hats(),
                axes: joystick.num_axes(),
            };
            let layout = PadLayout::Joystick(counts);
            (Pending::Joystick(joystick), layout)
        };
        Some(Found {
            handle: (id, device, layout, info.clone()),
            info,
            layout,
            what,
        })
    }

    /// Drops a pad whose map does not read its kind of device: a map of
    /// the standard layout for a plain joystick, or a generic map for a
    /// gamepad. `PlayerMaps` never gives one.
    fn refuse_map(&mut self, joystick: JoystickId, id: PadId, out: &mut Vec<InputEvent>) {
        debug_assert!(false, "a map that does not read its device");
        if let Err(e) = self.mapper.disconnect_gamepad(id, out) {
            self.notices.get_mut().push(lost_track(&e));
        }
        self.ignore(
            joystick,
            format!("SDL joystick {}", joystick.raw()),
            "its map does not read this kind of device".to_owned(),
        );
    }

    fn ignore(&mut self, id: JoystickId, what: String, reason: String) {
        self.ignored.push(id);
        self.notices
            .get_mut()
            .push(InputNotice::Ignored { what, reason });
    }

    fn read(&mut self, out: &mut Vec<InputEvent>) {
        for pad in &self.open {
            let mut apply = |input| self.mapper.apply_gamepad(pad.id, input, out);
            let result = match &pad.device {
                Device::Gamepad(gamepad, _) => read_gamepad(gamepad, &mut apply),
                Device::Joystick(joystick, reads, _) => read_joystick(joystick, *reads, &mut apply),
            };
            if let Err(e) = result {
                self.notices.get_mut().push(lost_track(&e));
            }
        }
    }
}

impl InputSource for SdlGamepads {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        self.gamepads.update();
        self.remove_disconnected(out);
        if self.polls_to_scan == 0 {
            self.scan(out);
            self.polls_to_scan = SCAN_INTERVAL_POLLS;
        }
        self.polls_to_scan -= 1;
        if std::mem::take(&mut self.remap) {
            self.remap(out);
        }
        self.read(out);
    }
}

impl PadRoster for SdlGamepads {
    fn roster(&self) -> Roster {
        self.mapper.roster()
    }

    fn roster_changes(&self) -> u64 {
        self.mapper.roster_changes()
    }

    fn take_notices(&self, out: &mut Vec<InputNotice>) {
        SdlGamepads::take_notices(self, out);
    }

    fn activity(&self) -> PadActivity {
        self.mapper.activity()
    }

    fn set_player_maps(&mut self, maps: &PlayerMaps) {
        self.maps = maps.clone();
        self.remap = true;
    }
}

impl std::fmt::Debug for SdlGamepads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let devices: Vec<Option<u32>> = self
            .open
            .iter()
            .map(|pad| self.mapper.device_of(pad.id))
            .collect();
        f.debug_struct("SdlGamepads")
            .field("devices", &devices)
            .field("polls_to_scan", &self.polls_to_scan)
            .finish_non_exhaustive()
    }
}

/// The open SDL3 device of a pad that has no map yet. A gamepad keeps its
/// joystick open too.
enum Pending {
    Gamepad(Gamepad, Joystick),
    Joystick(Joystick),
}

/// The open SDL3 device of a pad, with what it reads. A pad that reads raw
/// indices holds the counts it reads, so a joystick with nothing to read
/// cannot be built.
enum Device {
    /// A pad with a gamepad mapping, read in the standard layout.
    Gamepad(Gamepad, Joystick),
    /// A pad read by raw index: a joystick with the generic layout, or any
    /// pad with a map of the player. A gamepad read this way keeps its
    /// gamepad open, so a reset of the map reads the standard layout
    /// again with no reconnect.
    Joystick(Joystick, RawReads, Option<Gamepad>),
}

impl Device {
    /// The device that reads `pending` with `map`, or `None` when the map
    /// does not read that kind of device. A map of the player reads any
    /// pad by raw index, so it means the same thing for a gamepad and a
    /// plain joystick.
    fn with_map(pending: Pending, map: PadMap) -> Option<Self> {
        match (pending, map) {
            (Pending::Gamepad(pad, joystick), PadMap::Known) => Some(Self::Gamepad(pad, joystick)),
            (Pending::Joystick(joystick), PadMap::Generic(layout)) => {
                Some(Self::Joystick(joystick, layout.reads(), None))
            }
            (Pending::Gamepad(pad, joystick), PadMap::Player(player)) => {
                Some(Self::Joystick(joystick, player.reads(), Some(pad)))
            }
            (Pending::Joystick(joystick), PadMap::Player(player)) => {
                Some(Self::Joystick(joystick, player.reads(), None))
            }
            (Pending::Gamepad(..), PadMap::Generic(_)) | (Pending::Joystick(_), PadMap::Known) => {
                None
            }
        }
    }

    /// The open device with no map, to give it another one.
    fn into_pending(self) -> Pending {
        match self {
            Self::Gamepad(pad, joystick) | Self::Joystick(joystick, _, Some(pad)) => {
                Pending::Gamepad(pad, joystick)
            }
            Self::Joystick(joystick, _, None) => Pending::Joystick(joystick),
        }
    }

    fn joystick(&self) -> &Joystick {
        match self {
            Self::Gamepad(_, joystick) | Self::Joystick(joystick, ..) => joystick,
        }
    }

    fn connected(&self) -> bool {
        self.joystick().connected()
    }
}

struct OpenPad {
    device: Device,
    joystick: JoystickId,
    id: PadId,
    layout: PadLayout,
    info: PadInfo,
}

/// Gives `apply` the state of every button and axis of a gamepad.
fn read_gamepad(
    pad: &Gamepad,
    apply: &mut dyn FnMut(RawInput) -> Result<(), crate::MapperError>,
) -> Result<(), crate::MapperError> {
    for (sdl, button) in BUTTONS {
        apply(RawInput::Button {
            button,
            pressed: pad.button(sdl),
        })?;
    }
    for (sdl, axis) in AXES {
        apply(RawInput::Axis {
            axis,
            value: pad.axis(sdl),
        })?;
    }
    Ok(())
}

/// Gives `apply` the state of each button, hat and axis index of a
/// joystick in `reads`. A read that SDL3 refuses reads as neutral: an
/// index past the count that SDL3 gave.
fn read_joystick(
    joystick: &Joystick,
    reads: RawReads,
    apply: &mut dyn FnMut(RawInput) -> Result<(), crate::MapperError>,
) -> Result<(), crate::MapperError> {
    for index in 0..reads.buttons {
        let pressed = joystick.button(u32::from(index)).unwrap_or(false);
        apply(RawInput::JoyButton { index, pressed })?;
    }
    for index in 0..reads.hats {
        let hat = joystick
            .hat(u32::from(index))
            .map_or(HatState::default(), |h| HatState(h.to_raw()));
        apply(RawInput::JoyHat { index, hat })?;
    }
    for index in 0..reads.axes {
        let value = joystick.axis(u32::from(index)).unwrap_or(0);
        apply(RawInput::JoyAxis { index, value })?;
    }
    Ok(())
}

/// Adds each good line of the mapping file at `path` to SDL3, with a
/// notice for the file and for each line that is not used.
fn add_mappings(gamepads: &GamepadSubsystem, path: &Path, notices: &mut NoticeQueue) {
    let mappings = read_mapping_file(path, sdl3::get_platform());
    for notice in mappings.notices {
        notices.push(notice);
    }
    for (line, text) in mappings.lines {
        if let Err(e) = gamepads.add_mapping(&text) {
            notices.push(InputNotice::MappingFile {
                path: path.display().to_string(),
                line,
                reason: e.to_string(),
            });
        }
    }
}

/// Each SDL3 button that has a RetroPad control, and its name in the
/// standard layout.
const BUTTONS: [(Button, PadButton); 14] = [
    (Button::South, PadButton::South),
    (Button::East, PadButton::East),
    (Button::West, PadButton::West),
    (Button::North, PadButton::North),
    (Button::Back, PadButton::Back),
    (Button::Start, PadButton::Start),
    (Button::LeftStick, PadButton::LeftStick),
    (Button::RightStick, PadButton::RightStick),
    (Button::LeftShoulder, PadButton::LeftShoulder),
    (Button::RightShoulder, PadButton::RightShoulder),
    (Button::DPadUp, PadButton::DPadUp),
    (Button::DPadDown, PadButton::DPadDown),
    (Button::DPadLeft, PadButton::DPadLeft),
    (Button::DPadRight, PadButton::DPadRight),
];

/// Each SDL3 axis and its name in the standard layout.
const AXES: [(Axis, PadAxis); 6] = [
    (Axis::LeftX, PadAxis::LeftX),
    (Axis::LeftY, PadAxis::LeftY),
    (Axis::RightX, PadAxis::RightX),
    (Axis::RightY, PadAxis::RightY),
    (Axis::TriggerLeft, PadAxis::LeftTrigger),
    (Axis::TriggerRight, PadAxis::RightTrigger),
];

fn sdl_error(step: &'static str, error: &sdl3::Error) -> SdlInputError {
    SdlInputError::Sdl {
        step,
        message: error.to_string(),
    }
}

/// The notice for a mapper error on a pad that the source holds. The
/// source connects each pad before it reads it and disconnects it only
/// when it drops it, so this shows a defect, not a device problem.
fn lost_track(error: &crate::MapperError) -> InputNotice {
    InputNotice::Ignored {
        what: "an open gamepad".to_owned(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad_map::{PAD_BUTTON_MAP, pad_control};

    #[test]
    fn every_mapped_pad_button_is_read_from_sdl() {
        for (button, _) in PAD_BUTTON_MAP {
            assert!(
                BUTTONS.iter().any(|(_, b)| *b == button),
                "{button:?} is never read"
            );
        }
        for (_, button) in BUTTONS {
            assert!(pad_control(button).is_some(), "{button:?} has no control");
        }
    }

    #[test]
    fn the_sdl_names_match_the_standard_layout() {
        for (sdl, button) in BUTTONS {
            let name = format!("{sdl:?}");
            assert_eq!(name, format!("{button:?}"), "{sdl:?}");
        }
        let pairs: Vec<_> = AXES
            .iter()
            .map(|(sdl, axis)| (format!("{sdl:?}"), format!("{axis:?}")))
            .collect();
        assert_eq!(
            pairs,
            [
                ("LeftX", "LeftX"),
                ("LeftY", "LeftY"),
                ("RightX", "RightX"),
                ("RightY", "RightY"),
                ("TriggerLeft", "LeftTrigger"),
                ("TriggerRight", "RightTrigger"),
            ]
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
        );
    }
}
