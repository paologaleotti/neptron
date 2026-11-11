//! Device ids and ports.
//!
//! Every [`InputEvent`](neptron_types::InputEvent) carries a device id. The
//! id space has three ranges, so that the sources of one process never give
//! two devices the same id:
//!
//! | Ids | Devices | Port |
//! | --- | --- | --- |
//! | `0..MAX_PORTS` | Direct ids: the window keyboard (id 0), scripted input, the control API | the id |
//! | `GAMEPAD_DEVICE_BASE..+MAX_PORTS` | SDL3 gamepads | the id minus the base |
//! | `KEYBOARD_DEVICE_BASE..+MAX_KEYBOARDS` | evdev keyboards | 0 |
//!
//! A gamepad id holds its port, so the id stays the same while the device
//! stays connected, and a reconnect that gets its port back gets its id
//! back too.

/// The number of RetroPad ports, from port 0 (player one).
pub const MAX_PORTS: u8 = 8;

/// The number of keyboards that the keyboard source reads at one time.
pub const MAX_KEYBOARDS: u8 = 8;

/// The first device id of a gamepad. The gamepad on port `p` has the id
/// `GAMEPAD_DEVICE_BASE + p`.
pub const GAMEPAD_DEVICE_BASE: u32 = 0x100;

/// The first device id of a keyboard. Keyboard slot `k` has the id
/// `KEYBOARD_DEVICE_BASE + k`.
pub const KEYBOARD_DEVICE_BASE: u32 = 0x200;

/// The port of the events of `device`, or `None` for an id outside every
/// range.
pub fn port_of(device: u32) -> Option<u8> {
    let ports = u32::from(MAX_PORTS);
    let keyboards = u32::from(MAX_KEYBOARDS);
    if device < ports {
        u8::try_from(device).ok()
    } else if (GAMEPAD_DEVICE_BASE..GAMEPAD_DEVICE_BASE + ports).contains(&device) {
        u8::try_from(device - GAMEPAD_DEVICE_BASE).ok()
    } else if (KEYBOARD_DEVICE_BASE..KEYBOARD_DEVICE_BASE + keyboards).contains(&device) {
        Some(0)
    } else {
        None
    }
}

/// What tells two physical devices apart across a reconnect: a hash of the
/// parts that a backend can read, such as the SDL GUID (vendor, product,
/// version) and the serial number.
///
/// Two devices of one model with no serial number have the same identity.
/// A reconnect can then get the port of the other one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceIdentity(pub u64);

impl DeviceIdentity {
    /// The identity of a device from its parts, with 64-bit FNV-1a. The
    /// hash is fixed, so the identity is the same in every run.
    pub fn from_parts(parts: &[&[u8]]) -> Self {
        const OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01B3;
        let mut hash = OFFSET_BASIS;
        for part in parts {
            // The length goes in first, so that ("ab", "c") and ("a", "bc")
            // give different identities.
            let len = u64::try_from(part.len()).unwrap_or(u64::MAX);
            for byte in len.to_le_bytes().iter().chain(part.iter()) {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(PRIME);
            }
        }
        Self(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_ids_are_their_port() {
        assert_eq!(port_of(0), Some(0));
        assert_eq!(port_of(7), Some(7));
        assert_eq!(port_of(8), None);
        assert_eq!(port_of(0xFF), None);
    }

    #[test]
    fn gamepad_ids_hold_their_port() {
        assert_eq!(port_of(0x100), Some(0));
        assert_eq!(port_of(0x107), Some(7));
        assert_eq!(port_of(0x108), None);
    }

    #[test]
    fn keyboards_play_on_port_0() {
        assert_eq!(port_of(0x200), Some(0));
        assert_eq!(port_of(0x207), Some(0));
        assert_eq!(port_of(0x208), None);
    }

    #[test]
    fn ids_outside_the_ranges_have_no_port() {
        for id in [0x0FF, 0x1FF, 0x2FF, 0x300, u32::MAX] {
            assert_eq!(port_of(id), None, "{id:#x}");
        }
    }

    #[test]
    fn the_ranges_do_not_overlap() {
        let gamepads = GAMEPAD_DEVICE_BASE..GAMEPAD_DEVICE_BASE + u32::from(MAX_PORTS);
        let keyboards = KEYBOARD_DEVICE_BASE..KEYBOARD_DEVICE_BASE + u32::from(MAX_KEYBOARDS);
        assert!(u32::from(MAX_PORTS) <= gamepads.start);
        assert!(gamepads.end <= keyboards.start);
    }

    #[test]
    fn the_identity_is_fixed_and_separates_parts() {
        let pad = DeviceIdentity::from_parts(&[b"030000005e0400008e02000010010000", b"SN1"]);
        assert_eq!(
            pad,
            DeviceIdentity::from_parts(&[b"030000005e0400008e02000010010000", b"SN1"])
        );
        assert_ne!(
            pad,
            DeviceIdentity::from_parts(&[b"030000005e0400008e02000010010000", b"SN2"])
        );
        assert_ne!(
            DeviceIdentity::from_parts(&[b"ab", b"c"]),
            DeviceIdentity::from_parts(&[b"a", b"bc"])
        );
        assert_eq!(
            DeviceIdentity::from_parts(&[]),
            DeviceIdentity(0xCBF2_9CE4_8422_2325)
        );
    }
}
