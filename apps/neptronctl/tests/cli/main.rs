//! Runs the neptronctl binary against a server on a Unix socket in a
//! temporary directory, and checks its output and its exit code.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::{Command as Process, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use neptron_ipc::{
    ClockStepArgs, ControlListener, ErrorBody, FrameReached, Handler, InputScriptArgs, Screenshot,
    ScreenshotArgs, ScriptQueued, serve_stream,
};
use serde_json::Value;

struct Fixture;

impl Handler for Fixture {
    fn server_name(&self) -> &str {
        "cli fixture"
    }

    fn clock_step(&mut self, args: ClockStepArgs) -> Result<FrameReached, ErrorBody> {
        Ok(FrameReached {
            frame: u64::from(args.frames) + 100,
        })
    }

    fn screenshot(&mut self, args: ScreenshotArgs) -> Result<Screenshot, ErrorBody> {
        Ok(Screenshot {
            stage: args.stage,
            width: 1,
            height: 1,
            png: vec![0x89, b'P', b'N', b'G', 0, 1, 2],
        })
    }

    fn input_script(&mut self, args: InputScriptArgs) -> Result<ScriptQueued, ErrorBody> {
        Ok(ScriptQueued {
            start_frame: 5,
            events: u32::try_from(args.events.len()).unwrap(),
        })
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("nctl-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Cleanup is best effort: a leftover directory does not change a
        // test result.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs neptronctl with `args` against a server that serves one connection.
fn run_with_server(dir: &TempDir, args: &[&str]) -> Output {
    let socket = dir.path("c.sock");
    let listener = ControlListener::bind(&socket).unwrap();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        serve_stream(&stream, &mut Fixture).unwrap();
    });
    let output = neptronctl(&socket, args);
    server.join().unwrap();
    output
}

fn neptronctl(socket: &std::path::Path, args: &[&str]) -> Output {
    Process::new(env!("CARGO_BIN_EXE_neptronctl"))
        .env("NEPTRON_CONTROL_SOCKET", socket)
        .args(args)
        .output()
        .unwrap()
}

fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn a_reply_prints_its_data_as_one_json_line() {
    let dir = TempDir::new();
    let out = run_with_server(&dir, &["clock", "step", "3"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "{\"frame\":103}\n");
    assert!(out.stderr.is_empty());
}

#[test]
fn hello_names_the_server() {
    let dir = TempDir::new();
    let out = run_with_server(&dir, &["hello"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        json(&out.stdout),
        serde_json::json!({"protocol": 2, "server": "cli fixture"})
    );
}

#[test]
fn the_socket_flag_wins_over_the_environment() {
    let dir = TempDir::new();
    let socket = dir.path("c.sock");
    let listener = ControlListener::bind(&socket).unwrap();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap();
        serve_stream(&stream, &mut Fixture).unwrap();
    });
    let out = neptronctl(
        &dir.path("missing.sock"),
        &["--socket", socket.to_str().unwrap(), "hello"],
    );
    server.join().unwrap();
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn a_screenshot_is_written_to_the_local_file() {
    let dir = TempDir::new();
    let png = dir.path("shot.png");
    let out = run_with_server(&dir, &["screenshot", "ui", "--out", png.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert_eq!(
        std::fs::read(&png).unwrap(),
        [0x89, b'P', b'N', b'G', 0, 1, 2]
    );
    let summary = json(&out.stdout);
    assert_eq!(summary["stage"], "ui");
    assert_eq!(summary["bytes"], 7);
}

#[test]
fn an_input_script_is_read_from_a_local_file() {
    let dir = TempDir::new();
    let script = dir.path("script.json");
    std::fs::write(
        &script,
        r#"[{"frame":0,"port":0,"control":"a","value":1},{"frame":1,"port":0,"control":"a","value":0}]"#,
    )
    .unwrap();
    let out = run_with_server(&dir, &["input", "script", script.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert_eq!(
        json(&out.stdout),
        serde_json::json!({"start_frame": 5, "events": 2})
    );
}

#[test]
fn a_server_error_exits_with_one_and_prints_the_code() {
    let dir = TempDir::new();
    let out = run_with_server(&dir, &["stats"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(json(&out.stderr)["error"]["code"], "unavailable");
}

#[test]
fn a_usage_error_exits_with_two() {
    let dir = TempDir::new();
    let out = neptronctl(&dir.path("c.sock"), &["clock", "step"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(json(&out.stderr)["error"]["code"], "usage");
}

#[test]
fn a_missing_socket_exits_with_three() {
    let dir = TempDir::new();
    let out = neptronctl(&dir.path("c.sock"), &["hello"]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(json(&out.stderr)["error"]["code"], "connect");
}

#[test]
fn a_server_that_never_answers_exits_with_four() {
    let dir = TempDir::new();
    let socket = dir.path("c.sock");
    let listener = ControlListener::bind(&socket).unwrap();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        // Hold the connection open without an answer until the client
        // gives up and closes it.
        let mut sink = Vec::new();
        std::io::Read::read_to_end(&mut &stream, &mut sink).unwrap();
    });
    let out = neptronctl(&socket, &["--timeout-ms", "50", "hello"]);
    server.join().unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(json(&out.stderr)["error"]["code"], "no_response");
}

/// Accepts one connection, reads one line, answers with `answer` and
/// closes the connection.
fn answer_once(dir: &TempDir, answer: &'static [u8]) -> (PathBuf, thread::JoinHandle<()>) {
    let socket = dir.path("c.sock");
    let listener = ControlListener::bind(&socket).unwrap();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(&stream), &mut line).unwrap();
        std::io::Write::write_all(&mut &stream, answer).unwrap();
    });
    (socket, server)
}

#[test]
fn a_broken_reply_exits_with_four() {
    let dir = TempDir::new();
    let (socket, server) = answer_once(&dir, b"this is not json\n");
    let out = neptronctl(&socket, &["hello"]);
    server.join().unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(json(&out.stderr)["error"]["code"], "protocol");
}

#[test]
fn a_connection_closed_before_the_reply_exits_with_four() {
    let dir = TempDir::new();
    let (socket, server) = answer_once(&dir, b"");
    let out = neptronctl(&socket, &["hello"]);
    server.join().unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(json(&out.stderr)["error"]["code"], "transport");
}

#[test]
fn a_zero_timeout_exits_with_two() {
    let dir = TempDir::new();
    let out = neptronctl(&dir.path("c.sock"), &["--timeout-ms", "0", "hello"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(json(&out.stderr)["error"]["code"], "usage");
}

#[test]
fn a_screenshot_that_cannot_be_written_exits_with_five() {
    let dir = TempDir::new();
    let png = dir.path("missing").join("shot.png");
    let out = run_with_server(&dir, &["screenshot", "ui", "--out", png.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(5));
    assert_eq!(json(&out.stderr)["error"]["code"], "file");
}

#[test]
fn a_missing_local_file_exits_with_five() {
    let dir = TempDir::new();
    let out = neptronctl(
        &dir.path("c.sock"),
        &["input", "script", dir.path("none.json").to_str().unwrap()],
    );
    assert_eq!(out.status.code(), Some(5));
    assert_eq!(json(&out.stderr)["error"]["code"], "file");
}

fn monitors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors")
}

#[test]
fn a_selftest_dry_run_prints_the_plan_and_sets_no_mode() {
    let dir = TempDir::new();
    let out = neptronctl(
        &dir.path("c.sock"),
        &[
            "selftest",
            "video",
            "--profile",
            "pc_31_120",
            "--monitors",
            monitors_dir().to_str().unwrap(),
            "--sysfs",
            dir.path("no-sysfs").to_str().unwrap(),
            "--report",
            dir.path("r.json").to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let report = json(&std::fs::read(dir.path("r.json")).unwrap());
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["profile"]["id"], "pc_31_120");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("dry run, no mode is set"), "{text}");
    assert!(text.contains("Profile pc_31_120 (preset)"), "{text}");
    assert!(text.contains("480p"), "{text}");
}

#[test]
fn a_selftest_with_a_missing_preset_exits_with_six() {
    let dir = TempDir::new();
    let out = neptronctl(
        &dir.path("c.sock"),
        &[
            "selftest",
            "video",
            "--profile",
            "no_such_monitor",
            "--monitors",
            monitors_dir().to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(6));
    assert_eq!(json(&out.stderr)["error"]["code"], "selftest_setup");
}

#[test]
fn a_selftest_report_that_cannot_be_written_exits_with_five() {
    let dir = TempDir::new();
    let out = neptronctl(
        &dir.path("c.sock"),
        &[
            "selftest",
            "video",
            "--profile",
            "ntsc",
            "--monitors",
            monitors_dir().to_str().unwrap(),
            "--report",
            dir.path("missing").join("r.json").to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(5));
    assert_eq!(json(&out.stderr)["error"]["code"], "file");
}

fn selftest(dir: &TempDir, extra: &[&str]) -> Output {
    let monitors = monitors_dir();
    let sysfs = dir.path("no-sysfs");
    let report = dir.path("r.json");
    let mut args = vec![
        "selftest",
        "video",
        "--profile",
        "pc_31_120",
        "--monitors",
        monitors.to_str().unwrap(),
        "--sysfs",
        sysfs.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--device",
        "/nonexistent/card0",
    ];
    args.extend_from_slice(extra);
    neptronctl(&dir.path("c.sock"), &args)
}

#[test]
fn a_selftest_dry_run_prints_the_confirmation_to_add() {
    let dir = TempDir::new();
    let out = selftest(&dir, &[]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("--i-have-checked-the-monitor pc_31_120"),
        "{text}"
    );
}

#[test]
fn a_selftest_confirmation_must_name_the_profile() {
    let dir = TempDir::new();
    let out = selftest(&dir, &["--i-have-checked-the-monitor", "ntsc"]);
    assert_eq!(out.status.code(), Some(6), "{out:?}");
    let error = json(&out.stderr);
    assert_eq!(error["error"]["code"], "selftest_setup");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("\"pc_31_120\"")
    );
    assert!(!dir.path("r.json").exists(), "a refused run writes nothing");
}

#[test]
fn a_selftest_refuses_while_the_frontend_answers() {
    let dir = TempDir::new();
    let socket = dir.path("c.sock");
    let _listener = ControlListener::bind(&socket).unwrap();
    let out = selftest(&dir, &["--i-have-checked-the-monitor", "pc_31_120"]);
    assert_eq!(out.status.code(), Some(6), "{out:?}");
    let message = json(&out.stderr)["error"]["message"].to_string();
    assert!(message.contains("control socket"), "{message}");
}

#[cfg(not(all(feature = "kms", target_os = "linux")))]
#[test]
fn a_selftest_without_kms_says_so() {
    let dir = TempDir::new();
    let out = selftest(&dir, &["--i-have-checked-the-monitor", "pc_31_120"]);
    assert_eq!(out.status.code(), Some(6), "{out:?}");
    let message = json(&out.stderr)["error"]["message"].to_string();
    assert!(message.contains("no KMS backend"), "{message}");
}

#[cfg(all(feature = "kms", target_os = "linux"))]
#[test]
fn a_selftest_on_a_missing_device_exits_with_six() {
    let dir = TempDir::new();
    let out = selftest(&dir, &["--i-have-checked-the-monitor", "pc_31_120"]);
    assert_eq!(out.status.code(), Some(6), "{out:?}");
    assert_eq!(json(&out.stderr)["error"]["code"], "selftest_device");
}

#[test]
fn a_selftest_refuses_a_device_of_another_card() {
    let dir = TempDir::new();
    let tree = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/neptron-platform/tests/fixtures/sysfs/i3_6100t_vga");
    let out = neptronctl(
        &dir.path("c.sock"),
        &[
            "selftest",
            "video",
            "--profile",
            "pc_31_120",
            "--monitors",
            monitors_dir().to_str().unwrap(),
            "--sysfs",
            tree.to_str().unwrap(),
            "--report",
            dir.path("r.json").to_str().unwrap(),
            "--device",
            "/nonexistent/card1",
            "--i-have-checked-the-monitor",
            "pc_31_120",
        ],
    );
    assert_eq!(out.status.code(), Some(6), "{out:?}");
    let message = json(&out.stderr)["error"]["message"].to_string();
    assert!(
        message.contains("card0") && message.contains("card1"),
        "{message}"
    );
}
