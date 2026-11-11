//! Loads the built test core as a dynamic library and runs scripts through
//! the raw libretro API, as a frontend does.
//!
//! The core keeps its state in the process, and the script name is an
//! environment variable, so each test needs its own process. `cargo nextest`
//! runs every test in its own process.

use std::ffi::{c_uint, c_void};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use libloading::Library;
use neptron_libretro_sys::*;
use neptron_testcore::{PixelFormat, SCRIPT_ENV, Size, expected_hash, frames_after, hash, sample};

/// What the frontend callbacks saw.
#[derive(Debug, Clone, PartialEq)]
enum Seen {
    PixelFormat(retro_pixel_format),
    Geometry(u32, u32, u32, u32),
    AvInfo(u32, u32, u32, u32, f64, f64),
    Shutdown,
    Frame { size: Size, pitch: usize, hash: u64 },
    Dupe,
    Audio { first: u64, frames: usize },
    AudioPatternError(u64),
}

#[derive(Debug, Default)]
struct Frontend {
    seen: Vec<Seen>,
    refuse_pixel_format: bool,
    pixel_format: Option<retro_pixel_format>,
    buttons: u16,
    next_sample: u64,
}

static FRONTEND: Mutex<Frontend> = Mutex::new(Frontend {
    seen: Vec::new(),
    refuse_pixel_format: false,
    pixel_format: None,
    buttons: 0,
    next_sample: 0,
});

fn frontend() -> MutexGuard<'static, Frontend> {
    FRONTEND.lock().unwrap_or_else(|e| e.into_inner())
}

unsafe extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    let mut f = frontend();
    match cmd {
        RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME => true,
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
            // SAFETY: libretro.h: the data is a `const enum retro_pixel_format *`.
            let format = unsafe { *data.cast::<retro_pixel_format>() };
            f.seen.push(Seen::PixelFormat(format));
            if f.refuse_pixel_format {
                return false;
            }
            f.pixel_format = Some(format);
            true
        }
        RETRO_ENVIRONMENT_GET_CAN_DUPE => {
            // SAFETY: libretro.h: the data is a `bool *`.
            unsafe { *data.cast::<bool>() = true };
            true
        }
        RETRO_ENVIRONMENT_SET_GEOMETRY => {
            // SAFETY: libretro.h: the data is a `const struct retro_game_geometry *`.
            let g = unsafe { *data.cast::<retro_game_geometry>() };
            f.seen.push(Seen::Geometry(
                g.base_width,
                g.base_height,
                g.max_width,
                g.max_height,
            ));
            true
        }
        RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO => {
            // SAFETY: libretro.h: the data is a `const struct retro_system_av_info *`.
            let av = unsafe { *data.cast::<retro_system_av_info>() };
            f.seen.push(av_seen(&av));
            true
        }
        RETRO_ENVIRONMENT_SHUTDOWN => {
            f.seen.push(Seen::Shutdown);
            true
        }
        _ => false,
    }
}

unsafe extern "C" fn video_refresh(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    let mut f = frontend();
    if data.is_null() {
        f.seen.push(Seen::Dupe);
        return;
    }
    let format = match f.pixel_format {
        Some(RETRO_PIXEL_FORMAT_XRGB8888) => PixelFormat::Xrgb8888,
        Some(RETRO_PIXEL_FORMAT_RGB565) => PixelFormat::Rgb565,
        _ => PixelFormat::Rgb1555,
    };
    let size = Size { width, height };
    let len = pitch * (height as usize - 1) + width as usize * format.bytes_per_pixel();
    // SAFETY: libretro.h: `data` holds `height` rows `pitch` bytes apart, each
    // with `width` pixels of the pixel format in force.
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) };
    let hash = hash(bytes, pitch, size, format).unwrap_or_default();
    f.seen.push(Seen::Frame { size, pitch, hash });
}

unsafe extern "C" fn audio_sample_batch(data: *const i16, frames: usize) -> usize {
    let mut f = frontend();
    // SAFETY: libretro.h: `data` holds `frames` interleaved stereo frames.
    let samples = unsafe { std::slice::from_raw_parts(data, frames * 2) };
    let first = f.next_sample;
    for (i, pair) in samples.chunks_exact(2).enumerate() {
        let index = first + i as u64;
        if pair != sample(index) {
            f.seen.push(Seen::AudioPatternError(index));
            break;
        }
    }
    f.next_sample += frames as u64;
    f.seen.push(Seen::Audio { first, frames });
    frames
}

unsafe extern "C" fn audio_sample(_left: i16, _right: i16) {}

unsafe extern "C" fn input_poll() {}

unsafe extern "C" fn input_state(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16 {
    let f = frontend();
    if port == 0 && device == RETRO_DEVICE_JOYPAD && index == 0 && id < 16 {
        i16::from((f.buttons >> id) & 1 == 1)
    } else {
        0
    }
}

fn av_seen(av: &retro_system_av_info) -> Seen {
    Seen::AvInfo(
        av.geometry.base_width,
        av.geometry.base_height,
        av.geometry.max_width,
        av.geometry.max_height,
        av.timing.fps,
        av.timing.sample_rate,
    )
}

/// The entry points of the loaded core. The library stays open while the
/// struct lives, so the function pointers stay valid.
struct Core {
    _lib: Library,
    set_environment: RetroSetEnvironmentFn,
    set_video_refresh: RetroSetVideoRefreshFn,
    set_audio_sample: RetroSetAudioSampleFn,
    set_audio_sample_batch: RetroSetAudioSampleBatchFn,
    set_input_poll: RetroSetInputPollFn,
    set_input_state: RetroSetInputStateFn,
    init: RetroInitFn,
    api_version: RetroApiVersionFn,
    get_system_info: RetroGetSystemInfoFn,
    get_system_av_info: RetroGetSystemAvInfoFn,
    load_game: RetroLoadGameFn,
    run: RetroRunFn,
    reset: RetroResetFn,
    get_memory_data: RetroGetMemoryDataFn,
    get_memory_size: RetroGetMemorySizeFn,
    unload_game: RetroUnloadGameFn,
    deinit: RetroDeinitFn,
}

fn core_path() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    let deps = exe.parent().expect("deps directory");
    let name = libloading::library_filename("neptron_testcore");
    [
        deps.join(&name),
        deps.parent().expect("profile directory").join(&name),
    ]
    .into_iter()
    .find(|p| p.exists())
    .unwrap_or_else(|| panic!("no {name:?} next to {}", exe.display()))
}

impl Core {
    fn load() -> Self {
        let path = core_path();
        // SAFETY: the library is the test core of this workspace. Its
        // initialisers do nothing but set up the Rust runtime.
        let lib = unsafe { Library::new(&path) }.expect("load the test core");
        for name in CORE_API_SYMBOLS {
            // SAFETY: only the presence of the symbol is checked.
            let found = unsafe { lib.get::<*const c_void>(name) };
            assert!(found.is_ok(), "{name} is not exported");
        }
        macro_rules! sym {
            ($name:literal) => {
                // SAFETY: the type is the function pointer type of the header
                // for this entry point, and the export matches it.
                *unsafe { lib.get($name) }.expect($name)
            };
        }
        Self {
            set_environment: sym!("retro_set_environment"),
            set_video_refresh: sym!("retro_set_video_refresh"),
            set_audio_sample: sym!("retro_set_audio_sample"),
            set_audio_sample_batch: sym!("retro_set_audio_sample_batch"),
            set_input_poll: sym!("retro_set_input_poll"),
            set_input_state: sym!("retro_set_input_state"),
            init: sym!("retro_init"),
            api_version: sym!("retro_api_version"),
            get_system_info: sym!("retro_get_system_info"),
            get_system_av_info: sym!("retro_get_system_av_info"),
            load_game: sym!("retro_load_game"),
            run: sym!("retro_run"),
            reset: sym!("retro_reset"),
            get_memory_data: sym!("retro_get_memory_data"),
            get_memory_size: sym!("retro_get_memory_size"),
            unload_game: sym!("retro_unload_game"),
            deinit: sym!("retro_deinit"),
            _lib: lib,
        }
    }

    /// Sets the callbacks and calls `retro_init`, in the order of libretro.h.
    fn start(&self) {
        // SAFETY: each callback has the type that libretro.h names, and lives
        // for the whole process.
        unsafe {
            (self.set_environment)(Some(environment));
            (self.set_video_refresh)(Some(video_refresh));
            (self.set_audio_sample)(Some(audio_sample));
            (self.set_audio_sample_batch)(Some(audio_sample_batch));
            (self.set_input_poll)(Some(input_poll));
            (self.set_input_state)(Some(input_state));
            (self.init)();
        }
    }

    fn load_game(&self) -> bool {
        // SAFETY: the test core reads no content, and libretro.h allows null
        // with `SET_SUPPORT_NO_GAME`.
        unsafe { (self.load_game)(std::ptr::null()) }
    }

    fn run(&self, buttons: u16) {
        frontend().buttons = buttons;
        // SAFETY: a game is loaded, and the call comes from this thread only.
        unsafe { (self.run)() };
    }

    fn av_info(&self) -> retro_system_av_info {
        let mut av = retro_system_av_info {
            geometry: retro_game_geometry {
                base_width: 0,
                base_height: 0,
                max_width: 0,
                max_height: 0,
                aspect_ratio: 0.0,
            },
            timing: retro_system_timing {
                fps: 0.0,
                sample_rate: 0.0,
            },
        };
        // SAFETY: `av` is a writable `retro_system_av_info`.
        unsafe { (self.get_system_av_info)(&raw mut av) };
        av
    }

    /// The save RAM pointer and size. The test reads and writes it through
    /// the pointer only, as a frontend does, because the core writes the
    /// same bytes.
    fn save_ram(&self) -> (*mut u8, usize) {
        // SAFETY: plain calls with an integer argument.
        let (data, size) = unsafe {
            (
                (self.get_memory_data)(RETRO_MEMORY_SAVE_RAM),
                (self.get_memory_size)(RETRO_MEMORY_SAVE_RAM),
            )
        };
        assert!(!data.is_null());
        (data.cast(), size)
    }

    fn stop(&self) {
        // SAFETY: plain calls, in the order of libretro.h.
        unsafe {
            (self.unload_game)();
            (self.deinit)();
        }
    }
}

/// Writes the script to a new directory and returns the directory.
fn write_script(text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neptron-testcore-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("script.ntc");
    std::fs::write(&path, text).expect("write the script");
    set_script_path(&path);
    dir
}

fn set_script_path(path: &std::path::Path) {
    // SAFETY: the test has started no thread that reads the environment.
    unsafe { std::env::set_var(SCRIPT_ENV, path) };
}

const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 59940 44100
save_ram 16

at 1 geometry 320 240 1333
at 2 dupe
at 3 sram_write 5 42
at 3 av_info 320 240 0 320 240 50000 48000
at 4 shutdown
";

#[test]
fn a_scripted_run_through_the_raw_api() {
    let dir = write_script(SCRIPT);
    let core = Core::load();
    // SAFETY: a plain call.
    assert_eq!(unsafe { (core.api_version)() }, RETRO_API_VERSION);
    core.start();
    assert!(core.load_game());

    let mut info = retro_system_info {
        library_name: std::ptr::null(),
        library_version: std::ptr::null(),
        valid_extensions: std::ptr::null(),
        need_fullpath: false,
        block_extract: false,
    };
    // SAFETY: `info` is a writable `retro_system_info`.
    unsafe { (core.get_system_info)(&raw mut info) };
    // SAFETY: the core returns static NUL-terminated strings.
    let name = unsafe { std::ffi::CStr::from_ptr(info.library_name) };
    assert_eq!(name.to_str(), Ok("neptron-testcore"));
    assert!(info.need_fullpath);

    assert_eq!(
        av_seen(&core.av_info()),
        Seen::AvInfo(256, 224, 320, 240, 59.94, 44_100.0)
    );

    let (sram, sram_len) = core.save_ram();
    assert_eq!(sram_len, 16);
    // SAFETY: the core keeps 16 bytes at `sram` until the game is unloaded,
    // and no call into the core runs during an access.
    let sram_byte = |i: usize| unsafe { sram.add(i).read() };
    // SAFETY: as above.
    unsafe { sram.write(7) };

    let buttons = [0, 1 << RETRO_DEVICE_ID_JOYPAD_START, 0, 0xFFFF, 0x00F0];
    for b in buttons {
        core.run(b);
    }

    let format = PixelFormat::Rgb565;
    let small = Size {
        width: 256,
        height: 224,
    };
    let qvga = Size {
        width: 320,
        height: 240,
    };
    let frame = |size: Size, n: u64| Seen::Frame {
        size,
        pitch: 640,
        hash: expected_hash(size, format, n, buttons[n as usize]).unwrap(),
    };
    let ntsc = neptron_testcore::Timing {
        fps_mhz: 59_940,
        sample_rate_hz: 44_100,
    };
    let pal = neptron_testcore::Timing {
        fps_mhz: 50_000,
        sample_rate_hz: 48_000,
    };
    let audio = |first: u64, frames: u64| Seen::Audio {
        first,
        frames: frames as usize,
    };
    let n0 = frames_after(1, ntsc);
    let n1 = frames_after(2, ntsc) - n0;
    let n2 = frames_after(3, ntsc) - n0 - n1;
    let ntsc_total = frames_after(3, ntsc);
    let expected = vec![
        Seen::PixelFormat(RETRO_PIXEL_FORMAT_RGB565),
        frame(small, 0),
        audio(0, n0),
        Seen::Geometry(320, 240, 320, 240),
        frame(qvga, 1),
        audio(n0, n1),
        Seen::Dupe,
        audio(n0 + n1, n2),
        Seen::AvInfo(320, 240, 320, 240, 50.0, 48_000.0),
        frame(qvga, 3),
        audio(ntsc_total, frames_after(1, pal)),
        Seen::Shutdown,
        frame(qvga, 4),
        audio(
            ntsc_total + frames_after(1, pal),
            frames_after(2, pal) - frames_after(1, pal),
        ),
    ];
    assert_eq!(frontend().seen, expected);

    assert_eq!(sram_byte(0), 7, "the frontend's write stays");
    assert_eq!(sram_byte(5), 42, "the script's write is visible");

    // SAFETY: a plain call.
    unsafe { (core.reset)() };
    frontend().seen.clear();
    frontend().next_sample = 0;
    core.run(0);
    let seen = frontend().seen.clone();
    assert_eq!(seen[0], Seen::AvInfo(256, 224, 320, 240, 59.94, 44_100.0));
    assert_eq!(seen[1], frame(small, 0));
    assert_eq!(sram_byte(5), 42, "reset keeps the save RAM");
    core.stop();
    std::fs::remove_dir_all(dir).expect("remove the script");
}

#[test]
fn a_missing_script_fails_the_load() {
    set_script_path(std::path::Path::new("/nonexistent/neptron-testcore.ntc"));
    let core = Core::load();
    core.start();
    assert!(!core.load_game());
    core.run(0);
    assert!(frontend().seen.is_empty(), "no game, no output");
    core.stop();
}

#[test]
fn a_refused_pixel_format_fails_the_load() {
    let dir = write_script("neptron-testcore 1\npixel_format xrgb8888\n");
    frontend().refuse_pixel_format = true;
    let core = Core::load();
    core.start();
    assert!(!core.load_game());
    assert_eq!(
        frontend().seen,
        [Seen::PixelFormat(RETRO_PIXEL_FORMAT_XRGB8888)]
    );
    core.stop();
    std::fs::remove_dir_all(dir).expect("remove the script");
}
