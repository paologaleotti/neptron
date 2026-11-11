//! Input: SDL3 gamepads, evdev keyboards, RetroPad mapping and hotplug.
//!
//! The pure parts decide: device ids and ports, the mapping of raw device
//! input to the RetroPad, the port of each device across hotplug, and the
//! hotkeys of the frontend. The device backends behind the `sdl3` and
//! `evdev` features only read devices and feed the pure parts.

#![forbid(unsafe_code)]

mod device;
mod pad_map;
mod raw;

pub use device::{
    DeviceIdentity, GAMEPAD_DEVICE_BASE, KEYBOARD_DEVICE_BASE, MAX_KEYBOARDS, MAX_PORTS, port_of,
};
pub use pad_map::{
    AxisTarget, PAD_BUTTON_MAP, STICK_DEAD_ZONE, TRIGGER_THRESHOLD, axis_target, hat_values,
    pad_control, stick_value, trigger_value,
};
pub use raw::{HatState, PadAxis, PadButton, RawInput};
