//! The keyboard as a RetroPad.
//!
//! Keys are physical positions, as USB HID usage ids. SDL scancodes use the
//! same numbers, so the map does not change with the keyboard layout. The
//! defaults are the ones of the RetroArch keyboard driver.

use neptron_types::{Control, InputEvent};

/// The device id of the keyboard in every [`InputEvent`] it sends.
pub const KEYBOARD_DEVICE_ID: u32 = 0;

const HID_A: u32 = 0x04;
const HID_Q: u32 = 0x14;
const HID_S: u32 = 0x16;
const HID_W: u32 = 0x1A;
const HID_X: u32 = 0x1B;
const HID_Z: u32 = 0x1D;
const HID_RETURN: u32 = 0x28;
const HID_RIGHT: u32 = 0x4F;
const HID_LEFT: u32 = 0x50;
const HID_DOWN: u32 = 0x51;
const HID_UP: u32 = 0x52;
const HID_RIGHT_SHIFT: u32 = 0xE5;

/// Each mapped key, as a USB HID usage id, and its control.
pub(crate) const KEY_MAP: [(u32, Control); 12] = [
    (HID_UP, Control::Up),
    (HID_DOWN, Control::Down),
    (HID_LEFT, Control::Left),
    (HID_RIGHT, Control::Right),
    (HID_X, Control::A),
    (HID_Z, Control::B),
    (HID_S, Control::X),
    (HID_A, Control::Y),
    (HID_Q, Control::L),
    (HID_W, Control::R),
    (HID_RETURN, Control::Start),
    (HID_RIGHT_SHIFT, Control::Select),
];

/// The control of the key with USB HID usage id `key`, or `None` for a key
/// with no control.
pub(crate) fn control_of(key: u32) -> Option<Control> {
    KEY_MAP
        .iter()
        .find(|(hid, _)| *hid == key)
        .map(|(_, control)| *control)
}

/// The event for a key press (`pressed`) or release of `key`. A repeat that
/// the keyboard sends while a key stays down gives no event, because the
/// button did not change.
pub(crate) fn key_event(key: u32, pressed: bool, repeat: bool) -> Option<InputEvent> {
    if repeat {
        return None;
    }
    control_of(key).map(|control| InputEvent {
        device: KEYBOARD_DEVICE_ID,
        control,
        value: i16::from(pressed),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_are_the_d_pad() {
        assert_eq!(control_of(HID_UP), Some(Control::Up));
        assert_eq!(control_of(HID_DOWN), Some(Control::Down));
        assert_eq!(control_of(HID_LEFT), Some(Control::Left));
        assert_eq!(control_of(HID_RIGHT), Some(Control::Right));
    }

    #[test]
    fn the_letters_are_the_face_buttons_and_shoulders() {
        assert_eq!(control_of(HID_X), Some(Control::A));
        assert_eq!(control_of(HID_Z), Some(Control::B));
        assert_eq!(control_of(HID_S), Some(Control::X));
        assert_eq!(control_of(HID_A), Some(Control::Y));
        assert_eq!(control_of(HID_Q), Some(Control::L));
        assert_eq!(control_of(HID_W), Some(Control::R));
    }

    #[test]
    fn return_is_start_and_right_shift_is_select() {
        assert_eq!(control_of(HID_RETURN), Some(Control::Start));
        assert_eq!(control_of(HID_RIGHT_SHIFT), Some(Control::Select));
    }

    #[test]
    fn other_keys_have_no_control() {
        // Escape, space, left shift and an id past the HID table.
        for key in [0x29, 0x2C, 0xE1, 0, 0xFFFF] {
            assert_eq!(control_of(key), None, "{key:#x}");
        }
    }

    #[test]
    fn each_key_and_each_control_appears_once() {
        for (i, (key, control)) in KEY_MAP.iter().enumerate() {
            for (other_key, other_control) in &KEY_MAP[i + 1..] {
                assert_ne!(key, other_key);
                assert_ne!(control, other_control);
            }
        }
    }

    #[test]
    fn a_press_is_1_and_a_release_is_0() {
        let press = InputEvent {
            device: KEYBOARD_DEVICE_ID,
            control: Control::B,
            value: 1,
        };
        assert_eq!(key_event(HID_Z, true, false), Some(press));
        assert_eq!(
            key_event(HID_Z, false, false),
            Some(InputEvent { value: 0, ..press })
        );
    }

    #[test]
    fn a_repeat_gives_no_event() {
        assert_eq!(key_event(HID_Z, true, true), None);
    }

    #[test]
    fn an_unmapped_key_gives_no_event() {
        assert_eq!(key_event(0x2C, true, false), None);
        assert_eq!(key_event(0x2C, false, false), None);
    }
}
