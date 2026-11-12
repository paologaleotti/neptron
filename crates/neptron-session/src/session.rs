//! The session: one game from launch to exit.

mod frame;
mod launch;

use neptron_audio::{AudioPipeline, AudioStats, FlipRateMeter, Resampling};
use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::{Hotkey, Hotkeys, PortStates};
use neptron_libretro::{GameGeometry, Host, LogRecord, RunError, Timing};
use neptron_types::{Frame, InputEvent, SourceGeometry};
use neptron_video::{Outcome, VideoEngine, VideoStatusText};

use crate::pads::PadMask;
use crate::picture::Picture;
use crate::save::SaveRamWriter;
use crate::{Counters, ExitReport, Flush, RunRateSource, SaveStats};

/// Loop frames in a row with no completed flip before
/// [`Session::run_frame`] gives up with
/// [`SessionError::DisplayStalled`](crate::SessionError::DisplayStalled).
/// A bounded flip wait (500 ms on KMS) makes this at most a few seconds.
/// The engine keeps a flip that timed out queued until a wait returns it,
/// so a flip that needs a few waits passes.
pub const FLIPLESS_FRAMES_LIMIT: u32 = 10;

/// The events of one poll that the session keeps room for. A poll with more
/// grows the buffer once.
const INPUT_EVENTS_CAPACITY: usize = 256;

/// One game, from launch to exit: the core host, the ODREN engine on the
/// display, the audio pipeline, the input and the save RAM writer.
///
/// The display is the master clock. [`Session::run_frame`] runs one core
/// frame and presents it on one flip, or on two flips for double refresh.
/// While the quick menu is open, it presents the last frame under the UI
/// and does not run the core.
pub struct Session<D: Display, S: AudioSink, I: InputSource, C: Clock> {
    host: Host,
    engine: VideoEngine<D>,
    /// `None` while the source settles at launch, and after the pipeline
    /// could not be made again for a new core timing.
    audio: Option<AudioPipeline<S>>,
    resampling: Resampling,
    input: I,
    clock: C,
    events: Vec<InputEvent>,
    ports: PortStates,
    hotkeys: Hotkeys,
    fired: Vec<Hotkey>,
    mask: PadMask,
    picture: Picture,
    geometry: GameGeometry,
    timing: Timing,
    source: SourceGeometry,
    state: State,
    run_rate: RunRateSource,
    meter: FlipRateMeter,
    refused_run_rate: Option<u64>,
    /// `None` when the core has no save RAM or the content has no save
    /// file, and after the last flush.
    writer: Option<SaveRamWriter>,
    next_flush_ns: u64,
    message: String,
    message_frames: u32,
    flipless: u32,
    counters: Counters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Running,
    Paused,
    Ended,
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock> Session<D, S, I, C> {
    /// The quick menu is open: the core is paused and the audio is muted.
    pub fn paused(&self) -> bool {
        self.state == State::Paused
    }

    /// The session has ended. [`Session::quit`] is the only call left.
    pub fn ended(&self) -> bool {
        self.state == State::Ended
    }

    /// Closes the quick menu. The core runs again from the next frame. The
    /// buttons held now reach the core only after their release, so the
    /// button that chose "resume" does not reach the game. A modeset that
    /// failed on the device is tried again.
    pub fn resume(&mut self) {
        if self.state != State::Paused {
            return;
        }
        self.state = State::Running;
        self.mask.hold_back_pressed(&self.ports);
        let failed = self
            .engine
            .trace()
            .last()
            .is_some_and(|r| matches!(r.outcome, Outcome::SetModeFailed(_)));
        if failed {
            self.engine.retry();
        }
    }

    /// Resets the core (`retro_reset`).
    ///
    /// # Errors
    ///
    /// The host's [`RunError`].
    pub fn reset(&mut self) -> Result<(), RunError> {
        self.host.reset()
    }

    /// Sets how long Start held alone opens the quick menu, in nanoseconds;
    /// 0 is off. The same value again changes nothing, so the caller can
    /// pass the setting every frame.
    pub fn set_start_hold_ns(&mut self, hold_ns: u64) {
        self.hotkeys.set_start_hold_ns(hold_ns);
    }

    /// Ends the session: the last save RAM flush, then the writer thread
    /// stops and joins. Dropping the session unloads the core.
    pub fn quit(mut self) -> ExitReport {
        let (save, save_stats) = self.finish_writer();
        ExitReport {
            save,
            save_stats,
            counters: self.counters,
        }
    }

    /// The ODREN engine: the mode on screen, the trace, the refusal state
    /// and the live changes of monitor, policy and scanline settings.
    pub fn video(&self) -> &VideoEngine<D> {
        &self.engine
    }

    /// The ODREN engine, for the live changes of the quick menu, the Display
    /// panel and the safety layer (`set_settings`, `set_scanlines`,
    /// `set_monitor`, `retry`). Present and wait only through
    /// [`Session::run_frame`].
    pub fn video_mut(&mut self) -> &mut VideoEngine<D> {
        &mut self.engine
    }

    /// Writes the video status of the mode on screen, from its read back.
    /// `None` before the first mode.
    pub fn video_status<'o>(&self, out: &'o mut VideoStatusText) -> Option<&'o VideoStatusText> {
        self.engine.video_status(out)
    }

    /// The core host: the system info and the core options for the quick
    /// menu.
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// The core host, for `set_option` from the quick menu.
    pub fn host_mut(&mut self) -> &mut Host {
        &mut self.host
    }

    /// The log records of the last call into the core.
    pub fn core_logs(&self) -> &[LogRecord] {
        self.host.logs()
    }

    /// The last game frame, as the session presents it.
    pub fn last_frame(&self) -> Option<Frame<'_>> {
        self.picture.frame()
    }

    /// The source of the last core frame.
    pub fn source(&self) -> SourceGeometry {
        self.source
    }

    /// The input events of the last frame, for the UI navigation while the
    /// quick menu is open.
    pub fn input_events(&self) -> &[InputEvent] {
        &self.events
    }

    /// The RetroPad state of every port after the last frame.
    pub fn ports(&self) -> &PortStates {
        &self.ports
    }

    /// The core's last message (`SET_MESSAGE`), while its frame count runs.
    pub fn message(&self) -> Option<&str> {
        (self.message_frames > 0).then_some(self.message.as_str())
    }

    /// The counters of the audio pipeline. `None` after the audio was lost.
    pub fn audio_stats(&self) -> Option<&AudioStats> {
        self.audio.as_ref().map(AudioPipeline::stats)
    }

    /// The audio sink. `None` after the audio was lost.
    pub fn audio_sink(&self) -> Option<&S> {
        self.audio.as_ref().map(AudioPipeline::sink)
    }

    /// The run rate that audio rate control uses, in millihertz.
    pub fn run_rate_mhz(&self) -> Option<u64> {
        self.audio.as_ref().map(AudioPipeline::run_rate_mhz)
    }

    /// The counters of the save RAM writer. `None` when the game has no
    /// save RAM, and after the last flush.
    pub fn save_stats(&self) -> Option<&SaveStats> {
        self.writer.as_ref().map(SaveRamWriter::stats)
    }

    /// The counters of the session.
    pub fn counters(&self) -> &Counters {
        &self.counters
    }

    fn finish_writer(&mut self) -> (Flush, SaveStats) {
        let Some(mut writer) = self.writer.take() else {
            return (Flush::NoSaveRam, SaveStats::default());
        };
        let host = &mut self.host;
        let flush = writer.finish(|out| host.copy_save_ram(out));
        (flush, writer.take_stats())
    }
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock> Drop for Session<D, S, I, C> {
    fn drop(&mut self) {
        // A session dropped without `quit` still flushes its save RAM. Drop
        // has no caller to report to; `quit` returns the result.
        let _ = self.finish_writer();
    }
}
