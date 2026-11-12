//! One frame of the display-locked loop.

use neptron_audio::{AudioPipeline, CoreTiming, Resampling};
use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::Hotkey;
use neptron_libretro::{Event, RunError, Timing, Video};
use neptron_types::Frame;
use neptron_video::{Directive, VideoError};

use super::{FLIPLESS_FRAMES_LIMIT, Session, State};
use crate::pads::CORE_PORTS;
use crate::save::SAVE_RAM_FLUSH_INTERVAL_NS;
use crate::source::source_geometry;
use crate::{Counters, EndReason, FrameReport, RunRateSource, SessionError};

impl<D: Display, S: AudioSink, I: InputSource, C: Clock> Session<D, S, I, C> {
    /// Runs one frame of the loop, with `ui` drawn over the game (the
    /// 320x240 canvas of the quick menu or a notice, or `None`).
    ///
    /// 1. Polls the input once, updates the ports and the hotkeys. Select
    ///    and Start together, or the Start hold, open the quick menu, which
    ///    pauses the core. A second quick menu hotkey while it is open does
    ///    nothing: the menu is never a toggle; [`Session::resume`] closes it.
    /// 2. Runs one core frame, unless the menu is open.
    /// 3. Gives the engine the source, then presents the frame and waits for
    ///    its flip: once, or twice for double refresh. The audio is muted
    ///    while the engine asks for it, while the menu is open and while the
    ///    inputs refuse the mode on screen.
    /// 4. Ends the core frame of the audio pipeline, at the run rate of the
    ///    mode on screen.
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
    /// frame are in its report.
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
        };
        self.read_input(now_ns, &mut report);

        if self.state == State::Running {
            for port in 0..CORE_PORTS {
                let pad = self.mask.pad(&self.ports, port);
                // `port` is below the host's port count.
                let _ = self.host.set_pad(port, pad);
            }
            self.run_core(true).map_err(|e| self.fail(e))?;
            self.counters.core_frames += 1;
            report.ran_core = true;
            if self.state == State::Ended {
                report.ended = Some(EndReason::CoreShutdown);
            }
        }
        self.counters.loop_frames += 1;

        report.directive = self.engine.begin_frame(self.source);
        // The audio of the frame goes out before the flip wait, so the queue
        // is read at the same point of every frame. A modeset frame writes
        // silence, which covers the wait for the first flip of the new mode.
        let mute = self.must_mute();
        if let Some(audio) = &mut self.audio {
            if mute {
                audio.mute();
            }
            if report.ran_core {
                audio.end_frame();
            }
        }
        self.show(ui, &mut report);
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
    /// and, with `feed_audio`, the audio.
    pub(super) fn run_core(&mut self, feed_audio: bool) -> Result<(), RunError> {
        let out = self.host.run_frame()?;
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
                Event::PixelFormat(_) | Event::Refused(_) => {}
            }
        }
        match out.video {
            Video::Frame(frame) => self.picture.store(&frame),
            Video::Duplicate(_) | Video::Missing => counters.repeated_pictures += 1,
            Video::Rejected(_) => counters.frame_faults += 1,
        }
        if let Some(audio) = self.audio.as_mut().filter(|_| feed_audio) {
            audio.push_samples(out.audio);
        }
        let (width, height) = self
            .picture
            .size()
            .unwrap_or((self.geometry.base_width, self.geometry.base_height));
        self.source = source_geometry(width, height, &self.geometry, &self.timing);
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
    /// off from the menu.
    fn pause(&mut self) {
        self.state = State::Paused;
        if let Some(audio) = &mut self.audio {
            audio.mute();
        }
        self.flush_save_ram();
    }

    fn must_mute(&self) -> bool {
        self.state != State::Running || self.engine.muted() || self.engine.current_refused()
    }

    /// Presents the picture and waits for the flip, once per flip of a core
    /// frame. A display error ends the frame: the engine keeps a timed-out
    /// flip queued and drains it at the next present.
    fn show(&mut self, ui: Option<Frame<'_>>, report: &mut FrameReport) {
        loop {
            // The inputs refuse the mode on screen: no game picture is shown
            // on it as if it were confirmed.
            let game = if self.engine.current_refused() {
                None
            } else {
                self.picture.frame()
            };
            match self.engine.present(game, ui) {
                Ok(presented) => {
                    if let Some(flip) = presented.drained_flip {
                        self.meter.push(flip.time_ns);
                    }
                    if presented.modeset.is_some() {
                        self.meter.reset();
                        report.modeset = presented.modeset;
                    }
                }
                Err(error) => {
                    report.display_error = Some(error);
                    return;
                }
            }
            match self.engine.wait_flip() {
                Ok(flip) => {
                    self.meter.push(flip.time_ns);
                    report.flips += 1;
                    self.counters.flips += 1;
                }
                Err(error) => {
                    report.display_error = Some(error);
                    return;
                }
            }
            if report.flips >= self.flips_per_frame() {
                return;
            }
        }
    }

    /// Flips per core frame of the mode on screen: 2 for double refresh.
    fn flips_per_frame(&self) -> u8 {
        self.engine
            .current()
            .map_or(1, |c| c.chosen.proposal.timing.repeat().max(1))
    }

    fn update_run_rate(&mut self) {
        let flips = u64::from(self.flips_per_frame());
        let flip_rate = match self.run_rate {
            RunRateSource::ReadBack => self.engine.current().map(|c| c.read_back.field_rate_mhz()),
            RunRateSource::Measured => self.meter.rate_mhz(),
        };
        let Some(run_rate) = flip_rate.map(|r| r / flips) else {
            return;
        };
        let Some(audio) = &mut self.audio else {
            return;
        };
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

    fn flush_save_ram(&mut self) {
        let host = &mut self.host;
        if let Some(writer) = &mut self.writer {
            writer.flush(|out| host.copy_save_ram(out));
        }
    }

    fn check_stall(&mut self, report: &FrameReport) -> Result<(), SessionError> {
        if report.flips > 0 {
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
    match AudioPipeline::new(old.into_sink(), core, run_rate, resampling) {
        Ok(pipeline) => *audio = Some(pipeline),
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
