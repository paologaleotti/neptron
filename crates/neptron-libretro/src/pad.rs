//! The RetroPad state of one port, as the core reads it.

use neptron_libretro_sys::{
    RETRO_DEVICE_ANALOG, RETRO_DEVICE_ID_ANALOG_X, RETRO_DEVICE_ID_ANALOG_Y,
    RETRO_DEVICE_ID_JOYPAD_MASK, RETRO_DEVICE_INDEX_ANALOG_LEFT, RETRO_DEVICE_INDEX_ANALOG_RIGHT,
    RETRO_DEVICE_JOYPAD,
};
use neptron_types::Control;

/// The number of ports the host answers for. A core that reads a higher
/// port gets 0.
pub const MAX_PORTS: usize = 4;

/// The buttons and sticks of one RetroPad.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Pad {
    /// One bit per button, bit `n` for `RETRO_DEVICE_ID_JOYPAD` id `n`.
    pub buttons: u16,
    /// Left stick, x then y, -32768 to 32767.
    pub left: [i16; 2],
    /// Right stick, x then y.
    pub right: [i16; 2],
}

/// Why a pad could not be set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    /// The port is at or above [`MAX_PORTS`].
    #[error("port {port} does not exist; the host has ports 0 to {}", MAX_PORTS - 1)]
    NoSuchPort {
        /// The port given.
        port: usize,
    },
}

impl Pad {
    /// Sets one control from an input event value: a button is pressed when
    /// the value is not 0, an axis takes the value.
    pub fn apply(&mut self, control: Control, value: i16) {
        if let Some(id) = button_id(control) {
            let bit = 1u16 << id;
            if value != 0 {
                self.buttons |= bit;
            } else {
                self.buttons &= !bit;
            }
            return;
        }
        match control {
            Control::LeftX => self.left[0] = value,
            Control::LeftY => self.left[1] = value,
            Control::RightX => self.right[0] = value,
            Control::RightY => self.right[1] = value,
            _ => {}
        }
    }

    /// The answer to `input_state(port, device, index, id)` for this pad.
    pub(crate) fn state(&self, device: u32, index: u32, id: u32) -> i16 {
        match device {
            RETRO_DEVICE_JOYPAD if id == RETRO_DEVICE_ID_JOYPAD_MASK => {
                i16::from_ne_bytes(self.buttons.to_ne_bytes())
            }
            RETRO_DEVICE_JOYPAD if id < 16 => i16::from((self.buttons >> id) & 1 == 1),
            RETRO_DEVICE_ANALOG => {
                let stick = match index {
                    RETRO_DEVICE_INDEX_ANALOG_LEFT => self.left,
                    RETRO_DEVICE_INDEX_ANALOG_RIGHT => self.right,
                    _ => return 0,
                };
                match id {
                    RETRO_DEVICE_ID_ANALOG_X => stick[0],
                    RETRO_DEVICE_ID_ANALOG_Y => stick[1],
                    _ => 0,
                }
            }
            _ => 0,
        }
    }
}

/// The `RETRO_DEVICE_ID_JOYPAD` id of a button. The `Control` order is the
/// libretro id order.
fn button_id(control: Control) -> Option<u32> {
    Some(match control {
        Control::B => 0,
        Control::Y => 1,
        Control::Select => 2,
        Control::Start => 3,
        Control::Up => 4,
        Control::Down => 5,
        Control::Left => 6,
        Control::Right => 7,
        Control::A => 8,
        Control::X => 9,
        Control::L => 10,
        Control::R => 11,
        Control::L2 => 12,
        Control::R2 => 13,
        Control::L3 => 14,
        Control::R3 => 15,
        Control::LeftX | Control::LeftY | Control::RightX | Control::RightY => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use neptron_libretro_sys::*;

    #[test]
    fn button_ids_match_libretro() {
        let pairs = [
            (Control::B, RETRO_DEVICE_ID_JOYPAD_B),
            (Control::Y, RETRO_DEVICE_ID_JOYPAD_Y),
            (Control::Select, RETRO_DEVICE_ID_JOYPAD_SELECT),
            (Control::Start, RETRO_DEVICE_ID_JOYPAD_START),
            (Control::Up, RETRO_DEVICE_ID_JOYPAD_UP),
            (Control::Down, RETRO_DEVICE_ID_JOYPAD_DOWN),
            (Control::Left, RETRO_DEVICE_ID_JOYPAD_LEFT),
            (Control::Right, RETRO_DEVICE_ID_JOYPAD_RIGHT),
            (Control::A, RETRO_DEVICE_ID_JOYPAD_A),
            (Control::X, RETRO_DEVICE_ID_JOYPAD_X),
            (Control::L, RETRO_DEVICE_ID_JOYPAD_L),
            (Control::R, RETRO_DEVICE_ID_JOYPAD_R),
            (Control::L2, RETRO_DEVICE_ID_JOYPAD_L2),
            (Control::R2, RETRO_DEVICE_ID_JOYPAD_R2),
            (Control::L3, RETRO_DEVICE_ID_JOYPAD_L3),
            (Control::R3, RETRO_DEVICE_ID_JOYPAD_R3),
        ];
        for (control, id) in pairs {
            assert_eq!(button_id(control), Some(id), "{control:?}");
        }
    }

    #[test]
    fn buttons_and_mask_read_back() {
        let mut pad = Pad::default();
        pad.apply(Control::Start, 1);
        pad.apply(Control::R3, 1);
        pad.apply(Control::A, 1);
        pad.apply(Control::A, 0);
        let j = RETRO_DEVICE_JOYPAD;
        assert_eq!(pad.state(j, 0, RETRO_DEVICE_ID_JOYPAD_START), 1);
        assert_eq!(pad.state(j, 0, RETRO_DEVICE_ID_JOYPAD_A), 0);
        assert_eq!(pad.state(j, 0, RETRO_DEVICE_ID_JOYPAD_R3), 1);
        assert_eq!(pad.state(j, 0, 16), 0);
        assert_eq!(
            pad.state(j, 0, RETRO_DEVICE_ID_JOYPAD_MASK),
            i16::from_ne_bytes(0x8008u16.to_ne_bytes())
        );
    }

    #[test]
    fn sticks_read_back() {
        let mut pad = Pad::default();
        pad.apply(Control::LeftX, -32768);
        pad.apply(Control::LeftY, 5);
        pad.apply(Control::RightX, 32767);
        pad.apply(Control::RightY, -7);
        let a = RETRO_DEVICE_ANALOG;
        assert_eq!(pad.state(a, 0, 0), -32768);
        assert_eq!(pad.state(a, 0, 1), 5);
        assert_eq!(pad.state(a, 1, 0), 32767);
        assert_eq!(pad.state(a, 1, 1), -7);
        assert_eq!(pad.state(a, 2, 0), 0);
        assert_eq!(pad.state(a, 0, 2), 0);
        assert_eq!(pad.state(RETRO_DEVICE_MOUSE, 0, 0), 0);
    }
}
