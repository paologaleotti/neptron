//! The control protocol: the handlers on the frame thread, and the socket
//! on its own thread.
//!
//! The I/O thread reads request lines from the socket and sends each
//! command to the frame thread over a bounded channel. The frame thread
//! answers between two frames and never waits on the socket. A `wait`
//! stays on the frame thread until its condition holds or its time ends.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;
use std::time::Duration;

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_ipc::{
    ClockStepArgs, Command, ControlListener, ErrorBody, ErrorCode, FrameReached, Handler,
    InputQueued, InputScriptArgs, InputSendArgs, ListenError, LogLines, LogTailArgs, Reply,
    ScreenshotArgs, ScriptQueued, SessionLaunchArgs, SessionState, SessionStatus, SettingLevel,
    SettingValue, SettingWritten, SettingsGetArgs, SettingsSetArgs, Stats, UiRow, UiTree,
    VideoState, WaitArgs, WaitCondition, dispatch, serve_lines,
};
use neptron_settings::Level;
use neptron_types::Rect;

use crate::shell::Shell;

/// The commands the channel to the frame thread holds. A client that finds
/// it full gets `unavailable`.
const REQUESTS: usize = 8;
/// How long the I/O thread waits for the frame thread's answer.
const ANSWER_WAIT: Duration = Duration::from_secs(30);
/// The read and write timeout of a connection. A client that sends nothing
/// for this long is closed, so the thread can stop.
const STREAM_TIMEOUT: Duration = Duration::from_secs(2);
/// How often the I/O thread looks for a new connection and for the stop.
const ACCEPT_POLL: Duration = Duration::from_millis(50);
/// The wait commands that can wait at once.
const WAITS_MAX: usize = 8;
/// The longest wait a client can ask for, in milliseconds. A longer
/// `timeout_ms` is cut to it, so a socket thread never waits days for an
/// answer.
pub const WAIT_TIMEOUT_MAX_MS: u32 = 60_000;

/// One command for the frame thread and where its answer goes.
struct Request {
    command: Command,
    answer: SyncSender<Result<Reply, ErrorBody>>,
}

/// The frame thread's side of the control socket.
pub struct ControlPort {
    requests: Receiver<Request>,
    waits: Vec<Waiting>,
}

struct Waiting {
    until: WaitCondition,
    deadline_ns: u64,
    answer: SyncSender<Result<Reply, ErrorBody>>,
}

/// The socket thread. Drop stops and joins it.
pub struct ControlServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Why the control socket did not start.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    /// The socket cannot be bound.
    #[error("cannot open the control socket: {0}")]
    Listen(#[from] ListenError),
    /// The socket cannot be set up.
    #[error("cannot set up the control socket: {0}")]
    Io(#[from] std::io::Error),
}

impl ControlServer {
    /// Binds the socket at `path` and starts its thread. The socket file
    /// gets the permission bits of the owner only.
    ///
    /// # Errors
    ///
    /// [`ControlError`].
    pub fn start(path: &Path) -> Result<(Self, ControlPort), ControlError> {
        use std::os::unix::fs::PermissionsExt;
        let listener = ControlListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        listener.listener().set_nonblocking(true)?;
        let (tx, requests) = mpsc::sync_channel(REQUESTS);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread = std::thread::Builder::new()
            .name("neptron-control".to_owned())
            .spawn(move || accept_loop(&listener, &tx, &thread_stop))?;
        Ok((
            Self {
                stop,
                thread: Some(thread),
            },
            ControlPort {
                requests,
                waits: Vec::with_capacity(WAITS_MAX),
            },
        ))
    }
}

impl ControlServer {
    /// Ends the control protocol at the end of a run: every open wait and
    /// queued command gets `unavailable`, the frame thread's side closes,
    /// and the socket thread stops and joins. A waiting client is answered
    /// at once, and a busy client is closed at its next line.
    pub fn stop(self, mut port: ControlPort) {
        port.close();
        drop(port);
        drop(self);
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // Drop has no caller to report to; a panicked socket thread
            // leaves nothing to clean up.
            let _ = thread.join();
        }
    }
}

impl ControlPort {
    /// Answers the commands that came since the last frame, and the waits
    /// whose condition holds or whose time ended. Call it once per frame.
    pub fn serve<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone>(
        &mut self,
        shell: &mut Shell<D, S, I, C>,
        now_ns: u64,
    ) {
        loop {
            let request = match self.requests.try_recv() {
                Ok(request) => request,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            match request.command {
                Command::Wait(args) => self.add_wait(args, request.answer, now_ns),
                Command::ClockStep(_) => {
                    let error = unavailable(
                        "clock_step steps simulated time; this frontend runs in real time",
                    );
                    // A client that left needs no answer.
                    let _ = request.answer.try_send(Err(error));
                }
                command => {
                    let _ = request.answer.try_send(dispatch(shell, command));
                }
            }
        }
        let frame = shell.frame();
        self.waits.retain(|wait| {
            let reached = shell.holds(&wait.until);
            if reached {
                let _ = wait
                    .answer
                    .try_send(Ok(Reply::Wait(FrameReached { frame })));
            } else if now_ns >= wait.deadline_ns {
                let timeout =
                    ErrorBody::new(ErrorCode::Timeout, "the condition did not hold in time");
                let _ = wait.answer.try_send(Err(timeout));
            }
            !reached && now_ns < wait.deadline_ns
        });
    }

    /// The `wait` commands that wait on the frame thread now.
    pub fn open_waits(&self) -> usize {
        self.waits.len()
    }

    /// Answers every open wait and every queued command with
    /// `unavailable`: the run ends.
    pub fn close(&mut self) {
        for wait in self.waits.drain(..) {
            // A client that left needs no answer.
            let _ = wait.answer.try_send(Err(unavailable("the run has ended")));
        }
        while let Ok(request) = self.requests.try_recv() {
            let _ = request
                .answer
                .try_send(Err(unavailable("the run has ended")));
        }
    }

    fn add_wait(
        &mut self,
        args: WaitArgs,
        answer: SyncSender<Result<Reply, ErrorBody>>,
        now_ns: u64,
    ) {
        if self.waits.len() == WAITS_MAX {
            let _ = answer.try_send(Err(unavailable("too many waits")));
            return;
        }
        self.waits.push(Waiting {
            until: args.until,
            deadline_ns: now_ns
                .saturating_add(u64::from(args.timeout_ms.min(WAIT_TIMEOUT_MAX_MS)) * 1_000_000),
            answer,
        });
    }
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Shell<D, S, I, C> {
    /// True when the condition of a `wait` holds now.
    pub fn holds(&self, until: &WaitCondition) -> bool {
        match until {
            WaitCondition::Screen(id) => self.ui().tree.screen() == id,
            WaitCondition::SessionState(state) => self.session_status().state == *state,
            WaitCondition::Frame(n) => self.frame() >= *n,
        }
    }

    /// The state of the session for the protocol.
    pub fn session_status(&self) -> SessionStatus {
        let (state, core, rom) = match self.game_reference() {
            Some((reference, core)) => {
                let state = if self.paused() {
                    SessionState::QuickMenu
                } else {
                    SessionState::Running
                };
                (state, Some(core.to_owned()), Some(reference.path.clone()))
            }
            None if self.in_game() => (SessionState::Exiting, None, None),
            None => (SessionState::Idle, None, None),
        };
        SessionStatus {
            state,
            core,
            rom,
            frame: self.frame(),
        }
    }
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Handler for Shell<D, S, I, C> {
    fn server_name(&self) -> &str {
        concat!("neptron ", env!("CARGO_PKG_VERSION"))
    }

    fn screenshot(&mut self, _args: ScreenshotArgs) -> Result<neptron_ipc::Screenshot, ErrorBody> {
        Err(unavailable(
            "this build has no PNG encoder; see the open points of the README",
        ))
    }

    fn ui_tree(&mut self) -> Result<UiTree, ErrorBody> {
        let tree = &self.ui().tree;
        Ok(UiTree {
            screen: tree.screen().to_owned(),
            focused: tree.focused().map(|r| r.label.to_owned()),
            rows: tree
                .rows()
                .map(|row| UiRow {
                    id: row.label.to_owned(),
                    label: row.label.to_owned(),
                    value: row.value.map(str::to_owned),
                    setting: self.setting_key_of(row.label).map(str::to_owned),
                    source: row.source.map(wire_level),
                    clipped: row.clipped,
                    bounds: Rect {
                        x: row.rect.x,
                        y: row.rect.y,
                        w: row.rect.w,
                        h: row.rect.h,
                    },
                })
                .collect(),
        })
    }

    fn session_state(&mut self) -> Result<SessionStatus, ErrorBody> {
        Ok(self.session_status())
    }

    fn video_state(&mut self) -> Result<VideoState, ErrorBody> {
        let engine = self
            .engine()
            .ok_or_else(|| unavailable("the game has ended"))?;
        let mode = engine.current().map(|c| c.read_back);
        Ok(VideoState {
            mode,
            line_rate_mhz: mode.map_or(0, |m| m.line_rate_mhz()),
            field_rate_mhz: mode.map_or(0, |m| m.field_rate_mhz()),
            profile: Some(self.guard().active().monitor.profile().id.clone()),
            verdicts: Vec::new(),
            switch_count: engine.trace().modesets().count() as u64,
        })
    }

    fn stats(&mut self) -> Result<Stats, ErrorBody> {
        Ok(self.stats_now())
    }

    fn settings_get(&mut self, args: SettingsGetArgs) -> Result<SettingValue, ErrorBody> {
        let config = self.config();
        let entry = config
            .schema()
            .get(&args.key)
            .ok_or_else(|| invalid(format!("no setting {:?}", args.key)))?;
        let (value, source) = match args.level {
            None => {
                let resolved = config
                    .resolved(entry.key)
                    .ok_or_else(|| invalid(format!("no setting {:?}", args.key)))?;
                (Some(resolved.value), Some(source_level(resolved.source)))
            }
            Some(SettingLevel::Default) => (
                Some(entry.kind.default_value()),
                Some(SettingLevel::Default),
            ),
            Some(level) => {
                let value = settings_level(level)
                    .and_then(|l| config.get(l, entry.key))
                    .cloned();
                let source = value.as_ref().map(|_| level);
                (value, source)
            }
        };
        Ok(SettingValue {
            key: entry.key.to_owned(),
            value: value.map(json_of),
            source,
        })
    }

    fn log_tail(&mut self, args: LogTailArgs) -> Result<LogLines, ErrorBody> {
        Ok(LogLines {
            lines: self.log_lines(args.lines as usize),
        })
    }

    #[cfg(feature = "control")]
    fn input_send(&mut self, args: InputSendArgs) -> Result<InputQueued, ErrorBody> {
        let frame = self.queue_input(&[(0, args.port, args.control, args.value)]);
        Ok(InputQueued { frame })
    }

    #[cfg(feature = "control")]
    fn input_script(&mut self, args: InputScriptArgs) -> Result<ScriptQueued, ErrorBody> {
        let events: Vec<_> = args
            .events
            .iter()
            .map(|e| (e.frame, e.port, e.control, e.value))
            .collect();
        let start_frame = self.queue_input(&events);
        Ok(ScriptQueued {
            start_frame,
            events: u32::try_from(events.len()).unwrap_or(u32::MAX),
        })
    }

    #[cfg(feature = "control")]
    fn session_launch(&mut self, args: SessionLaunchArgs) -> Result<SessionStatus, ErrorBody> {
        self.launch_path(&args.core, &args.rom).map_err(invalid)?;
        Ok(self.session_status())
    }

    #[cfg(feature = "control")]
    fn settings_set(&mut self, args: SettingsSetArgs) -> Result<SettingWritten, ErrorBody> {
        let level = settings_level(args.level)
            .ok_or_else(|| invalid("the default level is not written".to_owned()))?;
        self.set_from_protocol(&args.key, level, &args.value)
            .map_err(invalid)?;
        Ok(SettingWritten {
            key: args.key,
            level: args.level,
        })
    }
}

/// The client's side of the socket thread: each command goes to the frame
/// thread, and the thread waits for its answer.
struct Forward<'a> {
    tx: &'a SyncSender<Request>,
}

impl Forward<'_> {
    fn call(&self, command: Command) -> Result<Reply, ErrorBody> {
        let (answer, rx) = mpsc::sync_channel(1);
        let wait = match &command {
            Command::Wait(args) => {
                Duration::from_millis(u64::from(args.timeout_ms.min(WAIT_TIMEOUT_MAX_MS)))
                    + ANSWER_WAIT
            }
            _ => ANSWER_WAIT,
        };
        match self.tx.try_send(Request { command, answer }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return Err(unavailable("the frontend is busy")),
            Err(TrySendError::Disconnected(_)) => {
                return Err(unavailable("the frontend has stopped"));
            }
        }
        match rx.recv_timeout(wait) {
            Ok(answer) => answer,
            Err(RecvTimeoutError::Timeout) => Err(ErrorBody::new(
                ErrorCode::Timeout,
                "the frontend did not answer in time",
            )),
            Err(RecvTimeoutError::Disconnected) => Err(unavailable("the frontend has stopped")),
        }
    }
}

/// Unwraps the reply of one command, or gives a protocol failure.
macro_rules! forward {
    ($self:ident, $command:expr, $variant:ident) => {
        match $self.call($command)? {
            Reply::$variant(data) => Ok(data),
            other => Err(ErrorBody::new(
                ErrorCode::Failed,
                format!("the frontend answered with {}", other.name()),
            )),
        }
    };
}

impl Handler for Forward<'_> {
    fn server_name(&self) -> &str {
        concat!("neptron ", env!("CARGO_PKG_VERSION"))
    }

    fn screenshot(&mut self, args: ScreenshotArgs) -> Result<neptron_ipc::Screenshot, ErrorBody> {
        forward!(self, Command::Screenshot(args), Screenshot)
    }

    fn ui_tree(&mut self) -> Result<UiTree, ErrorBody> {
        forward!(self, Command::UiTree, UiTree)
    }

    fn input_send(&mut self, args: InputSendArgs) -> Result<InputQueued, ErrorBody> {
        forward!(self, Command::InputSend(args), InputSend)
    }

    fn input_script(&mut self, args: InputScriptArgs) -> Result<ScriptQueued, ErrorBody> {
        forward!(self, Command::InputScript(args), InputScript)
    }

    fn clock_step(&mut self, args: ClockStepArgs) -> Result<FrameReached, ErrorBody> {
        forward!(self, Command::ClockStep(args), ClockStep)
    }

    fn wait(&mut self, args: WaitArgs) -> Result<FrameReached, ErrorBody> {
        forward!(self, Command::Wait(args), Wait)
    }

    fn session_launch(&mut self, args: SessionLaunchArgs) -> Result<SessionStatus, ErrorBody> {
        forward!(self, Command::SessionLaunch(args), SessionLaunch)
    }

    fn session_state(&mut self) -> Result<SessionStatus, ErrorBody> {
        forward!(self, Command::SessionState, SessionState)
    }

    fn video_state(&mut self) -> Result<VideoState, ErrorBody> {
        forward!(self, Command::VideoState, VideoState)
    }

    fn stats(&mut self) -> Result<Stats, ErrorBody> {
        forward!(self, Command::Stats, Stats)
    }

    fn settings_get(&mut self, args: SettingsGetArgs) -> Result<SettingValue, ErrorBody> {
        forward!(self, Command::SettingsGet(args), SettingsGet)
    }

    fn settings_set(&mut self, args: SettingsSetArgs) -> Result<SettingWritten, ErrorBody> {
        forward!(self, Command::SettingsSet(args), SettingsSet)
    }

    fn log_tail(&mut self, args: LogTailArgs) -> Result<LogLines, ErrorBody> {
        forward!(self, Command::LogTail(args), LogTail)
    }
}

fn accept_loop(listener: &ControlListener, tx: &SyncSender<Request>, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok(stream) => {
                let setup = stream
                    .set_nonblocking(false)
                    .and_then(|()| stream.set_read_timeout(Some(STREAM_TIMEOUT)))
                    .and_then(|()| stream.set_write_timeout(Some(STREAM_TIMEOUT)));
                if setup.is_ok() {
                    let mut reader = StopReader {
                        inner: BufReader::new(&stream),
                        stop,
                    };
                    let mut writer = &stream;
                    // A connection ends on a read timeout, a closed peer, a
                    // bad line or the stop; the next client gets a new one.
                    let _ = serve_lines(&mut reader, &mut writer, &mut Forward { tx });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_POLL);
            }
            Err(_) => std::thread::sleep(ACCEPT_POLL),
        }
    }
}

/// A reader that ends its stream when the stop is set, so the server
/// loop stops between two request lines.
struct StopReader<'a, R> {
    inner: R,
    stop: &'a AtomicBool,
}

impl<R: Read> Read for StopReader<'_, BufReader<R>> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.stop.load(Ordering::Relaxed) {
            return Ok(0);
        }
        self.inner.read(buf)
    }
}

impl<R: Read> BufRead for StopReader<'_, BufReader<R>> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.stop.load(Ordering::Relaxed) {
            return Ok(&[]);
        }
        self.inner.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.inner.consume(amount);
    }
}

fn unavailable(message: &str) -> ErrorBody {
    ErrorBody::new(ErrorCode::Unavailable, message)
}

fn invalid(message: String) -> ErrorBody {
    ErrorBody::new(ErrorCode::InvalidArguments, message)
}

fn wire_level(level: neptron_ui::snapshot::Level) -> SettingLevel {
    match level {
        neptron_ui::snapshot::Level::Default => SettingLevel::Default,
        neptron_ui::snapshot::Level::Global => SettingLevel::Global,
        neptron_ui::snapshot::Level::System => SettingLevel::System,
        neptron_ui::snapshot::Level::Game => SettingLevel::Game,
    }
}

fn source_level(source: neptron_settings::Source) -> SettingLevel {
    match source {
        neptron_settings::Source::Game => SettingLevel::Game,
        neptron_settings::Source::System => SettingLevel::System,
        neptron_settings::Source::Global => SettingLevel::Global,
        neptron_settings::Source::Default => SettingLevel::Default,
    }
}

fn settings_level(level: SettingLevel) -> Option<Level> {
    match level {
        SettingLevel::Game => Some(Level::Game),
        SettingLevel::System => Some(Level::System),
        SettingLevel::Global => Some(Level::Global),
        SettingLevel::Default => None,
    }
}

fn json_of(value: neptron_settings::Value) -> serde_json::Value {
    match value {
        neptron_settings::Value::Bool(v) => serde_json::Value::Bool(v),
        neptron_settings::Value::Int(v) => serde_json::Value::from(v),
        neptron_settings::Value::Text(v) => serde_json::Value::String(v),
    }
}

/// The schema value of a JSON value, or `None` for a JSON type no setting
/// has.
pub fn value_of_json(json: &serde_json::Value) -> Option<neptron_settings::Value> {
    match json {
        serde_json::Value::Bool(v) => Some(neptron_settings::Value::Bool(*v)),
        serde_json::Value::Number(n) => n.as_i64().map(neptron_settings::Value::Int),
        serde_json::Value::String(s) => Some(neptron_settings::Value::Text(s.clone())),
        _ => None,
    }
}
