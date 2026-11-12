//! Game lifecycle: launch, the display-locked loop, pause, quick menu, exit
//! and save RAM flush. It mediates between UI, core host and video.
//!
//! [`Session`] runs one game. The display is the master clock: each
//! [`Session::run_frame`] runs one core frame and presents it on one flip,
//! or on two flips for double refresh. The audio follows the run rate of
//! the mode on screen with dynamic rate control.

#![forbid(unsafe_code)]

mod config;
mod error;
mod pads;
mod picture;
mod report;
mod session;
mod source;

pub use config::{Devices, GameLaunch, RunRateSource, SessionOptions, VideoSetup};
pub use error::{LaunchError, SessionError};
pub use pads::CORE_PORTS;
pub use report::{Counters, EndReason, ExitReport, FrameReport};
pub use session::{FLIPLESS_FRAMES_LIMIT, Session};
pub use source::{
    INTERLACE_MIN_LINES, LAUNCH_SETTLE_MAX_FRAMES, LAUNCH_STABLE_FRAMES, source_geometry,
};
