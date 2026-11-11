use serde::{Deserialize, Serialize};

/// One completed page flip, as the display reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlipEvent {
    /// When the flip took effect, in nanoseconds on the backend clock.
    pub time_ns: u64,
    /// Count of vertical blanks since the display started. A gap of more
    /// than one between two events means a missed flip.
    pub sequence: u64,
}

/// One change of one control of an input device, after the mapping to the
/// RetroPad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InputEvent {
    /// The device, as an id that stays the same while the device stays
    /// connected.
    pub device: u32,
    /// The control that changed.
    pub control: Control,
    /// The new value: 0 or 1 for a button, -32768 to 32767 for an axis, as
    /// in libretro.
    pub value: i16,
}

/// A control of the libretro RetroPad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    /// Button B, the bottom face button.
    B,
    /// Button Y, the left face button.
    Y,
    /// Select.
    Select,
    /// Start.
    Start,
    /// D-pad up.
    Up,
    /// D-pad down.
    Down,
    /// D-pad left.
    Left,
    /// D-pad right.
    Right,
    /// Button A, the right face button.
    A,
    /// Button X, the top face button.
    X,
    /// Left shoulder.
    L,
    /// Right shoulder.
    R,
    /// Left trigger.
    L2,
    /// Right trigger.
    R2,
    /// Left stick press.
    L3,
    /// Right stick press.
    R3,
    /// Left stick, horizontal axis. Negative is left.
    LeftX,
    /// Left stick, vertical axis. Negative is up.
    LeftY,
    /// Right stick, horizontal axis. Negative is left.
    RightX,
    /// Right stick, vertical axis. Negative is up.
    RightY,
}
