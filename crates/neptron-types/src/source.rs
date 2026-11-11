use serde::{Deserialize, Serialize};

/// A rotation of the picture, in steps of 90 degrees counter-clockwise,
/// as libretro defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    /// No rotation.
    R0,
    /// 90 degrees counter-clockwise.
    R90,
    /// 180 degrees.
    R180,
    /// 270 degrees counter-clockwise.
    R270,
}

/// What a core draws: the input of the mode solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceGeometry {
    /// Width of the frame, in pixels.
    pub width: u16,
    /// Height of the frame, in lines. For an interlaced source this counts
    /// both fields.
    pub height: u16,
    /// Frame rate of the core, in millihertz.
    pub refresh_mhz: u32,
    /// Display aspect ratio, width divided by height, in thousandths:
    /// 1333 for 4:3.
    pub aspect_milli: u32,
    /// The core draws interlaced frames.
    pub interlaced: bool,
    /// The rotation of the picture after the rotation setting.
    pub rotation: Rotation,
}
