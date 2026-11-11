//! Function pointer types of the core entry points, for a frontend that loads
//! a core at run time.
//!
//! The header declares the entry points as functions that the frontend links
//! against. A frontend that opens a core as a dynamic library looks each one up
//! by name and calls it through these types. A test in this crate proves that
//! each type matches the declaration that bindgen generates from the header.

use std::os::raw::{c_char, c_uint, c_void};

use crate::{
    retro_audio_sample_batch_t, retro_audio_sample_t, retro_environment_t, retro_game_info,
    retro_input_poll_t, retro_input_state_t, retro_system_av_info, retro_system_info,
    retro_video_refresh_t,
};

/// `void retro_set_environment(retro_environment_t)`
pub type RetroSetEnvironmentFn = unsafe extern "C" fn(cb: retro_environment_t);
/// `void retro_set_video_refresh(retro_video_refresh_t)`
pub type RetroSetVideoRefreshFn = unsafe extern "C" fn(cb: retro_video_refresh_t);
/// `void retro_set_audio_sample(retro_audio_sample_t)`
pub type RetroSetAudioSampleFn = unsafe extern "C" fn(cb: retro_audio_sample_t);
/// `void retro_set_audio_sample_batch(retro_audio_sample_batch_t)`
pub type RetroSetAudioSampleBatchFn = unsafe extern "C" fn(cb: retro_audio_sample_batch_t);
/// `void retro_set_input_poll(retro_input_poll_t)`
pub type RetroSetInputPollFn = unsafe extern "C" fn(cb: retro_input_poll_t);
/// `void retro_set_input_state(retro_input_state_t)`
pub type RetroSetInputStateFn = unsafe extern "C" fn(cb: retro_input_state_t);
/// `void retro_init(void)`
pub type RetroInitFn = unsafe extern "C" fn();
/// `void retro_deinit(void)`
pub type RetroDeinitFn = unsafe extern "C" fn();
/// `unsigned retro_api_version(void)`
pub type RetroApiVersionFn = unsafe extern "C" fn() -> c_uint;
/// `void retro_get_system_info(struct retro_system_info *)`
pub type RetroGetSystemInfoFn = unsafe extern "C" fn(info: *mut retro_system_info);
/// `void retro_get_system_av_info(struct retro_system_av_info *)`
pub type RetroGetSystemAvInfoFn = unsafe extern "C" fn(info: *mut retro_system_av_info);
/// `void retro_set_controller_port_device(unsigned port, unsigned device)`
pub type RetroSetControllerPortDeviceFn = unsafe extern "C" fn(port: c_uint, device: c_uint);
/// `void retro_reset(void)`
pub type RetroResetFn = unsafe extern "C" fn();
/// `void retro_run(void)`
pub type RetroRunFn = unsafe extern "C" fn();
/// `size_t retro_serialize_size(void)`
pub type RetroSerializeSizeFn = unsafe extern "C" fn() -> usize;
/// `bool retro_serialize(void *data, size_t len)`
pub type RetroSerializeFn = unsafe extern "C" fn(data: *mut c_void, len: usize) -> bool;
/// `bool retro_unserialize(const void *data, size_t len)`
pub type RetroUnserializeFn = unsafe extern "C" fn(data: *const c_void, len: usize) -> bool;
/// `void retro_cheat_reset(void)`
pub type RetroCheatResetFn = unsafe extern "C" fn();
/// `void retro_cheat_set(unsigned index, bool enabled, const char *code)`
pub type RetroCheatSetFn = unsafe extern "C" fn(index: c_uint, enabled: bool, code: *const c_char);
/// `bool retro_load_game(const struct retro_game_info *)`
pub type RetroLoadGameFn = unsafe extern "C" fn(game: *const retro_game_info) -> bool;
/// `bool retro_load_game_special(unsigned, const struct retro_game_info *, size_t)`
pub type RetroLoadGameSpecialFn =
    unsafe extern "C" fn(game_type: c_uint, info: *const retro_game_info, num_info: usize) -> bool;
/// `void retro_unload_game(void)`
pub type RetroUnloadGameFn = unsafe extern "C" fn();
/// `unsigned retro_get_region(void)`
pub type RetroGetRegionFn = unsafe extern "C" fn() -> c_uint;
/// `void *retro_get_memory_data(unsigned id)`
pub type RetroGetMemoryDataFn = unsafe extern "C" fn(id: c_uint) -> *mut c_void;
/// `size_t retro_get_memory_size(unsigned id)`
pub type RetroGetMemorySizeFn = unsafe extern "C" fn(id: c_uint) -> usize;

/// The symbol names of every core entry point, in the order of the header. A
/// core must export all of them.
pub const CORE_API_SYMBOLS: [&str; 25] = [
    "retro_set_environment",
    "retro_set_video_refresh",
    "retro_set_audio_sample",
    "retro_set_audio_sample_batch",
    "retro_set_input_poll",
    "retro_set_input_state",
    "retro_init",
    "retro_deinit",
    "retro_api_version",
    "retro_get_system_info",
    "retro_get_system_av_info",
    "retro_set_controller_port_device",
    "retro_reset",
    "retro_run",
    "retro_serialize_size",
    "retro_serialize",
    "retro_unserialize",
    "retro_cheat_reset",
    "retro_cheat_set",
    "retro_load_game",
    "retro_load_game_special",
    "retro_unload_game",
    "retro_get_region",
    "retro_get_memory_data",
    "retro_get_memory_size",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_api_decls as decl;

    // Each item coerces to its function pointer type only when the parameter
    // and return types are equal. A `const` is evaluated but never emitted,
    // so the test binary needs no definition of the symbols.
    const _: RetroSetEnvironmentFn = decl::retro_set_environment;
    const _: RetroSetVideoRefreshFn = decl::retro_set_video_refresh;
    const _: RetroSetAudioSampleFn = decl::retro_set_audio_sample;
    const _: RetroSetAudioSampleBatchFn = decl::retro_set_audio_sample_batch;
    const _: RetroSetInputPollFn = decl::retro_set_input_poll;
    const _: RetroSetInputStateFn = decl::retro_set_input_state;
    const _: RetroInitFn = decl::retro_init;
    const _: RetroDeinitFn = decl::retro_deinit;
    const _: RetroApiVersionFn = decl::retro_api_version;
    const _: RetroGetSystemInfoFn = decl::retro_get_system_info;
    const _: RetroGetSystemAvInfoFn = decl::retro_get_system_av_info;
    const _: RetroSetControllerPortDeviceFn = decl::retro_set_controller_port_device;
    const _: RetroResetFn = decl::retro_reset;
    const _: RetroRunFn = decl::retro_run;
    const _: RetroSerializeSizeFn = decl::retro_serialize_size;
    const _: RetroSerializeFn = decl::retro_serialize;
    const _: RetroUnserializeFn = decl::retro_unserialize;
    const _: RetroCheatResetFn = decl::retro_cheat_reset;
    const _: RetroCheatSetFn = decl::retro_cheat_set;
    const _: RetroLoadGameFn = decl::retro_load_game;
    const _: RetroLoadGameSpecialFn = decl::retro_load_game_special;
    const _: RetroUnloadGameFn = decl::retro_unload_game;
    const _: RetroGetRegionFn = decl::retro_get_region;
    const _: RetroGetMemoryDataFn = decl::retro_get_memory_data;
    const _: RetroGetMemorySizeFn = decl::retro_get_memory_size;

    #[test]
    fn symbol_list_matches_the_header_in_order() {
        let header = include_str!("../vendor/libretro.h");
        let declared: Vec<&str> = header
            .lines()
            .filter_map(|line| line.strip_prefix("RETRO_API "))
            .filter_map(|rest| rest.split('(').next())
            .filter_map(|head| head.rsplit([' ', '*']).next())
            .collect();
        assert_eq!(declared, CORE_API_SYMBOLS);
    }
}
