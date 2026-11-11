//! What one frame of the core produced.

use neptron_types::{Frame, PixelFormat};

use crate::geometry::{AvInfo, GameGeometry, GeometryError};
use crate::log::LogRecord;

/// The result of [`Host::run_frame`](crate::Host::run_frame). It borrows the
/// host, so it is valid until the next call that changes the host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameOutput<'a> {
    /// The picture of this frame.
    pub video: Video<'a>,
    /// The audio of this frame: interleaved stereo, left first.
    pub audio: &'a [i16],
    /// What the core asked for during this frame, in call order. Events of
    /// a `load_game` or `reset` come with the first frame after it.
    pub events: &'a [Event],
    /// What the core and the host logged during this frame.
    pub logs: &'a [LogRecord],
    /// What did not fit in the buffers.
    pub dropped: Dropped,
}

/// The picture of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Video<'a> {
    /// A new frame. Its rows are packed: the pitch is the width times the
    /// bytes per pixel.
    Frame(Frame<'a>),
    /// The core sent no data: show the last frame again. The last good
    /// frame is given when there is one.
    Duplicate(Option<Frame<'a>>),
    /// The core did not call the video callback in this frame.
    Missing,
    /// The core sent a frame that failed a check. The frame was not copied.
    Rejected(FrameFault),
}

/// A request of the core during a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `SET_GEOMETRY`: a new base size or aspect, the same maximum.
    Geometry(GameGeometry),
    /// `SET_SYSTEM_AV_INFO`: new geometry and timing.
    AvInfo(AvInfo),
    /// `SET_PIXEL_FORMAT` during a frame.
    PixelFormat(PixelFormat),
    /// `SHUTDOWN`: the core asks to stop.
    Shutdown,
    /// `SET_MESSAGE`: a notice to show.
    Message {
        /// The text, at most 1,024 bytes.
        text: String,
        /// How long to show it, in frames.
        frames: u32,
    },
    /// The host refused a request of the core.
    Refused(Refusal),
}

/// A request of the core that the host refused, with the reason.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Refusal {
    /// `SET_GEOMETRY` with a value that failed a check.
    Geometry(GeometryError),
    /// `SET_SYSTEM_AV_INFO` with a value that failed a check.
    AvInfo(GeometryError),
    /// `SET_PIXEL_FORMAT` with a value that libretro does not define.
    PixelFormat {
        /// The value given.
        value: u32,
    },
    /// The buffers for a larger maximum size could not be allocated.
    OutOfMemory {
        /// The bytes asked for.
        bytes: usize,
    },
}

/// Why a frame of the core was not copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameFault {
    /// No geometry is known yet: the frame came before `load_game` read it.
    #[error("frame before the geometry is known")]
    NoGeometry,
    /// The width or the height is 0.
    #[error("frame {width}x{height} has a zero side")]
    ZeroSize {
        /// Width given.
        width: u32,
        /// Height given.
        height: u32,
    },
    /// The frame is larger than the maximum size of the core.
    #[error("frame {width}x{height} is larger than the maximum {max_width}x{max_height}")]
    OverMaximum {
        /// Width given.
        width: u32,
        /// Height given.
        height: u32,
        /// The maximum width in force.
        max_width: u16,
        /// The maximum height in force.
        max_height: u16,
    },
    /// The pitch is smaller than one row of pixels.
    #[error("pitch {pitch} is smaller than a row of {row_bytes} bytes")]
    PitchTooSmall {
        /// Pitch given, in bytes.
        pitch: usize,
        /// Bytes of one row.
        row_bytes: usize,
    },
    /// The pitch is larger than [`MAX_PITCH_BYTES`](crate::MAX_PITCH_BYTES).
    #[error("pitch {pitch} is above the limit of {limit} bytes")]
    PitchTooLarge {
        /// Pitch given, in bytes.
        pitch: usize,
        /// The limit.
        limit: usize,
    },
}

/// Counts of what did not fit in one call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dropped {
    /// Stereo audio frames.
    pub audio_frames: usize,
    /// Events.
    pub events: usize,
    /// Log records.
    pub log_records: usize,
}
