use neptron_video::{Directive, Outcome, VideoError};

use crate::{Flush, SaveStats};

/// What one call of [`Session::run_frame`](crate::Session::run_frame) did.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameReport {
    /// The core ran one frame. False while the quick menu is open.
    pub ran_core: bool,
    /// What the engine decided for the frame.
    pub directive: Directive,
    /// Flips that completed in this call: 1, or 2 for double refresh. In
    /// late pacing the first of them is the flip of the frame before,
    /// which this frame waited for after its core ran, and the last flip
    /// of this frame stays queued (`deferred`). 0 when the display
    /// failed; `display_error` says why.
    pub flips: u8,
    /// Flips of `flips` that the engine drained in a present after a
    /// failed wait. They count as presented, and not as progress of the
    /// display for [`FLIPLESS_FRAMES_LIMIT`](crate::FLIPLESS_FRAMES_LIMIT).
    pub drained_flips: u8,
    /// The last flip of this frame is queued, and the loop did not wait
    /// for it: the core is in late pacing. The next frame waits for it
    /// after its core ran.
    pub deferred: bool,
    /// The loop changed its pacing in this frame.
    pub pacing: Option<PacingChange>,
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

/// A change of the pacing of the loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacingChange {
    /// From wait pacing to late pacing: a core frame did not fit in its
    /// flip interval, or flips landed late in a row.
    Late,
    /// From late pacing back to wait pacing: the hold passed with frames
    /// that fit, or a frame with no core, a modeset, a display error or
    /// the end of the session.
    Wait,
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The core asked to stop (`RETRO_ENVIRONMENT_SHUTDOWN`).
    CoreShutdown,
    /// The core asked to stop before a picture of the game reached the
    /// screen, or after it logged an error during the launch: the game did
    /// not start. FinalBurn Neo does this at its own error screen.
    /// [`Session::start_errors`](crate::Session::start_errors) holds the
    /// words of the core.
    FailedStart,
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
    /// Flips that completed: the flips the loop waited for, and the flips
    /// that a later frame or the engine waited for first.
    pub flips: u64,
    /// Loop frames with no completed flip, the output on.
    pub flipless_frames: u64,
    /// Fields in which no new frame reached the display while the core
    /// ran: the vertical blanks between two flips beyond the blanks one
    /// flip of the mode takes (1, or 2 for an interlaced mode on a driver
    /// that flips once per frame). A flip late by one field adds 1. Each
    /// flip of a frame in which the core ran counts its gap to the flip
    /// before; the first flip of a mode counts nothing.
    pub missed_fields: u64,
    /// Loop frames with the output off.
    pub blanked_frames: u64,
    /// Core frames whose last flip the loop did not wait for, because the
    /// core is in late pacing: the next core frame ran while this one
    /// waited for its vertical blank.
    pub late_frames: u64,
    /// The last delay, in nanoseconds of the session clock, from an input
    /// poll that returned at least one event to the first flip of the
    /// frame that ran the core with it. The time of the flip event is
    /// moved to the session clock with the lowest offset between the two
    /// clocks that a wait showed. 0 before the first such flip.
    pub input_to_flip_ns: u64,
    /// Flips of a paced core frame that landed after the blank they aimed
    /// at although the work of the frame fit in the interval: the GPU or
    /// the display made them late.
    pub late_flips: u64,
    /// Frames the host refused (`Video::Rejected`).
    pub frame_faults: u64,
    /// Frames with no new picture (`Video::Duplicate` or `Video::Missing`).
    pub repeated_pictures: u64,
    /// Audio frames, events and log records that the host dropped.
    pub host_drops: u64,
    /// Log records of the core and the host. The app reads the records of
    /// the last frame with `Session::core_logs`.
    pub log_records: u64,
    /// Held core option values that did not apply after the load: a key
    /// the core did not declare, or a value it does not list.
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
