//! One frame of the display-locked loop.

use neptron_audio::{AudioPipeline, CoreTiming, Resampling};
use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::Hotkey;
use neptron_libretro::{Event, LogSource, RunError, Timing, Video};
use neptron_types::{FlipEvent, Frame};
use neptron_video::{Directive, VideoError};

use super::{FLIPLESS_FRAMES_LIMIT, Session, State};
use crate::pacing;
use crate::pads::CORE_PORTS;
use crate::save::SAVE_RAM_FLUSH_INTERVAL_NS;
use crate::source::{FrameRules, source_geometry};
use crate::speed::{self, SOUND_MUTE_PERMILLE, SOUND_RESUME_PERMILLE};
use crate::{Counters, FrameReport, PacingChange, RunRateSource, SessionError};

impl<D: Display, S: AudioSink, I: InputSource, C: Clock> Session<D, S, I, C> {
    /// Runs one frame of the loop, with `ui` drawn over the game (the
    /// 320x240 canvas of the quick menu or a notice, or `None`).
    ///
    /// 1. Polls the input once, updates the ports and the hotkeys. Select
    ///    and Start together, or the Start hold, open the quick menu, which
    ///    pauses the core. A second quick menu hotkey while it is open does
    ///    nothing: the menu is never a toggle; [`Session::resume`] closes it.
    /// 2. Runs one core frame, unless the menu is open or the engine has
    ///    turned the output off (`blanked`).
    /// 3. Gives the engine the source, then presents the frame and waits for
    ///    its flip: once, or twice for double refresh. In late pacing it
    ///    first waits for the flip that the frame before left queued, and
    ///    leaves its own last flip queued. The audio is muted
    ///    while the engine asks for it, while the menu is open and while the
    ///    inputs refuse the mode on screen.
    /// 4. Ends the core frame of the audio pipeline between the first
    ///    present and the first wait for a flip, so the resampler runs while
    ///    the GPU draws. The run rate is the one of the mode on screen.
    /// 5. Sends the save RAM to the writer thread when the timer is due.
    ///
    /// No allocation and no file I/O in a steady frame.
    ///
    /// # Errors
    ///
    /// [`SessionError::Ended`] after the session ended,
    /// [`SessionError::Core`] when the host fails, and
    /// [`SessionError::DisplayStalled`] after [`FLIPLESS_FRAMES_LIMIT`]
    /// frames in a row with no completed flip. The display errors of one
    /// frame are in its report. A frame with the output off is not a stall:
    /// it returns at once with `blanked`, and the caller paces the loop
    /// until the engine sets a mode again.
    pub fn run_frame(&mut self, ui: Option<Frame<'_>>) -> Result<FrameReport, SessionError> {
        if self.state == State::Ended {
            return Err(SessionError::Ended);
        }
        let now_ns = self.clock.now_ns();
        let mut report = FrameReport {
            ran_core: false,
            directive: Directive::Steady,
            flips: 0,
            modeset: None,
            display_error: None,
            menu_opened: false,
            blind_recovery: None,
            ended: None,
            blanked: false,
            drained_flips: 0,
            deferred: false,
            pacing: None,
        };
        self.read_input(now_ns, &mut report);
        self.follow_monitor();

        // With the output off nothing paces the core, so it waits with the
        // game frozen until the engine sets a mode again.
        if self.state == State::Running && !self.engine.blanked() {
            for port in 0..CORE_PORTS {
                let pad = self.mask.pad(&self.ports, port);
                // `port` is below the host's port count.
                let _ = self.host.set_pad(port, pad);
            }
            // A core too slow for sound feeds no audio: its frames are
            // idle frames of the pipeline, as under the quick menu.
            self.run_core(!self.too_slow_for_sound)
                .map_err(|e| self.fail(e))?;
            if let Err(e) = self.hw_fault() {
                self.state = State::Ended;
                return Err(e);
            }
            self.mask.core_ran();
            self.counters.core_frames += 1;
            report.ran_core = true;
            if self.state == State::Ended {
                report.ended = Some(if self.no_picture {
                    crate::EndReason::NoPicture
                } else {
                    crate::end_reason(self.start, now_ns)
                });
            }
            // After the stop check: the picture of this frame counts for a
            // later stop only. A refused mode hides it, and it still counts.
            self.start.picture |= self.picture.frame().is_some();
        }
        self.counters.loop_frames += 1;

        report.directive = self.engine.begin_frame(self.source);
        // The mute comes before the present, so its silence covers a
        // modeset in the present and the wait for the first flip of the new
        // mode.
        let mute = self.must_mute();
        if let Some(audio) = self.audio.as_mut().filter(|_| mute) {
            audio.mute();
        }
        let poll_ns = (report.ran_core && !self.events.is_empty()).then_some(now_ns);
        let was_late = self.pacing.late();
        self.show(ui, poll_ns, &mut report);
        let end_ns = self.clock.now_ns();
        self.cycle_ns = end_ns.saturating_sub(self.cycle_start_ns);
        match self.interval_ns() {
            Some(interval_ns) if report.deferred => self.speed.push(self.cycle_ns, interval_ns),
            _ if !self.pacing.late() => self.speed.clear(),
            _ => {}
        }
        self.cycle_start_ns = end_ns;
        report.pacing = match (was_late, self.pacing.late()) {
            (false, true) => Some(PacingChange::Late),
            (true, false) => Some(PacingChange::Wait),
            _ => None,
        };
        report.blanked = self.engine.blanked();
        let mute = self.must_mute();
        if let Some(audio) = self.audio.as_mut().filter(|_| !mute) {
            audio.unmute();
        }
        self.update_run_rate();

        if self.state == State::Running && now_ns >= self.next_flush_ns {
            self.flush_save_ram();
            self.next_flush_ns = now_ns.saturating_add(SAVE_RAM_FLUSH_INTERVAL_NS);
        }
        self.message_frames = self.message_frames.saturating_sub(1);
        self.check_stall(&report)?;
        Ok(report)
    }

    /// Runs one core frame and takes its output: the picture, the events
    /// and, with `feed_audio`, the audio. A GL core runs inside its
    /// context; a failure of the context is kept for
    /// [`Session::hw_fault`], and the core does not run when its context
    /// could not begin.
    pub(super) fn run_core(&mut self, feed_audio: bool) -> Result<(), RunError> {
        if !self.begin_core_frame() {
            return Ok(());
        }
        let out = self.host.run_frame();
        if let Some(hw) = self.hw.as_mut() {
            super::hw::end(&mut self.engine, hw, &mut self.counters);
        }
        let out = out?;
        let counters = &mut self.counters;
        counters.log_records += out.logs.len() as u64;
        counters.host_drops +=
            (out.dropped.audio_frames + out.dropped.events + out.dropped.log_records) as u64;
        for event in out.events {
            match event {
                Event::Geometry(geometry) => self.geometry = *geometry,
                Event::AvInfo(av) => {
                    self.geometry = av.geometry;
                    self.picture
                        .reserve(av.geometry.max_width, av.geometry.max_height);
                    if let Some(hw) = self.hw.as_mut() {
                        hw.grow((av.geometry.max_width, av.geometry.max_height));
                    }
                    if av.timing != self.timing {
                        self.timing = av.timing;
                        retime(&mut self.audio, &av.timing, self.resampling, counters);
                    }
                }
                Event::Shutdown => self.state = State::Ended,
                Event::Message { text, frames } => {
                    set_message(&mut self.message, text, MESSAGE_LIMIT);
                    self.message_frames = *frames;
                }
                Event::PixelFormat(_) | Event::Rotation(_) | Event::Refused(_) => {}
            }
        }
        let mut dark = false;
        match (out.video, self.hw.as_mut()) {
            (Video::Frame(frame), _) => self.picture.store(&frame),
            (Video::Hardware { width, height }, Some(hw)) => {
                self.picture.store_gpu(width, height);
                hw.saw_picture();
            }
            (Video::Duplicate(_) | Video::Missing, hw) => {
                counters.repeated_pictures += 1;
                dark = hw.is_some();
            }
            // The host sends a hardware frame only after it gave the core
            // a context, so a session with no context counts it as a fault.
            (Video::Rejected(_) | Video::Hardware { .. }, hw) => {
                counters.frame_faults += 1;
                dark = hw.is_some();
            }
        }
        if let Some(audio) = self.audio.as_mut().filter(|_| feed_audio) {
            audio.push_samples(out.audio);
        }
        let (width, height) = self
            .picture
            .size()
            .unwrap_or((self.geometry.base_width, self.geometry.base_height));
        self.picture_scale = match (self.hw.as_ref(), self.picture.gpu_size()) {
            (Some(hw), Some(_)) => {
                let max = (self.geometry.max_width, self.geometry.max_height);
                let scale = hw.frame_scale((width, height), max);
                if scale == 1 && hw.scale() > 1 {
                    counters.unscaled_frames += 1;
                }
                scale
            }
            _ => 1,
        };
        let (width, height) = (width / self.picture_scale, height / self.picture_scale);
        let rules = FrameRules {
            rotation: self.host.rotation(),
            always_progressive: self.always_progressive,
        };
        self.source = source_geometry(width, height, &self.geometry, &self.timing, rules);
        let last_error = self
            .host
            .error_logs()
            .iter()
            .rev()
            .find(|r| r.source == LogSource::Core);
        if let Some(record) = last_error {
            // The record holds at most `LOG_TEXT_MAX_BYTES`, the capacity
            // made at launch, so the copy does not allocate.
            self.last_error.clear();
            self.last_error.push_str(&record.text);
        }
        if dark {
            self.count_dark_frame();
        }
        Ok(())
    }

    fn read_input(&mut self, now_ns: u64, report: &mut FrameReport) {
        self.events.clear();
        self.input.poll(&mut self.events);
        self.ports.apply(&self.events);
        self.mask.release(&self.ports);
        self.fired.clear();
        self.hotkeys.update(&self.ports, now_ns, &mut self.fired);
        for index in 0..self.fired.len() {
            match self.fired[index] {
                Hotkey::QuickMenu { .. } if self.state == State::Running => {
                    self.pause();
                    report.menu_opened = true;
                }
                Hotkey::QuickMenu { .. } => {}
                Hotkey::BlindRecovery { port } => report.blind_recovery = Some(port),
            }
        }
    }

    /// Opens the quick menu: the core stops, the audio fades out, and the
    /// save RAM goes to the writer, in case the player turns the machine
    /// off from the menu. A press of the combination that the core did
    /// not get was for the menu, so it never reaches the game.
    fn pause(&mut self) {
        self.state = State::Paused;
        self.mask.drop_combo_presses();
        if let Some(audio) = &mut self.audio {
            audio.mute();
        }
        self.flush_save_ram();
    }

    fn must_mute(&self) -> bool {
        self.state != State::Running
            || self.engine.muted()
            || self.engine.current_refused()
            || self.engine.blanked()
            || self.too_slow_for_sound
    }

    /// Presents the picture and waits for the flip, once per flip of a core
    /// frame, and ends the audio of the frame after the first present. A
    /// display error ends the frame: the engine keeps a timed-out flip
    /// queued and drains it at the next present. `poll_ns` is the time of
    /// the input poll when it returned an event and the core ran with it.
    ///
    /// In late pacing the frame first waits for the flip that the frame
    /// before left queued, and leaves its own last flip queued.
    ///
    /// The work of the frame counts from the flip that it follows, on the
    /// session clock, so the time from that flip to the return of its wait
    /// counts too: in wait pacing from the flip of the frame before, in
    /// late pacing the core time plus the time from the later of that flip
    /// and the start of the wait.
    fn show(&mut self, ui: Option<Frame<'_>>, poll_ns: Option<u64>, report: &mut FrameReport) {
        let mut work_ns = 0;
        let mut mark_ns = self.flip_mark_ns();
        if self.deferred {
            self.deferred = false;
            let wait_start_ns = self.clock.now_ns();
            work_ns = wait_start_ns.saturating_sub(self.cycle_start_ns);
            match self.engine.wait_flip() {
                Ok(flip) => self.flip_done(flip, report),
                Err(error) => {
                    // The flip stays queued in the engine, which drains it
                    // at the next present. This frame shows nothing, and
                    // still ends its audio, as after a failed present.
                    self.end_audio_frame(report.ran_core && !self.too_slow_for_sound);
                    report.display_error = Some(error);
                    self.pacing.reset();
                    return;
                }
            }
            mark_ns = self
                .last_flip_session_ns
                .unwrap_or(wait_start_ns)
                .max(wait_start_ns);
        }
        let mut presents: u8 = 0;
        let mut defer = false;
        let mut drained = false;
        loop {
            // The inputs refuse the mode on screen: no game picture is shown
            // on it as if it were confirmed.
            let game = if self.engine.current_refused() || self.picture_hidden {
                None
            } else {
                super::hw::game_picture(&self.picture, self.hw.as_ref())
            };
            let presented = self.engine.present_picture(game, self.picture_scale, ui);
            presents = presents.saturating_add(1);
            // The present only queues the GPU's work. The audio runs while
            // the GPU draws, off the path from the flip event to the GPU
            // submit, and still at the same point of every frame. It runs
            // after a failed present too, so no frame loses its audio.
            if presents == 1 {
                self.end_audio_frame(report.ran_core && !self.too_slow_for_sound);
            }
            match presented {
                Ok(presented) => {
                    if let Some(flip) = presented.drained_flip {
                        drained = true;
                        report.drained_flips = report.drained_flips.saturating_add(1);
                        self.flip_done(flip, report);
                    }
                    if presented.modeset.is_some() {
                        self.meter.reset();
                        self.last_flip = None;
                        report.modeset = presented.modeset;
                    }
                }
                Err(error) => {
                    report.display_error = Some(error);
                    self.pacing.reset();
                    return;
                }
            }
            self.queued_ran_core = report.ran_core;
            if presents == 1 {
                self.latency_from = poll_ns;
                work_ns += self.clock.now_ns().saturating_sub(mark_ns);
                self.queued_paced = false;
                defer = if drained {
                    // The present waited for the flip of a failed wait, so
                    // the work of this frame is not known.
                    self.pacing.reset();
                    false
                } else {
                    self.pace(work_ns, report)
                };
            }
            let last = presents >= self.flips_per_frame();
            if last && defer {
                self.deferred = true;
                report.deferred = true;
                self.counters.late_frames += 1;
                return;
            }
            match self.engine.wait_flip() {
                Ok(flip) => self.flip_done(flip, report),
                Err(error) => {
                    report.display_error = Some(error);
                    self.pacing.reset();
                    return;
                }
            }
            if last {
                return;
            }
        }
    }

    /// Decides whether the loop leaves the last flip of this frame queued,
    /// from the frame's `work_ns`. Only a core frame of a running game, in
    /// a mode set before this frame, can: any other frame goes back to
    /// wait pacing.
    fn pace(&mut self, work_ns: u64, report: &FrameReport) -> bool {
        match self.interval_ns() {
            Some(interval_ns)
                if report.ran_core && report.modeset.is_none() && self.state == State::Running =>
            {
                self.queued_paced = true;
                self.pacing.on_present(work_ns, interval_ns)
            }
            _ => {
                self.pacing.reset();
                false
            }
        }
    }

    /// The flip interval of the mode on screen, in nanoseconds.
    fn interval_ns(&self) -> Option<u64> {
        let current = self.engine.current()?;
        pacing::interval_ns(current.read_back.field_rate_mhz(), self.fields_per_flip())
    }

    /// The start of the work of a frame in wait pacing: the last flip on
    /// the session clock, when it came within one interval before the end
    /// of the frame before; else that end.
    fn flip_mark_ns(&self) -> u64 {
        let start_ns = self.cycle_start_ns;
        match (self.last_flip_session_ns, self.interval_ns()) {
            (Some(flip_ns), Some(interval_ns))
                if flip_ns <= start_ns && start_ns - flip_ns <= interval_ns =>
            {
                flip_ns
            }
            _ => start_ns,
        }
    }

    /// Takes a completed flip: the flip rate, the missed fields of the
    /// frame that queued it, the pacing of that frame, the counts and the
    /// input delay.
    fn flip_done(&mut self, flip: FlipEvent, report: &mut FrameReport) {
        self.meter.push(flip.time_ns);
        let missed = self.count_missed_fields(&flip, self.queued_ran_core);
        if self.queued_paced && self.pacing.on_flip(missed.is_some_and(|m| m > 0)) {
            self.counters.late_flips += 1;
        }
        self.note_latency(&flip);
        report.flips = report.flips.saturating_add(1);
        self.counters.flips += 1;
    }

    /// Records the delay from the input poll of the frame whose `flip` just
    /// completed, with the flip time moved to the session clock.
    fn note_latency(&mut self, flip: &FlipEvent) {
        let flip_ns = self.flip_clock.flip_time(self.clock.now_ns(), flip.time_ns);
        self.last_flip_session_ns = Some(flip_ns);
        if let Some(from) = self.latency_from.take() {
            self.counters.input_to_flip_ns = flip_ns.saturating_sub(from);
        }
    }

    /// Ends the audio of the frame: the core's audio when the core ran,
    /// else an idle frame.
    fn end_audio_frame(&mut self, ran_core: bool) {
        if let Some(audio) = &mut self.audio {
            if ran_core {
                audio.end_frame();
            } else {
                audio.idle_frame();
            }
        }
    }

    /// Counts the fields between `flip` and the flip before, beyond the
    /// blanks one flip takes, when `ran_core`. A flip of a frame with no
    /// core (the quick menu) still marks the place the next gap counts
    /// from. Returns the missed fields, `None` when there is no gap to
    /// count.
    fn count_missed_fields(&mut self, flip: &FlipEvent, ran_core: bool) -> Option<u64> {
        let field_ns = self
            .engine
            .current()
            .and_then(|c| pacing::interval_ns(c.read_back.field_rate_mhz(), 1))
            .unwrap_or(1);
        let missed = self.last_flip.filter(|_| ran_core).map(|last| {
            let gap = pacing::flip_gap(
                (last.sequence, flip.sequence),
                (last.time_ns, flip.time_ns),
                field_ns,
            );
            gap.saturating_sub(self.fields_per_flip())
        });
        if let Some(missed) = missed {
            self.counters.missed_fields = self.counters.missed_fields.saturating_add(missed);
        }
        self.last_flip = Some(*flip);
        missed
    }

    /// Vertical blanks between two flips that come on time: 2 for an
    /// interlaced mode on a driver that flips once per frame, else 1.
    fn fields_per_flip(&self) -> u64 {
        let interlaced = self.engine.current().is_some_and(|c| c.read_back.interlace);
        if interlaced && !self.engine.display().caps().flip_per_field {
            2
        } else {
            1
        }
    }

    /// Flips per core frame of the mode on screen: 2 for double refresh.
    fn flips_per_frame(&self) -> u8 {
        self.engine
            .current()
            .map_or(1, |c| c.chosen.proposal.timing.repeat().max(1))
    }

    /// Gives the audio the run rate: the run rate of the mode, or the
    /// measured rate of a slow core when it is lower, trimmed to bring a
    /// queue outside its band back to its target. Once a measure or a trim
    /// was used, the run rate moves to its target by a bounded step per
    /// frame, until it is back at the mode's rate. A measured rate below the range
    /// of the pipeline mutes the audio instead, until it is back above a
    /// higher bound.
    fn update_run_rate(&mut self) {
        let Some(mode_rate) = self.mode_run_rate_mhz() else {
            self.too_slow_for_sound = false;
            return;
        };
        let measured = self.measured_rate_mhz().filter(|&rate| rate < mode_rate);
        let core_rate = u64::from(self.timing.fps_mhz);
        let bound = if self.too_slow_for_sound {
            SOUND_RESUME_PERMILLE
        } else {
            SOUND_MUTE_PERMILLE
        };
        self.too_slow_for_sound = measured
            .is_some_and(|rate| rate.saturating_mul(1_000) < core_rate.saturating_mul(bound));
        let Some(audio) = self.audio.as_mut().filter(|_| !self.too_slow_for_sound) else {
            return;
        };
        let level = audio.stats().level_frames as u64;
        self.queue_level = speed::smooth_level(self.queue_level, level);
        // The queue corrects the rate only while a measure is in use or
        // its effects settle: a game at full speed keeps the audio that the
        // rate control alone gives it.
        let in_play = measured.is_some() || self.run_rate_slewing;
        let target = if in_play {
            speed::run_rate_target(
                mode_rate,
                measured,
                speed::QueueLevel {
                    last: level,
                    smoothed: self.queue_level,
                    target: audio.control().target_frames() as u64,
                },
            )
        } else {
            mode_rate
        };
        let corrected = measured.is_some() || target != mode_rate;
        self.run_rate_slewing |= corrected;
        // The smoothed level lags: the correction ends only when the last
        // level is inside the band too.
        let settled =
            !corrected && speed::queue_in_band(level, audio.control().target_frames() as u64);
        // The pipeline refuses a run rate below half the core's rate: the
        // trim of a core at about half speed stays above it.
        let target = target.max(core_rate.div_ceil(2));
        let run_rate = if self.run_rate_slewing {
            speed::slew(audio.run_rate_mhz(), target, mode_rate, self.cycle_ns)
        } else {
            target
        };
        if settled && run_rate == target {
            self.run_rate_slewing = false;
        }
        if run_rate == audio.run_rate_mhz() || self.refused_run_rate == Some(run_rate) {
            return;
        }
        match audio.set_run_rate_mhz(run_rate) {
            Ok(()) => self.refused_run_rate = None,
            Err(_) => {
                self.refused_run_rate = Some(run_rate);
                self.counters.refused_run_rates += 1;
            }
        }
    }

    /// The measured rate of a slow late core, in millihertz.
    pub(super) fn measured_rate_mhz(&self) -> Option<u64> {
        self.speed.rate_mhz(self.interval_ns()?)
    }

    /// The run rate of the mode on screen, in millihertz: the read-back
    /// field rate, or the measured flip rate, over the flips per core
    /// frame.
    pub(super) fn mode_run_rate_mhz(&self) -> Option<u64> {
        let flips = u64::from(self.flips_per_frame());
        let flip_rate = match self.run_rate {
            RunRateSource::ReadBack => self.engine.current().map(|c| c.read_back.field_rate_mhz()),
            RunRateSource::Measured => self.meter.rate_mhz(),
        };
        flip_rate.map(|r| r / flips)
    }

    fn flush_save_ram(&mut self) {
        let host = &mut self.host;
        if let Some(writer) = &mut self.writer {
            writer.flush(|out| host.copy_save_ram(out));
        }
    }

    fn check_stall(&mut self, report: &FrameReport) -> Result<(), SessionError> {
        if report.blanked {
            self.counters.blanked_frames += 1;
            self.flipless = 0;
            return Ok(());
        }
        if report.flips > report.drained_flips || report.deferred {
            self.flipless = 0;
            return Ok(());
        }
        self.flipless += 1;
        self.counters.flipless_frames += 1;
        if self.flipless < FLIPLESS_FRAMES_LIMIT {
            return Ok(());
        }
        self.state = State::Ended;
        let last = report.display_error.clone().unwrap_or(VideoError::NoMode);
        Err(SessionError::DisplayStalled {
            frames: self.flipless,
            last,
        })
    }

    fn fail(&mut self, error: RunError) -> SessionError {
        self.state = State::Ended;
        SessionError::Core(error)
    }
}

/// The longest core message the session keeps, in bytes: the host's limit.
const MESSAGE_LIMIT: usize = neptron_libretro::MESSAGE_MAX_BYTES;

/// Copies `text` into `message`, cut at `limit` bytes on a character
/// boundary, so that the buffer made at launch is enough.
fn set_message(message: &mut String, text: &str, limit: usize) {
    message.clear();
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    message.push_str(&text[..end]);
}

/// Makes the audio pipeline again for a new core timing, as the spec asks.
/// The user volume and the rest of a cue carry over.
/// A timing the pipeline refuses keeps the old pipeline. A pipeline that
/// cannot be made loses the sink, and the session runs with no sound.
fn retime<S: AudioSink>(
    audio: &mut Option<AudioPipeline<S>>,
    timing: &Timing,
    resampling: Resampling,
    counters: &mut Counters,
) {
    let core = CoreTiming {
        sample_rate_mhz: u64::from(timing.sample_rate_mhz),
        frame_rate_mhz: u64::from(timing.fps_mhz),
    };
    if core.check().is_err() {
        counters.refused_timings += 1;
        return;
    }
    let Some(old) = audio.take() else {
        return;
    };
    let run_rate = old.run_rate_mhz();
    let run_rate = if core.check_run_rate(run_rate).is_ok() {
        run_rate
    } else {
        core.frame_rate_mhz
    };
    let volume = old.volume_percent();
    // A timing change is rare, so the copy of the rest of a cue costs no
    // steady-state frame.
    let cue = old.pending_cue().to_vec();
    match AudioPipeline::new(old.into_sink(), core, run_rate, resampling) {
        Ok(pipeline) => {
            let mut pipeline = pipeline.with_volume_percent(volume);
            pipeline.write_cue(&cue);
            *audio = Some(pipeline);
        }
        Err(_) => counters.audio_lost = true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_message_is_cut_on_a_character_boundary() {
        let mut message = String::with_capacity(8);
        set_message(&mut message, "abcdé", 5);
        assert_eq!(message, "abcd");
        set_message(&mut message, "ok", 5);
        assert_eq!(message, "ok");
    }
}
