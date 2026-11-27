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
mod scale;
mod session;
mod source;
mod speed;
mod start;

pub use config::{Devices, GameLaunch, RunRateSource, SessionOptions, VideoSetup};
pub use error::{LaunchError, SessionError};
/// The discs of a disc game, which the app reaches only through the
/// session.
pub use neptron_libretro::{DiscError, DiscState};
/// The hardware context that a core asked for and the host refused, for
/// the notice of the app, which reaches the host only through the session.
pub use neptron_libretro::{HwApi, HwRefusal, HwRefusalReason};
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
pub use scale::{INTERNAL_SCALE_LIMIT, InternalScale, Machine, internal_scale, scale_value};
pub use session::{CONTEXT_RECOVERIES_MAX, DISC_TRAY_OPEN_FRAMES, FLIPLESS_FRAMES_LIMIT, Session};
pub use source::{
    FrameRules, INTERLACE_MIN_LINES, LAUNCH_SETTLE_MAX_FRAMES, LAUNCH_STABLE_FRAMES,
    source_geometry,
};
pub use speed::{
    QUEUE_BAND_PERMILLE, QUEUE_SMOOTH_FRAMES, QUEUE_TRIM_PERMILLE, RUN_RATE_SLEW_PERMILLE_PER_S,
    SLOW_NOTICE_PERMILLE, SOUND_MUTE_PERMILLE, SOUND_RESUME_PERMILLE, SPEED_CHANGE_PERMILLE,
    SPEED_CLEAR_FAST_PERMILLE, SPEED_CLEAR_SLOW_FRAMES, SPEED_CLEAR_SLOW_PERMILLE,
    SPEED_MIN_FRAMES, SPEED_RECENT_FRAMES, SPEED_SLOW_MARGIN_PERMILLE, SPEED_WINDOW_FRAMES,
};
pub use start::{START_ERRORS_MAX, START_WINDOW_NS, StartFacts, end_reason};
