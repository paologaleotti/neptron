//! The control protocol on a real socket: the I/O thread hands each
//! command to the frame thread, which answers between two frames.

use std::sync::mpsc;

use neptron::control::ControlServer;
use neptron_ipc::{Client, Command, Reply, SessionState, SettingsGetArgs, WaitArgs, WaitCondition};

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
    drop(server);
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
