//! Layout of the structs that the host and the test core exchange.
//!
//! The expected values are worked out by hand from the C layout rules for an
//! LP64 target (4-byte `unsigned`, 8-byte pointers and `size_t`, 1-byte
//! `bool`), not taken from bindgen. The generated code checks every struct
//! against clang as well; these tests also catch an error in that chain.

use std::mem::{align_of, offset_of, size_of};

use neptron_libretro_sys::*;

#[test]
fn game_geometry() {
    assert_eq!(size_of::<retro_game_geometry>(), 20);
    assert_eq!(align_of::<retro_game_geometry>(), 4);
    assert_eq!(offset_of!(retro_game_geometry, base_width), 0);
    assert_eq!(offset_of!(retro_game_geometry, base_height), 4);
    assert_eq!(offset_of!(retro_game_geometry, max_width), 8);
    assert_eq!(offset_of!(retro_game_geometry, max_height), 12);
    assert_eq!(offset_of!(retro_game_geometry, aspect_ratio), 16);
}

#[test]
fn system_timing() {
    assert_eq!(size_of::<retro_system_timing>(), 16);
    assert_eq!(align_of::<retro_system_timing>(), 8);
    assert_eq!(offset_of!(retro_system_timing, fps), 0);
    assert_eq!(offset_of!(retro_system_timing, sample_rate), 8);
}

#[test]
fn system_av_info_pads_geometry_to_the_timing_alignment() {
    assert_eq!(size_of::<retro_system_av_info>(), 40);
    assert_eq!(align_of::<retro_system_av_info>(), 8);
    assert_eq!(offset_of!(retro_system_av_info, geometry), 0);
    assert_eq!(offset_of!(retro_system_av_info, timing), 24);
}

#[test]
fn game_info() {
    assert_eq!(size_of::<retro_game_info>(), 32);
    assert_eq!(offset_of!(retro_game_info, path), 0);
    assert_eq!(offset_of!(retro_game_info, data), 8);
    assert_eq!(offset_of!(retro_game_info, size), 16);
    assert_eq!(offset_of!(retro_game_info, meta), 24);
}

#[test]
fn system_info_packs_the_two_flags() {
    assert_eq!(size_of::<retro_system_info>(), 32);
    assert_eq!(offset_of!(retro_system_info, library_name), 0);
    assert_eq!(offset_of!(retro_system_info, library_version), 8);
    assert_eq!(offset_of!(retro_system_info, valid_extensions), 16);
    assert_eq!(offset_of!(retro_system_info, need_fullpath), 24);
    assert_eq!(offset_of!(retro_system_info, block_extract), 25);
}

#[test]
fn variable() {
    assert_eq!(size_of::<retro_variable>(), 16);
    assert_eq!(offset_of!(retro_variable, key), 0);
    assert_eq!(offset_of!(retro_variable, value), 8);
}

#[test]
fn log_callback_is_one_nullable_pointer() {
    assert_eq!(size_of::<retro_log_callback>(), 8);
    assert_eq!(size_of::<retro_environment_t>(), 8);
    assert_eq!(size_of::<retro_video_refresh_t>(), 8);
}

#[test]
fn message() {
    assert_eq!(size_of::<retro_message>(), 16);
    assert_eq!(offset_of!(retro_message, msg), 0);
    assert_eq!(offset_of!(retro_message, frames), 8);
}

#[test]
fn input_descriptor() {
    assert_eq!(size_of::<retro_input_descriptor>(), 24);
    assert_eq!(offset_of!(retro_input_descriptor, port), 0);
    assert_eq!(offset_of!(retro_input_descriptor, device), 4);
    assert_eq!(offset_of!(retro_input_descriptor, index), 8);
    assert_eq!(offset_of!(retro_input_descriptor, id), 12);
    assert_eq!(offset_of!(retro_input_descriptor, description), 16);
}

#[test]
fn controller_info() {
    assert_eq!(size_of::<retro_controller_description>(), 16);
    assert_eq!(offset_of!(retro_controller_description, id), 8);
    assert_eq!(size_of::<retro_controller_info>(), 16);
    assert_eq!(offset_of!(retro_controller_info, num_types), 8);
}

#[test]
fn enums_are_32_bit() {
    assert_eq!(size_of::<retro_pixel_format>(), 4);
    assert_eq!(size_of::<retro_log_level>(), 4);
}

#[test]
fn constants_used_by_the_test_core() {
    assert_eq!(RETRO_API_VERSION, 1);
    assert_eq!(RETRO_ENVIRONMENT_SHUTDOWN, 7);
    assert_eq!(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, 10);
    assert_eq!(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, 18);
    assert_eq!(RETRO_ENVIRONMENT_GET_LOG_INTERFACE, 27);
    assert_eq!(RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO, 32);
    assert_eq!(RETRO_ENVIRONMENT_SET_GEOMETRY, 37);
    assert_eq!(RETRO_ENVIRONMENT_GET_CAN_DUPE, 3);
    assert_eq!(RETRO_PIXEL_FORMAT_0RGB1555, 0);
    assert_eq!(RETRO_PIXEL_FORMAT_XRGB8888, 1);
    assert_eq!(RETRO_PIXEL_FORMAT_RGB565, 2);
    assert_eq!(RETRO_MEMORY_SAVE_RAM, 0);
    assert_eq!(RETRO_DEVICE_JOYPAD, 1);
    assert_eq!(RETRO_DEVICE_ID_JOYPAD_R3, 15);
}
