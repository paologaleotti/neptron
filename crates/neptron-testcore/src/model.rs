//! The state of a loaded test core and what it does in one frame.
//!
//! `Core::step` decides everything a `retro_run` does and returns it as a
//! [`Step`]. The FFI layer then makes the calls into the frontend. The step
//! owns the frame and audio buffers while the frontend reads them, and
//! [`Core::finish`] takes them back, so the core allocates no buffer after
//! load.

#![forbid(unsafe_code)]

use std::cell::Cell;
use std::mem;

use crate::audio::{frames_after, max_frames_per_run, sample};
use crate::frame::render;
use crate::script::{Action, AudioMode, AvInfo, Geometry, PixelFormat, Script, Size};

/// The byte that fills the frame buffer at load. The bytes of a row beyond
/// the frame width keep it until a wider frame covers them.
pub const PADDING_BYTE: u8 = 0xA5;

/// A call that the core makes into the frontend environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// `RETRO_ENVIRONMENT_SET_GEOMETRY`
    Geometry {
        /// The new geometry.
        geometry: Geometry,
        /// The maximum size in force. The frontend ignores it, but the
        /// struct carries it.
        max: Size,
    },
    /// `RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO`
    AvInfo(AvInfo),
    /// `RETRO_ENVIRONMENT_SHUTDOWN`
    Shutdown,
    /// `RETRO_ENVIRONMENT_GET_VARIABLE`. The answer code goes to the save
    /// RAM byte at `offset`.
    GetVariable {
        /// The index of the option in `CORE_OPTIONS`.
        option: usize,
        /// The save RAM offset in bytes.
        offset: u32,
    },
}

/// What the core sends to `video_refresh` in one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Video {
    /// The visible part of [`Step::frame_buffer`].
    Frame {
        /// The frame size.
        size: Size,
        /// Bytes from one row to the next.
        pitch: usize,
    },
    /// No data: the frontend shows the previous frame again.
    Dupe {
        /// The size of the previous frame.
        size: Size,
        /// Bytes from one row to the next.
        pitch: usize,
    },
    /// No call at all. Only a call of `retro_run` from inside a frontend
    /// callback gets this, because the buffers are then in use.
    Nothing,
}

/// Everything one `retro_run` does, in this order: the requests, the video,
/// the audio.
#[derive(Debug)]
pub struct Step {
    /// The frame number of this step.
    pub frame: u64,
    /// The joypad buttons that the frame shows.
    pub buttons: u16,
    /// Environment calls, in order.
    pub requests: Vec<Request>,
    /// The video call.
    pub video: Video,
    /// The frame buffer.
    pub frame_buffer: Vec<u8>,
    /// How the audio goes out.
    pub audio_mode: AudioMode,
    /// Interleaved stereo samples. Only the first `audio_frames` frames count.
    pub audio_buffer: Vec<i16>,
    /// Stereo frames to send.
    pub audio_frames: usize,
}

impl Step {
    /// The interleaved stereo samples to send.
    pub fn audio(&self) -> &[i16] {
        let len = self.audio_frames.saturating_mul(2);
        self.audio_buffer.get(..len).unwrap_or_default()
    }
}

/// The save RAM of the core.
///
/// The frontend reads and writes it through a raw pointer between calls into
/// the core. The bytes are `Cell`s, so the core changes them through a shared
/// reference only and never asserts unique access that the pointer would
/// break.
#[derive(Debug)]
pub struct SaveRam(Vec<Cell<u8>>);

impl SaveRam {
    /// Size in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when there is no save RAM.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Writes one byte. Returns false when `offset` is outside.
    pub fn write(&self, offset: usize, value: u8) -> bool {
        self.0.get(offset).map(|cell| cell.set(value)).is_some()
    }

    /// A copy of the bytes.
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.iter().map(Cell::get).collect()
    }

    /// A pointer to the first byte for the frontend, or null when empty. It
    /// stays valid while the `SaveRam` lives, because the vector never grows.
    /// It comes from a shared reference: the bytes are `Cell`s, so writes
    /// through it are allowed, and taking it again does not invalidate a
    /// pointer handed out before.
    pub fn as_ptr(&self) -> *mut u8 {
        if self.0.is_empty() {
            std::ptr::null_mut()
        } else {
            self.0.as_ptr().cast_mut().cast()
        }
    }
}

/// A loaded test core.
#[derive(Debug)]
pub struct Core {
    script: Script,
    can_dupe: bool,
    frame: u64,
    next_event: usize,
    av: AvInfo,
    size: Size,
    timing_start_frame: u64,
    next_sample: u64,
    last_frame_size: Option<Size>,
    announce_av: bool,
    pitch: usize,
    frame_buffer: Vec<u8>,
    audio_buffer: Vec<i16>,
    requests: Vec<Request>,
    save_ram: SaveRam,
}

impl Core {
    /// Makes the core for a script and allocates every buffer. `can_dupe` is
    /// the answer of the frontend to `GET_CAN_DUPE`.
    pub fn new(script: Script, can_dupe: bool) -> Self {
        let buffer = script.buffer_size();
        let pitch = to_usize(buffer.width) * script.pixel_format.bytes_per_pixel();
        let frame_buffer = vec![PADDING_BYTE; pitch * to_usize(buffer.height)];
        let audio_frames = script
            .events
            .iter()
            .filter_map(|e| match e.action {
                Action::AvInfo(av) => Some(av.timing),
                _ => None,
            })
            .chain([script.av.timing])
            .map(max_frames_per_run)
            .max()
            .unwrap_or(0);
        let requests = Vec::with_capacity(script.events.len() + 1);
        let save_ram = SaveRam(vec![Cell::new(0); to_usize(script.save_ram_bytes)]);
        Self {
            can_dupe,
            frame: 0,
            next_event: 0,
            av: script.av,
            size: script.av.geometry.size,
            timing_start_frame: 0,
            next_sample: 0,
            last_frame_size: None,
            announce_av: false,
            pitch,
            frame_buffer,
            audio_buffer: vec![0; audio_frames * 2],
            requests,
            save_ram,
            script,
        }
    }

    /// The pixel format of the script.
    pub fn pixel_format(&self) -> PixelFormat {
        self.script.pixel_format
    }

    /// Geometry, maximum size and timing as the core reports them now.
    pub fn av(&self) -> AvInfo {
        self.av
    }

    /// The number of the next frame.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// The save RAM.
    pub fn save_ram(&self) -> &SaveRam {
        &self.save_ram
    }

    /// Runs the script for one frame with the buttons the core read, and
    /// returns what the FFI layer must send. Hand the step back with
    /// [`Core::finish`].
    pub fn step(&mut self, buttons: u16) -> Step {
        let mut requests = mem::take(&mut self.requests);
        requests.clear();
        if mem::take(&mut self.announce_av) {
            requests.push(Request::AvInfo(self.av));
        }
        let dupe = self.run_events(&mut requests);
        let mut frame_buffer = mem::take(&mut self.frame_buffer);
        let video = self.video(&mut frame_buffer, buttons, dupe);
        let mut audio_buffer = mem::take(&mut self.audio_buffer);
        let audio_frames = self.audio(&mut audio_buffer);
        let step = Step {
            frame: self.frame,
            buttons,
            requests,
            video,
            frame_buffer,
            audio_mode: self.script.audio,
            audio_buffer,
            audio_frames,
        };
        self.frame = self.frame.saturating_add(1);
        step
    }

    /// Takes the buffers of a step back.
    pub fn finish(&mut self, step: Step) {
        if self.frame_buffer.is_empty() {
            self.frame_buffer = step.frame_buffer;
        }
        if self.audio_buffer.is_empty() {
            self.audio_buffer = step.audio_buffer;
        }
        if self.requests.capacity() == 0 {
            self.requests = step.requests;
        }
    }

    /// Starts the script again at frame 0. The save RAM stays. If the
    /// geometry or timing differ from the load values, the next step sends
    /// the load values with `SET_SYSTEM_AV_INFO`.
    pub fn reset(&mut self) {
        self.frame = 0;
        self.next_event = 0;
        self.timing_start_frame = 0;
        self.next_sample = 0;
        self.last_frame_size = None;
        self.size = self.script.av.geometry.size;
        if self.av != self.script.av {
            self.av = self.script.av;
            self.announce_av = true;
        }
    }

    /// Applies the events of this frame. Returns true when one is `dupe`.
    fn run_events(&mut self, requests: &mut Vec<Request>) -> bool {
        let mut dupe = false;
        while let Some(event) = self.script.events.get(self.next_event)
            && event.frame == self.frame
        {
            match event.action {
                Action::Geometry(g) => {
                    self.av.geometry = g;
                    self.size = g.size;
                    requests.push(Request::Geometry {
                        geometry: g,
                        max: self.av.max,
                    });
                }
                Action::AvInfo(av) => {
                    self.av = av;
                    self.size = av.geometry.size;
                    self.timing_start_frame = self.frame;
                    requests.push(Request::AvInfo(av));
                }
                Action::Size(size) => self.size = size,
                Action::Dupe => dupe = true,
                Action::SramWrite { offset, value } => {
                    // The parser checks the offset against the save RAM size.
                    self.save_ram.write(to_usize(offset), value);
                }
                Action::Shutdown => requests.push(Request::Shutdown),
                Action::GetVariable { option, offset } => {
                    requests.push(Request::GetVariable { option, offset });
                }
            }
            self.next_event += 1;
        }
        dupe
    }

    fn video(&mut self, buffer: &mut [u8], buttons: u16, dupe: bool) -> Video {
        if dupe && let Some(size) = self.last_frame_size {
            if self.can_dupe {
                return Video::Dupe {
                    size,
                    pitch: self.pitch,
                };
            }
            // The frontend cannot show a frame again: send the last one again.
            return Video::Frame {
                size,
                pitch: self.pitch,
            };
        }
        let format = self.script.pixel_format;
        match render(buffer, self.pitch, self.size, format, self.frame, buttons) {
            Some(()) => {
                self.last_frame_size = Some(self.size);
                Video::Frame {
                    size: self.size,
                    pitch: self.pitch,
                }
            }
            None => Video::Nothing,
        }
    }

    fn audio(&mut self, buffer: &mut [i16]) -> usize {
        if self.script.audio == AudioMode::Off {
            return 0;
        }
        let n = self.frame.saturating_sub(self.timing_start_frame);
        let due = frames_after(n.saturating_add(1), self.av.timing)
            .saturating_sub(frames_after(n, self.av.timing));
        let mut count: u64 = 0;
        for out in buffer.chunks_exact_mut(2).take(to_usize_u64(due)) {
            out.copy_from_slice(&sample(self.next_sample.wrapping_add(count)));
            count += 1;
        }
        self.next_sample = self.next_sample.wrapping_add(count);
        to_usize_u64(count)
    }
}

fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn to_usize_u64(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{expected_hash, hash};

    fn core(body: &str) -> Core {
        Core::new(
            Script::parse(&format!("neptron-testcore 1\n{body}")).unwrap(),
            true,
        )
    }

    fn run(core: &mut Core, buttons: u16) -> Step {
        let step = core.step(buttons);
        let copy = Step {
            frame: step.frame,
            buttons: step.buttons,
            requests: step.requests.clone(),
            video: step.video,
            frame_buffer: step.frame_buffer.clone(),
            audio_mode: step.audio_mode,
            audio_buffer: step.audio_buffer.clone(),
            audio_frames: step.audio_frames,
        };
        core.finish(step);
        copy
    }

    fn frame_hash(step: &Step, format: PixelFormat) -> Option<u64> {
        match step.video {
            Video::Frame { size, pitch } => hash(&step.frame_buffer, pitch, size, format),
            _ => None,
        }
    }

    const QVGA: Size = Size {
        width: 320,
        height: 240,
    };

    #[test]
    fn frames_have_the_expected_hash() {
        let mut c = core("max_size 400 240\n");
        for (frame, buttons) in [(0, 0), (1, 0b1001), (2, 0xFFFF)] {
            let step = run(&mut c, buttons);
            assert_eq!(step.frame, frame);
            assert_eq!(
                step.video,
                Video::Frame {
                    size: QVGA,
                    pitch: 1600
                }
            );
            assert_eq!(
                frame_hash(&step, PixelFormat::Xrgb8888),
                expected_hash(QVGA, PixelFormat::Xrgb8888, frame, buttons)
            );
        }
        assert_eq!(c.frame(), 3);
    }

    #[test]
    fn geometry_events_make_requests_and_resize() {
        let mut c = core(
            "geometry 256 224 0\nmax_size 320 240\n\
             at 1 geometry 320 240 1333\n\
             at 2 size 256 240\n\
             at 3 av_info 640 480 0 640 480 50000 48000\nat 3 shutdown\n",
        );
        let s0 = run(&mut c, 0);
        assert!(s0.requests.is_empty());
        let s1 = run(&mut c, 0);
        let g = Geometry {
            size: QVGA,
            aspect_milli: 1333,
        };
        assert_eq!(
            s1.requests,
            [Request::Geometry {
                geometry: g,
                max: QVGA
            }]
        );
        assert!(matches!(s1.video, Video::Frame { size: QVGA, .. }));
        assert_eq!(c.av().geometry, g);
        let s2 = run(&mut c, 0);
        assert!(
            s2.requests.is_empty(),
            "a size change tells the frontend nothing"
        );
        assert!(matches!(
            s2.video,
            Video::Frame {
                size: Size {
                    width: 256,
                    height: 240
                },
                ..
            }
        ));
        let s3 = run(&mut c, 0);
        let av = c.av();
        assert_eq!(s3.requests, [Request::AvInfo(av), Request::Shutdown]);
        assert_eq!(av.timing.fps_mhz, 50_000);
        assert!(matches!(
            s3.video,
            Video::Frame {
                size: Size {
                    width: 640,
                    height: 480
                },
                pitch: 2560
            }
        ));
        assert_eq!(
            frame_hash(&s3, PixelFormat::Xrgb8888),
            expected_hash(av.geometry.size, PixelFormat::Xrgb8888, 3, 0)
        );
    }

    #[test]
    fn dupe_sends_no_data_or_the_last_frame() {
        let mut c = core("at 0 dupe\nat 2 dupe\n");
        assert_eq!(
            run(&mut c, 0).video,
            Video::Frame {
                size: QVGA,
                pitch: 1280
            },
            "nothing to repeat yet"
        );
        assert!(matches!(run(&mut c, 0).video, Video::Frame { .. }));
        assert_eq!(
            run(&mut c, 0).video,
            Video::Dupe {
                size: QVGA,
                pitch: 1280
            }
        );

        let mut no_dupe = Core::new(
            Script::parse("neptron-testcore 1\nat 1 dupe\n").unwrap(),
            false,
        );
        let first = run(&mut no_dupe, 7);
        let second = run(&mut no_dupe, 0);
        assert_eq!(second.video, first.video);
        assert_eq!(
            frame_hash(&second, PixelFormat::Xrgb8888),
            expected_hash(QVGA, PixelFormat::Xrgb8888, 0, 7),
            "the frontend gets frame 0 again"
        );
    }

    #[test]
    fn audio_follows_the_rate_and_the_pattern() {
        let mut c = core(
            "timing 59940 44100\n\
             at 2 av_info 320 240 0 320 240 50000 48000\n",
        );
        let a = run(&mut c, 0);
        assert_eq!(a.audio_frames, 735);
        assert_eq!(&a.audio()[..4], [0, 0, 1, -1]);
        let b = run(&mut c, 0);
        assert_eq!(b.audio_frames, 736);
        assert_eq!(b.audio()[..2], sample(735));
        let c2 = run(&mut c, 0);
        assert_eq!(c2.audio_frames, 960, "the new rate starts at the event");
        assert_eq!(c2.audio()[..2], sample(735 + 736));
    }

    #[test]
    fn audio_off_sends_nothing() {
        let mut c = core("audio off\n");
        let s = run(&mut c, 0);
        assert_eq!(s.audio_frames, 0);
        assert!(s.audio().is_empty());
        assert_eq!(s.audio_mode, AudioMode::Off);
    }

    #[test]
    fn audio_buffer_fits_the_fastest_rate() {
        let c = core("timing 60000 22050\nat 5 av_info 320 240 0 320 240 50000 96000\n");
        assert_eq!(c.audio_buffer.len(), 2 * 1920);
    }

    #[test]
    fn get_variable_becomes_a_request_at_its_frame() {
        let mut c = core("save_ram 2\nat 1 get_variable neptron_testcore_mode 1\n");
        assert!(run(&mut c, 0).requests.is_empty());
        assert_eq!(
            run(&mut c, 0).requests,
            [Request::GetVariable {
                option: 0,
                offset: 1
            }]
        );
    }

    #[test]
    fn save_ram_writes_happen_at_their_frame() {
        let mut c = core("save_ram 4\nat 1 sram_write 3 42\n");
        assert_eq!(c.save_ram().len(), 4);
        run(&mut c, 0);
        assert_eq!(c.save_ram().to_vec(), [0, 0, 0, 0]);
        run(&mut c, 0);
        assert_eq!(c.save_ram().to_vec(), [0, 0, 0, 42]);
        assert!(!c.save_ram().write(4, 1));
        assert!(!c.save_ram().as_ptr().is_null());
        assert_eq!(c.save_ram().as_ptr(), c.save_ram().as_ptr());
        assert!(core("").save_ram().as_ptr().is_null());
    }

    #[test]
    fn reset_restarts_the_script_and_announces_the_load_values() {
        let mut c =
            core("save_ram 1\nat 1 av_info 320 240 0 320 240 50000 48000\nat 1 sram_write 0 9\n");
        let load_av = c.av();
        run(&mut c, 0);
        run(&mut c, 0);
        assert_ne!(c.av(), load_av);
        c.reset();
        assert_eq!(c.av(), load_av);
        assert_eq!(c.frame(), 0);
        let s = run(&mut c, 0);
        assert_eq!(s.requests, [Request::AvInfo(load_av)]);
        assert_eq!(s.frame, 0);
        assert_eq!(&s.audio()[..2], sample(0));
        assert_eq!(c.save_ram().to_vec(), [9], "reset keeps the save RAM");
        let s = run(&mut c, 0);
        assert_eq!(s.requests.len(), 1, "the script runs again");

        let mut plain = core("");
        run(&mut plain, 0);
        plain.reset();
        assert!(run(&mut plain, 0).requests.is_empty());
    }

    #[test]
    fn a_step_without_buffers_sends_nothing() {
        let mut c = core("");
        let outer = c.step(0);
        let inner = c.step(0);
        assert_eq!(inner.video, Video::Nothing);
        assert_eq!(inner.audio_frames, 0);
        c.finish(inner);
        c.finish(outer);
        assert!(matches!(run(&mut c, 0).video, Video::Frame { .. }));
    }

    #[test]
    fn sixteen_bit_formats_use_a_two_byte_pitch() {
        let mut c = core("pixel_format 0rgb1555\ngeometry 256 224 0\n");
        assert_eq!(c.pixel_format(), PixelFormat::Rgb1555);
        let s = run(&mut c, 0);
        assert_eq!(
            s.video,
            Video::Frame {
                size: Size {
                    width: 256,
                    height: 224
                },
                pitch: 512
            }
        );
    }
}
