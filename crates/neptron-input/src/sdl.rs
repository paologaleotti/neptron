//! SDL3 gamepads, read with no event pump and no video subsystem.

use neptron_backend::InputSource;
use neptron_types::InputEvent;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::joystick::JoystickId;
use sdl3::{GamepadSubsystem, JoystickSubsystem};

use crate::device::DeviceIdentity;
use crate::mapper::Mapper;
use crate::notice::{InputNotice, NoticeQueue};
use crate::raw::{PadAxis, PadButton, RawInput};

/// The number of polls between two scans for new gamepads. At 60 frames a
/// second a new gamepad is found within half a second.
pub const SCAN_INTERVAL_POLLS: u32 = 30;

/// The gamepads that SDL3 knows, mapped to the RetroPad, with hotplug.
///
/// The source turns off SDL's joystick and gamepad events and reads the
/// state of each gamepad itself: each `poll` calls `SDL_UpdateGamepads`,
/// which reads the devices, and then the state of every button and axis.
/// No event pump is needed, so the source works on the device, where no
/// video subsystem exists, and in a window build, where the window owns the
/// one event pump of the process.
///
/// A joystick that SDL has no gamepad mapping for is ignored, with a
/// notice. A gamepad that finds every port in use is ignored until it
/// reconnects.
///
/// Make the source and poll it on one thread. With the `sdl3` crate, every
/// `sdl3::init` of the process runs on the thread of the first one.
pub struct SdlGamepads {
    // Declared first, so that the gamepads close before the subsystems.
    open: Vec<OpenPad>,
    ignored: Vec<JoystickId>,
    mapper: Mapper,
    polls_to_scan: u32,
    notices: NoticeQueue,
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
    /// Starts the SDL3 joystick and gamepad subsystems. The first `poll`
    /// finds the gamepads that are already connected.
    ///
    /// # Errors
    ///
    /// [`SdlInputError::Sdl`] when SDL3 cannot start or cannot start a
    /// subsystem.
    pub fn open() -> Result<Self, SdlInputError> {
        let sdl = sdl3::init().map_err(|e| sdl_error("init", &e))?;
        let joysticks = sdl.joystick().map_err(|e| sdl_error("joystick init", &e))?;
        let gamepads = sdl.gamepad().map_err(|e| sdl_error("gamepad init", &e))?;
        // Nobody pumps events for this source, and a window build pumps them
        // elsewhere. With the events off, SDL queues none, and only
        // `SDL_UpdateGamepads` reads the devices.
        joysticks.set_joystick_events_enabled(false);
        gamepads.set_events_processing_state(false);
        Ok(Self {
            open: Vec::with_capacity(usize::from(crate::MAX_PORTS)),
            ignored: Vec::new(),
            mapper: Mapper::new(),
            polls_to_scan: 0,
            notices: NoticeQueue::default(),
            gamepads,
            joysticks,
        })
    }

    /// Moves the notices since the last call to `out`.
    pub fn take_notices(&mut self, out: &mut Vec<InputNotice>) {
        self.notices.drain_into(out);
    }

    fn remove_disconnected(&mut self, out: &mut Vec<InputEvent>) {
        let mut i = 0;
        while let Some(pad) = self.open.get(i) {
            if pad.pad.connected() {
                i += 1;
                continue;
            }
            let gone = self.open.remove(i);
            if let Err(e) = self.mapper.disconnect(gone.device, out) {
                self.notices.push(lost_track(gone.device, &e));
            }
            self.notices.push(InputNotice::Disconnected {
                device: gone.device,
            });
        }
    }

    fn scan(&mut self) {
        let ids = match self.joysticks.joysticks() {
            Ok(ids) => ids,
            Err(e) => {
                self.notices.push(InputNotice::Ignored {
                    what: "the SDL3 joystick list".to_owned(),
                    reason: e.to_string(),
                });
                return;
            }
        };
        self.ignored.retain(|id| ids.contains(id));
        for id in ids {
            let known = self.open.iter().any(|pad| pad.joystick == id);
            if !known && !self.ignored.contains(&id) {
                self.add(id);
            }
        }
    }

    fn add(&mut self, id: JoystickId) {
        let guid = self.gamepads.guid_for_id(id);
        let what = format!("SDL joystick {} with GUID {}", id.raw(), guid.string());
        let ignore = |source: &mut Self, what: String, reason: String| {
            source.ignored.push(id);
            source.notices.push(InputNotice::Ignored { what, reason });
        };
        if !self.gamepads.is_gamepad(id) {
            let reason = "SDL3 has no gamepad mapping for it".to_owned();
            return ignore(self, what, reason);
        }
        let pad = match self.gamepads.open(id) {
            Ok(pad) => pad,
            Err(e) => return ignore(self, what, e.to_string()),
        };
        let identity = DeviceIdentity::from_parts(&[&guid.raw().data]);
        let device = match self.mapper.connect_gamepad(identity) {
            Ok(device) => device,
            Err(e) => return ignore(self, what, e.to_string()),
        };
        self.open.push(OpenPad {
            pad,
            joystick: id,
            device,
        });
        self.notices.push(InputNotice::Connected { device, what });
    }

    fn read(&mut self, out: &mut Vec<InputEvent>) {
        for pad in &self.open {
            let buttons = BUTTONS.iter().map(|(sdl, button)| RawInput::Button {
                button: *button,
                pressed: pad.pad.button(*sdl),
            });
            let axes = AXES.iter().map(|(sdl, axis)| RawInput::Axis {
                axis: *axis,
                value: pad.pad.axis(*sdl),
            });
            for input in buttons.chain(axes) {
                if let Err(e) = self.mapper.apply(pad.device, input, out) {
                    self.notices.push(lost_track(pad.device, &e));
                }
            }
        }
    }
}

impl InputSource for SdlGamepads {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        self.gamepads.update();
        self.remove_disconnected(out);
        if self.polls_to_scan == 0 {
            self.scan();
            self.polls_to_scan = SCAN_INTERVAL_POLLS;
        }
        self.polls_to_scan -= 1;
        self.read(out);
    }
}

impl std::fmt::Debug for SdlGamepads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let devices: Vec<u32> = self.open.iter().map(|pad| pad.device).collect();
        f.debug_struct("SdlGamepads")
            .field("devices", &devices)
            .field("polls_to_scan", &self.polls_to_scan)
            .finish_non_exhaustive()
    }
}

struct OpenPad {
    pad: Gamepad,
    joystick: JoystickId,
    device: u32,
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

/// The notice for a mapper error on a device that the source holds. The
/// source connects each device before it reads it and disconnects it only
/// when it drops it, so this shows a defect, not a device problem.
fn lost_track(device: u32, error: &crate::MapperError) -> InputNotice {
    InputNotice::Ignored {
        what: format!("device {device:#x}"),
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
