//! Input: SDL3 gamepads, evdev keyboards, RetroPad mapping and hotplug.
//!
//! The pure parts decide: device ids and ports, the mapping of raw device
//! input to the RetroPad, the port of each device across hotplug, and the
//! hotkeys of the frontend. The device backends behind the `sdl3` and
//! `evdev` features only read devices and feed the pure parts.

#![forbid(unsafe_code)]

mod device;

pub use device::{
    DeviceIdentity, GAMEPAD_DEVICE_BASE, KEYBOARD_DEVICE_BASE, MAX_KEYBOARDS, MAX_PORTS, port_of,
};
