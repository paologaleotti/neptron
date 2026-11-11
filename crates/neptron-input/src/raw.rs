//! Raw input: what a device backend reads, before any mapping.

/// A button of the standard gamepad layout that SDL3 maps every known
/// controller to. SDL names the face buttons by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadButton {
    /// The bottom face button (Xbox A, PlayStation cross, Nintendo B).
    South,
    /// The right face button (Xbox B, PlayStation circle, Nintendo A).
    East,
    /// The left face button (Xbox X, PlayStation square, Nintendo Y).
    West,
    /// The top face button (Xbox Y, PlayStation triangle, Nintendo X).
    North,
    /// Back, Select, Share or Minus.
    Back,
    /// The guide or home button.
    Guide,
    /// Start, Options or Plus.
    Start,
    /// Left stick press.
    LeftStick,
    /// Right stick press.
    RightStick,
    /// Left shoulder.
    LeftShoulder,
    /// Right shoulder.
    RightShoulder,
    /// D-pad up.
    DPadUp,
    /// D-pad down.
    DPadDown,
    /// D-pad left.
    DPadLeft,
    /// D-pad right.
    DPadRight,
    /// Any other button: paddles, touchpad, the misc buttons.
    Other,
}

/// An axis of the standard gamepad layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadAxis {
    /// Left stick, horizontal, -32768 to 32767. Negative is left.
    LeftX,
    /// Left stick, vertical, -32768 to 32767. Negative is up.
    LeftY,
    /// Right stick, horizontal, -32768 to 32767. Negative is left.
    RightX,
    /// Right stick, vertical, -32768 to 32767. Negative is up.
    RightY,
    /// Left trigger, 0 (released) to 32767 (fully pressed).
    LeftTrigger,
    /// Right trigger, 0 (released) to 32767 (fully pressed).
    RightTrigger,
}

/// The state of a hat switch, as a set of the four direction bits. A
/// diagonal sets two bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HatState(pub u8);

impl HatState {
    /// The hat is up.
    pub const UP: u8 = 0x01;
    /// The hat is right.
    pub const RIGHT: u8 = 0x02;
    /// The hat is down.
    pub const DOWN: u8 = 0x04;
    /// The hat is left.
    pub const LEFT: u8 = 0x08;

    /// True when the hat points in `direction`, one of the bits above.
    pub fn has(self, direction: u8) -> bool {
        self.0 & direction != 0
    }
}

/// One change of one control of a device, as a backend reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawInput {
    /// A gamepad button changed.
    Button {
        /// The button.
        button: PadButton,
        /// True when it is down.
        pressed: bool,
    },
    /// A gamepad axis changed.
    Axis {
        /// The axis.
        axis: PadAxis,
        /// The value in the range of the axis.
        value: i16,
    },
    /// A hat switch changed.
    Hat(HatState),
}
