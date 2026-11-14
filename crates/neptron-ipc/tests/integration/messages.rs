//! Every command and every reply against its literal JSON, so that a rename
//! fails a test.

use std::fmt::Debug;

use neptron_ipc::{
    ClockStepArgs, Command, ErrorBody, ErrorCode, FrameReached, HelloReply, InputQueued,
    InputScriptArgs, InputSendArgs, LogLines, LogTailArgs, Reply, Screenshot, ScreenshotArgs,
    ScriptQueued, ScriptedInput, SessionLaunchArgs, SessionState, SessionStatus, SettingLevel,
    SettingValue, SettingWritten, SettingsGetArgs, SettingsSetArgs, Stage, Stats, UiRow, UiTree,
    Verdict, VideoState, WaitArgs, WaitCondition,
};
use neptron_types::{Control, Modeline, Rect};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The value writes as `json`, and `json` reads back as the value.
pub fn pin<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T, json: &str) {
    assert_eq!(serde_json::to_string(value).unwrap(), json);
    assert_eq!(&serde_json::from_str::<T>(json).unwrap(), value);
}

/// One sample of every command, with its literal JSON.
pub fn commands() -> Vec<(Command, &'static str)> {
    vec![
        (Command::Hello, r#"{"cmd":"hello"}"#),
        (
            Command::Screenshot(ScreenshotArgs { stage: Stage::Ui }),
            r#"{"cmd":"screenshot","args":{"stage":"ui"}}"#,
        ),
        (Command::UiTree, r#"{"cmd":"ui_tree"}"#),
        (
            Command::InputSend(InputSendArgs {
                port: 1,
                control: Control::Start,
                value: 1,
            }),
            r#"{"cmd":"input_send","args":{"port":1,"control":"start","value":1}}"#,
        ),
        (
            Command::InputScript(InputScriptArgs {
                events: vec![
                    ScriptedInput {
                        frame: 0,
                        port: 0,
                        control: Control::Down,
                        value: 1,
                    },
                    ScriptedInput {
                        frame: 2,
                        port: 0,
                        control: Control::LeftX,
                        value: -32768,
                    },
                ],
            }),
            concat!(
                r#"{"cmd":"input_script","args":{"events":["#,
                r#"{"frame":0,"port":0,"control":"down","value":1},"#,
                r#"{"frame":2,"port":0,"control":"left_x","value":-32768}]}}"#
            ),
        ),
        (
            Command::ClockStep(ClockStepArgs { frames: 60 }),
            r#"{"cmd":"clock_step","args":{"frames":60}}"#,
        ),
        (
            Command::Wait(WaitArgs {
                until: WaitCondition::Screen("game_list".into()),
                timeout_ms: 5000,
            }),
            r#"{"cmd":"wait","args":{"until":{"screen":"game_list"},"timeout_ms":5000}}"#,
        ),
        (
            Command::SessionLaunch(SessionLaunchArgs {
                core: "fceumm".into(),
                rom: "/roms/nes/240p.nes".into(),
            }),
            r#"{"cmd":"session_launch","args":{"core":"fceumm","rom":"/roms/nes/240p.nes"}}"#,
        ),
        (Command::SessionState, r#"{"cmd":"session_state"}"#),
        (Command::VideoState, r#"{"cmd":"video_state"}"#),
        (Command::Stats, r#"{"cmd":"stats"}"#),
        (
            Command::SettingsGet(SettingsGetArgs {
                key: "video.style".into(),
                level: None,
            }),
            r#"{"cmd":"settings_get","args":{"key":"video.style"}}"#,
        ),
        (
            Command::SettingsSet(SettingsSetArgs {
                key: "video.style".into(),
                level: SettingLevel::System,
                value: serde_json::json!("native"),
            }),
            r#"{"cmd":"settings_set","args":{"key":"video.style","level":"system","value":"native"}}"#,
        ),
        (
            Command::LogTail(LogTailArgs { lines: 50 }),
            r#"{"cmd":"log_tail","args":{"lines":50}}"#,
        ),
    ]
}

fn modeline() -> Modeline {
    Modeline {
        pclock_hz: 6_700_000,
        hactive: 320,
        hsync_start: 336,
        hsync_end: 368,
        htotal: 426,
        vactive: 240,
        vsync_start: 244,
        vsync_end: 247,
        vtotal: 262,
        interlace: false,
        doublescan: false,
        hsync_positive: false,
        vsync_positive: false,
    }
}

const MODELINE_JSON: &str = concat!(
    r#"{"pclock_hz":6700000,"hactive":320,"hsync_start":336,"hsync_end":368,"htotal":426,"#,
    r#""vactive":240,"vsync_start":244,"vsync_end":247,"vtotal":262,"interlace":false,"#,
    r#""doublescan":false,"hsync_positive":false,"vsync_positive":false}"#
);

/// One sample of every reply, with its literal JSON.
pub fn replies() -> Vec<(Reply, String)> {
    let status = SessionStatus {
        state: SessionState::Running,
        core: Some("fceumm".into()),
        rom: Some("/roms/nes/240p.nes".into()),
        frame: 600,
    };
    let status_json =
        r#"{"state":"running","core":"fceumm","rom":"/roms/nes/240p.nes","frame":600}"#;
    vec![
        (
            Reply::Hello(HelloReply {
                protocol: 1,
                server: "neptron 0.1.0".into(),
            }),
            r#"{"reply":"hello","data":{"protocol":1,"server":"neptron 0.1.0"}}"#.into(),
        ),
        (
            Reply::Screenshot(Screenshot {
                stage: Stage::Scanout,
                width: 320,
                height: 240,
                png: b"\x89PNG".to_vec(),
            }),
            concat!(
                r#"{"reply":"screenshot","data":{"stage":"scanout","width":320,"height":240,"#,
                r#""png_base64":"iVBORw=="}}"#
            )
            .into(),
        ),
        (
            Reply::UiTree(UiTree {
                screen: "settings/display".into(),
                focused: Some("style".into()),
                rows: vec![UiRow {
                    id: "style".into(),
                    label: "Picture style".into(),
                    value: Some("Native".into()),
                    setting: Some("video.style".into()),
                    source: Some(SettingLevel::Global),
                    clipped: false,
                    bounds: Rect {
                        x: 8,
                        y: 32,
                        w: 304,
                        h: 12,
                    },
                }],
            }),
            concat!(
                r#"{"reply":"ui_tree","data":{"screen":"settings/display","focused":"style","rows":[{"#,
                r#""id":"style","label":"Picture style","value":"Native","setting":"video.style","#,
                r#""source":"global","clipped":false,"bounds":{"x":8,"y":32,"w":304,"h":12}}]}}"#
            )
            .into(),
        ),
        (
            Reply::InputSend(InputQueued { frame: 61 }),
            r#"{"reply":"input_send","data":{"frame":61}}"#.into(),
        ),
        (
            Reply::InputScript(ScriptQueued {
                start_frame: 61,
                events: 2,
            }),
            r#"{"reply":"input_script","data":{"start_frame":61,"events":2}}"#.into(),
        ),
        (
            Reply::ClockStep(FrameReached { frame: 120 }),
            r#"{"reply":"clock_step","data":{"frame":120}}"#.into(),
        ),
        (
            Reply::Wait(FrameReached { frame: 130 }),
            r#"{"reply":"wait","data":{"frame":130}}"#.into(),
        ),
        (
            Reply::SessionLaunch(status.clone()),
            format!(r#"{{"reply":"session_launch","data":{status_json}}}"#),
        ),
        (
            Reply::SessionState(status),
            format!(r#"{{"reply":"session_state","data":{status_json}}}"#),
        ),
        (
            Reply::VideoState(VideoState {
                mode: Some(modeline()),
                line_rate_mhz: 15_727_700,
                field_rate_mhz: 60_029,
                profile: Some("generic_15".into()),
                verdicts: vec![Verdict {
                    mode: modeline(),
                    accepted: false,
                    rejection: Some("line rate 15727700 mHz above 15700000 mHz".into()),
                }],
                switch_count: 3,
            }),
            format!(
                concat!(
                    r#"{{"reply":"video_state","data":{{"mode":{m},"line_rate_mhz":15727700,"#,
                    r#""field_rate_mhz":60029,"profile":"generic_15","verdicts":[{{"mode":{m},"#,
                    r#""accepted":false,"rejection":"line rate 15727700 mHz above 15700000 mHz"}}],"#,
                    r#""switch_count":3}}}}"#
                ),
                m = MODELINE_JSON
            ),
        ),
        (
            Reply::Stats(Stats {
                frames_presented: 1000,
                frames_dropped: 1,
                frames_repeated: 2,
                audio_underruns: 0,
                audio_queued_frames: 1600,
                input_to_frame_ns: 16_683_333,
                fields_missed: Some(4),
            }),
            concat!(
                r#"{"reply":"stats","data":{"frames_presented":1000,"frames_dropped":1,"#,
                r#""frames_repeated":2,"audio_underruns":0,"audio_queued_frames":1600,"#,
                r#""input_to_frame_ns":16683333,"fields_missed":4}}"#
            )
            .into(),
        ),
        (
            Reply::SettingsGet(SettingValue {
                key: "video.style".into(),
                value: Some(serde_json::json!("native")),
                source: Some(SettingLevel::Default),
            }),
            r#"{"reply":"settings_get","data":{"key":"video.style","value":"native","source":"default"}}"#
                .into(),
        ),
        (
            Reply::SettingsSet(SettingWritten {
                key: "video.style".into(),
                level: SettingLevel::Game,
            }),
            r#"{"reply":"settings_set","data":{"key":"video.style","level":"game"}}"#.into(),
        ),
        (
            Reply::LogTail(LogLines {
                lines: vec!["boot".into(), "ready".into()],
            }),
            r#"{"reply":"log_tail","data":{"lines":["boot","ready"]}}"#.into(),
        ),
    ]
}

#[test]
fn every_command_has_its_literal_json() {
    for (command, json) in commands() {
        pin(&command, json);
    }
}

#[test]
fn every_reply_has_its_literal_json() {
    for (reply, json) in replies() {
        pin(&reply, &json);
    }
}

#[test]
fn the_samples_cover_every_command_and_reply_once() {
    let names: Vec<&str> = commands().iter().map(|(c, _)| c.name()).collect();
    assert_eq!(names, Command::NAMES);
    let names: Vec<&str> = replies().iter().map(|(r, _)| r.name()).collect();
    assert_eq!(names, Command::NAMES);
}

#[test]
fn the_name_is_the_tag_on_the_wire() {
    for (command, json) in commands() {
        assert!(json.starts_with(&format!(r#"{{"cmd":"{}""#, command.name())));
    }
    for (reply, json) in replies() {
        assert!(json.starts_with(&format!(r#"{{"reply":"{}""#, reply.name())));
    }
}

#[test]
fn every_wait_condition_has_its_literal_json() {
    pin(
        &WaitCondition::Screen("home".into()),
        r#"{"screen":"home"}"#,
    );
    pin(
        &WaitCondition::SessionState(SessionState::QuickMenu),
        r#"{"session_state":"quick_menu"}"#,
    );
    pin(&WaitCondition::Frame(1200), r#"{"frame":1200}"#);
}

#[test]
fn every_enum_value_has_its_wire_name() {
    let states = [
        (SessionState::Idle, "idle"),
        (SessionState::Loading, "loading"),
        (SessionState::Running, "running"),
        (SessionState::Paused, "paused"),
        (SessionState::QuickMenu, "quick_menu"),
        (SessionState::Exiting, "exiting"),
    ];
    for (state, name) in states {
        pin(&state, &format!("\"{name}\""));
    }
    let levels = [
        (SettingLevel::Game, "game"),
        (SettingLevel::System, "system"),
        (SettingLevel::Global, "global"),
        (SettingLevel::Default, "default"),
    ];
    for (level, name) in levels {
        pin(&level, &format!("\"{name}\""));
    }
    let stages = [
        (Stage::Ui, "ui"),
        (Stage::Output, "output"),
        (Stage::Scanout, "scanout"),
    ];
    for (stage, name) in stages {
        pin(&stage, &format!("\"{name}\""));
    }
}

#[test]
fn every_error_code_has_its_wire_name() {
    let codes = [
        (ErrorCode::Malformed, "malformed"),
        (ErrorCode::LineTooLong, "line_too_long"),
        (ErrorCode::InvalidRequest, "invalid_request"),
        (ErrorCode::UnsupportedVersion, "unsupported_version"),
        (ErrorCode::UnknownCommand, "unknown_command"),
        (ErrorCode::InvalidArguments, "invalid_arguments"),
        (ErrorCode::Unavailable, "unavailable"),
        (ErrorCode::Timeout, "timeout"),
        (ErrorCode::Failed, "failed"),
    ];
    for (code, name) in codes {
        pin(&code, &format!("\"{name}\""));
        assert_eq!(code.name(), name);
    }
    pin(
        &ErrorBody::new(ErrorCode::Timeout, "no frame 99"),
        r#"{"code":"timeout","message":"no frame 99"}"#,
    );
}

#[test]
fn an_error_code_of_a_later_version_reads_as_unknown() {
    let body: ErrorBody =
        serde_json::from_str(r#"{"code":"rate_limited","message":"slow down"}"#).unwrap();
    assert_eq!(body, ErrorBody::new(ErrorCode::Unknown, "slow down"));
    pin(&ErrorCode::Unknown, r#""unknown""#);
    assert_eq!(ErrorCode::Unknown.name(), "unknown");
}

#[test]
fn an_error_can_carry_the_version_range() {
    pin(
        &ErrorBody {
            min_version: Some(1),
            max_version: Some(3),
            ..ErrorBody::new(ErrorCode::UnsupportedVersion, "no")
        },
        r#"{"code":"unsupported_version","message":"no","min_version":1,"max_version":3}"#,
    );
}

#[test]
fn reply_data_ignores_keys_a_later_version_adds() {
    let json = r#"{"reply":"clock_step","data":{"frame":7,"added_later":true}}"#;
    assert_eq!(
        serde_json::from_str::<Reply>(json).unwrap(),
        Reply::ClockStep(FrameReached { frame: 7 })
    );
}

#[test]
fn command_args_refuse_unknown_keys() {
    let json = r#"{"cmd":"clock_step","args":{"frames":1,"speed":2}}"#;
    let err = serde_json::from_str::<Command>(json).unwrap_err();
    assert!(err.to_string().contains("speed"), "{err}");
}

#[test]
fn a_missing_optional_reply_field_reads_as_none() {
    let json = r#"{"reply":"settings_get","data":{"key":"k"}}"#;
    assert_eq!(
        serde_json::from_str::<Reply>(json).unwrap(),
        Reply::SettingsGet(SettingValue {
            key: "k".into(),
            value: None,
            source: None,
        })
    );
}
