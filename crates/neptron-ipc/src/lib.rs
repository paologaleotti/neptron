//! The control protocol: JSON lines on a Unix socket for screenshot, UI tree,
//! input, clock, session, video state, stats, settings and log. A TCP
//! listener exists only with the `control` feature.
//!
//! The message types and the codec do no I/O. The socket layer is thin, and
//! the handlers that serve each command live in the app.

#![forbid(unsafe_code)]

pub mod base64;
mod command;
mod error;
mod reply;

pub use command::{
    ClockStepArgs, Command, InputScriptArgs, InputSendArgs, LogTailArgs, ScreenshotArgs,
    ScriptedInput, SessionLaunchArgs, SessionState, SettingLevel, SettingsGetArgs, SettingsSetArgs,
    Stage, WaitArgs, WaitCondition,
};
pub use error::{ErrorBody, ErrorCode};
pub use reply::{
    FrameReached, HelloReply, InputQueued, LogLines, Reply, Screenshot, ScriptQueued,
    SessionStatus, SettingValue, SettingWritten, Stats, UiRow, UiTree, Verdict, VideoState,
};
