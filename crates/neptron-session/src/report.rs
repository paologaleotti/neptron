use neptron_video::{Directive, Outcome, VideoError};

use crate::{Flush, SaveStats};

/// What one call of [`Session::run_frame`](crate::Session::run_frame) did.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameReport {
    /// The core ran one frame. False while the quick menu is open.
    pub ran_core: bool,
    /// What the engine decided for the frame.
    pub directive: Directive,
    /// Flips of this frame that completed: 1, or 2 for double refresh. 0
    /// when the display failed; `display_error` says why.
    pub flips: u8,
    /// The outcome of a modeset in this frame.
    pub modeset: Option<Outcome>,
    /// The first display error of this frame.
    pub display_error: Option<VideoError>,
    /// The quick menu opened in this frame. The core is paused.
    pub menu_opened: bool,
    /// The blind recovery combination completed its hold on this port.
    /// The safety layer acts on it, also while the quick menu is open.
    pub blind_recovery: Option<u8>,
    /// The session ended in this frame.
    pub ended: Option<EndReason>,
    /// The engine has turned the output off: the inputs refuse the mode on
    /// the display and no valid mode exists. The core waits, the audio is
    /// muted, and no flip paces the loop, so the caller must. An input
    /// change (the safety layer's revert) or `retry` sets a mode again.
    pub blanked: bool,
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The core asked to stop (`RETRO_ENVIRONMENT_SHUTDOWN`).
    CoreShutdown,
}

/// The counters of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counters {
    /// Core frames run at launch, before the first mode, while the source
    /// settled.
    pub settle_frames: u64,
    /// Core frames run in the frame loop.
    pub core_frames: u64,
    /// Loop frames: core frames plus frames under the quick menu.
    pub loop_frames: u64,
    /// Flips that completed.
    pub flips: u64,
    /// Loop frames with no completed flip, the output on.
    pub flipless_frames: u64,
    /// Loop frames with the output off.
    pub blanked_frames: u64,
    /// Frames the host refused (`Video::Rejected`).
    pub frame_faults: u64,
    /// Frames with no new picture (`Video::Duplicate` or `Video::Missing`).
    pub repeated_pictures: u64,
    /// Audio frames, events and log records that the host dropped.
    pub host_drops: u64,
    /// Log records of the core and the host. The app reads the records of
    /// the last frame with `Session::core_logs`.
    pub log_records: u64,
    /// Core option values from the settings that the core refused.
    pub refused_options: u64,
    /// Run rates that the audio pipeline refused (more than twice or less
    /// than half the core's rate). The last good rate stays.
    pub refused_run_rates: u64,
    /// Core timing changes that the audio pipeline refused. The old
    /// pipeline stays, at the old rate.
    pub refused_timings: u64,
    /// The audio pipeline could not be made again after a timing change.
    /// The session runs with no sound from then on.
    pub audio_lost: bool,
}

/// What [`Session::quit`](crate::Session::quit) did.
#[derive(Debug)]
pub struct ExitReport {
    /// The last flush of the save RAM.
    pub save: Flush,
    /// The counters of the save RAM writer, with the last error.
    pub save_stats: SaveStats,
    /// The counters of the session.
    pub counters: Counters,
}
