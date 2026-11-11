//! The exported libretro entry points, and the calls into the frontend.
//!
//! This is the only module with `unsafe` code. Every other module of the
//! crate forbids it.
//!
//! **Global state.** The libretro API has no instance handle, so the core
//! keeps its state in one static, [`GLOBAL`], behind a `Mutex`. libretro.h
//! says that the frontend calls a core from one thread; the mutex keeps the
//! state sound also when a frontend breaks that rule. No lock is held while
//! the core calls into the frontend, so a frontend callback can call back into
//! the core. A call that finds the lock taken (a second thread) does nothing
//! and returns the default value.
//!
//! **Callbacks.** The six `retro_set_*` functions are `unsafe`: their caller
//! promises that each callback conforms to libretro.h for as long as the core
//! is loaded. Every later call into a callback relies on that promise. The
//! other entry points need no promise beyond the types of their arguments,
//! except the two that write through a pointer, which say so.
//!
//! **Panics.** No panic crosses the `extern "C"` boundary. Every entry point
//! runs its body in `catch_unwind`. After a panic the core is dead for the
//! life of the process: [`FAILED`] is set, and every later call does nothing
//! and returns the default value. With `panic = "abort"` a panic ends the
//! process instead, which is also safe.

use std::ffi::{CStr, CString, c_char, c_uint, c_void};
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use neptron_libretro_sys::*;

use crate::model::{Core, Request, Step, UNKNOWN_ENVIRONMENT_COMMAND, Video};
use crate::options::{VALUE_NOT_LISTED, VALUE_NOT_SET, value_code};
use crate::script::{AudioMode, AvInfo, Geometry, PixelFormat, Script, Size};

/// The callbacks that the frontend gives the core.
#[derive(Debug, Clone, Copy)]
struct Callbacks {
    environment: retro_environment_t,
    video_refresh: retro_video_refresh_t,
    audio_sample: retro_audio_sample_t,
    audio_sample_batch: retro_audio_sample_batch_t,
    input_poll: retro_input_poll_t,
    input_state: retro_input_state_t,
    log: retro_log_printf_t,
}

impl Callbacks {
    const NONE: Self = Self {
        environment: None,
        video_refresh: None,
        audio_sample: None,
        audio_sample_batch: None,
        input_poll: None,
        input_state: None,
        log: None,
    };
}

/// All state of the core.
struct Global {
    callbacks: Callbacks,
    core: Option<Core>,
}

static GLOBAL: Mutex<Global> = Mutex::new(Global {
    callbacks: Callbacks::NONE,
    core: None,
});

static FAILED: AtomicBool = AtomicBool::new(false);

const LIBRARY_NAME: &[u8] = b"neptron-testcore\0";
const LIBRARY_VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
const VALID_EXTENSIONS: &[u8] = b"ntc\0";
const JOYPAD_BUTTONS: c_uint = 16;
// The C form of `CORE_OPTIONS`; a test checks that both say the same.
const OPTION_KEYS: [&CStr; 2] = [c"neptron_testcore_mode", c"neptron_testcore_level"];
const OPTION_DECLARATIONS: [&CStr; 2] = [c"Test mode; alpha|beta|gamma", c"Test level; 1|2|3"];

// Each export coerces to the function pointer type of the header only when
// its signature matches.
const _: RetroSetEnvironmentFn = retro_set_environment;
const _: RetroSetVideoRefreshFn = retro_set_video_refresh;
const _: RetroSetAudioSampleFn = retro_set_audio_sample;
const _: RetroSetAudioSampleBatchFn = retro_set_audio_sample_batch;
const _: RetroSetInputPollFn = retro_set_input_poll;
const _: RetroSetInputStateFn = retro_set_input_state;
const _: RetroInitFn = retro_init;
const _: RetroDeinitFn = retro_deinit;
const _: RetroApiVersionFn = retro_api_version;
const _: RetroGetSystemInfoFn = retro_get_system_info;
const _: RetroGetSystemAvInfoFn = retro_get_system_av_info;
const _: RetroSetControllerPortDeviceFn = retro_set_controller_port_device;
const _: RetroResetFn = retro_reset;
const _: RetroRunFn = retro_run;
const _: RetroSerializeSizeFn = retro_serialize_size;
const _: RetroSerializeFn = retro_serialize;
const _: RetroUnserializeFn = retro_unserialize;
const _: RetroCheatResetFn = retro_cheat_reset;
const _: RetroCheatSetFn = retro_cheat_set;
const _: RetroLoadGameFn = retro_load_game;
const _: RetroLoadGameSpecialFn = retro_load_game_special;
const _: RetroUnloadGameFn = retro_unload_game;
const _: RetroGetRegionFn = retro_get_region;
const _: RetroGetMemoryDataFn = retro_get_memory_data;
const _: RetroGetMemorySizeFn = retro_get_memory_size;

/// Stores the environment callback, declares that the core runs without
/// content, asks for the log interface, and declares the core options.
///
/// # Safety
///
/// `cb` is null, or a function that conforms to libretro.h for as long as
/// the core is loaded. The core stores it and calls it from later entry
/// points. The log interface that `cb` returns conforms to libretro.h for
/// as long as the core is loaded too.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_set_environment(cb: retro_environment_t) {
    guarded((), || {
        let callbacks = with_global(|g| {
            g.callbacks.environment = cb;
            g.callbacks
        })
        .unwrap_or(Callbacks::NONE);
        let mut no_game = true;
        env(
            &callbacks,
            RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME,
            &raw mut no_game,
        );
        let mut log = retro_log_callback { log: None };
        if env(
            &callbacks,
            RETRO_ENVIRONMENT_GET_LOG_INTERFACE,
            &raw mut log,
        ) {
            with_global(|g| g.callbacks.log = log.log);
        }
        declare_options(&callbacks);
    });
}

/// Stores the video callback.
///
/// # Safety
///
/// `cb` is null, or a function that conforms to libretro.h for as long as
/// the core is loaded. The core stores it and calls it from later entry
/// points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_set_video_refresh(cb: retro_video_refresh_t) {
    update(|g| g.callbacks.video_refresh = cb);
}

/// Stores the single-sample audio callback.
///
/// # Safety
///
/// `cb` is null, or a function that conforms to libretro.h for as long as
/// the core is loaded. The core stores it and calls it from later entry
/// points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_set_audio_sample(cb: retro_audio_sample_t) {
    update(|g| g.callbacks.audio_sample = cb);
}

/// Stores the batch audio callback.
///
/// # Safety
///
/// `cb` is null, or a function that conforms to libretro.h for as long as
/// the core is loaded. The core stores it and calls it from later entry
/// points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_set_audio_sample_batch(cb: retro_audio_sample_batch_t) {
    update(|g| g.callbacks.audio_sample_batch = cb);
}

/// Stores the input poll callback.
///
/// # Safety
///
/// `cb` is null, or a function that conforms to libretro.h for as long as
/// the core is loaded. The core stores it and calls it from later entry
/// points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_set_input_poll(cb: retro_input_poll_t) {
    update(|g| g.callbacks.input_poll = cb);
}

/// Stores the input state callback.
///
/// # Safety
///
/// `cb` is null, or a function that conforms to libretro.h for as long as
/// the core is loaded. The core stores it and calls it from later entry
/// points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_set_input_state(cb: retro_input_state_t) {
    update(|g| g.callbacks.input_state = cb);
}

/// Does nothing: the core loads its script in `retro_load_game`.
#[unsafe(no_mangle)]
pub extern "C" fn retro_init() {}

/// Drops a loaded script.
#[unsafe(no_mangle)]
pub extern "C" fn retro_deinit() {
    update(|g| g.core = None);
}

/// Returns `RETRO_API_VERSION`.
#[unsafe(no_mangle)]
pub extern "C" fn retro_api_version() -> c_uint {
    RETRO_API_VERSION
}

/// Writes the name, the version and the content rules of the core.
///
/// # Safety
///
/// `info` is null or points to a writable `retro_system_info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_get_system_info(info: *mut retro_system_info) {
    if info.is_null() {
        return;
    }
    let value = retro_system_info {
        library_name: LIBRARY_NAME.as_ptr().cast(),
        library_version: LIBRARY_VERSION.as_ptr().cast(),
        valid_extensions: VALID_EXTENSIONS.as_ptr().cast(),
        need_fullpath: true,
        block_extract: false,
    };
    // SAFETY: `info` is not null, and the caller guarantees that it points to
    // a writable `retro_system_info`. `write_unaligned` needs no alignment.
    // The strings are static and end in NUL.
    unsafe { info.write_unaligned(value) };
}

/// Writes the geometry and timing in force. Before a load it writes zeros.
///
/// # Safety
///
/// `info` is null or points to a writable `retro_system_av_info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_get_system_av_info(info: *mut retro_system_av_info) {
    if info.is_null() {
        return;
    }
    let av = guarded(None, || {
        with_global(|g| g.core.as_ref().map(Core::av)).flatten()
    });
    let value = av.map_or(
        retro_system_av_info {
            geometry: game_geometry(ZERO_GEOMETRY, ZERO_SIZE),
            timing: retro_system_timing {
                fps: 0.0,
                sample_rate: 0.0,
            },
        },
        system_av_info,
    );
    // SAFETY: `info` is not null, and the caller guarantees that it points to
    // a writable `retro_system_av_info`. `write_unaligned` needs no alignment.
    unsafe { info.write_unaligned(value) };
}

/// Ignores the device choice: the core reads a joypad on port 0.
#[unsafe(no_mangle)]
pub extern "C" fn retro_set_controller_port_device(_port: c_uint, _device: c_uint) {}

/// Starts the script again at frame 0.
#[unsafe(no_mangle)]
pub extern "C" fn retro_reset() {
    update(|g| {
        if let Some(core) = g.core.as_mut() {
            core.reset();
        }
    });
}

/// Runs one frame of the script: reads the joypad, makes the environment
/// calls of the frame, sends the frame, then the audio.
#[unsafe(no_mangle)]
pub extern "C" fn retro_run() {
    guarded((), || {
        let Some(Some(callbacks)) = with_global(|g| g.core.as_ref().map(|_| g.callbacks)) else {
            return;
        };
        let buttons = read_buttons(&callbacks);
        let Some(Some(step)) = with_global(|g| g.core.as_mut().map(|core| core.step(buttons)))
        else {
            return;
        };
        send(&callbacks, &step);
        with_global(|g| {
            if let Some(core) = g.core.as_mut() {
                core.finish(step);
            }
        });
    });
}

/// Returns 0: the core has no save states.
#[unsafe(no_mangle)]
pub extern "C" fn retro_serialize_size() -> usize {
    0
}

/// Returns false: the core has no save states. It does not touch `data`.
#[unsafe(no_mangle)]
pub extern "C" fn retro_serialize(_data: *mut c_void, _len: usize) -> bool {
    false
}

/// Returns false: the core has no save states. It does not touch `data`.
#[unsafe(no_mangle)]
pub extern "C" fn retro_unserialize(_data: *const c_void, _len: usize) -> bool {
    false
}

/// Does nothing: the core has no cheats.
#[unsafe(no_mangle)]
pub extern "C" fn retro_cheat_reset() {}

/// Does nothing: the core has no cheats. It does not touch `code`.
#[unsafe(no_mangle)]
pub extern "C" fn retro_cheat_set(_index: c_uint, _enabled: bool, _code: *const c_char) {}

/// Loads the script that `NEPTRON_TESTCORE_SCRIPT` names and sets the pixel
/// format. The content is not used, so `game` may be null and is not read.
/// Returns false and logs the reason when the script has an error or the
/// frontend refuses the pixel format.
#[unsafe(no_mangle)]
pub extern "C" fn retro_load_game(_game: *const retro_game_info) -> bool {
    guarded(false, || {
        let Some(callbacks) = with_global(|g| g.callbacks) else {
            return false;
        };
        let script = match Script::from_env() {
            Ok(script) => script,
            Err(e) => {
                log(&callbacks, RETRO_LOG_ERROR, &e.to_string());
                return false;
            }
        };
        if let Some(format) = pixel_format_request(script.pixel_format) {
            let mut value = format;
            if !env(
                &callbacks,
                RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
                &raw mut value,
            ) {
                log(
                    &callbacks,
                    RETRO_LOG_ERROR,
                    &format!(
                        "the frontend refused pixel format {:?}",
                        script.pixel_format
                    ),
                );
                return false;
            }
        }
        let mut can_dupe = false;
        let can_dupe = env(
            &callbacks,
            RETRO_ENVIRONMENT_GET_CAN_DUPE,
            &raw mut can_dupe,
        ) && can_dupe;
        let core = Core::new(script, can_dupe);
        with_global(|g| g.core = Some(core)).is_some()
    })
}

/// Returns false: the core has no special content types.
#[unsafe(no_mangle)]
pub extern "C" fn retro_load_game_special(
    _game_type: c_uint,
    _info: *const retro_game_info,
    _num_info: usize,
) -> bool {
    false
}

/// Drops the loaded script and its save RAM.
#[unsafe(no_mangle)]
pub extern "C" fn retro_unload_game() {
    update(|g| g.core = None);
}

/// Returns `RETRO_REGION_NTSC`.
#[unsafe(no_mangle)]
pub extern "C" fn retro_get_region() -> c_uint {
    RETRO_REGION_NTSC
}

/// Returns the save RAM for `RETRO_MEMORY_SAVE_RAM`, or null. The pointer
/// stays valid until `retro_unload_game` or `retro_deinit`.
#[unsafe(no_mangle)]
pub extern "C" fn retro_get_memory_data(id: c_uint) -> *mut c_void {
    if id != RETRO_MEMORY_SAVE_RAM {
        return ptr::null_mut();
    }
    guarded(ptr::null_mut(), || {
        with_global(|g| g.core.as_ref().map(|c| c.save_ram().as_ptr()))
            .flatten()
            .unwrap_or(ptr::null_mut())
            .cast()
    })
}

/// Returns the save RAM size for `RETRO_MEMORY_SAVE_RAM`, or 0.
#[unsafe(no_mangle)]
pub extern "C" fn retro_get_memory_size(id: c_uint) -> usize {
    if id != RETRO_MEMORY_SAVE_RAM {
        return 0;
    }
    guarded(0, || {
        with_global(|g| g.core.as_ref().map_or(0, |c| c.save_ram().len())).unwrap_or(0)
    })
}

/// Runs `f` and turns a panic into `default`. After a panic every later call
/// does nothing.
fn guarded<R>(default: R, f: impl FnOnce() -> R) -> R {
    if FAILED.load(Ordering::Acquire) {
        return default;
    }
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            FAILED.store(true, Ordering::Release);
            default
        }
    }
}

/// Runs `f` on the global state, with no result and no panic.
fn update(f: impl FnOnce(&mut Global)) {
    guarded((), || {
        // A busy or poisoned lock means a broken frontend or a dead core; the
        // call then has no effect, as documented at the top of the module.
        let _ = with_global(f);
    });
}

/// Runs `f` on the global state. Returns `None` when the lock is taken or a
/// panic poisoned it.
fn with_global<R>(f: impl FnOnce(&mut Global) -> R) -> Option<R> {
    let mut global = GLOBAL.try_lock().ok()?;
    Some(f(&mut global))
}

/// Calls the environment callback. Returns false when there is none.
///
/// `T` is the type that libretro.h names for `cmd`. Every caller passes a
/// constant `cmd` with a pointer to its matching type.
fn env<T>(callbacks: &Callbacks, cmd: c_uint, data: *mut T) -> bool {
    let Some(f) = callbacks.environment else {
        return false;
    };
    // SAFETY: `f` came through `retro_set_environment`, whose caller promises
    // that it conforms to libretro.h while the core is loaded. `data` is null or
    // points to a live value of the type that `cmd` names, for the length of
    // the call.
    unsafe { f(cmd, data.cast()) }
}

fn log(callbacks: &Callbacks, level: retro_log_level, message: &str) {
    let text = CString::new(message.replace('\0', " ")).unwrap_or_default();
    match callbacks.log {
        // SAFETY: `f` came through `GET_LOG_INTERFACE`, which the safety
        // contract of `retro_set_environment` covers. The format
        // `%s\n` takes one C string, and `text` is one, alive for the call.
        Some(f) => unsafe { f(level, c"%s\n".as_ptr(), text.as_ptr()) },
        None => {
            // Nothing else can report the error; a closed stderr loses it.
            let _ = writeln!(std::io::stderr(), "neptron-testcore: {message}");
        }
    }
}

fn read_buttons(callbacks: &Callbacks) -> u16 {
    if let Some(poll) = callbacks.input_poll {
        // SAFETY: `poll` came through `retro_set_input_poll`, whose caller
        // promises that it conforms to libretro.h while the core is loaded.
        unsafe { poll() };
    }
    let Some(state) = callbacks.input_state else {
        return 0;
    };
    (0..JOYPAD_BUTTONS).fold(0, |mask, id| {
        // SAFETY: `state` came through `retro_set_input_state`, whose caller
        // promises that it conforms to libretro.h while the core is loaded.
        // The arguments are plain integers.
        let pressed = unsafe { state(0, RETRO_DEVICE_JOYPAD, 0, id) } != 0;
        if pressed { mask | (1 << id) } else { mask }
    })
}

/// Makes the calls of one step into the frontend, in order.
fn send(callbacks: &Callbacks, step: &Step) {
    for request in &step.requests {
        // The script decides the frames whatever the frontend answers; a
        // frontend that refuses a change shows it in its own trace.
        let _accepted = match *request {
            Request::Geometry { geometry, max } => {
                let mut value = game_geometry(geometry, max);
                env(callbacks, RETRO_ENVIRONMENT_SET_GEOMETRY, &raw mut value)
            }
            Request::AvInfo(av) => {
                let mut value = system_av_info(av);
                env(
                    callbacks,
                    RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO,
                    &raw mut value,
                )
            }
            Request::Shutdown => {
                env::<c_void>(callbacks, RETRO_ENVIRONMENT_SHUTDOWN, ptr::null_mut())
            }
            Request::UnknownEnv => {
                env::<c_void>(callbacks, UNKNOWN_ENVIRONMENT_COMMAND, ptr::null_mut())
            }
            Request::GetVariable { option, offset } => {
                let code = get_variable(callbacks, option);
                update(|g| {
                    if let Some(core) = g.core.as_ref() {
                        core.save_ram()
                            .write(usize::try_from(offset).unwrap_or(usize::MAX), code);
                    }
                });
                code != VALUE_NOT_SET
            }
        };
    }
    send_video(callbacks, step);
    send_audio(callbacks, step);
}

/// Declares the core options with `SET_VARIABLES`. libretro.h makes the
/// frontend keep its own copy of the array; the strings are static.
fn declare_options(callbacks: &Callbacks) {
    let mut variables = [
        variable(OPTION_KEYS[0], OPTION_DECLARATIONS[0]),
        variable(OPTION_KEYS[1], OPTION_DECLARATIONS[1]),
        retro_variable {
            key: ptr::null(),
            value: ptr::null(),
        },
    ];
    // The core works with the defaults when the frontend has no options.
    let _declared = env(
        callbacks,
        RETRO_ENVIRONMENT_SET_VARIABLES,
        variables.as_mut_ptr(),
    );
}

fn variable(key: &'static CStr, value: &'static CStr) -> retro_variable {
    retro_variable {
        key: key.as_ptr(),
        value: value.as_ptr(),
    }
}

/// Asks for option `option` with `GET_VARIABLE` and returns its answer code.
fn get_variable(callbacks: &Callbacks, option: usize) -> u8 {
    let Some(key) = OPTION_KEYS.get(option) else {
        return VALUE_NOT_SET;
    };
    let mut var = retro_variable {
        key: key.as_ptr(),
        value: ptr::null(),
    };
    if !env(callbacks, RETRO_ENVIRONMENT_GET_VARIABLE, &raw mut var) || var.value.is_null() {
        return VALUE_NOT_SET;
    }
    // SAFETY: the environment callback conforms to libretro.h (the contract
    // of `retro_set_environment`). On success it sets `value` to a
    // NUL-terminated string, valid until the next call into the frontend.
    let value = unsafe { CStr::from_ptr(var.value) };
    value
        .to_str()
        .map_or(VALUE_NOT_LISTED, |v| value_code(option, Some(v)))
}

fn send_video(callbacks: &Callbacks, step: &Step) {
    let Some(refresh) = callbacks.video_refresh else {
        return;
    };
    match step.video {
        Video::Frame { size, pitch } => {
            let needed = pitch.checked_mul(usize::try_from(size.height).unwrap_or(usize::MAX));
            if needed.is_none_or(|n| n > step.frame_buffer.len()) {
                return;
            }
            // SAFETY: `refresh` came through `retro_set_video_refresh`, whose
            // caller promises that it conforms to libretro.h while the core is
            // loaded. The buffer holds `height` rows of
            // `pitch` bytes (checked above) and lives until the call returns.
            unsafe {
                refresh(
                    step.frame_buffer.as_ptr().cast(),
                    size.width,
                    size.height,
                    pitch,
                );
            }
        }
        Video::Dupe { size, pitch } => {
            // SAFETY: as above. A null pointer is the libretro signal for a
            // duplicate frame, which the frontend allowed with `GET_CAN_DUPE`.
            unsafe { refresh(ptr::null(), size.width, size.height, pitch) };
        }
        Video::Nothing => {}
    }
}

fn send_audio(callbacks: &Callbacks, step: &Step) {
    let samples = step.audio();
    match step.audio_mode {
        AudioMode::Batch => {
            let Some(batch) = callbacks.audio_sample_batch else {
                return;
            };
            let mut rest = samples;
            while !rest.is_empty() {
                // SAFETY: `batch` came through `retro_set_audio_sample_batch`,
                // whose caller promises that it conforms to libretro.h while
                // the core is loaded. `rest` holds `len / 2`
                // interleaved stereo frames and lives until the call returns.
                let taken = unsafe { batch(rest.as_ptr(), rest.len() / 2) };
                if taken == 0 {
                    break;
                }
                rest = rest.get(taken.saturating_mul(2)..).unwrap_or_default();
            }
        }
        AudioMode::Single => {
            let Some(single) = callbacks.audio_sample else {
                return;
            };
            for frame in samples.chunks_exact(2) {
                // SAFETY: `single` came through `retro_set_audio_sample`, whose
                // caller promises that it conforms to libretro.h while the core
                // is loaded. The arguments are plain integers.
                unsafe { single(frame[0], frame[1]) };
            }
        }
        AudioMode::Off => {}
    }
}

fn pixel_format_request(format: PixelFormat) -> Option<retro_pixel_format> {
    match format {
        PixelFormat::Xrgb8888 => Some(RETRO_PIXEL_FORMAT_XRGB8888),
        PixelFormat::Rgb565 => Some(RETRO_PIXEL_FORMAT_RGB565),
        PixelFormat::Rgb1555 => None,
    }
}

const ZERO_SIZE: Size = Size {
    width: 0,
    height: 0,
};
const ZERO_GEOMETRY: Geometry = Geometry {
    size: ZERO_SIZE,
    aspect_milli: 0,
};

fn game_geometry(geometry: Geometry, max: Size) -> retro_game_geometry {
    retro_game_geometry {
        base_width: geometry.size.width,
        base_height: geometry.size.height,
        max_width: max.width,
        max_height: max.height,
        aspect_ratio: geometry.aspect_milli as f32 / 1000.0,
    }
}

fn system_av_info(av: AvInfo) -> retro_system_av_info {
    retro_system_av_info {
        geometry: game_geometry(av.geometry, av.max),
        timing: retro_system_timing {
            fps: f64::from(av.timing.fps_mhz) / 1000.0,
            sample_rate: f64::from(av.timing.sample_rate_hz),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::Timing;

    #[test]
    fn av_info_converts_millihertz_and_thousandths() {
        let av = AvInfo {
            geometry: Geometry {
                size: Size {
                    width: 256,
                    height: 224,
                },
                aspect_milli: 1333,
            },
            max: Size {
                width: 320,
                height: 240,
            },
            timing: Timing {
                fps_mhz: 59_940,
                sample_rate_hz: 44_100,
            },
        };
        let info = system_av_info(av);
        assert_eq!(info.geometry.base_width, 256);
        assert_eq!(info.geometry.base_height, 224);
        assert_eq!(info.geometry.max_width, 320);
        assert_eq!(info.geometry.max_height, 240);
        assert_eq!(info.geometry.aspect_ratio, 1.333);
        assert_eq!(info.timing.fps, 59.94);
        assert_eq!(info.timing.sample_rate, 44_100.0);
    }

    #[test]
    fn only_the_non_default_formats_are_requested() {
        assert_eq!(
            pixel_format_request(PixelFormat::Xrgb8888),
            Some(RETRO_PIXEL_FORMAT_XRGB8888)
        );
        assert_eq!(
            pixel_format_request(PixelFormat::Rgb565),
            Some(RETRO_PIXEL_FORMAT_RGB565)
        );
        assert_eq!(pixel_format_request(PixelFormat::Rgb1555), None);
    }

    #[test]
    fn a_panic_becomes_the_default_value() {
        assert_eq!(guarded(7, || 8), 8);
        assert!(!FAILED.load(Ordering::Acquire));
        let value = guarded(7, || -> i32 { panic!("inside the core") });
        assert_eq!(value, 7);
        assert!(FAILED.load(Ordering::Acquire));
        assert_eq!(guarded(7, || 8), 7, "a dead core does nothing");
        FAILED.store(false, Ordering::Release);
    }

    #[test]
    fn calls_without_a_loaded_script_return_defaults() {
        assert_eq!(retro_api_version(), RETRO_API_VERSION);
        assert!(retro_get_memory_data(RETRO_MEMORY_SAVE_RAM).is_null());
        assert_eq!(retro_get_memory_size(RETRO_MEMORY_SAVE_RAM), 0);
        assert!(!retro_serialize(ptr::null_mut(), 0));
        retro_run();
        retro_reset();
    }

    #[test]
    fn c_option_strings_match_the_option_table() {
        for (i, option) in crate::options::CORE_OPTIONS.iter().enumerate() {
            assert_eq!(OPTION_KEYS[i].to_str(), Ok(option.key));
            assert_eq!(
                OPTION_DECLARATIONS[i].to_str().map(str::to_owned),
                Ok(crate::options::declaration(option))
            );
        }
    }

    #[test]
    fn library_strings_end_in_nul() {
        for s in [LIBRARY_NAME, LIBRARY_VERSION, VALID_EXTENSIONS] {
            assert_eq!(s.last(), Some(&0));
            assert_eq!(s.iter().filter(|&&b| b == 0).count(), 1);
        }
    }
}
