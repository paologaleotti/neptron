//! The session: one game from launch to exit.

mod frame;
mod hw;
mod launch;

use neptron_audio::{AudioPipeline, AudioStats, FlipRateMeter, Resampling};
use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::{Hotkey, Hotkeys, PortStates};
use neptron_libretro::{GameGeometry, Host, LogRecord, RunError, Timing};
use neptron_types::{FlipEvent, Frame, InputEvent, SourceGeometry};
use neptron_video::{VideoEngine, VideoStatusText};

pub use hw::CONTEXT_RECOVERIES_MAX;

use crate::pacing::{FlipClock, Pacing};
use crate::pads::PadMask;
use crate::picture::Picture;
use crate::save::SaveRamWriter;
use crate::speed::{SLOW_NOTICE_PERMILLE, Speed};
use crate::{Counters, ExitReport, Flush, RunRateSource, SaveStats};

/// Loop frames in a row with no completed flip before
/// [`Session::run_frame`] gives up with
/// [`SessionError::DisplayStalled`](crate::SessionError::DisplayStalled).
/// When each frame's wait times out, the bounded flip wait (500 ms on KMS)
/// makes this at most a few seconds. When the display fails at once (an
/// immediate error of `present` or of the wait, or `NoMode`), no wait
/// paces the loop, and the session ends after this many unpaced frames:
/// the core runs ten frames with no display. The engine keeps a flip that timed out queued until a wait returns it,
/// so a flip that needs a few waits passes. A frame with the output off
/// (`FrameReport::blanked`) does not count.
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
    /// The GL context of a core that draws with GL; `None` for a core
    /// that draws in memory.
    hw: Option<hw::Hw>,
    /// The internal resolution of the last picture: 1, or the scale of a
    /// GL core whose picture is that many times its source.
    picture_scale: u16,
    /// The rule of the internal resolution of a GL core with such an
    /// option, for a live change of the scanlines.
    scale_rule: Option<hw::ScaleRule>,
    geometry: GameGeometry,
    timing: Timing,
    /// The system data says that every frame is progressive.
    always_progressive: bool,
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
    /// The error-level core log lines of the launch, made with room for
    /// [`START_ERRORS_MAX`](crate::START_ERRORS_MAX) at launch.
    start_errors: Vec<String>,
    /// The log records that the full queue of the host dropped during the
    /// launch.
    start_dropped_logs: usize,
    /// What the session knows about the start, for [`crate::end_reason`].
    start: crate::StartFacts,
    /// The last error line of the core in a frame, a settle frame of the
    /// launch or a loop frame, in a buffer made at launch with room for the
    /// longest record.
    last_error: String,
    flipless: u32,
    /// The vertical blank count of the last flip; `None` before the first
    /// flip of a mode.
    last_flip: Option<FlipEvent>,
    /// The time of the last flip on the session clock.
    last_flip_session_ns: Option<u64>,
    /// The time of the input poll of the frame whose flip is queued, when
    /// that poll returned an event and the core ran with it.
    latency_from: Option<u64>,
    /// The time of the flip events on the session clock.
    flip_clock: FlipClock,
    /// Whether the loop waits for each flip, and the measured rate of a
    /// late core.
    pacing: Pacing,
    /// The last flip of the frame before is queued, and no wait returned
    /// it yet: the frame before was in late pacing.
    deferred: bool,
    /// The core ran in the frame of the queued flip.
    queued_ran_core: bool,
    /// The frame of the queued flip was paced: a core frame of a running
    /// game in a mode set before it, whose work is known.
    queued_paced: bool,
    /// The end of the frame before, on the session clock: the start of the
    /// work of this frame.
    cycle_start_ns: u64,
    /// The time from the end of the frame before to the end of the last
    /// frame.
    cycle_ns: u64,
    /// The cycles of the late frames, for the speed of a slow core.
    speed: Speed,
    /// The audio run rate moves to its target by bounded steps: a measure
    /// was used, and the run rate is not back at the mode's rate yet.
    run_rate_slewing: bool,
    /// The level of the audio queue before each write, smoothed over
    /// [`QUEUE_SMOOTH_FRAMES`](crate::QUEUE_SMOOTH_FRAMES) frames.
    queue_level: u64,
    /// The measured rate of the late core went below
    /// [`SOUND_MUTE_PERMILLE`](crate::SOUND_MUTE_PERMILLE) of the core's
    /// rate and not yet back above
    /// [`SOUND_RESUME_PERMILLE`](crate::SOUND_RESUME_PERMILLE): the audio
    /// is muted.
    too_slow_for_sound: bool,
    /// The app asked for frames with no game picture while paused.
    picture_hidden: bool,
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
    /// button that chose "resume" does not reach the game, and a hold of
    /// Select and Start in progress now opens no menu; a new hold starts
    /// after a release of one of them. A source that the
    /// engine blocked after a device error of its modeset is tried again.
    pub fn resume(&mut self) {
        if self.state != State::Paused {
            return;
        }
        self.state = State::Running;
        self.picture_hidden = false;
        self.mask.hold_back_pressed(&self.ports);
        self.hotkeys.cancel_quick_menu();
        if self.engine.source_blocked() {
            self.engine.retry();
        }
    }

    /// While `hide` is true and the session is paused, each frame shows the
    /// UI alone, with no game picture: on a flat panel the frozen picture
    /// then does not stay on screen around the UI. [`Session::resume`]
    /// ends it. A running game always shows its picture.
    pub fn hide_picture(&mut self, hide: bool) {
        self.picture_hidden = hide && self.state == State::Paused;
    }

    /// Resets the core (`retro_reset`).
    ///
    /// # Errors
    ///
    /// The host's [`RunError`].
    pub fn reset(&mut self) -> Result<(), RunError> {
        self.in_core_context(Host::reset)
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

    /// The last game frame in memory, as the session presents it. `None`
    /// for the picture of a GL core, which is in the image of the display:
    /// [`Session::read_picture`] reads it back.
    pub fn last_frame(&self) -> Option<Frame<'_>> {
        self.picture.frame()
    }

    /// The internal resolution of the last picture: 1, or `k` for a GL
    /// core whose picture is `k` times its source.
    pub fn picture_scale(&self) -> u16 {
        self.picture_scale
    }

    /// The hardware context of a GL core, or `None` for a core that draws
    /// in memory.
    pub fn hw_request(&self) -> Option<neptron_backend::ContextRequest> {
        self.host.hw_request().filter(|_| self.hw.is_some())
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

    /// The input source, for what the app reads of it beyond its events,
    /// such as the port of each pad.
    pub fn input(&self) -> &I {
        &self.input
    }

    /// The error-level log lines of the core during the launch (the load
    /// and the frames before the first mode), at most
    /// [`START_ERRORS_MAX`](crate::START_ERRORS_MAX): the words of a core
    /// that cannot run the game.
    pub fn start_errors(&self) -> &[String] {
        &self.start_errors
    }

    /// The log records of the launch that the host dropped because its
    /// queue was full. The host keeps the first error lines of each call
    /// apart, so [`Session::start_errors`] can still hold them.
    pub fn start_dropped_logs(&self) -> usize {
        self.start_dropped_logs
    }

    /// The last error line of the core in a frame: a settle frame of the
    /// launch or a loop frame. `None` when the core logged none in a frame;
    /// the lines of the load are in [`Session::start_errors`].
    pub fn last_error(&self) -> Option<&str> {
        (!self.last_error.is_empty()).then_some(self.last_error.as_str())
    }

    /// The core's last message (`SET_MESSAGE`), also after its frame count
    /// ran out. `None` when the core sent none.
    pub fn last_message(&self) -> Option<&str> {
        (!self.message.is_empty()).then_some(self.message.as_str())
    }

    /// The core's last message (`SET_MESSAGE`), while its frame count runs.
    pub fn message(&self) -> Option<&str> {
        (self.message_frames > 0).then_some(self.message.as_str())
    }

    /// The counters of the audio pipeline. `None` after the audio was lost.
    pub fn audio_stats(&self) -> Option<&AudioStats> {
        self.audio.as_ref().map(AudioPipeline::stats)
    }

    /// Sets the user volume in percent, live: the gain ramps to it over
    /// [`VOLUME_RAMP_MS`](neptron_audio::VOLUME_RAMP_MS), with no device
    /// opened again. 100 is the core's own level, 0 is silence, more than
    /// 100 is 100. The mute of the quick menu and of a mode switch stays
    /// as it is. Nothing happens after the audio was lost.
    pub fn set_volume_percent(&mut self, percent: u32) {
        if let Some(audio) = &mut self.audio {
            audio.set_volume_percent(percent);
        }
    }

    /// The user volume the audio is at or ramps to, in percent. `None`
    /// after the audio was lost.
    pub fn volume_percent(&self) -> Option<u32> {
        self.audio.as_ref().map(AudioPipeline::volume_percent)
    }

    /// The audio sink. `None` after the audio was lost.
    pub fn audio_sink(&self) -> Option<&S> {
        self.audio.as_ref().map(AudioPipeline::sink)
    }

    /// Plays a short cue of the app, as stereo frames, mixed into the next
    /// core frames ([`AudioPipeline::write_cue`]): it bypasses the mute,
    /// the user volume does not scale it, and while the core does not run
    /// (the quick menu, the output off) each frame plays its next slice
    /// ([`AudioPipeline::idle_frame`]). Returns the frames of the cue: 0
    /// after the audio was lost.
    pub fn write_cue(&mut self, stereo: &[i16]) -> usize {
        self.audio
            .as_mut()
            .map_or(0, |audio| audio.write_cue(stereo))
    }

    /// The run rate that audio rate control uses, in millihertz.
    pub fn run_rate_mhz(&self) -> Option<u64> {
        self.audio.as_ref().map(AudioPipeline::run_rate_mhz)
    }

    /// The speed of the game, in percent of the run rate of the mode on
    /// screen, while the core cannot keep up: the loop is in late pacing,
    /// the measure holds a full window of
    /// [`SPEED_WINDOW_FRAMES`](crate::SPEED_WINDOW_FRAMES) frames, and the
    /// speed is below [`SLOW_NOTICE_PERMILLE`](crate::SLOW_NOTICE_PERMILLE).
    /// `None` otherwise, so a single late frame gives nothing.
    pub fn slow_speed_percent(&self) -> Option<u32> {
        if !self.speed.full() {
            return None;
        }
        let measured = self.measured_rate_mhz()?;
        let mode = self.mode_run_rate_mhz().filter(|&rate| rate > 0)?;
        if measured * 1_000 >= mode * SLOW_NOTICE_PERMILLE {
            return None;
        }
        u32::try_from(measured * 100 / mode).ok()
    }

    /// The game runs too slowly for its sound: below half the core's frame
    /// rate, the range of the audio pipeline, and not yet back above
    /// [`SOUND_RESUME_PERMILLE`](crate::SOUND_RESUME_PERMILLE) of it. The
    /// audio is muted.
    pub fn sound_too_slow(&self) -> bool {
        self.too_slow_for_sound
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
        // The host is the first field, so it drops next: the core unloads
        // inside its context, and the display destroys the context after.
        self.end_hw();
    }
}
