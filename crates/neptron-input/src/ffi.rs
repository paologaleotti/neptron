//! The one SDL3 call that the `sdl3` crate does not wrap: the device path
//! of a joystick by its instance id. The crate wraps only the gamepad call
//! of it, which unwraps the text as UTF-8.

use sdl3::joystick::JoystickId;

/// The device path of the joystick `id`, for a plain joystick and a
/// gamepad alike, or `None` when SDL3 has none. Bytes that are not UTF-8
/// are read with a lossy conversion.
pub(crate) fn joystick_path(id: JoystickId) -> Option<String> {
    // SAFETY: SDL_GetJoystickPathForID takes any instance id: it locks the
    // joystick list itself and gives NULL for an id it does not know. It
    // reads no pointer of the caller.
    let raw = unsafe { sdl3::sys::joystick::SDL_GetJoystickPathForID(id.into()) };
    if raw.is_null() {
        return None;
    }
    // SAFETY: `raw` is not null, and SDL3 returns a NUL-terminated string
    // that it keeps at least until the next SDL3 call. The text is copied
    // here, before any other SDL3 call, and the pointer is not kept.
    let text = unsafe { std::ffi::CStr::from_ptr(raw) };
    Some(text.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_joystick_that_sdl_does_not_know_has_no_path() {
        let sdl = sdl3::init().expect("SDL3 starts");
        let _joysticks = sdl.joystick().expect("the joystick subsystem starts");
        assert_eq!(joystick_path(JoystickId::new(u32::MAX)), None);
    }
}
