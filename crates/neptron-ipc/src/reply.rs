use neptron_types::{Modeline, Rect};
use serde::{Deserialize, Serialize};

use crate::command::{SessionState, SettingLevel, Stage};

/// The answer to a successful command.
///
/// On the wire the reply is two keys of the response object: `reply` holds
/// the name of the command it answers and `data` holds the result. A client
/// ignores keys in `data` that it does not know, so a later version can add
/// fields. A field that a later version adds is an `Option` or has a
/// `#[serde(default)]`, so that a reply of an older server still reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reply", content = "data", rename_all = "snake_case")]
pub enum Reply {
    /// Answers [`Command::Hello`](crate::Command::Hello).
    Hello(HelloReply),
    /// Answers [`Command::Screenshot`](crate::Command::Screenshot).
    Screenshot(Screenshot),
    /// Answers [`Command::UiTree`](crate::Command::UiTree).
    UiTree(UiTree),
    /// Answers [`Command::InputSend`](crate::Command::InputSend).
    InputSend(InputQueued),
    /// Answers [`Command::InputScript`](crate::Command::InputScript).
    InputScript(ScriptQueued),
    /// Answers [`Command::ClockStep`](crate::Command::ClockStep).
    ClockStep(FrameReached),
    /// Answers [`Command::Wait`](crate::Command::Wait).
    Wait(FrameReached),
    /// Answers [`Command::SessionLaunch`](crate::Command::SessionLaunch).
    SessionLaunch(SessionStatus),
    /// Answers [`Command::SessionState`](crate::Command::SessionState).
    SessionState(SessionStatus),
    /// Answers [`Command::VideoState`](crate::Command::VideoState).
    VideoState(VideoState),
    /// Answers [`Command::Stats`](crate::Command::Stats).
    Stats(Stats),
    /// Answers [`Command::SettingsGet`](crate::Command::SettingsGet).
    SettingsGet(SettingValue),
    /// Answers [`Command::SettingsSet`](crate::Command::SettingsSet).
    SettingsSet(SettingWritten),
    /// Answers [`Command::LogTail`](crate::Command::LogTail).
    LogTail(LogLines),
}

/// The data of [`Reply::Hello`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HelloReply {
    /// The protocol version of the server.
    pub protocol: u32,
    /// The name and version of the server program.
    pub server: String,
}

/// The data of [`Reply::Screenshot`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Screenshot {
    /// The stage the picture comes from.
    pub stage: Stage,
    /// Width of the picture, in pixels.
    pub width: u32,
    /// Height of the picture, in pixels.
    pub height: u32,
    /// The PNG file. On the wire it is one base64 string.
    #[serde(rename = "png_base64", with = "crate::base64::serde_bytes")]
    pub png: Vec<u8>,
}

/// The data of [`Reply::UiTree`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiTree {
    /// The id of the screen on show.
    pub screen: String,
    /// The id of the focused row, if a row has the focus.
    pub focused: Option<String>,
    /// Every visible row, from top to bottom.
    pub rows: Vec<UiRow>,
}

/// One visible row of the UI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiRow {
    /// The id of the row, unique on its screen.
    pub id: String,
    /// The label text.
    pub label: String,
    /// The value text, for a row that shows a value.
    pub value: Option<String>,
    /// The schema key, for a row that edits a setting.
    pub setting: Option<String>,
    /// The level the value comes from, for a row that edits a setting.
    pub source: Option<SettingLevel>,
    /// The layout cut some text of the row.
    pub clipped: bool,
    /// Where the row is on the 320x240 canvas.
    pub bounds: Rect,
}

/// The data of [`Reply::InputSend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InputQueued {
    /// The frame at which the change takes effect.
    pub frame: u64,
}

/// The data of [`Reply::InputScript`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScriptQueued {
    /// The frame at which the script starts: its frame 0.
    pub start_frame: u64,
    /// The number of events queued.
    pub events: u32,
}

/// The data of [`Reply::ClockStep`] and [`Reply::Wait`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameReached {
    /// The frame counter after the command.
    pub frame: u64,
}

/// The data of [`Reply::SessionLaunch`] and [`Reply::SessionState`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionStatus {
    /// The lifecycle state.
    pub state: SessionState,
    /// The loaded core, if any.
    pub core: Option<String>,
    /// The loaded game file, if any.
    pub rom: Option<String>,
    /// The frame counter.
    pub frame: u64,
}

/// The data of [`Reply::VideoState`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoState {
    /// The mode on the display, as the driver read it back, if a mode is set.
    pub mode: Option<Modeline>,
    /// The line rate of `mode`, in millihertz. 0 with no mode.
    pub line_rate_mhz: u64,
    /// The field rate of `mode`, in millihertz. 0 with no mode.
    pub field_rate_mhz: u64,
    /// The id of the active monitor profile, if any.
    pub profile: Option<String>,
    /// The most recent validator verdicts, the oldest first.
    pub verdicts: Vec<Verdict>,
    /// The number of mode switches since the start.
    pub switch_count: u64,
}

/// One verdict of the safety validator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Verdict {
    /// The mode the validator judged.
    pub mode: Modeline,
    /// The validator accepted the mode.
    pub accepted: bool,
    /// The rejection, as text with its value and limit, for a refused mode.
    pub rejection: Option<String>,
}

/// The data of [`Reply::Stats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stats {
    /// Frames shown on the display.
    pub frames_presented: u64,
    /// Frames the core made that the display never showed.
    pub frames_dropped: u64,
    /// Flips that showed the previous frame again.
    pub frames_repeated: u64,
    /// Times the audio buffer ran empty.
    pub audio_underruns: u64,
    /// Audio frames in the output buffer now.
    pub audio_queued_frames: u64,
    /// The most recent delay from an input change to the first frame that
    /// shows it, in nanoseconds.
    pub input_to_frame_ns: u64,
}

/// The data of [`Reply::SettingsGet`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingValue {
    /// The key of the setting.
    pub key: String,
    /// The value, or `None` when the level read has no value.
    pub value: Option<serde_json::Value>,
    /// The level the value comes from, or `None` with no value.
    pub source: Option<SettingLevel>,
}

/// The data of [`Reply::SettingsSet`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SettingWritten {
    /// The key of the setting.
    pub key: String,
    /// The level written.
    pub level: SettingLevel,
}

/// The data of [`Reply::LogTail`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogLines {
    /// The lines, the oldest first.
    pub lines: Vec<String>,
}

impl Reply {
    /// The wire name of the reply: the name of the command it answers.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hello(_) => "hello",
            Self::Screenshot(_) => "screenshot",
            Self::UiTree(_) => "ui_tree",
            Self::InputSend(_) => "input_send",
            Self::InputScript(_) => "input_script",
            Self::ClockStep(_) => "clock_step",
            Self::Wait(_) => "wait",
            Self::SessionLaunch(_) => "session_launch",
            Self::SessionState(_) => "session_state",
            Self::VideoState(_) => "video_state",
            Self::Stats(_) => "stats",
            Self::SettingsGet(_) => "settings_get",
            Self::SettingsSet(_) => "settings_set",
            Self::LogTail(_) => "log_tail",
        }
    }
}
