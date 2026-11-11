//! The control protocol: JSON lines on a Unix socket for screenshot, UI tree,
//! input, clock, session, video state, stats, settings and log. A TCP
//! listener exists only with the `control` feature.
//!
//! The message types and the codec do no I/O. The socket layer is thin, and
//! the handlers that serve each command live in the app.

#![forbid(unsafe_code)]

pub mod base64;
mod command;
mod envelope;
mod error;
mod line;
mod reply;
mod server;
mod transport;

pub use command::{
    ClockStepArgs, Command, InputScriptArgs, InputSendArgs, LogTailArgs, ScreenshotArgs,
    ScriptedInput, SessionLaunchArgs, SessionState, SettingLevel, SettingsGetArgs, SettingsSetArgs,
    Stage, WaitArgs, WaitCondition,
};
pub use envelope::{
    CodecError, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, Refusal, Request, Response, decode_request,
    decode_response, encode_request, encode_response,
};
pub use error::{ErrorBody, ErrorCode};
pub use line::{LineRead, MAX_REQUEST_LINE_BYTES, MAX_RESPONSE_LINE_BYTES, read_line};
pub use reply::{
    FrameReached, HelloReply, InputQueued, LogLines, Reply, Screenshot, ScriptQueued,
    SessionStatus, SettingValue, SettingWritten, Stats, UiRow, UiTree, Verdict, VideoState,
};
pub use server::{Handler, ServeError, dispatch, handle_line, serve_lines};
pub use transport::{
    Client, ClientError, ControlListener, DEFAULT_SOCKET_PATH, ListenError, SOCKET_PATH_ENV,
    serve_stream,
};
