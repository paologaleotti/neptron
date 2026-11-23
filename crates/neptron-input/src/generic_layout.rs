//! The generic layout: the map by position of a joystick that SDL3 has no
//! gamepad mapping for.
//!
//! The buttons follow the order of the PlayStation 3 pad, the common order
//! of HID pads with no mapping. The face buttons map by position, as
//! [`crate::PAD_BUTTON_MAP`] does: Square (left) is Y, Cross (bottom) B,
//! Circle (right) A and Triangle (top) X.

use neptron_types::Control;

use crate::raw_state::RawReads;

/// The control of each raw button index, in the PS3 order: Square, Cross,
/// Circle, Triangle, L1, R1, L2, R2, Select, Start, L3, R3, PS. The PS
/// button drives nothing, and a button past the table drives nothing.
pub const GENERIC_BUTTON_ORDER: [Option<Control>; 13] = [
    Some(Control::Y),
    Some(Control::B),
    Some(Control::A),
    Some(Control::X),
    Some(Control::L),
    Some(Control::R),
    Some(Control::L2),
    Some(Control::R2),
    Some(Control::Select),
    Some(Control::Start),
    Some(Control::L3),
    Some(Control::R3),
    None,
];

/// The fewest buttons of a gamepad: accept and back.
pub const MIN_GENERIC_BUTTONS: u32 = 2;

/// The buttons under which the PS3 order gives no accept: A is raw
/// button 2. A pad with fewer buttons gets [`SMALL_BUTTON_ORDER`].
const ACCEPT_INDEX_BUTTONS: u32 = 3;

/// The controls of a pad with [`MIN_GENERIC_BUTTONS`] buttons: raw 0 is
/// A, so the pad has accept; raw 1 stays B, as in the PS3 order.
const SMALL_BUTTON_ORDER: [Option<Control>; 2] = [Some(Control::A), Some(Control::B)];

/// The axis travel, in axis units, at which an axis that drives the D-pad
/// sets a direction: half travel.
pub const GENERIC_DPAD_THRESHOLD: i16 = 16_384;

/// The axes that the generic layout reads: the left and the right stick.
pub const GENERIC_AXES: usize = 4;

/// What a joystick has, as SDL3 counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct JoystickCounts {
    /// The number of buttons.
    pub buttons: u32,
    /// The number of hat switches.
    pub hats: u32,
    /// The number of axes.
    pub axes: u32,
}

/// What one axis of a joystick drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AxisRole {
    /// The RetroPad axis, through the stick dead zone.
    pub stick: Control,
    /// The D-pad directions of the negative and the positive side, through
    /// [`GENERIC_DPAD_THRESHOLD`], for a pad with no hat.
    pub dpad: Option<(Control, Control)>,
}

/// The table of a joystick with the generic layout: the controls of each
/// raw button, hat and axis index. It is a fixed value, so the mapper
/// keeps one per pad with no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GenericLayout {
    buttons: [Option<Control>; GENERIC_BUTTON_ORDER.len()],
    dpad_hat: bool,
    axes: [Option<AxisRole>; GENERIC_AXES],
}

/// Why a joystick gets no generic layout: it is not a gamepad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NotAGamepad {
    /// It has fewer buttons than a gamepad needs.
    #[error("it has {buttons} buttons; a gamepad has at least {min}")]
    TooFewButtons {
        /// The buttons it has.
        buttons: u32,
        /// The fewest a gamepad has.
        min: u32,
    },
    /// It has no hat and no axis, so no direction.
    #[error("it has no hat and no axis, so it gives no direction")]
    NoDirection,
}

impl GenericLayout {
    /// The generic layout of a joystick with `counts`.
    ///
    /// # Errors
    ///
    /// [`NotAGamepad`] for fewer than [`MIN_GENERIC_BUTTONS`] buttons, or
    /// for no hat and no axis.
    pub fn for_counts(counts: JoystickCounts) -> Result<Self, NotAGamepad> {
        if counts.buttons < MIN_GENERIC_BUTTONS {
            return Err(NotAGamepad::TooFewButtons {
                buttons: counts.buttons,
                min: MIN_GENERIC_BUTTONS,
            });
        }
        if counts.hats == 0 && counts.axes == 0 {
            return Err(NotAGamepad::NoDirection);
        }
        let mut layout = Self {
            dpad_hat: counts.hats > 0,
            ..Self::default()
        };
        for (slot, control) in layout.buttons.iter_mut().zip(GENERIC_BUTTON_ORDER) {
            *slot = control;
        }
        if counts.buttons < ACCEPT_INDEX_BUTTONS {
            for (slot, control) in layout.buttons.iter_mut().zip(SMALL_BUTTON_ORDER) {
                *slot = control;
            }
        }
        let shown = usize::try_from(counts.buttons).unwrap_or(usize::MAX);
        for slot in layout.buttons.iter_mut().skip(shown) {
            *slot = None;
        }
        let sticks = [
            (Control::LeftX, (Control::Left, Control::Right)),
            (Control::LeftY, (Control::Up, Control::Down)),
            (Control::RightX, (Control::Left, Control::Right)),
            (Control::RightY, (Control::Up, Control::Down)),
        ];
        let axes = usize::try_from(counts.axes).unwrap_or(usize::MAX);
        for (i, (slot, (stick, dpad))) in layout.axes.iter_mut().zip(sticks).enumerate() {
            if i < axes {
                let left_stick = i < 2;
                *slot = Some(AxisRole {
                    stick,
                    dpad: (left_stick && !layout.dpad_hat).then_some(dpad),
                });
            }
        }
        Ok(layout)
    }

    /// The control of raw button `index`.
    pub fn button(&self, index: u8) -> Option<Control> {
        self.buttons.get(usize::from(index)).copied().flatten()
    }

    /// True when hat `index` is the D-pad: hat 0 of a pad with a hat.
    pub fn hat_is_dpad(&self, index: u8) -> bool {
        self.dpad_hat && index == 0
    }

    /// What raw axis `index` drives.
    pub fn axis(&self, index: u8) -> Option<AxisRole> {
        self.axes.get(usize::from(index)).copied().flatten()
    }

    /// The button indices that drive a control, from 0: the backend reads
    /// these and no other.
    pub fn buttons_read(&self) -> u8 {
        last_used(&self.buttons)
    }

    /// The hat indices that the backend reads, from 0.
    pub fn hats_read(&self) -> u8 {
        u8::from(self.dpad_hat)
    }

    /// The axis indices that the backend reads, from 0.
    pub fn axes_read(&self) -> u8 {
        last_used(&self.axes)
    }

    /// The indices that the backend reads.
    pub fn reads(&self) -> RawReads {
        RawReads {
            buttons: self.buttons_read(),
            hats: self.hats_read(),
            axes: self.axes_read(),
        }
    }

    /// True when the layout has `control` on a button.
    pub fn has_button(&self, control: Control) -> bool {
        self.buttons.contains(&Some(control))
    }
}

/// The number of entries up to the last one that is set.
fn last_used<T>(table: &[Option<T>]) -> u8 {
    let count = table.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
    // The tables have at most 13 entries.
    u8::try_from(count).unwrap_or(u8::MAX)
}

/// The value of the D-pad directions of an axis that drives the D-pad.
pub fn dpad_axis_values(
    (negative, positive): (Control, Control),
    value: i16,
) -> [(Control, i16); 2] {
    [
        (negative, i16::from(value <= -GENERIC_DPAD_THRESHOLD)),
        (positive, i16::from(value >= GENERIC_DPAD_THRESHOLD)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(buttons: u32, hats: u32, axes: u32) -> JoystickCounts {
        JoystickCounts {
            buttons,
            hats,
            axes,
        }
    }

    fn buttons(layout: &GenericLayout) -> Vec<Option<Control>> {
        (0..16).map(|i| layout.button(i)).collect()
    }

    #[test]
    fn a_full_pad_maps_every_button_its_hat_and_both_sticks() {
        let layout = GenericLayout::for_counts(counts(13, 1, 4)).unwrap();
        let mut expected: Vec<_> = GENERIC_BUTTON_ORDER.to_vec();
        expected.extend([None; 3]);
        assert_eq!(buttons(&layout), expected);
        assert_eq!(layout.button(0), Some(Control::Y), "Square is left");
        assert_eq!(layout.button(1), Some(Control::B), "Cross is bottom");
        assert_eq!(layout.button(2), Some(Control::A), "Circle is right");
        assert_eq!(layout.button(3), Some(Control::X), "Triangle is top");
        assert_eq!(layout.button(12), None, "PS drives nothing");
        assert!(layout.hat_is_dpad(0));
        assert!(!layout.hat_is_dpad(1));
        let sticks: Vec<_> = (0..5).map(|i| layout.axis(i)).collect();
        let stick = |stick| Some(AxisRole { stick, dpad: None });
        assert_eq!(
            sticks,
            [
                stick(Control::LeftX),
                stick(Control::LeftY),
                stick(Control::RightX),
                stick(Control::RightY),
                None
            ]
        );
        assert_eq!(
            (
                layout.buttons_read(),
                layout.hats_read(),
                layout.axes_read()
            ),
            (12, 1, 4)
        );
    }

    #[test]
    fn eight_buttons_and_a_hat_map_what_exists_in_order() {
        let layout = GenericLayout::for_counts(counts(8, 1, 0)).unwrap();
        assert_eq!(
            buttons(&layout)[..9],
            [
                Some(Control::Y),
                Some(Control::B),
                Some(Control::A),
                Some(Control::X),
                Some(Control::L),
                Some(Control::R),
                Some(Control::L2),
                Some(Control::R2),
                None
            ]
        );
        assert!(!layout.has_button(Control::Start));
        assert!(!layout.has_button(Control::Select));
        assert!(layout.has_button(Control::A) && layout.has_button(Control::B));
        assert!(layout.hat_is_dpad(0));
        assert_eq!(layout.axis(0), None);
        assert_eq!(
            (
                layout.buttons_read(),
                layout.hats_read(),
                layout.axes_read()
            ),
            (8, 1, 0)
        );
    }

    #[test]
    fn every_gamepad_has_accept_and_back() {
        for n in MIN_GENERIC_BUTTONS..=20 {
            let layout = GenericLayout::for_counts(counts(n, 1, 0)).unwrap();
            assert!(
                layout.has_button(Control::A) && layout.has_button(Control::B),
                "{n} buttons"
            );
        }
        let two = GenericLayout::for_counts(counts(2, 0, 2)).unwrap();
        assert_eq!(
            buttons(&two)[..3],
            [Some(Control::A), Some(Control::B), None],
            "button 1 stays B, as in the order"
        );
    }

    #[test]
    fn with_no_hat_the_left_stick_axes_also_drive_the_d_pad() {
        let layout = GenericLayout::for_counts(counts(12, 0, 2)).unwrap();
        assert!(!layout.hat_is_dpad(0));
        assert_eq!(layout.hats_read(), 0);
        assert_eq!(
            layout.axis(0),
            Some(AxisRole {
                stick: Control::LeftX,
                dpad: Some((Control::Left, Control::Right))
            })
        );
        assert_eq!(
            layout.axis(1),
            Some(AxisRole {
                stick: Control::LeftY,
                dpad: Some((Control::Up, Control::Down))
            })
        );
        assert_eq!(layout.axis(2), None);
        assert!(layout.has_button(Control::Start) && layout.has_button(Control::R3));
    }

    #[test]
    fn the_right_stick_never_drives_the_d_pad() {
        let layout = GenericLayout::for_counts(counts(4, 0, 4)).unwrap();
        assert_eq!(layout.axis(2).and_then(|a| a.dpad), None);
        assert_eq!(layout.axis(3).and_then(|a| a.dpad), None);
    }

    #[test]
    fn start_and_select_come_from_ten_buttons() {
        for n in 0..=20 {
            let Ok(layout) = GenericLayout::for_counts(counts(n, 1, 0)) else {
                assert!(n < MIN_GENERIC_BUTTONS, "{n}");
                continue;
            };
            let both = layout.has_button(Control::Start) && layout.has_button(Control::Select);
            assert_eq!(both, n >= 10, "{n} buttons");
        }
    }

    #[test]
    fn a_joystick_that_is_not_a_gamepad_gets_no_layout() {
        assert_eq!(
            GenericLayout::for_counts(counts(1, 1, 2)),
            Err(NotAGamepad::TooFewButtons { buttons: 1, min: 2 })
        );
        assert_eq!(
            GenericLayout::for_counts(counts(0, 0, 0)),
            Err(NotAGamepad::TooFewButtons { buttons: 0, min: 2 })
        );
        assert_eq!(
            GenericLayout::for_counts(counts(10, 0, 0)),
            Err(NotAGamepad::NoDirection)
        );
        assert_eq!(
            NotAGamepad::TooFewButtons { buttons: 1, min: 2 }.to_string(),
            "it has 1 buttons; a gamepad has at least 2"
        );
        assert!(GenericLayout::for_counts(counts(2, 0, 1)).is_ok());
    }

    #[test]
    fn huge_counts_read_only_the_table() {
        let layout = GenericLayout::for_counts(counts(u32::MAX, u32::MAX, u32::MAX)).unwrap();
        assert_eq!(
            (
                layout.buttons_read(),
                layout.hats_read(),
                layout.axes_read()
            ),
            (12, 1, 4)
        );
        assert_eq!(layout.button(u8::MAX), None);
        assert_eq!(layout.axis(u8::MAX), None);
    }

    #[test]
    fn the_d_pad_threshold_is_inclusive_on_both_sides() {
        let pair = (Control::Left, Control::Right);
        assert_eq!(
            dpad_axis_values(pair, 0),
            [(Control::Left, 0), (Control::Right, 0)]
        );
        assert_eq!(
            dpad_axis_values(pair, GENERIC_DPAD_THRESHOLD - 1),
            [(Control::Left, 0), (Control::Right, 0)]
        );
        assert_eq!(
            dpad_axis_values(pair, GENERIC_DPAD_THRESHOLD),
            [(Control::Left, 0), (Control::Right, 1)]
        );
        assert_eq!(
            dpad_axis_values(pair, -GENERIC_DPAD_THRESHOLD),
            [(Control::Left, 1), (Control::Right, 0)]
        );
        assert_eq!(
            dpad_axis_values(pair, i16::MIN),
            [(Control::Left, 1), (Control::Right, 0)]
        );
    }
}
