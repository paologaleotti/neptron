use serde::{Deserialize, Serialize};

/// What the GPU and its output can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GpuCaps {
    /// Lowest pixel clock the output can make, in hertz.
    pub pclock_min_hz: u32,
    /// Highest pixel clock the output can make, in hertz.
    pub pclock_max_hz: u32,
    /// The output can send an interlaced mode.
    pub interlace: bool,
    /// The driver signals a page flip per field of an interlaced mode, not
    /// per frame.
    pub flip_per_field: bool,
    /// Widest mode the output can send, in pixels.
    pub max_width: u16,
    /// Tallest mode the output can send, in lines.
    pub max_height: u16,
}
