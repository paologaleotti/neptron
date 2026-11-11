//! The state that the callbacks of the core read and write. The `ffi`
//! module turns each raw callback argument into a plain value and calls a
//! method here, which decides.

use std::ffi::{CStr, CString};

use neptron_types::PixelFormat;

use crate::audio::AudioBuffer;
use crate::geometry::{AvInfo, GameGeometry, Timing};
use crate::log::{LogLevel, LogQueue, LogRecord, LogSource};
use crate::options::{CoreOption, OptionError, OptionStore};
use crate::output::{Dropped, Event, FrameFault, FrameOutput, Refusal};
use crate::pad::{MAX_PORTS, Pad};
use crate::video::{FramePlan, VideoBuffer, pixel_format_from_core};

/// The most events kept in one frame.
pub const EVENT_CAPACITY: usize = 32;
/// The most environment command numbers that get their one log record.
pub const MAX_LOGGED_COMMANDS: usize = 64;
/// The longest `SET_MESSAGE` text kept, in bytes.
pub const MESSAGE_MAX_BYTES: usize = 1024;

/// The fields of a `retro_system_av_info`, as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CoreAvInfo {
    pub base_width: u32,
    pub base_height: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub aspect: f32,
    pub fps: f64,
    pub sample_rate: f64,
}

impl CoreAvInfo {
    pub(crate) fn convert(&self) -> Result<AvInfo, crate::GeometryError> {
        Ok(AvInfo {
            geometry: GameGeometry::from_core(
                self.base_width,
                self.base_height,
                self.max_width,
                self.max_height,
                self.aspect,
            )?,
            timing: Timing::from_core(self.fps, self.sample_rate)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct State {
    pixel_format: PixelFormat,
    av: Option<AvInfo>,
    supports_no_game: bool,
    system_dir: CString,
    save_dir: CString,
    pads: [Pad; MAX_PORTS],
    video: VideoBuffer,
    audio: AudioBuffer,
    events: Vec<Event>,
    dropped_events: usize,
    /// The queued events went out with a frame; the next call clears them.
    events_delivered: bool,
    logs: LogQueue,
    logged_commands: Vec<u32>,
    in_run: bool,
    options: OptionStore,
}

impl State {
    pub(crate) fn new(system_dir: CString, save_dir: CString) -> Self {
        Self {
            pixel_format: PixelFormat::Xrgb1555,
            av: None,
            supports_no_game: false,
            system_dir,
            save_dir,
            pads: [Pad::default(); MAX_PORTS],
            video: VideoBuffer::new(),
            audio: AudioBuffer::new(),
            events: Vec::with_capacity(EVENT_CAPACITY),
            dropped_events: 0,
            events_delivered: false,
            logs: LogQueue::new(),
            logged_commands: Vec::with_capacity(MAX_LOGGED_COMMANDS),
            in_run: false,
            options: OptionStore::default(),
        }
    }

    /// Starts a public call of the host: the log shows that call only.
    pub(crate) fn begin_call(&mut self) {
        self.logs.clear();
    }

    /// Starts a call that is not a frame (open, `load_game`, `reset`): its
    /// events wait for the next frame.
    pub(crate) fn begin_outside_run(&mut self) {
        self.clear_delivered_events();
    }

    pub(crate) fn begin_run(&mut self) {
        self.clear_delivered_events();
        self.audio.clear();
        self.video.begin();
        self.in_run = true;
    }

    pub(crate) fn end_run(&mut self) {
        self.in_run = false;
        self.events_delivered = true;
    }

    fn clear_delivered_events(&mut self) {
        if self.events_delivered {
            self.events.clear();
            self.dropped_events = 0;
            self.events_delivered = false;
        }
    }

    /// Takes the AV info read after `retro_load_game` and makes the buffers.
    /// The error is the number of bytes that could not be allocated.
    pub(crate) fn set_loaded_av(&mut self, av: AvInfo) -> Result<(), usize> {
        self.video.reserve_for(&av.geometry)?;
        self.audio.reserve_for(&av.timing)?;
        self.av = Some(av);
        Ok(())
    }

    pub(crate) fn output(&self) -> FrameOutput<'_> {
        FrameOutput {
            video: self.video.video(),
            audio: self.audio.samples(),
            events: &self.events,
            logs: self.logs.records(),
            dropped: Dropped {
                audio_frames: self.audio.dropped_frames(),
                events: self.dropped_events,
                log_records: self.logs.dropped(),
            },
        }
    }

    pub(crate) fn set_pad(&mut self, port: usize, pad: Pad) -> bool {
        match self.pads.get_mut(port) {
            Some(slot) => {
                *slot = pad;
                true
            }
            None => false,
        }
    }

    pub(crate) fn av(&self) -> Option<AvInfo> {
        self.av
    }

    pub(crate) fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub(crate) fn supports_no_game(&self) -> bool {
        self.supports_no_game
    }

    pub(crate) fn logs(&self) -> &[LogRecord] {
        self.logs.records()
    }

    pub(crate) fn options(&self) -> &[CoreOption] {
        self.options.options()
    }

    pub(crate) fn set_option(&mut self, key: &str, value: &str) -> Result<(), OptionError> {
        self.options.set(key, value)
    }

    /// `SET_VARIABLES`: replaces the declared options and logs each entry
    /// that was skipped.
    pub(crate) fn declare_options(&mut self, entries: &[(&[u8], &[u8])]) {
        for (key, reason) in self.options.declare(entries) {
            self.logs.push_fmt(
                LogLevel::Warn,
                LogSource::Host,
                format_args!(
                    "core option {} skipped: {reason}",
                    String::from_utf8_lossy(key)
                ),
            );
        }
    }

    /// `GET_VARIABLE`: the current value of a declared option.
    pub(crate) fn option_value(&self, key: &[u8]) -> Option<&CStr> {
        self.options.value(key)
    }

    /// `GET_VARIABLE_UPDATE`: true once after a value changed.
    pub(crate) fn take_option_update(&mut self) -> bool {
        self.options.take_update()
    }

    pub(crate) fn system_dir(&self) -> &CStr {
        &self.system_dir
    }

    pub(crate) fn save_dir(&self) -> &CStr {
        &self.save_dir
    }

    pub(crate) fn set_support_no_game(&mut self, supported: bool) {
        self.supports_no_game = supported;
    }

    pub(crate) fn set_pixel_format(&mut self, value: u32) -> bool {
        let Some(format) = pixel_format_from_core(value) else {
            self.push_event(Event::Refused(Refusal::PixelFormat { value }));
            return false;
        };
        if self.in_run && format != self.pixel_format {
            self.push_event(Event::PixelFormat(format));
        }
        self.pixel_format = format;
        true
    }

    /// `SET_GEOMETRY`: a new base size and aspect inside the maximum.
    pub(crate) fn set_geometry(&mut self, cmd: u32, width: u32, height: u32, aspect: f32) -> bool {
        let Some(av) = self.av.as_mut() else {
            self.before_load(cmd);
            return true;
        };
        match av.geometry.resized(width, height, aspect) {
            Ok(geometry) => {
                av.geometry = geometry;
                self.push_event(Event::Geometry(geometry));
                true
            }
            Err(e) => {
                self.push_event(Event::Refused(Refusal::Geometry(e)));
                false
            }
        }
    }

    /// `SET_SYSTEM_AV_INFO`: new geometry and timing, with larger buffers
    /// when the maximum or the rate grew.
    pub(crate) fn set_av_info(&mut self, cmd: u32, raw: &CoreAvInfo) -> bool {
        if self.av.is_none() {
            self.before_load(cmd);
            return true;
        }
        let av = match raw.convert() {
            Ok(av) => av,
            Err(e) => {
                self.push_event(Event::Refused(Refusal::AvInfo(e)));
                return false;
            }
        };
        let grown = self
            .video
            .reserve_for(&av.geometry)
            .and_then(|()| self.audio.reserve_for(&av.timing));
        if let Err(bytes) = grown {
            self.push_event(Event::Refused(Refusal::OutOfMemory { bytes }));
            return false;
        }
        self.av = Some(av);
        self.push_event(Event::AvInfo(av));
        true
    }

    pub(crate) fn shutdown(&mut self) {
        self.push_event(Event::Shutdown);
    }

    pub(crate) fn message(&mut self, text: &[u8], frames: u32) {
        let mut cut = text.len().min(MESSAGE_MAX_BYTES);
        let text = loop {
            match std::str::from_utf8(text.get(..cut).unwrap_or_default()) {
                Ok(s) => break s.to_owned(),
                // Cut inside a character: step back to its start.
                Err(e) if e.error_len().is_none() => cut = e.valid_up_to(),
                Err(_) => {
                    break String::from_utf8_lossy(text.get(..cut).unwrap_or_default())
                        .into_owned();
                }
            }
        };
        self.push_event(Event::Message { text, frames });
    }

    /// A known call that the host does not support: `false`, logged once.
    pub(crate) fn refuse(&mut self, cmd: u32, reason: &str) -> bool {
        if self.first_time(cmd) {
            self.logs.push_fmt(
                LogLevel::Info,
                LogSource::Host,
                format_args!("environment call {cmd:#x} refused: {reason}"),
            );
        }
        false
    }

    /// A call that the host does not know: `false`, logged once.
    pub(crate) fn unknown(&mut self, cmd: u32) -> bool {
        if self.first_time(cmd) {
            self.logs.push_fmt(
                LogLevel::Warn,
                LogSource::Host,
                format_args!("unknown environment call {cmd:#x}"),
            );
        }
        false
    }

    pub(crate) fn input(&self, port: u32, device: u32, index: u32, id: u32) -> i16 {
        usize::try_from(port)
            .ok()
            .and_then(|p| self.pads.get(p))
            .map_or(0, |pad| pad.state(device, index, id))
    }

    pub(crate) fn core_log(&mut self, level: u32, text: &[u8]) {
        self.logs
            .push_bytes(LogLevel::from_core(level), LogSource::Core, text);
    }

    pub(crate) fn frame_plan(
        &self,
        width: u32,
        height: u32,
        pitch: usize,
    ) -> Result<FramePlan, FrameFault> {
        FramePlan::check(
            width,
            height,
            pitch,
            self.pixel_format,
            self.av.as_ref().map(|av| &av.geometry),
        )
    }

    pub(crate) fn video_mut(&mut self) -> &mut VideoBuffer {
        &mut self.video
    }

    pub(crate) fn audio_mut(&mut self) -> &mut AudioBuffer {
        &mut self.audio
    }

    fn before_load(&mut self, cmd: u32) {
        if self.first_time(cmd) {
            self.logs.push_fmt(
                LogLevel::Debug,
                LogSource::Host,
                format_args!(
                    "environment call {cmd:#x} before the AV info is read: ignored, the load reads it"
                ),
            );
        }
    }

    fn push_event(&mut self, event: Event) {
        if self.events.len() < EVENT_CAPACITY {
            self.events.push(event);
        } else {
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
    }

    /// True the first time `cmd` comes here, for up to
    /// [`MAX_LOGGED_COMMANDS`] numbers.
    fn first_time(&mut self, cmd: u32) -> bool {
        if self.logged_commands.contains(&cmd) || self.logged_commands.len() >= MAX_LOGGED_COMMANDS
        {
            return false;
        }
        self.logged_commands.push(cmd);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::GeometryError;
    use crate::output::Video;

    fn state() -> State {
        State::new(c"/bios".to_owned(), c"/saves".to_owned())
    }

    const AV: CoreAvInfo = CoreAvInfo {
        base_width: 256,
        base_height: 224,
        max_width: 320,
        max_height: 240,
        aspect: 0.0,
        fps: 60.0,
        sample_rate: 48_000.0,
    };

    fn loaded() -> State {
        let mut s = state();
        s.set_loaded_av(AV.convert().unwrap()).unwrap();
        s
    }

    #[test]
    fn geometry_before_the_load_is_ignored_and_logged_once() {
        let mut s = state();
        s.begin_call();
        assert!(s.set_geometry(37, 320, 240, 0.0));
        assert!(s.set_geometry(37, 320, 240, 0.0));
        assert!(s.set_av_info(32, &AV));
        assert_eq!(s.av(), None);
        assert_eq!(s.logs().len(), 2, "one record per command");
        assert!(s.output().events.is_empty());
    }

    #[test]
    fn geometry_changes_become_events() {
        let mut s = loaded();
        s.begin_run();
        assert!(s.set_geometry(37, 320, 240, 4.0 / 3.0));
        assert!(!s.set_geometry(37, 321, 240, 0.0));
        let bigger = CoreAvInfo {
            max_width: 640,
            max_height: 480,
            base_width: 640,
            base_height: 480,
            fps: 50.0,
            ..AV
        };
        assert!(s.set_av_info(32, &bigger));
        let bad = CoreAvInfo {
            fps: f64::NAN,
            ..AV
        };
        assert!(!s.set_av_info(32, &bad));
        let geometry = GameGeometry {
            base_width: 320,
            base_height: 240,
            max_width: 320,
            max_height: 240,
            aspect_milli: 1333,
        };
        let out = s.output();
        assert_eq!(out.events.len(), 4);
        assert_eq!(out.events[0], Event::Geometry(geometry));
        assert!(matches!(
            out.events[1],
            Event::Refused(Refusal::Geometry(GeometryError::BaseOverMax { .. }))
        ));
        assert_eq!(out.events[2], Event::AvInfo(bigger.convert().unwrap()));
        assert!(matches!(
            out.events[3],
            Event::Refused(Refusal::AvInfo(GeometryError::FrameRate { .. }))
        ));
        assert_eq!(s.av(), Some(bigger.convert().unwrap()));
    }

    #[test]
    fn pixel_format_changes_in_a_run_are_events() {
        let mut s = loaded();
        assert!(s.set_pixel_format(1));
        assert_eq!(s.pixel_format(), PixelFormat::Xrgb8888);
        s.begin_run();
        assert!(s.set_pixel_format(1), "no change, no event");
        assert!(s.set_pixel_format(2));
        assert!(!s.set_pixel_format(9));
        assert_eq!(
            s.output().events,
            [
                Event::PixelFormat(PixelFormat::Rgb565),
                Event::Refused(Refusal::PixelFormat { value: 9 })
            ]
        );
        assert_eq!(s.pixel_format(), PixelFormat::Rgb565);
    }

    #[test]
    fn unknown_and_refused_calls_are_logged_once_each() {
        let mut s = loaded();
        s.begin_call();
        for _ in 0..3 {
            assert!(!s.unknown(0x24E54));
            assert!(!s.refuse(1, "rotation is not supported"));
        }
        let logs = s.logs();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].level, LogLevel::Warn);
        assert_eq!(logs[0].text, "unknown environment call 0x24e54");
        assert_eq!(logs[1].level, LogLevel::Info);
        assert_eq!(
            logs[1].text,
            "environment call 0x1 refused: rotation is not supported"
        );
    }

    #[test]
    fn logging_stops_after_the_command_limit() {
        let mut s = loaded();
        s.begin_call();
        for cmd in 0..(MAX_LOGGED_COMMANDS as u32 + 10) {
            s.unknown(1000 + cmd);
        }
        assert_eq!(s.logs().len(), MAX_LOGGED_COMMANDS);
    }

    #[test]
    fn events_beyond_the_capacity_are_counted() {
        let mut s = loaded();
        s.begin_run();
        for _ in 0..EVENT_CAPACITY + 2 {
            s.shutdown();
        }
        let out = s.output();
        assert_eq!(out.events.len(), EVENT_CAPACITY);
        assert_eq!(out.dropped.events, 2);
        s.end_run();
        s.begin_run();
        assert_eq!(s.output().dropped.events, 0);
    }

    #[test]
    fn messages_are_cut_at_a_character_boundary() {
        let mut s = loaded();
        s.begin_run();
        s.message(b"Saved", 180);
        let long = "é".repeat(MESSAGE_MAX_BYTES);
        let mut odd = vec![b'a'];
        odd.extend_from_slice(long.as_bytes());
        s.message(&odd, 1);
        s.message(b"bad \xFF byte", 2);
        let out = s.output();
        assert_eq!(
            out.events[0],
            Event::Message {
                text: "Saved".to_owned(),
                frames: 180
            }
        );
        let Event::Message { text, .. } = &out.events[1] else {
            panic!("not a message")
        };
        assert_eq!(text.len(), MESSAGE_MAX_BYTES - 1);
        assert_eq!(
            out.events[2],
            Event::Message {
                text: "bad \u{FFFD} byte".to_owned(),
                frames: 2
            }
        );
    }

    #[test]
    fn input_reads_the_pads_and_nothing_beyond() {
        let mut s = loaded();
        let pad = Pad {
            buttons: 1 << 3,
            ..Pad::default()
        };
        assert!(s.set_pad(1, pad));
        assert!(!s.set_pad(MAX_PORTS, pad));
        assert_eq!(s.input(1, 1, 0, 3), 1);
        assert_eq!(s.input(0, 1, 0, 3), 0);
        assert_eq!(s.input(MAX_PORTS as u32, 1, 0, 3), 0);
        assert_eq!(s.input(u32::MAX, 1, 0, 3), 0);
    }

    #[test]
    fn a_run_starts_empty() {
        let mut s = loaded();
        s.begin_run();
        s.audio_mut().push(&[1, 2]);
        s.video_mut().duplicate();
        s.begin_run();
        let out = s.output();
        assert!(out.audio.is_empty());
        assert_eq!(out.video, Video::Missing);
    }

    #[test]
    fn events_outside_a_frame_wait_for_the_next_frame() {
        let mut s = loaded();
        s.begin_outside_run();
        s.shutdown();
        s.begin_run();
        s.message(b"in the frame", 1);
        s.end_run();
        assert_eq!(s.output().events.len(), 2, "the load's event comes first");
        s.begin_outside_run();
        assert!(s.output().events.is_empty(), "delivered events go");
        s.shutdown();
        s.begin_run();
        s.end_run();
        assert_eq!(s.output().events, [Event::Shutdown]);
    }
}
