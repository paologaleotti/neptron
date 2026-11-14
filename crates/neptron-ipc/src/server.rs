use std::io::{self, BufRead, Write};

use crate::command::{
    ClockStepArgs, Command, InputScriptArgs, InputSendArgs, LogTailArgs, ScreenshotArgs,
    SessionLaunchArgs, SettingsGetArgs, SettingsSetArgs, WaitArgs,
};
use crate::envelope::{CodecError, PROTOCOL_VERSION, Response, decode_request, encode_response};
use crate::error::{ErrorBody, ErrorCode};
use crate::line::{LineRead, MAX_REQUEST_LINE_BYTES, read_line};
use crate::reply::{
    FrameReached, HelloReply, InputQueued, LogLines, Reply, Screenshot, ScriptQueued,
    SessionStatus, SettingValue, SettingWritten, Stats, UiTree, VideoState,
};

/// The server side of the control API: one method per command.
///
/// [`dispatch`] calls the method that matches the command and wraps its
/// result in the matching [`Reply`] variant, so a handler cannot answer a
/// command with the reply of another. Every method has a default that
/// answers `unavailable`, so a server serves only the commands it
/// implements. A handler that changes the display (a launch, a setting of
/// the video group) passes the mode through the safety validator, as every
/// other path does.
pub trait Handler {
    /// The name and version of the server program, for `hello`.
    fn server_name(&self) -> &str;

    /// Serves `screenshot`.
    fn screenshot(&mut self, _args: ScreenshotArgs) -> Result<Screenshot, ErrorBody> {
        Err(unavailable("screenshot"))
    }

    /// Serves `ui_tree`.
    fn ui_tree(&mut self) -> Result<UiTree, ErrorBody> {
        Err(unavailable("ui_tree"))
    }

    /// Serves `input_send`.
    fn input_send(&mut self, _args: InputSendArgs) -> Result<InputQueued, ErrorBody> {
        Err(unavailable("input_send"))
    }

    /// Serves `input_script`.
    fn input_script(&mut self, _args: InputScriptArgs) -> Result<ScriptQueued, ErrorBody> {
        Err(unavailable("input_script"))
    }

    /// Serves `clock_step`.
    fn clock_step(&mut self, _args: ClockStepArgs) -> Result<FrameReached, ErrorBody> {
        Err(unavailable("clock_step"))
    }

    /// Serves `wait`. The method answers the `timeout` error when
    /// `args.timeout_ms` passes before the condition holds.
    fn wait(&mut self, _args: WaitArgs) -> Result<FrameReached, ErrorBody> {
        Err(unavailable("wait"))
    }

    /// Serves `session_launch`.
    fn session_launch(&mut self, _args: SessionLaunchArgs) -> Result<SessionStatus, ErrorBody> {
        Err(unavailable("session_launch"))
    }

    /// Serves `session_state`.
    fn session_state(&mut self) -> Result<SessionStatus, ErrorBody> {
        Err(unavailable("session_state"))
    }

    /// Serves `video_state`.
    fn video_state(&mut self) -> Result<VideoState, ErrorBody> {
        Err(unavailable("video_state"))
    }

    /// Serves `stats`.
    fn stats(&mut self) -> Result<Stats, ErrorBody> {
        Err(unavailable("stats"))
    }

    /// Serves `settings_get`.
    fn settings_get(&mut self, _args: SettingsGetArgs) -> Result<SettingValue, ErrorBody> {
        Err(unavailable("settings_get"))
    }

    /// Serves `settings_set`.
    fn settings_set(&mut self, _args: SettingsSetArgs) -> Result<SettingWritten, ErrorBody> {
        Err(unavailable("settings_set"))
    }

    /// Serves `log_tail`.
    fn log_tail(&mut self, _args: LogTailArgs) -> Result<LogLines, ErrorBody> {
        Err(unavailable("log_tail"))
    }
}

/// A failure that ends a connection.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    /// The request stream failed.
    #[error("cannot read a request: {0}")]
    Read(io::Error),
    /// The response stream failed.
    #[error("cannot write a response: {0}")]
    Write(io::Error),
    /// A response did not encode, and the error that replaces it did not
    /// encode either.
    #[error(transparent)]
    Encode(CodecError),
}

/// Calls the handler method for `command` and wraps its result in the
/// matching reply.
pub fn dispatch<H: Handler + ?Sized>(
    handler: &mut H,
    command: Command,
) -> Result<Reply, ErrorBody> {
    Ok(match command {
        Command::Hello => Reply::Hello(HelloReply {
            protocol: PROTOCOL_VERSION,
            server: handler.server_name().to_owned(),
        }),
        Command::Screenshot(args) => Reply::Screenshot(handler.screenshot(args)?),
        Command::UiTree => Reply::UiTree(handler.ui_tree()?),
        Command::InputSend(args) => Reply::InputSend(handler.input_send(args)?),
        Command::InputScript(args) => Reply::InputScript(handler.input_script(args)?),
        Command::ClockStep(args) => Reply::ClockStep(handler.clock_step(args)?),
        Command::Wait(args) => Reply::Wait(handler.wait(args)?),
        Command::SessionLaunch(args) => Reply::SessionLaunch(handler.session_launch(args)?),
        Command::SessionState => Reply::SessionState(handler.session_state()?),
        Command::VideoState => Reply::VideoState(handler.video_state()?),
        Command::Stats => Reply::Stats(handler.stats()?),
        Command::SettingsGet(args) => Reply::SettingsGet(handler.settings_get(args)?),
        Command::SettingsSet(args) => Reply::SettingsSet(handler.settings_set(args)?),
        Command::LogTail(args) => Reply::LogTail(handler.log_tail(args)?),
    })
}

/// Answers one request line: decodes it, dispatches it and gives the
/// response. A line that does not decode gets an error response.
pub fn handle_line<H: Handler + ?Sized>(handler: &mut H, line: &[u8]) -> Response {
    match decode_request(line) {
        Ok(request) => Response {
            version: request.version,
            id: Some(request.id),
            outcome: dispatch(handler, request.command).map(|r| r.for_version(request.version)),
        },
        Err(refusal) => Response {
            version: refusal.version,
            id: refusal.id,
            outcome: Err(refusal.error),
        },
    }
}

/// Serves one connection: reads request lines from `reader` and writes one
/// response line to `writer` for each, in order.
///
/// The function returns when the peer closes the stream. A line longer than
/// [`MAX_REQUEST_LINE_BYTES`] gets the `line_too_long` error, and then the
/// function returns, because the rest of that line can be of any length and
/// the stream has no next line to resume at.
pub fn serve_lines<R, W, H>(
    reader: &mut R,
    writer: &mut W,
    handler: &mut H,
) -> Result<(), ServeError>
where
    R: BufRead + ?Sized,
    W: Write + ?Sized,
    H: Handler + ?Sized,
{
    let mut line = Vec::new();
    loop {
        let read =
            read_line(reader, MAX_REQUEST_LINE_BYTES, &mut line).map_err(ServeError::Read)?;
        let response = match read {
            LineRead::End => return Ok(()),
            LineRead::Line => handle_line(handler, &line),
            LineRead::TooLong => Response {
                version: PROTOCOL_VERSION,
                id: None,
                outcome: Err(ErrorBody::new(
                    ErrorCode::LineTooLong,
                    format!(
                        "the line has more than {MAX_REQUEST_LINE_BYTES} bytes; the server closes the connection"
                    ),
                )),
            },
        };
        write_response(writer, &response)?;
        if read == LineRead::TooLong {
            return Ok(());
        }
    }
}

fn write_response<W: Write + ?Sized>(
    writer: &mut W,
    response: &Response,
) -> Result<(), ServeError> {
    let bytes = match encode_response(response) {
        Ok(bytes) => bytes,
        Err(e) => encode_response(&Response {
            version: response.version,
            id: response.id,
            outcome: Err(ErrorBody::new(ErrorCode::Failed, e.to_string())),
        })
        .map_err(ServeError::Encode)?,
    };
    writer.write_all(&bytes).map_err(ServeError::Write)?;
    writer.flush().map_err(ServeError::Write)
}

fn unavailable(command: &str) -> ErrorBody {
    ErrorBody::new(
        ErrorCode::Unavailable,
        format!("this server does not serve {command:?}"),
    )
}
