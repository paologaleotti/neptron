//! The transport over a real Unix socket in a temporary directory. Every
//! blocking read has a timeout, so a fault fails the test and never hangs
//! it.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use neptron_ipc::{
    ClientError, ClockStepArgs, Command, ControlListener, ErrorCode, FrameReached, ListenError,
    Reply, ScreenshotArgs, Stage, serve_stream,
};

use crate::fixture::FixedHandler;

const GUARD: Duration = Duration::from_secs(10);

/// A directory that the test owns and removes at the end.
pub struct TempDir(PathBuf);

impl TempDir {
    /// A new empty directory under the system temporary directory.
    pub fn new() -> Self {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("nipc-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// The socket path in the directory.
    pub fn socket(&self) -> PathBuf {
        self.0.join("c.sock")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Cleanup is best effort: a leftover directory does not change a
        // test result.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Serves one connection with a [`FixedHandler`] on a thread, and gives
/// back the handler when the client closes the connection.
pub fn serve_one(listener: ControlListener) -> thread::JoinHandle<FixedHandler> {
    thread::spawn(move || {
        let stream = listener.accept().unwrap();
        stream.set_read_timeout(Some(GUARD)).unwrap();
        let mut handler = FixedHandler::default();
        serve_stream(&stream, &mut handler).unwrap();
        handler
    })
}

fn client(path: &Path) -> neptron_ipc::Client<UnixStream> {
    let client = neptron_ipc::Client::connect(path).unwrap();
    client.set_timeout(Some(GUARD)).unwrap();
    client
}

#[test]
fn a_client_calls_a_server_over_the_socket() {
    let dir = TempDir::new();
    let server = serve_one(ControlListener::bind(&dir.socket()).unwrap());
    let mut client = client(&dir.socket());

    let Reply::Hello(hello) = client.call(Command::Hello).unwrap() else {
        panic!("not a hello reply")
    };
    assert_eq!(hello.server, "fixture 1.0");
    assert_eq!(
        client
            .call(Command::ClockStep(ClockStepArgs { frames: 4 }))
            .unwrap(),
        Reply::ClockStep(FrameReached { frame: 4 })
    );
    let Reply::Screenshot(shot) = client
        .call(Command::Screenshot(ScreenshotArgs {
            stage: Stage::Output,
        }))
        .unwrap()
    else {
        panic!("not a screenshot reply")
    };
    assert_eq!(shot.png, [0x89, b'P', b'N', b'G', 0, 255]);

    drop(client);
    assert_eq!(server.join().unwrap().frame, 4);
}

#[test]
fn a_server_error_reaches_the_client() {
    let dir = TempDir::new();
    let server = serve_one(ControlListener::bind(&dir.socket()).unwrap());
    let mut client = client(&dir.socket());
    let err = client
        .call(Command::Wait(neptron_ipc::WaitArgs {
            until: neptron_ipc::WaitCondition::Frame(100),
            timeout_ms: 1,
        }))
        .unwrap_err();
    let ClientError::Server(body) = err else {
        panic!("{err}")
    };
    assert_eq!(body.code, ErrorCode::Timeout);
    drop(client);
    server.join().unwrap();
}

#[test]
fn a_malformed_line_on_the_socket_gets_an_error_line() {
    let dir = TempDir::new();
    let server = serve_one(ControlListener::bind(&dir.socket()).unwrap());
    let mut stream = UnixStream::connect(dir.socket()).unwrap();
    stream.set_read_timeout(Some(GUARD)).unwrap();
    stream.write_all(b"}{\n").unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    assert!(
        line.starts_with(r#"{"v":2,"id":null,"error":{"code":"malformed""#),
        "{line}"
    );
    drop(stream);
    server.join().unwrap();
}

#[test]
fn the_socket_file_goes_away_with_the_listener() {
    let dir = TempDir::new();
    let listener = ControlListener::bind(&dir.socket()).unwrap();
    assert_eq!(listener.path(), dir.socket());
    assert!(dir.socket().exists());
    drop(listener);
    assert!(!dir.socket().exists());
}

#[test]
fn a_live_server_keeps_its_socket() {
    let dir = TempDir::new();
    let _first = ControlListener::bind(&dir.socket()).unwrap();
    let err = ControlListener::bind(&dir.socket()).unwrap_err();
    assert!(matches!(err, ListenError::InUse { .. }), "{err}");
}

#[test]
fn a_stale_socket_file_is_replaced() {
    let dir = TempDir::new();
    drop(UnixListener::bind(dir.socket()).unwrap());
    assert!(dir.socket().exists());
    let listener = ControlListener::bind(&dir.socket()).unwrap();
    assert!(UnixStream::connect(listener.path()).is_ok());
}

#[test]
fn a_file_that_is_not_a_socket_is_left_alone() {
    let dir = TempDir::new();
    std::fs::write(dir.socket(), b"keep me").unwrap();
    let err = ControlListener::bind(&dir.socket()).unwrap_err();
    assert!(matches!(err, ListenError::NotASocket { .. }), "{err}");
    assert_eq!(std::fs::read(dir.socket()).unwrap(), b"keep me");
}

#[test]
fn a_socket_that_refuses_for_another_reason_is_left_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new();
    let live = UnixListener::bind(dir.socket()).unwrap();
    std::fs::set_permissions(dir.socket(), std::fs::Permissions::from_mode(0o000)).unwrap();
    let probe = UnixStream::connect(dir.socket());
    let err = ControlListener::bind(&dir.socket()).unwrap_err();
    std::fs::set_permissions(dir.socket(), std::fs::Permissions::from_mode(0o700)).unwrap();
    match probe {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            assert!(
                matches!(
                    err,
                    ListenError::Io {
                        action: "connect to",
                        ..
                    }
                ),
                "{err}"
            );
        }
        // A user that bypasses file permissions connects, so the socket is
        // live.
        _ => assert!(matches!(err, ListenError::InUse { .. }), "{err}"),
    }
    assert!(UnixStream::connect(dir.socket()).is_ok());
    drop(live);
}

#[test]
fn a_file_swapped_in_after_the_bind_survives_the_drop() {
    let dir = TempDir::new();
    let listener = ControlListener::bind(&dir.socket()).unwrap();
    std::fs::remove_file(dir.socket()).unwrap();
    std::fs::write(dir.socket(), b"keep me").unwrap();
    drop(listener);
    assert_eq!(std::fs::read(dir.socket()).unwrap(), b"keep me");
}

#[test]
fn a_socket_swapped_in_after_the_bind_survives_the_drop() {
    let dir = TempDir::new();
    let listener = ControlListener::bind(&dir.socket()).unwrap();
    std::fs::remove_file(dir.socket()).unwrap();
    let other = UnixListener::bind(dir.socket()).unwrap();
    drop(listener);
    assert!(UnixStream::connect(dir.socket()).is_ok());
    drop(other);
}

#[test]
fn connecting_to_a_missing_socket_fails() {
    let dir = TempDir::new();
    let err = neptron_ipc::Client::connect(&dir.socket()).unwrap_err();
    assert!(matches!(err, ClientError::Connect { .. }), "{err}");
}

#[test]
fn a_server_that_never_answers_times_out() {
    let dir = TempDir::new();
    let listener = ControlListener::bind(&dir.socket()).unwrap();
    let client_stream = UnixStream::connect(dir.socket()).unwrap();
    let _held = listener.accept().unwrap();
    let mut client = neptron_ipc::Client::from_stream(client_stream);
    client
        .stream()
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let err = client.call(Command::Hello).unwrap_err();
    assert!(matches!(err, ClientError::Timeout), "{err}");
}

#[test]
fn a_server_that_closes_is_reported() {
    let dir = TempDir::new();
    let listener = ControlListener::bind(&dir.socket()).unwrap();
    let mut client = client(&dir.socket());
    drop(listener.accept().unwrap());
    let err = client.call(Command::Hello).unwrap_err();
    assert!(
        matches!(err, ClientError::Closed | ClientError::Io(_)),
        "{err}"
    );
}

/// Accepts one connection, reads one line, and answers with `answer`.
fn answer_once(listener: ControlListener, answer: &'static [u8]) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stream = listener.accept().unwrap();
        stream.set_read_timeout(Some(GUARD)).unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        (&stream).write_all(answer).unwrap();
    })
}

#[test]
fn a_too_long_response_is_refused() {
    let dir = TempDir::new();
    let server = answer_once(
        ControlListener::bind(&dir.socket()).unwrap(),
        br#"{"v":1,"id":1,"reply":"log_tail","data":{"lines":["a long line"]}}"#,
    );
    let mut client = client(&dir.socket());
    client.set_max_line_bytes(16);
    let err = client.call(Command::Hello).unwrap_err();
    assert!(matches!(err, ClientError::TooLong { limit: 16 }), "{err}");
    server.join().unwrap();
}

#[test]
fn a_response_for_another_request_is_refused() {
    let dir = TempDir::new();
    let server = answer_once(
        ControlListener::bind(&dir.socket()).unwrap(),
        b"{\"v\":1,\"id\":1,\"reply\":\"clock_step\",\"data\":{\"frame\":1}}\n",
    );
    let mut client = client(&dir.socket());
    let err = client.call(Command::Hello).unwrap_err();
    assert!(
        matches!(
            err,
            ClientError::Mismatch {
                sent_id: 1,
                command: "hello",
                got_id: Some(1),
                reply: Some("clock_step"),
            }
        ),
        "{err}"
    );
    server.join().unwrap();
}
