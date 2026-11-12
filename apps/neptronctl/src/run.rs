//! Runs one invocation: connects, sends, prints and picks the exit code.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use neptron_ipc::{
    Client, ClientError, Command, DEFAULT_SOCKET_PATH, InputScriptArgs, Reply, ScreenshotArgs,
    ScriptedInput,
};
use serde_json::{Value, json};

use crate::args::{self, Action, Invocation, USAGE};
use crate::selftest::{self, SelftestError};

/// The process exit codes. Scripts depend on them: add codes, never change
/// one.
pub mod exit {
    /// The command succeeded.
    pub const OK: u8 = 0;
    /// The server answered with an error.
    pub const SERVER_ERROR: u8 = 1;
    /// The command line does not follow the usage.
    pub const USAGE: u8 = 2;
    /// The client cannot connect to the socket.
    pub const CONNECT: u8 = 3;
    /// No response came in time, or the response is broken.
    pub const TRANSPORT: u8 = 4;
    /// A local file cannot be read or written.
    pub const LOCAL_FILE: u8 = 5;
    /// The self-test cannot start: no profile, no EDID profile, the run is
    /// refused, or the device cannot be used.
    pub const SELFTEST_SETUP: u8 = 6;
    /// The self-test ran, and a check failed.
    pub const SELFTEST_FAILED: u8 = 7;
}

/// The client timeout when the command line gives none, in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u32 = 10_000;

/// The extra time the client waits after the timeout of a `wait`, for the
/// server to answer `timeout`, in milliseconds.
pub const WAIT_MARGIN_MS: u32 = 5_000;

/// Runs the command line `args` and gives the exit code. `socket_env` is
/// the value of `NEPTRON_CONTROL_SOCKET`.
pub fn main(
    args: &[String],
    socket_env: Option<OsString>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match run(args, socket_env, out) {
        Ok(()) => exit::OK,
        Err(failure) => report(&failure, err),
    }
}

/// Prints a usage error that the parser cannot see, and gives its exit
/// code.
pub fn usage_error(message: &str, err: &mut dyn Write) -> u8 {
    report(&Failure::new(exit::USAGE, "usage", message), err)
}

fn report(failure: &Failure, err: &mut dyn Write) -> u8 {
    // stderr is the last channel left; a failed write cannot be reported
    // anywhere, and the exit code still says what failed.
    let _ = writeln!(err, "{}", failure.json());
    failure.exit_code
}

/// A failed invocation: the exit code and the error line.
struct Failure {
    exit_code: u8,
    code: String,
    message: String,
}

impl Failure {
    fn new(exit_code: u8, code: &str, message: impl Into<String>) -> Self {
        Self {
            exit_code,
            code: code.to_owned(),
            message: message.into(),
        }
    }

    fn json(&self) -> Value {
        json!({"error": {"code": self.code, "message": self.message}})
    }
}

fn run(args: &[String], socket_env: Option<OsString>, out: &mut dyn Write) -> Result<(), Failure> {
    let invocation =
        args::parse(args).map_err(|e| Failure::new(exit::USAGE, "usage", e.to_string()))?;
    let command = match &invocation.action {
        Action::Help => return print(out, USAGE.trim_end()),
        Action::SelftestVideo(video) => {
            let socket = socket_path(invocation.socket.clone(), socket_env);
            return selftest::video(video, &socket, out).map_err(|e| selftest_failure(&e));
        }
        Action::Call(command) => command.clone(),
        Action::Screenshot { stage, .. } => Command::Screenshot(ScreenshotArgs { stage: *stage }),
        Action::InputScript { file } => Command::InputScript(InputScriptArgs {
            events: read_script(file)?,
        }),
    };
    let socket = socket_path(invocation.socket.clone(), socket_env);
    let mut client = Client::connect(&socket).map_err(client_failure)?;
    let timeout = Duration::from_millis(u64::from(timeout_ms(&invocation, &command)));
    client
        .set_timeout(Some(timeout))
        .map_err(|e| Failure::new(exit::TRANSPORT, "transport", e.to_string()))?;
    let reply = client.call(command).map_err(client_failure)?;
    match (&invocation.action, reply) {
        (Action::Screenshot { out: path, .. }, Reply::Screenshot(shot)) => {
            std::fs::write(path, &shot.png).map_err(|e| {
                Failure::new(
                    exit::LOCAL_FILE,
                    "file",
                    format!("cannot write {}: {e}", path.display()),
                )
            })?;
            let summary = json!({
                "stage": shot.stage,
                "width": shot.width,
                "height": shot.height,
                "path": path,
                "bytes": shot.png.len(),
            });
            print(out, &summary.to_string())
        }
        (_, reply) => print(out, &reply_data(&reply)?.to_string()),
    }
}

/// The socket path: the command line, else the environment, else the
/// default.
fn socket_path(flag: Option<PathBuf>, env: Option<OsString>) -> PathBuf {
    flag.or_else(|| env.filter(|v| !v.is_empty()).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH))
}

/// The client timeout. A `wait` gets its own timeout plus a margin, so that
/// the server answers `timeout` before the client gives up.
fn timeout_ms(invocation: &Invocation, command: &Command) -> u32 {
    match (invocation.timeout_ms, command) {
        (Some(ms), _) => ms,
        (None, Command::Wait(args)) => args
            .timeout_ms
            .saturating_add(WAIT_MARGIN_MS)
            .max(DEFAULT_TIMEOUT_MS),
        (None, _) => DEFAULT_TIMEOUT_MS,
    }
}

fn read_script(path: &Path) -> Result<Vec<ScriptedInput>, Failure> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        Failure::new(
            exit::LOCAL_FILE,
            "file",
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    serde_json::from_str(&text).map_err(|e| {
        Failure::new(
            exit::LOCAL_FILE,
            "file",
            format!(
                "{} is not a JSON array of input events: {e}",
                path.display()
            ),
        )
    })
}

/// The `data` of a reply, with the fields that this client version knows.
/// The client asks at its own version, and a server answers it only with
/// names of that version, so a correct server sends no other field.
fn reply_data(reply: &Reply) -> Result<Value, Failure> {
    let mut value = serde_json::to_value(reply)
        .map_err(|e| Failure::new(exit::TRANSPORT, "protocol", e.to_string()))?;
    Ok(value
        .get_mut("data")
        .map(Value::take)
        .unwrap_or(Value::Null))
}

fn client_failure(error: ClientError) -> Failure {
    match error {
        ClientError::Server(body) => {
            Failure::new(exit::SERVER_ERROR, body.code.name(), body.message)
        }
        ClientError::Connect { .. } => Failure::new(exit::CONNECT, "connect", error.to_string()),
        ClientError::Timeout => Failure::new(exit::TRANSPORT, "no_response", error.to_string()),
        ClientError::Closed | ClientError::Io(_) => {
            Failure::new(exit::TRANSPORT, "transport", error.to_string())
        }
        ClientError::TooLong { .. } | ClientError::Codec(_) | ClientError::Mismatch { .. } => {
            Failure::new(exit::TRANSPORT, "protocol", error.to_string())
        }
    }
}

fn selftest_failure(error: &SelftestError) -> Failure {
    let (exit_code, code) = match error {
        SelftestError::Preset { .. }
        | SelftestError::NoEdidProfile { .. }
        | SelftestError::Refused { .. }
        | SelftestError::NoKms
        | SelftestError::Signal { .. } => (exit::SELFTEST_SETUP, "selftest_setup"),
        SelftestError::Device { .. } => (exit::SELFTEST_SETUP, "selftest_device"),
        SelftestError::Failed { .. } => (exit::SELFTEST_FAILED, "selftest_failed"),
        SelftestError::Output { .. } => (exit::LOCAL_FILE, "stdout"),
        SelftestError::Report { .. } => (exit::LOCAL_FILE, "file"),
    };
    Failure::new(exit_code, code, error.to_string())
}

fn print(out: &mut dyn Write, line: &str) -> Result<(), Failure> {
    writeln!(out, "{line}").map_err(|e| Failure::new(exit::LOCAL_FILE, "stdout", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use neptron_ipc::{WaitArgs, WaitCondition};

    fn invocation(timeout_ms: Option<u32>) -> Invocation {
        Invocation {
            socket: None,
            timeout_ms,
            action: Action::Help,
        }
    }

    fn wait(ms: u32) -> Command {
        Command::Wait(WaitArgs {
            until: WaitCondition::Frame(1),
            timeout_ms: ms,
        })
    }

    #[test]
    fn the_socket_comes_from_the_flag_then_the_environment() {
        let flag = Some(PathBuf::from("/a"));
        let env = Some(OsString::from("/b"));
        assert_eq!(socket_path(flag.clone(), env.clone()), PathBuf::from("/a"));
        assert_eq!(socket_path(None, env), PathBuf::from("/b"));
        assert_eq!(
            socket_path(None, Some(OsString::new())),
            PathBuf::from(DEFAULT_SOCKET_PATH)
        );
        assert_eq!(socket_path(None, None), PathBuf::from(DEFAULT_SOCKET_PATH));
    }

    #[test]
    fn a_wait_gets_a_client_timeout_above_its_own() {
        assert_eq!(
            timeout_ms(&invocation(None), &Command::Hello),
            DEFAULT_TIMEOUT_MS
        );
        assert_eq!(
            timeout_ms(&invocation(None), &wait(100)),
            DEFAULT_TIMEOUT_MS
        );
        assert_eq!(timeout_ms(&invocation(None), &wait(60_000)), 65_000);
        assert_eq!(timeout_ms(&invocation(None), &wait(u32::MAX)), u32::MAX);
        assert_eq!(timeout_ms(&invocation(Some(7)), &wait(60_000)), 7);
    }

    #[test]
    fn usage_errors_exit_with_two_and_print_json() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = main(&["warp".into()], None, &mut out, &mut err);
        assert_eq!(code, exit::USAGE);
        assert!(out.is_empty());
        let line: Value = serde_json::from_slice(&err).unwrap();
        assert_eq!(line["error"]["code"], "usage");
    }

    #[test]
    fn help_prints_the_usage_and_exits_with_zero() {
        let mut out = Vec::new();
        let code = main(&["help".into()], None, &mut out, &mut Vec::new());
        assert_eq!(code, exit::OK);
        assert!(
            String::from_utf8(out)
                .unwrap()
                .starts_with("usage: neptronctl")
        );
    }
}
