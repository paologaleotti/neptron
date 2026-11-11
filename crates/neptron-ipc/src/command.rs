use neptron_types::Control;
use serde::{Deserialize, Serialize};

/// One command of the control API: what a client asks the frontend to do.
///
/// On the wire the command is two keys of the request object: `cmd` holds
/// the name and `args` holds the arguments. A command without arguments has
/// no `args` key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]
pub enum Command {
    /// Asks for the protocol version and the name of the server.
    Hello,
    /// Takes a PNG picture of one stage of the video path.
    Screenshot(ScreenshotArgs),
    /// Reads the widget tree of the screen on show.
    UiTree,
    /// Sets one control of one port to a value.
    InputSend(InputSendArgs),
    /// Queues a timed sequence of input changes.
    InputScript(InputScriptArgs),
    /// Advances the simulated clock by a number of frames.
    ClockStep(ClockStepArgs),
    /// Blocks until a condition holds, or until a timeout.
    Wait(WaitArgs),
    /// Starts a game with a core.
    SessionLaunch(SessionLaunchArgs),
    /// Reads the lifecycle state of the game session.
    SessionState,
    /// Reads the current video mode and the recent validator verdicts.
    VideoState,
    /// Reads the pacing, audio and input counters.
    Stats,
    /// Reads one setting.
    SettingsGet(SettingsGetArgs),
    /// Writes one setting at one level.
    SettingsSet(SettingsSetArgs),
    /// Reads the most recent log lines.
    LogTail(LogTailArgs),
}

/// Arguments of [`Command::Screenshot`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotArgs {
    /// The stage to read.
    pub stage: Stage,
}

/// A stage of the video path that a screenshot can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The 320x240 logical UI canvas.
    Ui,
    /// The picture after scaling and rotation, before it goes to the display.
    Output,
    /// What the display received, read back from the display device.
    Scanout,
}

/// Arguments of [`Command::InputSend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputSendArgs {
    /// The input port, from 0, as in libretro. Port 0 is player one.
    pub port: u8,
    /// The control.
    pub control: Control,
    /// The new value: 0 or 1 for a button, -32768 to 32767 for an axis.
    pub value: i16,
}

/// Arguments of [`Command::InputScript`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputScriptArgs {
    /// The changes, in any order.
    pub events: Vec<ScriptedInput>,
}

/// One input change of a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptedInput {
    /// The frame of the change, counted from the frame at which the script
    /// starts. 0 is that frame.
    pub frame: u64,
    /// The input port, from 0.
    pub port: u8,
    /// The control.
    pub control: Control,
    /// The new value.
    pub value: i16,
}

/// Arguments of [`Command::ClockStep`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockStepArgs {
    /// The number of frames to advance. 0 advances nothing and reports the
    /// current frame.
    pub frames: u32,
}

/// Arguments of [`Command::Wait`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitArgs {
    /// The condition to wait for.
    pub until: WaitCondition,
    /// The longest wait, in milliseconds of server time. The server answers
    /// with the `timeout` error when it passes.
    pub timeout_ms: u32,
}

/// A condition that [`Command::Wait`] waits for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    /// The screen with this id is on show.
    Screen(String),
    /// The session is in this state.
    SessionState(SessionState),
    /// The frame counter is at this value or above it.
    Frame(u64),
}

/// The lifecycle state of the game session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// No game is loaded. The menus are on show.
    Idle,
    /// A core and a game are loading.
    Loading,
    /// The game runs.
    Running,
    /// The game is paused.
    Paused,
    /// The game is paused and the quick menu is on show.
    QuickMenu,
    /// The session is closing: save RAM is flushed and the core unloads.
    Exiting,
}

/// Arguments of [`Command::SessionLaunch`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionLaunchArgs {
    /// The core, by name or by path, as the server resolves it.
    pub core: String,
    /// The path of the game file on the server.
    pub rom: String,
}

/// A level of the settings model, from the most specific to the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingLevel {
    /// The value for one game.
    Game,
    /// The value for one system.
    System,
    /// The value for every system.
    Global,
    /// The value of the schema.
    Default,
}

/// Arguments of [`Command::SettingsGet`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsGetArgs {
    /// The key of the setting, as in the schema.
    pub key: String,
    /// The level to read. With no level, the server gives the resolved
    /// value and the level it comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<SettingLevel>,
}

/// Arguments of [`Command::SettingsSet`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsSetArgs {
    /// The key of the setting, as in the schema.
    pub key: String,
    /// The level to write.
    pub level: SettingLevel,
    /// The new value, as JSON. The server checks it against the schema.
    pub value: serde_json::Value,
}

/// Arguments of [`Command::LogTail`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogTailArgs {
    /// The largest number of lines to return.
    pub lines: u32,
}

impl Command {
    /// The wire name of every command, in the order of the enum.
    pub const NAMES: &'static [&'static str] = &[
        "hello",
        "screenshot",
        "ui_tree",
        "input_send",
        "input_script",
        "clock_step",
        "wait",
        "session_launch",
        "session_state",
        "video_state",
        "stats",
        "settings_get",
        "settings_set",
        "log_tail",
    ];

    /// The wire name of the command, the value of the `cmd` key.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hello => "hello",
            Self::Screenshot(_) => "screenshot",
            Self::UiTree => "ui_tree",
            Self::InputSend(_) => "input_send",
            Self::InputScript(_) => "input_script",
            Self::ClockStep(_) => "clock_step",
            Self::Wait(_) => "wait",
            Self::SessionLaunch(_) => "session_launch",
            Self::SessionState => "session_state",
            Self::VideoState => "video_state",
            Self::Stats => "stats",
            Self::SettingsGet(_) => "settings_get",
            Self::SettingsSet(_) => "settings_set",
            Self::LogTail(_) => "log_tail",
        }
    }
}
