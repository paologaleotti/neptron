//! The keyboard mapped to the RetroPad, with the defaults of the RetroArch
//! keyboard driver for player one.

use neptron_types::Control;

use crate::raw::KeyCode;

/// Each mapped key, its RetroArch key name, and its control. The key name
/// ties the table to the shared fixture `tests/fixtures/retroarch-keyboard.txt`.
pub const KEY_MAP: [(KeyCode, &str, Control); 12] = [
    (KEY_UP, "up", Control::Up),
    (KEY_DOWN, "down", Control::Down),
    (KEY_LEFT, "left", Control::Left),
    (KEY_RIGHT, "right", Control::Right),
    (KEY_X, "x", Control::A),
    (KEY_Z, "z", Control::B),
    (KEY_S, "s", Control::X),
    (KEY_A, "a", Control::Y),
    (KEY_Q, "q", Control::L),
    (KEY_W, "w", Control::R),
    (KEY_ENTER, "enter", Control::Start),
    (KEY_RIGHTSHIFT, "rshift", Control::Select),
];

/// The control of `code`, or `None` for a key with no control.
pub fn key_control(code: KeyCode) -> Option<Control> {
    KEY_MAP
        .iter()
        .find(|(key, _, _)| *key == code)
        .map(|(_, _, control)| *control)
}

/// True when a device with these key codes is a keyboard that the keyboard
/// source reads: it has a key of [`KEY_MAP`], and no joystick or gamepad
/// button. `has_key` tells whether the device reports a key code.
///
/// Gamepads and joysticks belong to SDL3, and a device that is only a
/// power button or a media remote has no key of the map. A keyboard with a
/// built-in touchpad on one device node is a keyboard: its mouse events are
/// ignored.
pub fn looks_like_keyboard(has_key: impl Fn(u16) -> bool) -> bool {
    let has_mapped_key = KEY_MAP.iter().any(|(key, _, _)| has_key(key.0));
    let has_other_buttons = [BTN_TRIGGER, BTN_SOUTH].into_iter().any(&has_key);
    has_mapped_key && !has_other_buttons
}

const KEY_Q: KeyCode = KeyCode(16);
const KEY_W: KeyCode = KeyCode(17);
const KEY_ENTER: KeyCode = KeyCode(28);
const KEY_A: KeyCode = KeyCode(30);
const KEY_S: KeyCode = KeyCode(31);
const KEY_Z: KeyCode = KeyCode(44);
const KEY_X: KeyCode = KeyCode(45);
const KEY_RIGHTSHIFT: KeyCode = KeyCode(54);
const KEY_UP: KeyCode = KeyCode(103);
const KEY_LEFT: KeyCode = KeyCode(105);
const KEY_RIGHT: KeyCode = KeyCode(106);
const KEY_DOWN: KeyCode = KeyCode(108);

/// The first joystick button.
const BTN_TRIGGER: u16 = 0x120;
/// The first gamepad button.
const BTN_SOUTH: u16 = 0x130;

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/retroarch-keyboard.txt");

    fn control_name(control: Control) -> String {
        format!("{control:?}").to_lowercase()
    }

    #[test]
    fn the_table_gives_exactly_the_pairs_of_the_shared_fixture() {
        let fixture: Vec<(&str, &str)> = FIXTURE
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(|line| line.split_once(' '))
            .collect();
        assert_eq!(fixture.len(), KEY_MAP.len());
        for (name, control) in &fixture {
            let entry = KEY_MAP.iter().find(|(_, key_name, _)| key_name == name);
            let Some((_, _, mapped)) = entry else {
                panic!("the fixture key {name} is not in the table");
            };
            assert_eq!(control_name(*mapped), *control, "{name}");
        }
    }

    #[test]
    fn each_key_code_matches_its_linux_name() {
        // Values of linux/input-event-codes.h.
        let expected = [
            ("up", 103),
            ("down", 108),
            ("left", 105),
            ("right", 106),
            ("x", 45),
            ("z", 44),
            ("s", 31),
            ("a", 30),
            ("q", 16),
            ("w", 17),
            ("enter", 28),
            ("rshift", 54),
        ];
        for (name, code) in expected {
            let entry = KEY_MAP.iter().find(|(_, key_name, _)| *key_name == name);
            assert_eq!(entry.map(|(key, _, _)| key.0), Some(code), "{name}");
        }
    }

    #[test]
    fn each_key_and_each_control_appears_once() {
        for (i, (key, name, control)) in KEY_MAP.iter().enumerate() {
            for (other_key, other_name, other_control) in &KEY_MAP[i + 1..] {
                assert_ne!(key, other_key);
                assert_ne!(name, other_name);
                assert_ne!(control, other_control);
            }
        }
    }

    #[test]
    fn mapped_keys_give_their_control_and_others_none() {
        assert_eq!(key_control(KEY_ENTER), Some(Control::Start));
        assert_eq!(key_control(KEY_RIGHTSHIFT), Some(Control::Select));
        // Escape, space, left shift, F1 and a code past the key range.
        for code in [1, 57, 42, 59, 0x2FF] {
            assert_eq!(key_control(KeyCode(code)), None, "{code}");
        }
    }

    #[test]
    fn a_keyboard_is_recognised_and_other_devices_are_not() {
        let keyboard = |code: u16| code < 0x100;
        assert!(looks_like_keyboard(keyboard));
        let encoder = |code: u16| [103, 105, 106, 108, 28].contains(&code);
        assert!(looks_like_keyboard(encoder));
        let power_button = |code: u16| code == 116;
        assert!(!looks_like_keyboard(power_button));
        let gamepad = |code: u16| (0x130..0x13F).contains(&code) || code == 28;
        assert!(!looks_like_keyboard(gamepad));
        let joystick = |code: u16| (0x120..0x12F).contains(&code) || code == 103;
        assert!(!looks_like_keyboard(joystick));
        let keyboard_with_mouse = |code: u16| code < 0x100 || code == 0x110;
        assert!(looks_like_keyboard(keyboard_with_mouse));
    }
}
