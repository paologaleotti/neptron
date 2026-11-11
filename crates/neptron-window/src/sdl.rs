//! The SDL3 window and keyboard.

mod display;
mod error;
mod gpu;
mod keyboard;

pub use display::{WINDOW_CAPS, WINDOW_MAX_SIDE_PX, WindowClock, WindowDisplay};
pub use error::WindowError;
pub use keyboard::KeyboardPad;

/// The window size before the first mode, in points.
const START_SIZE_PT: (u32, u32) = (640, 480);

/// Opens the window and the keyboard. The window is black until the first
/// [`set_mode`](neptron_backend::Display::set_mode).
///
/// Call it on the main thread: SDL video runs only there on macOS. Both
/// returned values stay on that thread, and the caller polls the
/// [`KeyboardPad`] once per frame, because that also keeps the window
/// responsive.
///
/// # Errors
///
/// [`WindowError::Sdl`] when SDL3 cannot start, or cannot open the window
/// or the event queue.
pub fn open(title: &str) -> Result<(WindowDisplay, KeyboardPad), WindowError> {
    let sdl = sdl3::init().map_err(|e| WindowError::sdl("init", e))?;
    let video = sdl.video().map_err(|e| WindowError::sdl("video init", e))?;
    let pump = sdl
        .event_pump()
        .map_err(|e| WindowError::sdl("event pump", e))?;
    // SDL reads the hint when it makes a renderer, so it must be set before
    // the first `set_mode`.
    if !sdl3::hint::set(sdl3::hint::names::RENDER_VSYNC, "1") {
        return Err(WindowError::sdl("vsync hint", sdl3::get_error()));
    }
    let window = open_window(&video, title).map_err(|e| WindowError::sdl("window", e))?;
    Ok((
        WindowDisplay::new(window, video, title),
        KeyboardPad::new(pump),
    ))
}

fn open_window(
    video: &sdl3::VideoSubsystem,
    title: &str,
) -> Result<sdl3::video::Window, sdl3::video::WindowBuildError> {
    video
        .window(title, START_SIZE_PT.0, START_SIZE_PT.1)
        .position_centered()
        .high_pixel_density()
        .build()
}
