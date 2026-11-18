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

/// The shortest late press of the combination, in core frames. Some arcade
/// games read a coin only when it stays down for 2 frames or more, so a
/// late press of one frame can be lost.
pub const LATE_PRESS_MIN_FRAMES: u8 = 3;

/// The longest late press of the combination, in core frames: about the
/// 0.5 s of the quick menu hold at 60 Hz. A combination that cannot open
/// the menu (after a Start hold that fired) can be held back for longer,
/// and its press must not replay for seconds.
pub const LATE_PRESS_MAX_FRAMES: u8 = 30;

/// Buttons that the core sees as up until the player releases them, and
/// the presses of the hotkey combination that the core gets late.
///
/// When the quick menu closes, the buttons still held were pressed for the
/// menu, not for the game. The core sees each one up until its release, so
/// it sees no new press. A Start hold that opened the menu is one of them:
/// the game saw Start go down, and sees it go up when it resumes.
///
/// Select and Start down together can be the quick menu hold, and on an
/// arcade stick they are also a coin and a start that overlap. The button
/// that completes the combination is held back while the combination is
/// down, so the core does not see both down during a hold. When the
/// combination ends before the menu opens, the held-back press reaches the
/// core late: from that frame until its release, or, when it is up
/// already, for as many core frames as it was held back, from
/// [`LATE_PRESS_MIN_FRAMES`] to [`LATE_PRESS_MAX_FRAMES`]. So a short tap of
/// Select and Start reaches the game, as on the original hardware, and a
/// game's own combination with them works with a tap. When the menu opens,
/// the held-back press is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PadMask {
    held_back: [u16; CORE_PORTS],
    /// The buttons down at the last poll.
    last: [u16; CORE_PORTS],
    /// The hotkey combination is down on the port.
    combo: [bool; CORE_PORTS],
    /// The buttons of the combination that completed it: the core does not
    /// see them while it is down.
    pending: [u16; CORE_PORTS],
    /// The core frames in which the combination in progress held back a
    /// press.
    combo_frames: [u8; CORE_PORTS],
    /// The buttons that the core sees down although they are up, a late
    /// press, and the core frames left of it.
    owed: [u16; CORE_PORTS],
    owed_frames: [u8; CORE_PORTS],
}

impl PadMask {
    /// Holds back every button that is down now, and drops every late
    /// press of the combination.
    pub(crate) fn hold_back_pressed(&mut self, ports: &PortStates) {
        for (port, bits) in self.held_back.iter_mut().enumerate() {
            *bits = pressed(ports, port_id(port));
        }
        self.drop_combo_presses();
    }

    /// Drops the presses of the combination that the core did not get: the
    /// quick menu opened, so they were for the menu.
    pub(crate) fn drop_combo_presses(&mut self) {
        self.pending = [0; CORE_PORTS];
        self.owed = [0; CORE_PORTS];
        self.owed_frames = [0; CORE_PORTS];
    }

    /// Ends the hold of each button that is up now, and follows the
    /// hotkey combination. Call it after the input poll of each loop
    /// frame, also when the core does not run.
    pub(crate) fn release(&mut self, ports: &PortStates) {
        let combo_bits = mask_of(&QUICK_MENU_COMBO);
        for port in 0..CORE_PORTS {
            let now = pressed(ports, port_id(port));
            self.held_back[port] &= now;
            if now & combo_bits == combo_bits {
                if !self.combo[port] {
                    self.combo[port] = true;
                    self.pending[port] = combo_bits & !self.last[port];
                    self.combo_frames[port] = 0;
                }
            } else if self.combo[port] {
                self.combo[port] = false;
                let late = self.pending[port] & !now;
                if late != 0 {
                    self.owed[port] = late;
                    self.owed_frames[port] =
                        self.combo_frames[port].clamp(LATE_PRESS_MIN_FRAMES, LATE_PRESS_MAX_FRAMES);
                }
                self.pending[port] = 0;
            }
            self.last[port] = now;
        }
    }

    /// Counts a frame in which the core ran with the pads of this mask.
    /// Only such frames count, so a late press does not run out while the
    /// core is paused or the output is off.
    pub(crate) fn core_ran(&mut self) {
        for port in 0..CORE_PORTS {
            self.owed_frames[port] = self.owed_frames[port].saturating_sub(1);
            if self.owed_frames[port] == 0 {
                self.owed[port] = 0;
            }
            if self.combo[port] && self.pending[port] != 0 {
                self.combo_frames[port] = self.combo_frames[port].saturating_add(1);
            }
        }
    }

    /// The pad of `port` for the core.
    pub(crate) fn pad(&self, ports: &PortStates, port: usize) -> Pad {
        let id = port_id(port);
        let hidden = self.held_back.get(port).copied().unwrap_or(0)
            | self.pending.get(port).copied().unwrap_or(0);
        let owed = self.owed.get(port).copied().unwrap_or(0);
        let mut pad = Pad::default();
        for (bit, control) in BUTTONS.iter().enumerate() {
            let held = ports.is_held(id, *control) && hidden & (1 << bit) == 0;
            let down = held || owed & (1 << bit) != 0;
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

/// The bits of `controls` in a mask of `BUTTONS`.
fn mask_of(controls: &[Control]) -> u16 {
    BUTTONS
        .iter()
        .enumerate()
        .filter(|(_, control)| controls.contains(control))
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

    /// One frame in which the core runs: the port state changes, the mask
    /// follows it, and the core reads the pad.
    fn poll(mask: &mut PadMask, ports: &mut PortStates, changes: &[(Control, i16)]) -> Pad {
        for (control, value) in changes {
            set(ports, 0, *control, *value);
        }
        mask.release(ports);
        let pad = mask.pad(ports, 0);
        mask.core_ran();
        pad
    }

    #[test]
    fn a_coin_and_a_start_that_overlap_reach_the_core_once_each() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        assert_eq!(
            poll(&mut mask, &mut ports, &[(select, 1)]),
            pad_with(&[select])
        );
        assert_eq!(
            poll(&mut mask, &mut ports, &[(start, 1)]),
            pad_with(&[select]),
            "Start completes the combination and waits"
        );
        assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[select]));
        assert_eq!(
            poll(&mut mask, &mut ports, &[(select, 0)]),
            pad_with(&[start]),
            "the coin ends, the start reaches the core"
        );
        assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[start]));
        assert_eq!(poll(&mut mask, &mut ports, &[(start, 0)]), Pad::default());
        assert_eq!(poll(&mut mask, &mut ports, &[]), Pad::default());
    }

    #[test]
    fn a_start_tapped_inside_the_coin_reaches_the_core_late_for_its_length() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        poll(&mut mask, &mut ports, &[(select, 1)]);
        for _ in 0..3 {
            assert_eq!(
                poll(&mut mask, &mut ports, &[(start, 1)]),
                pad_with(&[select])
            );
        }
        for _ in 0..3 {
            assert_eq!(
                poll(&mut mask, &mut ports, &[(start, 0)]),
                pad_with(&[select, start]),
                "three polls of a late start"
            );
        }
        assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[select]));
        assert_eq!(poll(&mut mask, &mut ports, &[(select, 0)]), Pad::default());
    }

    #[test]
    fn a_short_late_press_lasts_the_minimum() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        assert_eq!(
            poll(&mut mask, &mut ports, &[(start, 1)]),
            pad_with(&[start])
        );
        assert_eq!(
            poll(&mut mask, &mut ports, &[(select, 1)]),
            pad_with(&[start])
        );
        assert_eq!(
            poll(&mut mask, &mut ports, &[(select, 0)]),
            pad_with(&[start, select]),
            "a coin held back for one frame"
        );
        for _ in 1..LATE_PRESS_MIN_FRAMES {
            assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[start, select]));
        }
        assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[start]));
    }

    #[test]
    fn a_long_late_press_lasts_the_maximum() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        poll(&mut mask, &mut ports, &[(start, 1)]);
        // A combination that opens no menu, held back for 5 s at 60 Hz.
        poll(&mut mask, &mut ports, &[(select, 1)]);
        for _ in 0..300 {
            assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[start]));
        }
        for _ in 0..LATE_PRESS_MAX_FRAMES {
            assert_eq!(
                poll(&mut mask, &mut ports, &[(select, 0)]),
                pad_with(&[start, select])
            );
        }
        assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[start]));
    }

    #[test]
    fn a_late_press_counts_only_frames_in_which_the_core_runs() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        poll(&mut mask, &mut ports, &[(select, 1)]);
        set(&mut ports, 0, start, 1);
        for _ in 0..40 {
            mask.release(&ports);
        }
        assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[select]));
        set(&mut ports, 0, start, 0);
        for _ in 0..40 {
            mask.release(&ports);
        }
        for _ in 0..LATE_PRESS_MIN_FRAMES {
            assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[select, start]));
        }
        assert_eq!(poll(&mut mask, &mut ports, &[]), pad_with(&[select]));
    }

    #[test]
    fn the_late_press_bounds_are_ordered() {
        assert!((2..=LATE_PRESS_MAX_FRAMES).contains(&LATE_PRESS_MIN_FRAMES));
    }

    #[test]
    fn both_down_in_one_poll_wait_together() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        assert_eq!(
            poll(
                &mut mask,
                &mut ports,
                &[(select, 1), (start, 1), (Control::A, 1)]
            ),
            pad_with(&[Control::A])
        );
        for _ in 0..LATE_PRESS_MIN_FRAMES {
            assert_eq!(
                poll(&mut mask, &mut ports, &[(select, 0)]),
                pad_with(&[Control::A, select, start]),
                "Start still down, Select for the shortest late press"
            );
        }
        assert_eq!(
            poll(&mut mask, &mut ports, &[]),
            pad_with(&[Control::A, start])
        );
    }

    #[test]
    fn the_quick_menu_drops_the_held_back_press_and_leaves_no_stuck_button() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        poll(&mut mask, &mut ports, &[(select, 1)]);
        poll(&mut mask, &mut ports, &[(start, 1)]);
        // The hold opens the menu: the session drops the press for it.
        mask.drop_combo_presses();
        poll(&mut mask, &mut ports, &[(start, 0)]);
        poll(&mut mask, &mut ports, &[(select, 0)]);
        // The menu closes with nothing held.
        mask.hold_back_pressed(&ports);
        assert_eq!(
            poll(&mut mask, &mut ports, &[]),
            Pad::default(),
            "no late start"
        );
        assert_eq!(
            poll(&mut mask, &mut ports, &[(start, 1)]),
            pad_with(&[start])
        );
    }

    #[test]
    fn a_resume_drops_a_late_press_of_a_combination_in_the_menu() {
        let mut ports = PortStates::new();
        let mut mask = PadMask::default();
        let (select, start) = (Control::Select, Control::Start);
        poll(&mut mask, &mut ports, &[(select, 1), (start, 1)]);
        poll(&mut mask, &mut ports, &[]);
        poll(&mut mask, &mut ports, &[(select, 0), (start, 0)]);
        mask.hold_back_pressed(&ports);
        assert_eq!(mask.pad(&ports, 0), Pad::default());
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
