//! A handler that serves every command from fixed data, for the server and
//! socket tests.

use neptron_ipc::{
    ClockStepArgs, ErrorBody, ErrorCode, FrameReached, Handler, InputQueued, InputScriptArgs,
    InputSendArgs, LogLines, LogTailArgs, Screenshot, ScreenshotArgs, ScriptQueued,
    SessionLaunchArgs, SessionState, SessionStatus, SettingValue, SettingWritten, SettingsGetArgs,
    SettingsSetArgs, Stats, UiTree, VideoState, WaitArgs, WaitCondition,
};

/// Serves every command. The clock is a frame counter that `clock_step`
/// advances.
#[derive(Debug, Default)]
pub struct FixedHandler {
    /// The frame counter.
    pub frame: u64,
}

impl Handler for FixedHandler {
    fn server_name(&self) -> &str {
        "fixture 1.0"
    }

    fn screenshot(&mut self, args: ScreenshotArgs) -> Result<Screenshot, ErrorBody> {
        Ok(Screenshot {
            stage: args.stage,
            width: 2,
            height: 1,
            png: vec![0x89, b'P', b'N', b'G', 0, 255],
        })
    }

    fn ui_tree(&mut self) -> Result<UiTree, ErrorBody> {
        Ok(UiTree {
            screen: "home".into(),
            focused: None,
            rows: Vec::new(),
        })
    }

    fn input_send(&mut self, _args: InputSendArgs) -> Result<InputQueued, ErrorBody> {
        Ok(InputQueued {
            frame: self.frame + 1,
        })
    }

    fn input_script(&mut self, args: InputScriptArgs) -> Result<ScriptQueued, ErrorBody> {
        Ok(ScriptQueued {
            start_frame: self.frame + 1,
            events: u32::try_from(args.events.len()).unwrap_or(u32::MAX),
        })
    }

    fn clock_step(&mut self, args: ClockStepArgs) -> Result<FrameReached, ErrorBody> {
        self.frame += u64::from(args.frames);
        Ok(FrameReached { frame: self.frame })
    }

    fn wait(&mut self, args: WaitArgs) -> Result<FrameReached, ErrorBody> {
        match args.until {
            WaitCondition::Frame(n) if n > self.frame => Err(ErrorBody::new(
                ErrorCode::Timeout,
                format!("frame {n} not reached in {} ms", args.timeout_ms),
            )),
            _ => Ok(FrameReached { frame: self.frame }),
        }
    }

    fn session_launch(&mut self, args: SessionLaunchArgs) -> Result<SessionStatus, ErrorBody> {
        Ok(SessionStatus {
            state: SessionState::Loading,
            core: Some(args.core),
            rom: Some(args.rom),
            frame: self.frame,
        })
    }

    fn session_state(&mut self) -> Result<SessionStatus, ErrorBody> {
        Ok(SessionStatus {
            state: SessionState::Idle,
            core: None,
            rom: None,
            frame: self.frame,
        })
    }

    fn video_state(&mut self) -> Result<VideoState, ErrorBody> {
        Ok(VideoState {
            mode: None,
            line_rate_mhz: 0,
            field_rate_mhz: 0,
            profile: None,
            verdicts: Vec::new(),
            switch_count: 0,
        })
    }

    fn stats(&mut self) -> Result<Stats, ErrorBody> {
        Ok(Stats {
            frames_presented: self.frame,
            frames_dropped: 0,
            frames_repeated: 0,
            audio_underruns: 0,
            audio_queued_frames: 0,
            input_to_frame_ns: 0,
        })
    }

    fn settings_get(&mut self, args: SettingsGetArgs) -> Result<SettingValue, ErrorBody> {
        Ok(SettingValue {
            key: args.key,
            value: None,
            source: None,
        })
    }

    fn settings_set(&mut self, args: SettingsSetArgs) -> Result<SettingWritten, ErrorBody> {
        Ok(SettingWritten {
            key: args.key,
            level: args.level,
        })
    }

    fn log_tail(&mut self, args: LogTailArgs) -> Result<LogLines, ErrorBody> {
        Ok(LogLines {
            lines: (0..args.lines).map(|i| format!("line {i}")).collect(),
        })
    }
}

/// Serves only `hello`, from the defaults of the trait.
#[derive(Debug, Default)]
pub struct EmptyHandler;

impl Handler for EmptyHandler {
    fn server_name(&self) -> &str {
        "empty"
    }
}
