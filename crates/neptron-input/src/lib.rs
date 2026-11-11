//! Input: SDL3 gamepads, evdev keyboards, RetroPad mapping and hotplug.
//!
//! The pure parts decide: device ids and ports, the mapping of raw device
//! input to the RetroPad, the port of each device across hotplug, and the
//! hotkeys of the frontend. The device backends behind the `sdl3` and
//! `evdev` features only read devices and feed the pure parts.

#![forbid(unsafe_code)]

mod device;
mod hotkeys;
mod key_map;
mod mapper;
mod pad_map;
mod ports;
mod raw;
mod state;

pub use device::{
    DeviceIdentity, GAMEPAD_DEVICE_BASE, KEYBOARD_DEVICE_BASE, MAX_KEYBOARDS, MAX_PORTS, port_of,
};
pub use hotkeys::{
    BLIND_RECOVERY_COMBO, BLIND_RECOVERY_HOLD_NS, Hotkey, Hotkeys, QUICK_MENU_COMBO,
};
pub use key_map::{KEY_MAP, key_control, looks_like_keyboard};
pub use mapper::{Mapper, MapperError};
pub use pad_map::{
    AxisTarget, PAD_BUTTON_MAP, STICK_DEAD_ZONE, TRIGGER_THRESHOLD, axis_target, hat_values,
    pad_control, stick_value, trigger_value,
};
pub use ports::PortStates;
pub use raw::{HatState, KeyAction, KeyCode, PadAxis, PadButton, RawInput};
