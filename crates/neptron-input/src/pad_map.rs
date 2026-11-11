//! The standard gamepad layout mapped to the RetroPad.
//!
//! The face buttons map by position, as in RetroArch: the RetroPad is laid
//! out like a SNES pad, so the bottom button is B, the right one A, the
//! left one Y and the top one X. On an Xbox pad the label A is then the
//! RetroPad B, and on a Nintendo pad every label matches.

use neptron_types::Control;

use crate::raw::{HatState, PadAxis, PadButton};

/// The stick dead zone, in axis units. A stick value with a magnitude up to
/// this limit gives 0. Above it, the value is scaled so that the output
/// starts at 0 and still reaches the full range.
pub const STICK_DEAD_ZONE: i16 = 4096;

/// The trigger travel, in axis units, at which L2 or R2 is down. SDL3 gives
/// triggers from 0 to 32767, so this is half travel.
pub const TRIGGER_THRESHOLD: i16 = 16_384;

/// Each button of the standard layout that has a RetroPad control, and the
/// control. The guide button and the other buttons have none.
pub const PAD_BUTTON_MAP: [(PadButton, Control); 14] = [
    (PadButton::South, Control::B),
    (PadButton::East, Control::A),
    (PadButton::West, Control::Y),
    (PadButton::North, Control::X),
    (PadButton::Back, Control::Select),
    (PadButton::Start, Control::Start),
    (PadButton::LeftShoulder, Control::L),
    (PadButton::RightShoulder, Control::R),
    (PadButton::LeftStick, Control::L3),
    (PadButton::RightStick, Control::R3),
    (PadButton::DPadUp, Control::Up),
    (PadButton::DPadDown, Control::Down),
    (PadButton::DPadLeft, Control::Left),
    (PadButton::DPadRight, Control::Right),
];

/// What an axis of the standard layout drives on the RetroPad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisTarget {
    /// A RetroPad axis, through the stick dead zone.
    Stick(Control),
    /// A RetroPad button, through the trigger threshold.
    Trigger(Control),
}

/// The RetroPad control of `button`, or `None` for a button with no
/// control.
pub fn pad_control(button: PadButton) -> Option<Control> {
    PAD_BUTTON_MAP
        .iter()
        .find(|(b, _)| *b == button)
        .map(|(_, control)| *control)
}

/// What `axis` drives.
pub fn axis_target(axis: PadAxis) -> AxisTarget {
    match axis {
        PadAxis::LeftX => AxisTarget::Stick(Control::LeftX),
        PadAxis::LeftY => AxisTarget::Stick(Control::LeftY),
        PadAxis::RightX => AxisTarget::Stick(Control::RightX),
        PadAxis::RightY => AxisTarget::Stick(Control::RightY),
        PadAxis::LeftTrigger => AxisTarget::Trigger(Control::L2),
        PadAxis::RightTrigger => AxisTarget::Trigger(Control::R2),
    }
}

/// The RetroPad axis value of a raw stick value, after the dead zone. The
/// output is symmetric: -32768 gives -32767.
pub fn stick_value(raw: i16) -> i16 {
    let magnitude = i32::from(raw).abs();
    let dead = i32::from(STICK_DEAD_ZONE);
    if magnitude <= dead {
        return 0;
    }
    let full = i32::from(i16::MAX);
    let scaled = ((magnitude - dead) * full / (full - dead)).min(full);
    let signed = if raw < 0 { -scaled } else { scaled };
    // `scaled` is at most i16::MAX, so the signed value fits.
    i16::try_from(signed).unwrap_or(0)
}

/// The RetroPad button value (0 or 1) of a raw trigger value.
pub fn trigger_value(raw: i16) -> i16 {
    i16::from(raw >= TRIGGER_THRESHOLD)
}

/// The value of each D-pad direction for a hat state.
pub fn hat_values(hat: HatState) -> [(Control, i16); 4] {
    [
        (Control::Up, i16::from(hat.has(HatState::UP))),
        (Control::Down, i16::from(hat.has(HatState::DOWN))),
        (Control::Left, i16::from(hat.has(HatState::LEFT))),
        (Control::Right, i16::from(hat.has(HatState::RIGHT))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pad_button_maps_as_the_fixture_says() {
        let fixture = [
            (PadButton::South, Some(Control::B)),
            (PadButton::East, Some(Control::A)),
            (PadButton::West, Some(Control::Y)),
            (PadButton::North, Some(Control::X)),
            (PadButton::Back, Some(Control::Select)),
            (PadButton::Guide, None),
            (PadButton::Start, Some(Control::Start)),
            (PadButton::LeftStick, Some(Control::L3)),
            (PadButton::RightStick, Some(Control::R3)),
            (PadButton::LeftShoulder, Some(Control::L)),
            (PadButton::RightShoulder, Some(Control::R)),
            (PadButton::DPadUp, Some(Control::Up)),
            (PadButton::DPadDown, Some(Control::Down)),
            (PadButton::DPadLeft, Some(Control::Left)),
            (PadButton::DPadRight, Some(Control::Right)),
            (PadButton::Other, None),
        ];
        for (button, control) in fixture {
            assert_eq!(pad_control(button), control, "{button:?}");
        }
    }

    #[test]
    fn every_pad_axis_maps_as_the_fixture_says() {
        let fixture = [
            (PadAxis::LeftX, AxisTarget::Stick(Control::LeftX)),
            (PadAxis::LeftY, AxisTarget::Stick(Control::LeftY)),
            (PadAxis::RightX, AxisTarget::Stick(Control::RightX)),
            (PadAxis::RightY, AxisTarget::Stick(Control::RightY)),
            (PadAxis::LeftTrigger, AxisTarget::Trigger(Control::L2)),
            (PadAxis::RightTrigger, AxisTarget::Trigger(Control::R2)),
        ];
        for (axis, target) in fixture {
            assert_eq!(axis_target(axis), target, "{axis:?}");
        }
    }

    #[test]
    fn the_dead_zone_boundary_is_inclusive() {
        assert_eq!(stick_value(0), 0);
        assert_eq!(stick_value(STICK_DEAD_ZONE), 0);
        assert_eq!(stick_value(-STICK_DEAD_ZONE), 0);
        assert_eq!(stick_value(STICK_DEAD_ZONE + 1), 1);
        assert_eq!(stick_value(-STICK_DEAD_ZONE - 1), -1);
    }

    #[test]
    fn the_stick_reaches_the_full_range_on_both_sides() {
        assert_eq!(stick_value(i16::MAX), i16::MAX);
        assert_eq!(stick_value(i16::MIN), -i16::MAX);
        assert_eq!(stick_value(-i16::MAX), -i16::MAX);
    }

    #[test]
    fn the_stick_scale_is_monotonic_and_odd() {
        let mut last = 0;
        for raw in (0..=i16::MAX).step_by(7) {
            let value = stick_value(raw);
            assert!(value >= last, "{raw}");
            assert_eq!(stick_value(-raw), -value, "{raw}");
            last = value;
        }
    }

    #[test]
    fn the_trigger_threshold_boundary_is_inclusive() {
        assert_eq!(trigger_value(0), 0);
        assert_eq!(trigger_value(TRIGGER_THRESHOLD - 1), 0);
        assert_eq!(trigger_value(TRIGGER_THRESHOLD), 1);
        assert_eq!(trigger_value(i16::MAX), 1);
        assert_eq!(trigger_value(i16::MIN), 0);
    }

    #[test]
    fn a_hat_sets_the_d_pad_and_a_diagonal_sets_two_directions() {
        assert_eq!(
            hat_values(HatState::default()),
            [
                (Control::Up, 0),
                (Control::Down, 0),
                (Control::Left, 0),
                (Control::Right, 0)
            ]
        );
        assert_eq!(
            hat_values(HatState(HatState::UP | HatState::LEFT)),
            [
                (Control::Up, 1),
                (Control::Down, 0),
                (Control::Left, 1),
                (Control::Right, 0)
            ]
        );
        assert_eq!(
            hat_values(HatState(HatState::DOWN | HatState::RIGHT)),
            [
                (Control::Up, 0),
                (Control::Down, 1),
                (Control::Left, 0),
                (Control::Right, 1)
            ]
        );
    }
}
