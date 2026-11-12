//! The control protocol on a real socket: the I/O thread hands each
//! command to the frame thread, which answers between two frames.

use std::sync::mpsc;

use neptron::control::ControlServer;
use neptron_ipc::{
    Client, ClientError, Command, ErrorCode, Reply, SessionState, SettingsGetArgs, WaitArgs,
    WaitCondition,
};

use crate::rig::Rig;

#[test]
fn the_socket_serves_the_frame_thread() {
    let dir = std::env::temp_dir().join(format!("nctl-app-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    rig.boot(None);
    let socket = dir.join("c.sock");
    let (server, mut port) = ControlServer::start(&socket).expect("the socket");

    let (done_tx, done) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let mut client = Client::connect(&socket).expect("connect");
        let mut call = |command| client.call(command).expect("a reply");
        let hello = call(Command::Hello);
        let tree = call(Command::UiTree);
        let state = call(Command::SessionState);
        let style = call(Command::SettingsGet(SettingsGetArgs {
            key: "display.style".to_owned(),
            level: None,
        }));
        let waited = call(Command::Wait(WaitArgs {
            until: WaitCondition::Frame(20),
            timeout_ms: 10_000,
        }));
        let step = client.call(Command::ClockStep(neptron_ipc::ClockStepArgs { frames: 1 }));
        // The frame thread stops when the client is done.
        let _ = done_tx.send(());
        (hello, tree, state, style, waited, step)
    });
    while done.try_recv().is_err() {
        let now_ns = rig.shell().now_ns();
        port.serve(rig.shell(), now_ns);
        rig.frames(1);
    }
    let (hello, tree, state, style, waited, step) = client.join().expect("the client");
    server.stop(port);
    rig.finish();

    assert!(matches!(hello, Reply::Hello(h) if h.server.starts_with("neptron")));
    assert!(
        matches!(tree, Reply::UiTree(t) if t.screen == "confirm"),
        "the boot prompt"
    );
    assert!(matches!(state, Reply::SessionState(s) if s.state == SessionState::Idle));
    let Reply::SettingsGet(style) = style else {
        panic!("settings_get");
    };
    assert_eq!(
        style.value,
        Some(serde_json::Value::String("auto".to_owned()))
    );
    assert!(matches!(waited, Reply::Wait(f) if f.frame >= 20));
    assert!(step.is_err(), "clock_step needs simulated time");
}

/// Starts a run in the game list with a control socket under a new folder.
fn socket_rig(name: &str) -> (Rig, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("nctl-{name}-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    rig.boot(None);
    let socket = dir.join("c.sock");
    (rig, socket)
}

/// The longest a test waits for its client thread: a guard against a hang
/// only, far above what a passing run takes.
const HANG_GUARD: std::time::Duration = std::time::Duration::from_secs(60);

/// The frames a test runs at most while it waits for its client.
const FRAMES_MAX: u32 = 1_000_000;

/// Serves frames until `ready` holds, at most [`FRAMES_MAX`].
fn serve_until(
    rig: &mut Rig,
    port: &mut neptron::control::ControlPort,
    ready: impl Fn(&neptron::control::ControlPort) -> bool,
) {
    for _ in 0..FRAMES_MAX {
        if ready(port) {
            return;
        }
        let now_ns = rig.shell().now_ns();
        port.serve(rig.shell(), now_ns);
        rig.frames(1);
    }
    panic!("the client did not reach the port in {FRAMES_MAX} frames");
}

#[test]
fn a_wait_open_at_the_end_of_the_run_gets_unavailable() {
    let (mut rig, socket) = socket_rig("wait");
    let (server, mut port) = ControlServer::start(&socket).expect("the socket");
    let (answer_tx, answer) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let mut client = Client::connect(&socket).expect("connect");
        let reply = client.call(Command::Wait(WaitArgs {
            until: WaitCondition::Frame(u64::MAX),
            timeout_ms: u32::MAX,
        }));
        // The test thread may have panicked; then nobody reads the answer.
        let _ = answer_tx.send(reply);
    });
    serve_until(&mut rig, &mut port, |p| p.open_waits() == 1);
    server.stop(port);
    let reply = answer
        .recv_timeout(HANG_GUARD)
        .expect("the answer of the wait");
    let Err(ClientError::Server(error)) = reply else {
        panic!("the open wait gets an error: {reply:?}");
    };
    assert_eq!(error.code, ErrorCode::Unavailable, "{error:?}");
    client.join().expect("the client");
    rig.finish();
}

#[test]
fn a_busy_client_does_not_hold_the_end_of_the_run() {
    let (mut rig, socket) = socket_rig("busy");
    let (server, mut port) = ControlServer::start(&socket).expect("the socket");
    let (busy_tx, busy) = mpsc::channel();
    let (calls_tx, calls) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let mut client = Client::connect(&socket).expect("connect");
        let mut count = 0u64;
        while client.call(Command::Hello).is_ok() {
            count += 1;
            if count == 10 {
                // The test thread may have panicked; then nobody reads it.
                let _ = busy_tx.send(());
            }
        }
        let _ = calls_tx.send(count);
    });
    serve_until(&mut rig, &mut port, |_| busy.try_recv().is_ok());
    server.stop(port);
    let count = calls
        .recv_timeout(HANG_GUARD)
        .expect("the client ends after the stop");
    assert!(count >= 10, "{count}");
    client.join().expect("the client");
    rig.finish();
}

#[test]
fn the_output_cannot_be_set_over_the_protocol() {
    let (mut rig, _) = socket_rig("output");
    let result = rig.shell().set_from_protocol(
        "display.output",
        neptron_settings::Level::Global,
        &serde_json::Value::String("VGA-1".to_owned()),
    );
    assert!(result.is_err(), "{result:?}");
    rig.finish();
}
