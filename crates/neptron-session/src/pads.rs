//! The RetroPad of each port as the core sees it: the port state, less the
//! buttons that the quick menu holds back.

use neptron_libretro::Pad;
use neptron_types::Control;

use neptron_input::{PortStates, QUICK_MENU_COMBO};

/// The ports the core gets: the ports of the libretro host.
pub const CORE_PORTS: usize = neptron_libretro::MAX_PORTS;

/// The 16 RetroPad buttons. A mask bit is the index in this list.
const BUTTONS: [Control; 16] = [
    Control::B,
    Control::Y,
    Control::Select,
    Control::Start,
    Control::Up,
    Control::Down,
    Control::Left,
    Control::Right,
    Control::A,
    Control::X,
    Control::L,
    Control::R,
    Control::L2,
    Control::R2,
    Control::L3,
    Control::R3,
];

const AXES: [Control; 4] = [
    Control::LeftX,
    Control::LeftY,
    Control::RightX,
    Control::RightY,
];

/// Buttons that the core sees as up until the player releases them.
///
/// When the quick menu closes, the buttons still held were pressed for the
/// menu, not for the game. The core sees each one up until its release, so
/// it sees no new press. A Start hold that opened the menu is one of them:
/// the game saw Start go down, and sees it go up when it resumes.
///
/// Select and Start down together are the quick menu and the blind
/// recovery hold, never game input: from the frame they are both down
/// until both are up again, the core sees neither, so a game does not
/// pause itself or take a reset combination during a 5 s recovery hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PadMask {
    held_back: [u16; CORE_PORTS],
    /// The hotkey combination went down on the port, and not every button
    /// of it is up yet.
    combo: [bool; CORE_PORTS],
}

impl PadMask {
    /// Holds back every button that is down now.
    pub(crate) fn hold_back_pressed(&mut self, ports: &PortStates) {
        for (port, bits) in self.held_back.iter_mut().enumerate() {
            *bits = pressed(ports, port_id(port));
        }
    }

    /// Ends the hold of each button that is up now, and follows the
    /// hotkey combination. Call it once per input poll.
    pub(crate) fn release(&mut self, ports: &PortStates) {
        for (port, bits) in self.held_back.iter_mut().enumerate() {
            *bits &= pressed(ports, port_id(port));
        }
        for (port, combo) in self.combo.iter_mut().enumerate() {
            let id = port_id(port);
            if QUICK_MENU_COMBO.iter().all(|c| ports.is_held(id, *c)) {
                *combo = true;
            } else if QUICK_MENU_COMBO.iter().all(|c| !ports.is_held(id, *c)) {
                *combo = false;
            }
        }
    }

    /// The pad of `port` for the core.
    pub(crate) fn pad(&self, ports: &PortStates, port: usize) -> Pad {
        let id = port_id(port);
        let held_back = self.held_back.get(port).copied().unwrap_or(0);
        let combo = self.combo.get(port).copied().unwrap_or(false);
        let mut pad = Pad::default();
        for (bit, control) in BUTTONS.iter().enumerate() {
            let hotkey = combo && QUICK_MENU_COMBO.contains(control);
            let down = ports.is_held(id, *control) && held_back & (1 << bit) == 0 && !hotkey;
            pad.apply(*control, i16::from(down));
        }
        for control in AXES {
            pad.apply(control, ports.value(id, control));
        }
        pad
    }
}

/// The buttons down on `port`, one bit per entry of `BUTTONS`.
fn pressed(ports: &PortStates, port: u8) -> u16 {
    BUTTONS
        .iter()
        .enumerate()
        .filter(|(_, control)| ports.is_held(port, **control))
        .fold(0, |bits, (bit, _)| bits | 1 << bit)
}

/// `CORE_PORTS` is 4, so every core port fits a `u8`.
fn port_id(port: usize) -> u8 {
    u8::try_from(port).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use neptron_types::InputEvent;

    use super::*;

    fn set(ports: &mut PortStates, device: u32, control: Control, value: i16) {
        ports.apply(&[InputEvent {
            device,
            control,
            value,
        }]);
    }

    fn pad_with(controls: &[Control]) -> Pad {
        let mut pad = Pad::default();
        for control in controls {
            pad.apply(*control, 1);
        }
        pad
    }

    #[test]
    fn with_no_mask_the_pad_is_the_port_state() {
        let mut ports = PortStates::new();
        set(&mut ports, 1, Control::A, 1);
        set(&mut ports, 1, Control::LeftX, -20_000);
        let mask = PadMask::default();
        let mut expected = pad_with(&[Control::A]);
        expected.left[0] = -20_000;
        assert_eq!(mask.pad(&ports, 1), expected);
        assert_eq!(mask.pad(&ports, 0), Pad::default());
    }

    #[test]
    fn a_held_back_button_stays_up_until_it_is_released() {
        let mut ports = PortStates::new();
        set(&mut ports, 0, Control::Start, 1);
        set(&mut ports, 0, Control::A, 1);
        let mut mask = PadMask::default();
        mask.hold_back_pressed(&ports);
        assert_eq!(mask.pad(&ports, 0), Pad::default());

        set(&mut ports, 0, Control::B, 1);
        mask.release(&ports);
        assert_eq!(mask.pad(&ports, 0), pad_with(&[Control::B]));

        set(&mut ports, 0, Control::Start, 0);
        mask.release(&ports);
        set(&mut ports, 0, Control::Start, 1);
        mask.release(&ports);
        assert_eq!(
            mask.pad(&ports, 0),
            pad_with(&[Control::B, Control::Start]),
            "a new press after the release reaches the core"
        );
        assert_eq!(
            mask.pad(&ports, 0).buttons & 0x0100,
            0,
            "A is still held back"
        );
    }

    #[test]
    fn select_and_start_together_never_reach_the_core_until_both_are_up() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        set(&mut ports, 0, Control::Select, 1);
        mask.release(&ports);
        assert_eq!(mask.pad(&ports, 0), pad_with(&[Control::Select]));
        set(&mut ports, 0, Control::Start, 1);
        set(&mut ports, 0, Control::A, 1);
        mask.release(&ports);
        assert_eq!(mask.pad(&ports, 0), pad_with(&[Control::A]));
        set(&mut ports, 0, Control::Select, 0);
        mask.release(&ports);
        assert_eq!(mask.pad(&ports, 0), pad_with(&[Control::A]), "Start alone");
        set(&mut ports, 0, Control::Start, 0);
        mask.release(&ports);
        set(&mut ports, 0, Control::Start, 1);
        mask.release(&ports);
        assert_eq!(
            mask.pad(&ports, 0),
            pad_with(&[Control::A, Control::Start]),
            "a new press of Start alone"
        );
    }

    #[test]
    fn the_mask_does_not_hold_back_axes() {
        let mut ports = PortStates::new();
        set(&mut ports, 2, Control::RightY, 9_000);
        let mut mask = PadMask::default();
        mask.hold_back_pressed(&ports);
        assert_eq!(mask.pad(&ports, 2).right, [0, 9_000]);
    }
}
