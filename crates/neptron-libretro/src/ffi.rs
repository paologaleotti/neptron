//! The only module with `unsafe` code: the library, the calls into the core,
//! the callbacks that the core calls, and the reads of pointers that the
//! core gives.
//!
//! **Trust.** The host trusts that the library is a libretro core that keeps
//! the memory rules of libretro.h. Everything else that comes from the core
//! is checked before use: null pointers, sizes, counts, strings (bounded),
//! floating-point values. The checks and decisions are in safe code
//! ([`State`] and the validators); this module only moves raw values in and
//! out.
//!
//! **State.** Callbacks carry no user pointer. The host owns the callback
//! state in a `Box` and lends it to the thread-local [`SLOT`] for each call
//! into the core ([`lend`]). A callback finds it there with `try_with` and
//! `try_borrow_mut`. A callback on another thread, outside a call from the
//! host, or nested in another callback finds nothing and returns its
//! default. One core per process makes one slot enough; `Host` enforces
//! that, and it is `!Send`, so every call into the core comes from the
//! thread that opened it.
//!
//! **Panics.** Each callback body runs in `catch_unwind` and has no panic
//! path of its own; a panic becomes the default answer and never crosses
//! the C boundary.

mod env;
#[cfg(test)]
mod fake_core;

use std::cell::RefCell;
use std::ffi::{c_char, c_uint, c_void};
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use libloading::Library;
use neptron_libretro_sys::*;

use crate::content::LoadedContent;
use crate::state::{CoreAvInfo, State};

thread_local! {
    /// The state of the host while it is inside a call into the core.
    static SLOT: RefCell<Option<Box<State>>> = const { RefCell::new(None) };
}

/// The longest string that the host reads from a core, NUL included.
pub const MAX_CORE_STRING_BYTES: usize = 16_384;

/// The state did not come back from the thread-local slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Poisoned;

/// Moves `state` into the thread-local slot, runs `call`, and moves it
/// back. A missing state, or a slot that is taken, gives `Poisoned` and
/// `call` does not run.
pub(crate) fn lend<R>(
    state: &mut Option<Box<State>>,
    call: impl FnOnce() -> R,
) -> Result<R, Poisoned> {
    let mut pending = Some(state.take().ok_or(Poisoned)?);
    // The closure runs only when the thread-local still exists; otherwise
    // `pending` keeps the state and it goes back below.
    let _ = SLOT.try_with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && slot.is_none()
        {
            *slot = pending.take();
        }
    });
    if let Some(back) = pending {
        *state = Some(back);
        return Err(Poisoned);
    }
    let result = call();
    *state = SLOT
        .try_with(|slot| slot.try_borrow_mut().ok().and_then(|mut s| s.take()))
        .ok()
        .flatten();
    if state.is_none() {
        return Err(Poisoned);
    }
    Ok(result)
}

/// Runs `f` on the lent state. `None` when no state is lent to this thread
/// or it is borrowed already.
fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    SLOT.try_with(|slot| {
        let mut slot = slot.try_borrow_mut().ok()?;
        let state = slot.as_deref_mut()?;
        Some(f(state))
    })
    .ok()
    .flatten()
}

/// Runs a callback body and turns a panic into `default`.
fn guard<R: Copy>(default: R, f: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

unsafe extern "C" {
    /// Sets the function that `neptron_libretro_log_printf` passes each
    /// formatted message to.
    fn neptron_libretro_set_log_sink(sink: Option<unsafe extern "C" fn(c_uint, *const c_char)>);
    /// The log function for `GET_LOG_INTERFACE`, defined in `log_shim.c`.
    fn neptron_libretro_log_printf(level: retro_log_level, fmt: *const c_char, ...);
}

/// Points the C log shim at [`log_sink`]. The host calls it before it gives
/// the core the log function, so the sink is always set when a core logs.
pub(crate) fn install_log_sink() {
    // SAFETY: the shim stores the pointer atomically. `log_sink` has the
    // signature that the shim calls and lives for the whole process.
    unsafe { neptron_libretro_set_log_sink(Some(log_sink)) };
}

/// Why a library could not be opened as a core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpenFailure {
    /// The dynamic loader refused the file.
    Library(String),
    /// A libretro entry point is not exported.
    MissingSymbol(&'static str),
}

/// The fields of a `retro_system_info`, copied out of the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoreSystemInfo {
    /// Empty for a null pointer; `None` for a string with no NUL in
    /// [`MAX_CORE_STRING_BYTES`].
    pub name: Option<Vec<u8>>,
    pub version: Option<Vec<u8>>,
    pub extensions: Option<Vec<u8>>,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

/// A loaded core: its entry points. The library is never closed, so the
/// entry points stay valid for the rest of the process.
pub(crate) struct Core {
    _library: ManuallyDrop<Option<Library>>,
    set_environment: RetroSetEnvironmentFn,
    set_video_refresh: RetroSetVideoRefreshFn,
    set_audio_sample: RetroSetAudioSampleFn,
    set_audio_sample_batch: RetroSetAudioSampleBatchFn,
    set_input_poll: RetroSetInputPollFn,
    set_input_state: RetroSetInputStateFn,
    init: RetroInitFn,
    deinit: RetroDeinitFn,
    api_version: RetroApiVersionFn,
    get_system_info: RetroGetSystemInfoFn,
    get_system_av_info: RetroGetSystemAvInfoFn,
    set_controller_port_device: RetroSetControllerPortDeviceFn,
    reset: RetroResetFn,
    run: RetroRunFn,
    load_game: RetroLoadGameFn,
    unload_game: RetroUnloadGameFn,
}

/// The entry points of a core, for a core that is not a library (the fake
/// core of the tests).
#[derive(Clone, Copy)]
pub(crate) struct EntryPoints {
    pub set_environment: RetroSetEnvironmentFn,
    pub set_video_refresh: RetroSetVideoRefreshFn,
    pub set_audio_sample: RetroSetAudioSampleFn,
    pub set_audio_sample_batch: RetroSetAudioSampleBatchFn,
    pub set_input_poll: RetroSetInputPollFn,
    pub set_input_state: RetroSetInputStateFn,
    pub init: RetroInitFn,
    pub deinit: RetroDeinitFn,
    pub api_version: RetroApiVersionFn,
    pub get_system_info: RetroGetSystemInfoFn,
    pub get_system_av_info: RetroGetSystemAvInfoFn,
    pub set_controller_port_device: RetroSetControllerPortDeviceFn,
    pub reset: RetroResetFn,
    pub run: RetroRunFn,
    pub load_game: RetroLoadGameFn,
    pub unload_game: RetroUnloadGameFn,
}

impl Core {
    /// Opens the library at `path` and finds all 25 entry points.
    pub(crate) fn open(path: &Path) -> Result<Self, OpenFailure> {
        // SAFETY: opening the library runs its initialisers. The host
        // trusts that the file is a libretro core (see the module comment);
        // the caller chose it from the installed cores.
        let library =
            unsafe { Library::new(path) }.map_err(|e| OpenFailure::Library(e.to_string()))?;
        for name in CORE_API_SYMBOLS {
            // SAFETY: only the presence of the symbol is checked; the
            // pointer is not used.
            if unsafe { library.get::<*const c_void>(name) }.is_err() {
                return Err(OpenFailure::MissingSymbol(name));
            }
        }
        macro_rules! entry {
            ($name:literal) => {
                // SAFETY: the type is the function pointer type that
                // libretro.h declares for this entry point, and the symbol
                // exists (checked above). The library is never closed, so
                // the pointer stays valid.
                *unsafe { library.get($name) }.map_err(|_| OpenFailure::MissingSymbol($name))?
            };
        }
        let points = EntryPoints {
            set_environment: entry!("retro_set_environment"),
            set_video_refresh: entry!("retro_set_video_refresh"),
            set_audio_sample: entry!("retro_set_audio_sample"),
            set_audio_sample_batch: entry!("retro_set_audio_sample_batch"),
            set_input_poll: entry!("retro_set_input_poll"),
            set_input_state: entry!("retro_set_input_state"),
            init: entry!("retro_init"),
            deinit: entry!("retro_deinit"),
            api_version: entry!("retro_api_version"),
            get_system_info: entry!("retro_get_system_info"),
            get_system_av_info: entry!("retro_get_system_av_info"),
            set_controller_port_device: entry!("retro_set_controller_port_device"),
            reset: entry!("retro_reset"),
            run: entry!("retro_run"),
            load_game: entry!("retro_load_game"),
            unload_game: entry!("retro_unload_game"),
        };
        Ok(Self::with_entry_points(Some(library), points))
    }

    /// A core from entry points that live for the whole process: the
    /// library they come from, or none for entry points in this program.
    pub(crate) fn with_entry_points(library: Option<Library>, p: EntryPoints) -> Self {
        Self {
            _library: ManuallyDrop::new(library),
            set_environment: p.set_environment,
            set_video_refresh: p.set_video_refresh,
            set_audio_sample: p.set_audio_sample,
            set_audio_sample_batch: p.set_audio_sample_batch,
            set_input_poll: p.set_input_poll,
            set_input_state: p.set_input_state,
            init: p.init,
            deinit: p.deinit,
            api_version: p.api_version,
            get_system_info: p.get_system_info,
            get_system_av_info: p.get_system_av_info,
            set_controller_port_device: p.set_controller_port_device,
            reset: p.reset,
            run: p.run,
            load_game: p.load_game,
            unload_game: p.unload_game,
        }
    }

    pub(crate) fn api_version(&self) -> u32 {
        // SAFETY: an entry point of the core (see `Core`), with no arguments.
        unsafe { (self.api_version)() }
    }

    /// Gives the core the six callbacks, environment first, as libretro.h
    /// asks before `retro_init`.
    pub(crate) fn set_callbacks(&self) {
        // SAFETY: entry points of the core. Each callback has the type that
        // libretro.h names and lives for the whole process.
        unsafe {
            (self.set_environment)(Some(environment));
            (self.set_video_refresh)(Some(video_refresh));
            (self.set_audio_sample)(Some(audio_sample));
            (self.set_audio_sample_batch)(Some(audio_sample_batch));
            (self.set_input_poll)(Some(input_poll));
            (self.set_input_state)(Some(input_state));
        }
    }

    pub(crate) fn init(&self) {
        // SAFETY: an entry point of the core, after `set_callbacks`.
        unsafe { (self.init)() }
    }

    pub(crate) fn deinit(&self) {
        // SAFETY: an entry point of the core, after `retro_init`.
        unsafe { (self.deinit)() }
    }

    pub(crate) fn run(&self) {
        // SAFETY: an entry point of the core; the host calls it only with a
        // game loaded.
        unsafe { (self.run)() }
    }

    pub(crate) fn reset(&self) {
        // SAFETY: an entry point of the core; the host calls it only with a
        // game loaded.
        unsafe { (self.reset)() }
    }

    pub(crate) fn unload_game(&self) {
        // SAFETY: an entry point of the core; the host calls it once for
        // each successful `retro_load_game`.
        unsafe { (self.unload_game)() }
    }

    pub(crate) fn set_controller_port_device(&self, port: u32, device: u32) {
        // SAFETY: an entry point of the core, with plain integers.
        unsafe { (self.set_controller_port_device)(port, device) }
    }

    pub(crate) fn system_info(&self) -> CoreSystemInfo {
        let mut info = retro_system_info {
            library_name: std::ptr::null(),
            library_version: std::ptr::null(),
            valid_extensions: std::ptr::null(),
            need_fullpath: false,
            block_extract: false,
        };
        // SAFETY: an entry point of the core; `info` is a writable
        // `retro_system_info` for the length of the call.
        unsafe { (self.get_system_info)(&raw mut info) };
        let copy = |p: *const c_char| {
            if p.is_null() {
                return Some(Vec::new());
            }
            // SAFETY: libretro.h: the strings are NUL-terminated and stay
            // valid while the core is loaded. They are copied at once.
            unsafe { c_str_bytes(p) }.map(<[u8]>::to_vec)
        };
        CoreSystemInfo {
            name: copy(info.library_name),
            version: copy(info.library_version),
            extensions: copy(info.valid_extensions),
            need_fullpath: info.need_fullpath,
            block_extract: info.block_extract,
        }
    }

    pub(crate) fn system_av_info(&self) -> CoreAvInfo {
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
        // SAFETY: an entry point of the core; `av` is a writable
        // `retro_system_av_info` for the length of the call.
        unsafe { (self.get_system_av_info)(&raw mut av) };
        core_av_info(&av)
    }

    /// Calls `retro_load_game` with the content, or with null.
    pub(crate) fn load_game(&self, content: Option<&LoadedContent>) -> bool {
        let info = content.map_or(std::ptr::null(), |c| std::ptr::from_ref(c.info()));
        // SAFETY: an entry point of the core. `info` is null or points to a
        // `retro_game_info` whose pointers `LoadedContent` keeps valid for
        // its own life; the host keeps it until `retro_unload_game`.
        unsafe { (self.load_game)(info) }
    }
}

fn core_av_info(av: &retro_system_av_info) -> CoreAvInfo {
    CoreAvInfo {
        base_width: av.geometry.base_width,
        base_height: av.geometry.base_height,
        max_width: av.geometry.max_width,
        max_height: av.geometry.max_height,
        aspect: av.geometry.aspect_ratio,
        fps: av.timing.fps,
        sample_rate: av.timing.sample_rate,
    }
}

/// The bytes of a C string, without the NUL. `None` for a null pointer or
/// a string with no NUL in its first [`MAX_CORE_STRING_BYTES`] bytes.
///
/// # Safety
///
/// `ptr` is null or points to a NUL-terminated string that stays valid and
/// unchanged for `'a`. The read stops at the NUL, so it never reads past
/// the string.
unsafe fn c_str_bytes<'a>(ptr: *const c_char) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return None;
    }
    let bytes = ptr.cast::<u8>();
    for len in 0..MAX_CORE_STRING_BYTES {
        // SAFETY: the caller guarantees a NUL-terminated string, and the
        // loop stops at its NUL, so `len` is inside the string.
        if unsafe { bytes.add(len).read() } == 0 {
            // SAFETY: the `len` bytes before the NUL were just read.
            return Some(unsafe { std::slice::from_raw_parts(bytes, len) });
        }
    }
    None
}

/// `retro_environment_t`.
///
/// # Safety
///
/// The core keeps libretro.h: `data` is null or points to the type that
/// `cmd` names, valid for the length of the call.
unsafe extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    guard(false, || {
        // SAFETY: the caller's guarantee, passed on.
        with_state(|state| unsafe { env::dispatch(state, cmd, data) }).unwrap_or(false)
    })
}

/// `retro_video_refresh_t`. A null `data` is a duplicate frame.
///
/// # Safety
///
/// The core keeps libretro.h: `data` is null or holds `height` rows `pitch`
/// bytes apart, each with `width` pixels of the pixel format in force,
/// valid for the length of the call.
unsafe extern "C" fn video_refresh(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    guard((), || {
        with_state(|state| {
            if data.is_null() {
                state.video_mut().duplicate();
                return;
            }
            let plan = match state.frame_plan(width, height, pitch) {
                Ok(plan) => plan,
                Err(fault) => {
                    state.video_mut().reject(fault);
                    return;
                }
            };
            let video = state.video_mut();
            let Some(target) = video.target(&plan) else {
                return;
            };
            let source = data.cast::<u8>();
            for (row, out) in target.chunks_exact_mut(plan.row_bytes).enumerate() {
                // SAFETY: `row < height`, and the plan checked that
                // `pitch * (height - 1) + row_bytes` fits in `usize`. The
                // caller guarantees that this row of `row_bytes` bytes is in
                // the frame.
                let line = unsafe {
                    std::slice::from_raw_parts(source.add(row * plan.pitch), plan.row_bytes)
                };
                out.copy_from_slice(line);
            }
            video.finish(plan);
        });
    });
}

/// `retro_audio_sample_t`: one stereo frame.
extern "C" fn audio_sample(left: i16, right: i16) {
    guard((), || {
        with_state(|state| state.audio_mut().push(&[left, right]));
    });
}

/// `retro_audio_sample_batch_t`. Returns `frames` whether the samples fit
/// or not, so a core that waits until its audio is taken does not hang.
///
/// # Safety
///
/// The core keeps libretro.h: `data` is null or holds `frames` interleaved
/// stereo frames, valid for the length of the call.
unsafe extern "C" fn audio_sample_batch(data: *const i16, frames: usize) -> usize {
    guard(frames, || {
        with_state(|state| {
            let audio = state.audio_mut();
            if data.is_null() || !data.is_aligned() {
                audio.drop_frames(frames);
                return;
            }
            let take = frames.min(audio.room_frames());
            // SAFETY: `data` is not null and aligned, and holds `frames`
            // stereo frames (the caller's guarantee); `take <= frames`, and
            // `take * 2` fits because `take` is at most the buffer room.
            let samples = unsafe { std::slice::from_raw_parts(data, take * 2) };
            audio.push(samples);
            audio.drop_frames(frames - take);
        });
        frames
    })
}

/// `retro_input_poll_t`. The pads are set before the frame, so nothing is
/// polled here.
extern "C" fn input_poll() {}

/// `retro_input_state_t`.
extern "C" fn input_state(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16 {
    guard(0, || {
        with_state(|state| state.input(port, device, index, id)).unwrap_or(0)
    })
}

/// The sink of the C log shim: one formatted message.
///
/// # Safety
///
/// `text` is null or a NUL-terminated string that is valid for the length
/// of the call. The shim passes its own stack buffer.
unsafe extern "C" fn log_sink(level: c_uint, text: *const c_char) {
    guard((), || {
        // SAFETY: the caller's guarantee.
        let Some(bytes) = (unsafe { c_str_bytes(text) }) else {
            return;
        };
        with_state(|state| state.core_log(level, bytes));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    fn state() -> Option<Box<State>> {
        Some(Box::new(State::new(
            c"/bios".to_owned(),
            c"/saves".to_owned(),
        )))
    }

    #[test]
    fn a_lent_state_is_found_and_comes_back() {
        let mut s = state();
        let seen = lend(&mut s, || with_state(|_| 7)).unwrap();
        assert_eq!(seen, Some(7));
        assert!(s.is_some());
        assert_eq!(with_state(|_| 7), None, "nothing is lent now");
    }

    #[test]
    fn a_nested_lookup_finds_nothing() {
        let mut s = state();
        let nested = lend(&mut s, || with_state(|_| with_state(|_| 1))).unwrap();
        assert_eq!(nested, Some(None));
    }

    #[test]
    fn no_state_means_poisoned() {
        let mut none = None;
        assert_eq!(lend(&mut none, || 1), Err(Poisoned));
    }

    #[test]
    fn another_thread_finds_no_state() {
        let mut s = state();
        let other = lend(&mut s, || {
            std::thread::scope(|scope| scope.spawn(|| with_state(|_| 1)).join().unwrap())
        })
        .unwrap();
        assert_eq!(other, None);
    }

    #[test]
    fn callbacks_without_a_host_return_defaults() {
        let mut pixel = 0u32;
        // SAFETY: the data matches each command, and nothing is lent.
        unsafe {
            assert!(!environment(
                RETRO_ENVIRONMENT_GET_CAN_DUPE,
                (&raw mut pixel).cast()
            ));
            video_refresh(std::ptr::null(), 1, 1, 4);
            assert_eq!(audio_sample_batch(std::ptr::null(), 99), 99);
            log_sink(1, c"lost".as_ptr());
        }
        audio_sample(1, 2);
        assert_eq!(input_state(0, RETRO_DEVICE_JOYPAD, 0, 0), 0);
    }

    #[test]
    fn a_c_string_is_bounded() {
        let long = vec![b'x'; MAX_CORE_STRING_BYTES + 10];
        let mut terminated = long.clone();
        terminated.push(0);
        // SAFETY: each pointer is a live buffer. The unterminated one is
        // longer than the bound, so the read stops inside it.
        unsafe {
            assert_eq!(c_str_bytes(c"abc".as_ptr()), Some(&b"abc"[..]));
            assert_eq!(c_str_bytes(std::ptr::null()), None);
            assert_eq!(c_str_bytes(terminated.as_ptr().cast()), None);
            assert_eq!(c_str_bytes(long.as_ptr().cast()), None);
        }
    }
}
