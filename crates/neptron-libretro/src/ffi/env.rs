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

use super::{
    HwCallback, c_str_bytes, core_av_info, hw_get_current_framebuffer, hw_get_proc_address,
    neptron_libretro_log_printf,
};
use crate::descriptor::{MAX_INPUT_DESCRIPTORS, RawDescriptor};
use crate::hw::RawHwRender;
use crate::options::MAX_OPTIONS;
use crate::state::{MAX_CONTENT_OVERRIDES, State};

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
        RETRO_ENVIRONMENT_SET_VARIABLES => {
            if data.is_null() {
                return false;
            }
            // SAFETY: SET_VARIABLES takes an array of `struct retro_variable`
            // that ends with a null key, with NUL-terminated strings, valid
            // for the length of the call.
            let entries = unsafe { variables(data.cast()) };
            state.declare_options(&entries);
            true
        }
        RETRO_ENVIRONMENT_GET_VARIABLE => {
            // SAFETY: GET_VARIABLE takes a `struct retro_variable *`.
            let Some(variable) = (unsafe { read::<retro_variable>(data) }) else {
                return false;
            };
            // SAFETY: libretro.h: the key is a NUL-terminated string, valid
            // for the length of the call.
            let key = unsafe { c_str_bytes(variable.key) };
            let value = key.and_then(|k| state.option_value(k));
            let answer = retro_variable {
                key: variable.key,
                value: value.map_or(std::ptr::null(), |v| v.as_ptr()),
            };
            // SAFETY: as above. The value lives in the state until the
            // option changes, which happens only between calls into the core.
            let written = unsafe { write(data, answer) };
            written && value.is_some()
        }
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
            let updated = state.take_option_update();
            // SAFETY: GET_VARIABLE_UPDATE takes a `bool *`.
            unsafe { write(data, updated) }
        }
        RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION => {
            // SAFETY: GET_CORE_OPTIONS_VERSION takes an `unsigned *`.
            unsafe { write::<u32>(data, 0) }
        }
        RETRO_ENVIRONMENT_SET_CORE_OPTIONS
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_UPDATE_DISPLAY_CALLBACK => {
            state.refuse(cmd, "only the version 0 option API is supported")
        }
        RETRO_ENVIRONMENT_SET_CONTENT_INFO_OVERRIDE => {
            if data.is_null() {
                return false;
            }
            // SAFETY: SET_CONTENT_INFO_OVERRIDE takes an array of
            // `struct retro_system_content_info_override` that ends with null
            // extensions, valid for the length of the call.
            let entries = unsafe { content_overrides(data.cast()) };
            state.set_content_overrides(&entries);
            true
        }
        RETRO_ENVIRONMENT_GET_GAME_INFO_EXT => {
            let info = state.game_info_ext();
            // SAFETY: GET_GAME_INFO_EXT takes a
            // `const struct retro_game_info_ext **`. The pointee lives in the
            // host until `retro_unload_game`.
            !info.is_null() && unsafe { write(data, info) }
        }
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS => {
            if data.is_null() {
                return false;
            }
            // SAFETY: SET_INPUT_DESCRIPTORS takes an array of
            // `struct retro_input_descriptor` that ends with a null
            // description, with NUL-terminated strings, valid for the length
            // of the call.
            let (entries, cut) = unsafe { input_descriptors(data.cast()) };
            state.set_input_descriptors(cmd, &entries, cut);
            true
        }
        RETRO_ENVIRONMENT_SET_CONTROLLER_INFO | RETRO_ENVIRONMENT_GET_INPUT_BITMASKS => true,
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
        RETRO_ENVIRONMENT_SET_ROTATION => {
            // SAFETY: SET_ROTATION takes a `const unsigned *`.
            let value = unsafe { read::<u32>(data) };
            value.is_some_and(|v| state.set_rotation(v))
        }
        RETRO_ENVIRONMENT_SET_MEMORY_MAPS => state.refuse(cmd, "memory maps are not used"),
        RETRO_ENVIRONMENT_SET_HW_RENDER => {
            // SAFETY: SET_HW_RENDER takes a `struct retro_hw_render_callback *`,
            // read with its `bool` fields as bytes.
            let Some(raw) = (unsafe { read::<HwRenderBytes>(data) }) else {
                return false;
            };
            let request = RawHwRender {
                context_type: raw.context_type,
                version_major: raw.version_major,
                version_minor: raw.version_minor,
                depth: raw.depth != 0,
                stencil: raw.stencil != 0,
                bottom_left_origin: raw.bottom_left_origin != 0,
            };
            let reset = HwCallback::of(raw.context_reset);
            let destroy = HwCallback::of(raw.context_destroy);
            if !state.set_hw_render(&request, reset, destroy) {
                return false;
            }
            let answer = HwRenderBytes {
                get_current_framebuffer: Some(hw_get_current_framebuffer),
                get_proc_address: Some(hw_get_proc_address),
                ..raw
            };
            // SAFETY: as above; the core fills the struct and the frontend
            // writes the two callbacks into it.
            unsafe { write(data, answer) }
        }
        RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER => match state.preferred_hw_render() {
            // SAFETY: GET_PREFERRED_HW_RENDER takes an `unsigned *`.
            Some(context_type) => unsafe { write::<u32>(data, context_type) },
            None => state.refuse(cmd, "the display offers no hardware context"),
        },
        RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE => {
            state.refuse(cmd, "a GL context has no render interface")
        }
        RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE
        | RETRO_ENVIRONMENT_GET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_SUPPORT => {
            state.refuse(cmd, "context negotiation is for Vulkan only")
        }
        RETRO_ENVIRONMENT_SET_HW_SHARED_CONTEXT => {
            state.refuse(cmd, "the core never shares the context of the frontend")
        }
        _ => state.unknown(cmd),
    }
}

/// The key and declaration of each `retro_variable` up to the null key, at
/// most `MAX_OPTIONS + 1` of them, so the store can report the excess. An
/// entry with an unterminated or null string is passed as empty, and the
/// store skips it.
///
/// # Safety
///
/// `array` points to `retro_variable` entries that end with a null key, with
/// NUL-terminated strings, valid for `'a`.
unsafe fn variables<'a>(array: *const retro_variable) -> Vec<(&'a [u8], &'a [u8])> {
    let mut entries = Vec::new();
    for i in 0..=MAX_OPTIONS {
        // SAFETY: the array goes on until the null key, and the loop stops
        // there, so entry `i` exists. The read needs no alignment.
        let entry = unsafe { array.add(i).read_unaligned() };
        if entry.key.is_null() {
            break;
        }
        // SAFETY: the strings are NUL-terminated and valid for `'a`.
        let (key, value) = unsafe { (c_str_bytes(entry.key), c_str_bytes(entry.value)) };
        entries.push((key.unwrap_or_default(), value.unwrap_or_default()));
    }
    entries
}

/// The entries of a `retro_input_descriptor` array up to the null
/// description, at most [`MAX_INPUT_DESCRIPTORS`], and whether the array
/// goes on after them. An entry with an unterminated description is
/// skipped.
///
/// # Safety
///
/// `array` points to entries that end with a null description, with
/// NUL-terminated strings, valid for `'a`.
unsafe fn input_descriptors<'a>(
    array: *const retro_input_descriptor,
) -> (Vec<RawDescriptor<'a>>, bool) {
    let mut entries = Vec::new();
    for i in 0..=MAX_INPUT_DESCRIPTORS {
        // SAFETY: the array goes on until the null description, and the
        // loop stops there, so entry `i` exists. The read needs no
        // alignment.
        let entry = unsafe { array.add(i).read_unaligned() };
        if entry.description.is_null() {
            return (entries, false);
        }
        if i == MAX_INPUT_DESCRIPTORS {
            break;
        }
        // SAFETY: the string is NUL-terminated and valid for `'a`.
        if let Some(description) = unsafe { c_str_bytes(entry.description) } {
            entries.push(RawDescriptor {
                port: entry.port,
                device: entry.device,
                index: entry.index,
                id: entry.id,
                description,
            });
        }
    }
    (entries, true)
}

/// The extensions and `need_fullpath` of each override entry up to the one
/// with null extensions, at most [`MAX_CONTENT_OVERRIDES`] of them. An entry
/// with an unterminated string is skipped.
///
/// # Safety
///
/// `array` points to entries that end with null extensions, with
/// NUL-terminated strings, valid for `'a`.
unsafe fn content_overrides<'a>(
    array: *const retro_system_content_info_override,
) -> Vec<(&'a [u8], bool)> {
    let mut entries = Vec::new();
    for i in 0..MAX_CONTENT_OVERRIDES {
        // SAFETY: the array goes on until the null entry, and the loop stops
        // there, so entry `i` exists. The read needs no alignment; the
        // `bool` field is read as a byte below.
        let entry = unsafe { array.add(i).cast::<OverrideBytes>().read_unaligned() };
        if entry.extensions.is_null() {
            break;
        }
        // SAFETY: the string is NUL-terminated and valid for `'a`.
        if let Some(extensions) = unsafe { c_str_bytes(entry.extensions) } {
            entries.push((extensions, entry.need_fullpath != 0));
        }
    }
    entries
}

/// `retro_system_content_info_override` with its `bool` fields as bytes, so
/// that any byte the core wrote is a valid value.
#[repr(C)]
#[derive(Clone, Copy)]
struct OverrideBytes {
    extensions: *const c_char,
    need_fullpath: u8,
    persistent_data: u8,
}

const _: () = assert!(
    size_of::<OverrideBytes>() == size_of::<retro_system_content_info_override>()
        && align_of::<OverrideBytes>() == align_of::<retro_system_content_info_override>()
);

/// `retro_hw_render_callback` with its `bool` fields as bytes, so that any
/// byte the core wrote is a valid value.
#[repr(C)]
#[derive(Clone, Copy)]
struct HwRenderBytes {
    context_type: retro_hw_context_type,
    context_reset: retro_hw_context_reset_t,
    get_current_framebuffer: retro_hw_get_current_framebuffer_t,
    get_proc_address: retro_hw_get_proc_address_t,
    depth: u8,
    stencil: u8,
    bottom_left_origin: u8,
    version_major: u32,
    version_minor: u32,
    cache_context: u8,
    context_destroy: retro_hw_context_reset_t,
    debug_context: u8,
}

const _: () = {
    use std::mem::offset_of;
    type C = retro_hw_render_callback;
    type B = HwRenderBytes;
    assert!(size_of::<B>() == size_of::<C>() && align_of::<B>() == align_of::<C>());
    assert!(offset_of!(B, context_reset) == offset_of!(C, context_reset));
    assert!(offset_of!(B, get_current_framebuffer) == offset_of!(C, get_current_framebuffer));
    assert!(offset_of!(B, get_proc_address) == offset_of!(C, get_proc_address));
    assert!(offset_of!(B, depth) == offset_of!(C, depth));
    assert!(offset_of!(B, stencil) == offset_of!(C, stencil));
    assert!(offset_of!(B, bottom_left_origin) == offset_of!(C, bottom_left_origin));
    assert!(offset_of!(B, version_major) == offset_of!(C, version_major));
    assert!(offset_of!(B, version_minor) == offset_of!(C, version_minor));
    assert!(offset_of!(B, cache_context) == offset_of!(C, cache_context));
    assert!(offset_of!(B, context_destroy) == offset_of!(C, context_destroy));
    assert!(offset_of!(B, debug_context) == offset_of!(C, debug_context));
};

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
