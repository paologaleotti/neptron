//! A fake core made of Rust `extern "C"` functions, for tests that need a
//! core to misbehave: a wrong API version, hostile frames and audio,
//! environment calls with bad data. The host drives it through the same
//! entry point table as a loaded library.
//!
//! The fake keeps its state in statics, so each test needs its own process,
//! which `cargo nextest` gives.

use std::ffi::{CStr, c_char, c_uint};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use neptron_libretro_sys::*;
use neptron_types::PixelFormat;

use super::{Core, EntryPoints};
use crate::host::Host;
use crate::state::State;
use crate::{
    ContentRequest, Event, FrameFault, LoadError, LogLevel, LogSource, OpenError, Pad, Refusal,
    RunError, Video,
};

/// The callbacks that the host gave the fake.
#[derive(Clone, Copy)]
pub(crate) struct Callbacks {
    environment: retro_environment_t,
    video_refresh: retro_video_refresh_t,
    audio_sample: retro_audio_sample_t,
    audio_sample_batch: retro_audio_sample_batch_t,
    input_poll: retro_input_poll_t,
    input_state: retro_input_state_t,
}

type Hook = fn(&Callbacks);

/// What the fake does.
struct Script {
    api_version: c_uint,
    need_fullpath: bool,
    extensions: &'static CStr,
    load_result: bool,
    av: retro_system_av_info,
    on_set_environment: Option<Hook>,
    on_load: Option<Hook>,
    on_run: Option<Hook>,
    on_reset: Option<Hook>,
}

/// What the fake saw.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Seen {
    pub calls: Vec<String>,
    pub game_was_null: bool,
    pub path: Option<String>,
    pub data: Option<Vec<u8>>,
}

struct Fake {
    callbacks: Callbacks,
    script: Script,
    seen: Seen,
}

static FAKE: Mutex<Fake> = Mutex::new(Fake {
    callbacks: Callbacks {
        environment: None,
        video_refresh: None,
        audio_sample: None,
        audio_sample_batch: None,
        input_poll: None,
        input_state: None,
    },
    script: Script {
        api_version: RETRO_API_VERSION,
        need_fullpath: false,
        extensions: c"NES|fds",
        load_result: true,
        av: retro_system_av_info {
            geometry: retro_game_geometry {
                base_width: 256,
                base_height: 224,
                max_width: 320,
                max_height: 240,
                aspect_ratio: 0.0,
            },
            timing: retro_system_timing {
                fps: 60.0,
                sample_rate: 48_000.0,
            },
        },
        on_set_environment: None,
        on_load: None,
        on_run: None,
        on_reset: None,
    },
    seen: Seen {
        calls: Vec::new(),
        game_was_null: false,
        path: None,
        data: None,
    },
});

fn fake() -> MutexGuard<'static, Fake> {
    FAKE.lock().unwrap_or_else(|e| e.into_inner())
}

fn seen() -> Seen {
    fake().seen.clone()
}

fn record(call: &str) {
    fake().seen.calls.push(call.to_owned());
}

/// Runs a hook with the lock released, so the callbacks can run.
fn run_hook(pick: fn(&Script) -> Option<Hook>) {
    let (hook, callbacks) = {
        let f = fake();
        (pick(&f.script), f.callbacks)
    };
    if let Some(hook) = hook {
        hook(&callbacks);
    }
}

impl Callbacks {
    pub(crate) fn env<T>(&self, cmd: u32, data: *mut T) -> bool {
        // SAFETY: the host's environment callback; each test passes the
        // type of `cmd` or null on purpose.
        unsafe { (self.environment.unwrap())(cmd, data.cast()) }
    }

    pub(crate) fn video(&self, data: Option<&[u8]>, width: u32, height: u32, pitch: usize) {
        let ptr = data.map_or(std::ptr::null(), |d| d.as_ptr().cast());
        // SAFETY: the host's video callback; `data` holds the frame.
        unsafe { (self.video_refresh.unwrap())(ptr, width, height, pitch) }
    }

    pub(crate) fn audio(&self, data: *const i16, frames: usize) -> usize {
        // SAFETY: the host's audio callback; `data` holds `frames` frames or
        // is a bad pointer that the host must refuse without reading.
        unsafe { (self.audio_sample_batch.unwrap())(data, frames) }
    }

    pub(crate) fn sample(&self, left: i16, right: i16) {
        // SAFETY: the host's single-sample callback.
        unsafe { (self.audio_sample.unwrap())(left, right) }
    }

    pub(crate) fn input(&self, port: u32, device: u32, index: u32, id: u32) -> i16 {
        // SAFETY: the host's input callbacks, with plain integers.
        unsafe {
            (self.input_poll.unwrap())();
            (self.input_state.unwrap())(port, device, index, id)
        }
    }
}

extern "C" fn set_environment(cb: retro_environment_t) {
    record("set_environment");
    fake().callbacks.environment = cb;
    run_hook(|s| s.on_set_environment);
}
extern "C" fn set_video_refresh(cb: retro_video_refresh_t) {
    record("set_video_refresh");
    fake().callbacks.video_refresh = cb;
}
extern "C" fn set_audio_sample(cb: retro_audio_sample_t) {
    fake().callbacks.audio_sample = cb;
}
extern "C" fn set_audio_sample_batch(cb: retro_audio_sample_batch_t) {
    fake().callbacks.audio_sample_batch = cb;
}
extern "C" fn set_input_poll(cb: retro_input_poll_t) {
    fake().callbacks.input_poll = cb;
}
extern "C" fn set_input_state(cb: retro_input_state_t) {
    fake().callbacks.input_state = cb;
}
extern "C" fn init() {
    record("init");
}
extern "C" fn deinit() {
    record("deinit");
}
extern "C" fn api_version() -> c_uint {
    fake().script.api_version
}
unsafe extern "C" fn get_system_info(info: *mut retro_system_info) {
    let script = &fake().script;
    let value = retro_system_info {
        library_name: c"fake".as_ptr(),
        library_version: c"1.0".as_ptr(),
        valid_extensions: script.extensions.as_ptr(),
        need_fullpath: script.need_fullpath,
        block_extract: false,
    };
    // SAFETY: the host passes a writable `retro_system_info`.
    unsafe { info.write(value) };
}
unsafe extern "C" fn get_system_av_info(info: *mut retro_system_av_info) {
    let av = fake().script.av;
    // SAFETY: the host passes a writable `retro_system_av_info`.
    unsafe { info.write(av) };
}
extern "C" fn set_controller_port_device(port: c_uint, device: c_uint) {
    record(&format!("port {port} device {device}"));
}
extern "C" fn reset() {
    record("reset");
    run_hook(|s| s.on_reset);
}
extern "C" fn run() {
    run_hook(|s| s.on_run);
}
unsafe extern "C" fn load_game(game: *const retro_game_info) -> bool {
    record("load_game");
    // SAFETY: the host passes null or a valid `retro_game_info` whose
    // strings and data are valid during the call.
    let (null, path, data) = unsafe {
        match game.as_ref() {
            None => (true, None, None),
            Some(g) => (
                false,
                (!g.path.is_null()).then(|| CStr::from_ptr(g.path).to_string_lossy().into_owned()),
                (!g.data.is_null())
                    .then(|| std::slice::from_raw_parts(g.data.cast::<u8>(), g.size).to_vec()),
            ),
        }
    };
    {
        let seen = &mut fake().seen;
        seen.game_was_null = null;
        seen.path = path;
        seen.data = data;
    }
    run_hook(|s| s.on_load);
    fake().script.load_result
}
extern "C" fn unload_game() {
    record("unload_game");
}

fn entry_points() -> EntryPoints {
    EntryPoints {
        set_environment,
        set_video_refresh,
        set_audio_sample,
        set_audio_sample_batch,
        set_input_poll,
        set_input_state,
        init,
        deinit,
        api_version,
        get_system_info,
        get_system_av_info,
        set_controller_port_device,
        reset,
        run,
        load_game,
        unload_game,
    }
}

fn script(f: impl FnOnce(&mut Script)) {
    f(&mut fake().script);
}

fn open() -> Result<Host, OpenError> {
    Host::start(
        Core::with_entry_points(None, entry_points()),
        State::new(c"/bios".to_owned(), c"/saves".to_owned()),
    )
}

fn loaded() -> Host {
    script(|s| {
        s.on_set_environment = Some(|cb| {
            let mut yes = true;
            assert!(cb.env(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, &raw mut yes));
        });
    });
    let mut host = open().unwrap();
    host.load_game(None).unwrap();
    host
}

#[test]
fn a_wrong_api_version_is_refused() {
    script(|s| s.api_version = RETRO_API_VERSION + 1);
    assert!(matches!(
        open(),
        Err(OpenError::ApiVersion { found, expected }) if found == RETRO_API_VERSION + 1 && expected == RETRO_API_VERSION
    ));
    assert!(seen().calls.is_empty(), "nothing is called after the check");
}

#[test]
fn open_and_drop_call_the_core_in_order() {
    let host = open().unwrap();
    let info = host.system_info();
    assert_eq!(info.name, "fake");
    assert_eq!(info.version, "1.0");
    assert_eq!(info.extensions, ["nes", "fds"]);
    drop(host);
    assert_eq!(
        seen().calls,
        ["set_environment", "set_video_refresh", "init", "deinit"]
    );
}

#[test]
fn load_and_drop_call_the_core_in_order() {
    let host = loaded();
    assert!(seen().game_was_null);
    drop(host);
    assert_eq!(
        seen().calls,
        [
            "set_environment",
            "set_video_refresh",
            "init",
            "load_game",
            "port 0 device 1",
            "port 1 device 1",
            "unload_game",
            "deinit"
        ]
    );
}

#[test]
fn no_content_needs_support_for_no_game() {
    let mut host = open().unwrap();
    assert!(matches!(host.load_game(None), Err(LoadError::NoContent)));
    assert!(!seen().calls.contains(&"load_game".to_owned()));
}

#[test]
fn plain_content_gives_the_core_path_and_data() {
    let dir = std::env::temp_dir().join(format!("neptron-libretro-fake-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rom = dir.join("game.nes");
    std::fs::write(&rom, b"NES\x1a rom").unwrap();
    let mut host = open().unwrap();
    let exts = ["nes".to_owned()];
    let av = host
        .load_game(Some(ContentRequest {
            path: &rom,
            extensions: &exts,
        }))
        .unwrap();
    assert_eq!(av.geometry.base_width, 256);
    assert_eq!(av.timing.fps_mhz, 60_000);
    let seen = seen();
    assert_eq!(seen.path.as_deref(), rom.to_str());
    assert_eq!(seen.data.as_deref(), Some(&b"NES\x1a rom"[..]));
    assert!(matches!(
        host.load_game(None),
        Err(LoadError::GameAlreadyLoaded)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_refused_load_keeps_the_host_open_with_the_core_log() {
    script(|s| {
        s.load_result = false;
        s.need_fullpath = true;
        s.on_load = Some(|cb| {
            let mut log = retro_log_callback { log: None };
            assert!(cb.env(RETRO_ENVIRONMENT_GET_LOG_INTERFACE, &raw mut log));
            // SAFETY: the host's log function, with a format that takes one
            // C string and one int.
            unsafe {
                log.log.unwrap()(
                    RETRO_LOG_ERROR,
                    c"bad %s: %d\n".as_ptr(),
                    c"header".as_ptr(),
                    42,
                )
            };
        });
    });
    let mut host = open().unwrap();
    let exts = ["nes".to_owned()];
    let request = ContentRequest {
        path: Path::new("/no/such/rom.nes"),
        extensions: &exts,
    };
    assert!(matches!(
        host.load_game(Some(request)),
        Err(LoadError::Refused)
    ));
    let seen = seen();
    assert_eq!(seen.path.as_deref(), Some("/no/such/rom.nes"));
    assert_eq!(seen.data, None, "need_fullpath: no data");
    let logs = host.logs();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].source, LogSource::Core);
    assert_eq!(logs[0].level, LogLevel::Error);
    assert_eq!(logs[0].text, "bad header: 42");
    assert!(matches!(host.run_frame(), Err(RunError::NoGame)));
}

#[test]
fn invalid_av_info_unloads_the_game() {
    script(|s| {
        s.av.timing.fps = f64::NAN;
        s.on_set_environment = Some(|cb| {
            let mut yes = true;
            assert!(cb.env(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, &raw mut yes));
        });
    });
    let mut host = open().unwrap();
    assert!(matches!(
        host.load_game(None),
        Err(LoadError::InvalidAvInfo(
            crate::GeometryError::FrameRate { .. }
        ))
    ));
    assert!(seen().calls.contains(&"unload_game".to_owned()));
    assert!(matches!(host.run_frame(), Err(RunError::NoGame)));
}

#[test]
fn frames_are_copied_packed_and_checked() {
    script(|s| {
        s.on_run = Some(|cb| {
            let mut format = RETRO_PIXEL_FORMAT_XRGB8888;
            assert!(cb.env(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, &raw mut format));
            // Two rows of 2 pixels, 12 bytes apart: 4 bytes of padding.
            let mut frame = [0u8; 20];
            frame[..8].copy_from_slice(&[1; 8]);
            frame[8..12].copy_from_slice(&[0xEE; 4]);
            frame[12..20].copy_from_slice(&[2; 8]);
            cb.video(Some(&frame), 2, 2, 12);
        });
    });
    let mut host = loaded();
    let out = host.run_frame().unwrap();
    let Video::Frame(frame) = out.video else {
        panic!("no frame: {:?}", out.video)
    };
    assert_eq!(frame.format, PixelFormat::Xrgb8888);
    assert_eq!((frame.width, frame.height, frame.pitch), (2, 2, 8));
    assert_eq!(frame.data, [[1u8; 8], [2u8; 8]].concat());
    assert_eq!(
        out.events,
        [Event::PixelFormat(PixelFormat::Xrgb8888)],
        "the format changed during the frame"
    );
}

#[test]
fn hostile_frames_are_rejected_and_the_last_frame_stays() {
    script(|s| {
        s.on_run = Some(|cb| {
            let n = fake().seen.calls.len();
            fake().seen.calls.push("run".to_owned());
            let buf = [9u8; 2 * 320 * 241];
            match n % 6 {
                0 => cb.video(Some(&buf), 4, 4, 8),
                1 => cb.video(Some(&buf), 321, 1, 642),
                2 => cb.video(Some(&buf), 4, 4, 7),
                3 => cb.video(Some(&buf), 0, 4, 8),
                4 => cb.video(Some(&buf), 4, 4, usize::MAX),
                _ => cb.video(None, 4, 4, 8),
            }
        });
    });
    let mut host = loaded();
    fake().seen.calls.clear();
    let first = host.run_frame().unwrap().video;
    assert!(matches!(first, Video::Frame(f) if f.width == 4 && f.data == [9; 32]));
    let faults: Vec<Option<FrameFault>> = (0..4)
        .map(|_| match host.run_frame().unwrap().video {
            Video::Rejected(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(
        faults,
        [
            Some(FrameFault::OverMaximum {
                width: 321,
                height: 1,
                max_width: 320,
                max_height: 240
            }),
            Some(FrameFault::PitchTooSmall {
                pitch: 7,
                row_bytes: 8
            }),
            Some(FrameFault::ZeroSize {
                width: 0,
                height: 4
            }),
            Some(FrameFault::PitchTooLarge {
                pitch: usize::MAX,
                limit: crate::MAX_PITCH_BYTES
            }),
        ]
    );
    let dupe = host.run_frame().unwrap().video;
    assert!(
        matches!(dupe, Video::Duplicate(Some(f)) if f.width == 4 && f.data == [9; 32]),
        "{dupe:?}"
    );
}

#[test]
fn audio_is_copied_and_bad_batches_are_dropped() {
    script(|s| {
        s.on_run = Some(|cb| {
            let samples: Vec<i16> = (0..8).collect();
            assert_eq!(cb.audio(samples.as_ptr(), 4), 4);
            cb.sample(100, -100);
            assert_eq!(cb.audio(std::ptr::null(), 5), 5);
            let bytes = [0u8; 9];
            let misaligned = bytes.as_ptr().wrapping_add(1).cast::<i16>();
            if !misaligned.is_aligned() {
                assert_eq!(cb.audio(misaligned, 2), 2);
            } else {
                assert_eq!(cb.audio(std::ptr::null(), 2), 2);
            }
            let many = vec![1i16; 2 * 5000];
            assert_eq!(cb.audio(many.as_ptr(), 5000), 5000);
        });
    });
    let mut host = loaded();
    let out = host.run_frame().unwrap();
    assert_eq!(out.audio[..10], [0, 1, 2, 3, 4, 5, 6, 7, 100, -100]);
    assert_eq!(out.audio.len(), 2 * 4096);
    assert_eq!(out.dropped.audio_frames, 5 + 2 + (5000 - (4096 - 5)));
}

#[test]
fn input_reads_the_pad_of_each_port() {
    script(|s| {
        s.on_run = Some(|cb| {
            let a = cb.input(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_A);
            let mask = cb.input(1, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_MASK);
            let x = cb.input(1, RETRO_DEVICE_ANALOG, 0, 0);
            let none = cb.input(7, RETRO_DEVICE_JOYPAD, 0, 0);
            fake().seen.calls.push(format!("{a} {mask} {x} {none}"));
        });
    });
    let mut host = loaded();
    let mut pad = Pad::default();
    pad.apply(neptron_types::Control::A, 1);
    host.set_pad(0, pad).unwrap();
    let pad1 = Pad {
        buttons: 0x0102,
        left: [-5, 0],
        right: [0, 0],
    };
    host.set_pad(1, pad1).unwrap();
    assert!(host.set_pad(crate::MAX_PORTS, pad).is_err());
    host.run_frame().unwrap();
    assert_eq!(seen().calls.last().unwrap(), "1 258 -5 0");
}

#[test]
fn events_of_load_and_reset_come_with_the_next_frame() {
    script(|s| {
        s.on_load = Some(|cb| {
            let mut msg = retro_message {
                msg: c"loaded".as_ptr(),
                frames: 1,
            };
            assert!(cb.env(RETRO_ENVIRONMENT_SET_MESSAGE, &raw mut msg));
        });
        s.on_reset = Some(|cb| {
            assert!(cb.env(RETRO_ENVIRONMENT_SHUTDOWN, std::ptr::null_mut::<u8>()));
        });
    });
    let mut host = loaded();
    let loaded_message = Event::Message {
        text: "loaded".to_owned(),
        frames: 1,
    };
    assert_eq!(host.run_frame().unwrap().events, [loaded_message]);
    assert!(host.run_frame().unwrap().events.is_empty());
    host.reset().unwrap();
    assert_eq!(host.run_frame().unwrap().events, [Event::Shutdown]);
    assert!(host.run_frame().unwrap().events.is_empty());
}

#[test]
fn environment_answers() {
    script(|s| {
        s.on_run = Some(|cb| {
            let mut out = Vec::new();
            let mut dupe = false;
            out.push(cb.env(RETRO_ENVIRONMENT_GET_CAN_DUPE, &raw mut dupe) && dupe);
            out.push(!cb.env(RETRO_ENVIRONMENT_GET_CAN_DUPE, std::ptr::null_mut::<bool>()));
            let mut dir: *const c_char = std::ptr::null();
            out.push(cb.env(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY, &raw mut dir));
            // SAFETY: the host gave a string that lives as long as it.
            out.push(unsafe { CStr::from_ptr(dir) } == c"/bios");
            out.push(cb.env(RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY, &raw mut dir));
            // SAFETY: as above.
            out.push(unsafe { CStr::from_ptr(dir) } == c"/saves");
            let mut language = 99u32;
            out.push(cb.env(RETRO_ENVIRONMENT_GET_LANGUAGE, &raw mut language));
            out.push(language == RETRO_LANGUAGE_ENGLISH);
            let mut av = 0u32;
            out.push(cb.env(RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE, &raw mut av) && av == 3);
            out.push(cb.env(
                RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE,
                std::ptr::null_mut::<u32>(),
            ));
            out.push(cb.env(
                RETRO_ENVIRONMENT_GET_INPUT_BITMASKS,
                std::ptr::null_mut::<u8>(),
            ));
            let mut rotation = 1u32;
            out.push(!cb.env(RETRO_ENVIRONMENT_SET_ROTATION, &raw mut rotation));
            out.push(!cb.env(RETRO_ENVIRONMENT_SET_ROTATION, &raw mut rotation));
            out.push(!cb.env(0x24E54, std::ptr::null_mut::<u8>()));
            out.push(!cb.env(0x24E54, std::ptr::null_mut::<u8>()));
            let mut msg = retro_message {
                msg: c"Saved".as_ptr(),
                frames: 120,
            };
            out.push(cb.env(RETRO_ENVIRONMENT_SET_MESSAGE, &raw mut msg));
            let mut bad = retro_message {
                msg: std::ptr::null(),
                frames: 1,
            };
            out.push(!cb.env(RETRO_ENVIRONMENT_SET_MESSAGE, &raw mut bad));
            let mut format = 7u32;
            out.push(!cb.env(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, &raw mut format));
            let mut geometry = retro_game_geometry {
                base_width: 320,
                base_height: 240,
                max_width: 0,
                max_height: 0,
                aspect_ratio: f32::NAN,
            };
            out.push(cb.env(RETRO_ENVIRONMENT_SET_GEOMETRY, &raw mut geometry));
            out.push(!cb.env(RETRO_ENVIRONMENT_SET_GEOMETRY, std::ptr::null_mut::<u8>()));
            out.push(cb.env(RETRO_ENVIRONMENT_SHUTDOWN, std::ptr::null_mut::<u8>()));
            // A callback from another thread finds no host.
            let other = std::thread::scope(|s| {
                s.spawn(|| {
                    let mut d = false;
                    cb.env(RETRO_ENVIRONMENT_GET_CAN_DUPE, &raw mut d)
                })
                .join()
                .unwrap()
            });
            out.push(!other);
            let failed: Vec<usize> = (0..out.len()).filter(|&i| !out[i]).collect();
            fake().seen.calls.push(format!("failed {failed:?}"));
        });
    });
    let mut host = loaded();
    let out = host.run_frame().unwrap();
    assert_eq!(
        out.events,
        [
            Event::Message {
                text: "Saved".to_owned(),
                frames: 120
            },
            Event::Refused(Refusal::PixelFormat { value: 7 }),
            Event::Geometry(crate::GameGeometry {
                base_width: 320,
                base_height: 240,
                max_width: 320,
                max_height: 240,
                aspect_milli: 1333
            }),
            Event::Shutdown,
        ]
    );
    let logs: Vec<_> = out.logs.iter().map(|l| (l.level, l.text.clone())).collect();
    assert_eq!(
        logs,
        [
            (
                LogLevel::Info,
                "environment call 0x1 refused: rotation is not supported".to_owned()
            ),
            (
                LogLevel::Warn,
                "unknown environment call 0x24e54".to_owned()
            ),
        ]
    );
    assert_eq!(seen().calls.last().unwrap(), "failed []");
}
