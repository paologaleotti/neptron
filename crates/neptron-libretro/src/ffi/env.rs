//! The environment callback: reads the data of each command, hands plain
//! values to [`State`], and writes the answer back.
//!
//! Every `read` and `write` below relies on the guarantee of [`dispatch`]:
//! `data` is null or points to the type that libretro.h names for `cmd`.
//! Both helpers check for null and need no alignment, and each type read is
//! valid for any bit pattern (integers, floats, raw pointers; a C `bool` is
//! read as a byte).

use std::ffi::{c_char, c_void};

use neptron_libretro_sys::*;

use super::{c_str_bytes, core_av_info, neptron_libretro_log_printf};
use crate::state::State;

/// Answers one environment call.
///
/// # Safety
///
/// `data` is null or points to the type that libretro.h names for `cmd`,
/// valid for the length of the call.
pub(super) unsafe fn dispatch(state: &mut State, cmd: u32, data: *mut c_void) -> bool {
    match cmd {
        RETRO_ENVIRONMENT_GET_CAN_DUPE => {
            // SAFETY: GET_CAN_DUPE takes a `bool *`.
            unsafe { write(data, true) }
        }
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
            // SAFETY: SET_PIXEL_FORMAT takes a `const enum retro_pixel_format *`.
            let value = unsafe { read::<retro_pixel_format>(data) };
            value.is_some_and(|v| state.set_pixel_format(v))
        }
        RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY => {
            // SAFETY: GET_SYSTEM_DIRECTORY takes a `const char **`. The
            // string lives in the state for the life of the host.
            unsafe { write::<*const c_char>(data, state.system_dir().as_ptr()) }
        }
        RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY => {
            // SAFETY: GET_SAVE_DIRECTORY takes a `const char **`. The string
            // lives in the state for the life of the host.
            unsafe { write::<*const c_char>(data, state.save_dir().as_ptr()) }
        }
        RETRO_ENVIRONMENT_GET_LOG_INTERFACE => {
            let callback = retro_log_callback {
                log: Some(neptron_libretro_log_printf),
            };
            // SAFETY: GET_LOG_INTERFACE takes a `struct retro_log_callback *`.
            unsafe { write(data, callback) }
        }
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
        | RETRO_ENVIRONMENT_SET_CONTROLLER_INFO
        | RETRO_ENVIRONMENT_GET_INPUT_BITMASKS => true,
        RETRO_ENVIRONMENT_SHUTDOWN => {
            state.shutdown();
            true
        }
        RETRO_ENVIRONMENT_SET_GEOMETRY => {
            // SAFETY: SET_GEOMETRY takes a `const struct retro_game_geometry *`.
            let geometry = unsafe { read::<retro_game_geometry>(data) };
            geometry.is_some_and(|g| {
                state.set_geometry(cmd, g.base_width, g.base_height, g.aspect_ratio)
            })
        }
        RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO => {
            // SAFETY: SET_SYSTEM_AV_INFO takes a
            // `const struct retro_system_av_info *`.
            let av = unsafe { read::<retro_system_av_info>(data) };
            av.is_some_and(|av| state.set_av_info(cmd, &core_av_info(&av)))
        }
        RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME => {
            // SAFETY: SET_SUPPORT_NO_GAME takes a `const bool *`, read as a
            // byte.
            let flag = unsafe { read::<u8>(data) };
            flag.map(|f| state.set_support_no_game(f != 0)).is_some()
        }
        RETRO_ENVIRONMENT_GET_LANGUAGE => {
            // SAFETY: GET_LANGUAGE takes an `unsigned *`.
            unsafe { write::<retro_language>(data, RETRO_LANGUAGE_ENGLISH) }
        }
        RETRO_ENVIRONMENT_SET_MESSAGE => {
            // SAFETY: SET_MESSAGE takes a `const struct retro_message *`.
            let Some(message) = (unsafe { read::<retro_message>(data) }) else {
                return false;
            };
            // SAFETY: libretro.h: `msg` is a NUL-terminated string, valid
            // for the length of the call. It is copied at once.
            let Some(text) = (unsafe { c_str_bytes(message.msg) }) else {
                return false;
            };
            state.message(text, message.frames);
            true
        }
        RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE => {
            let enabled: retro_av_enable_flags = RETRO_AV_ENABLE_VIDEO | RETRO_AV_ENABLE_AUDIO;
            // libretro.h allows null: the core only asks whether the call
            // exists.
            // SAFETY: GET_AUDIO_VIDEO_ENABLE takes a
            // `enum retro_av_enable_flags *`.
            data.is_null() || unsafe { write(data, enabled) }
        }
        RETRO_ENVIRONMENT_SET_ROTATION => state.refuse(cmd, "rotation is not supported"),
        RETRO_ENVIRONMENT_SET_MEMORY_MAPS => state.refuse(cmd, "memory maps are not used"),
        RETRO_ENVIRONMENT_SET_HW_RENDER
        | RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE
        | RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE
        | RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER => {
            state.refuse(cmd, "hardware rendering is not supported")
        }
        _ => state.unknown(cmd),
    }
}

/// Reads a `T` from `data`. `None` for null.
///
/// # Safety
///
/// `data` is null or points to a readable `T`, of any alignment, whose
/// bytes are a valid `T`.
unsafe fn read<T: Copy>(data: *mut c_void) -> Option<T> {
    if data.is_null() {
        return None;
    }
    // SAFETY: the caller's guarantee; the read needs no alignment.
    Some(unsafe { data.cast::<T>().read_unaligned() })
}

/// Writes `value` to `data`. False for null.
///
/// # Safety
///
/// `data` is null or points to a writable `T`, of any alignment.
unsafe fn write<T>(data: *mut c_void, value: T) -> bool {
    if data.is_null() {
        return false;
    }
    // SAFETY: the caller's guarantee; the write needs no alignment.
    unsafe { data.cast::<T>().write_unaligned(value) };
    true
}
