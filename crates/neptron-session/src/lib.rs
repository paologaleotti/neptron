//! Game lifecycle: launch, the display-locked loop, pause, quick menu, exit
//! and save RAM flush. It mediates between UI, core host and video.
//!
//! [`Session`] runs one game. The display is the master clock: each
//! [`Session::run_frame`] runs one core frame and presents it on one flip,
//! or on two flips for double refresh. The audio follows the run rate of
//! the mode on screen with dynamic rate control. The save RAM goes to a
//! writer thread on a timer, when the quick menu opens, and at exit.

#![forbid(unsafe_code)]

mod config;
mod error;
mod pacing;
mod pads;
mod picture;
mod report;
mod save;
mod session;
mod source;
mod start;

pub use config::{Devices, GameLaunch, RunRateSource, SessionOptions, VideoSetup};
pub use error::{LaunchError, SessionError};
/// The zip listing of the core host, for the arcade set check of the app,
/// which reaches the host only through the session.
pub use neptron_libretro::{ZipDirectoryEntry, ZipError, zip_entries};
pub use pacing::{
    HOLD_DECAY_FRAMES, LATE_FLIPS_TO_ENTER, LATE_HOLD_FRAMES, LATE_HOLD_MAX_FRAMES,
    TRY_MARGIN_PERMILLE,
};
pub use pads::{CORE_PORTS, LATE_PRESS_MAX_FRAMES, LATE_PRESS_MIN_FRAMES};
pub use report::{Counters, EndReason, ExitReport, FrameReport, PacingChange};
pub use save::{Flush, SAVE_RAM_FLUSH_INTERVAL_NS, SaveError, SaveStats};
pub use session::{FLIPLESS_FRAMES_LIMIT, Session};
pub use source::{
    FrameRules, INTERLACE_MIN_LINES, LAUNCH_SETTLE_MAX_FRAMES, LAUNCH_STABLE_FRAMES,
    source_geometry,
};
pub use start::{START_ERRORS_MAX, START_WINDOW_NS, StartFacts, end_reason};
