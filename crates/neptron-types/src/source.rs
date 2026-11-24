use serde::{Deserialize, Serialize};

use crate::Rotation;

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
    /// 1333 for 4:3. It is the aspect of the frame as the core draws it,
    /// before `rotation`: the policy swaps the axes of a source turned by
    /// 90 or 270 degrees and inverts this aspect.
    pub aspect_milli: u32,
    /// The core draws interlaced frames.
    pub interlaced: bool,
    /// The turn, counter-clockwise, that the picture needs to show
    /// upright: the turn that the core asks for with `SET_ROTATION`.
    pub rotation: Rotation,
}
