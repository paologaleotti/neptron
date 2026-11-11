use neptron_backend::InputSource;
use neptron_types::InputEvent;
use sdl3::EventPump;
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Scancode;

use crate::keymap::key_event;

/// The keyboard as a RetroPad, with the device id
/// [`KEYBOARD_DEVICE_ID`](crate::KEYBOARD_DEVICE_ID). It reads the SDL3
/// event queue, which also keeps the window responsive.
pub struct KeyboardPad {
    pump: EventPump,
    quit: bool,
}

impl KeyboardPad {
    pub(crate) fn new(pump: EventPump) -> Self {
        Self { pump, quit: false }
    }

    /// True after the user closed the window. The flag stays set.
    pub fn quit_requested(&self) -> bool {
        self.quit
    }
}

impl InputSource for KeyboardPad {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        for event in self.pump.poll_iter() {
            let mapped = match event {
                Event::KeyDown {
                    scancode: Some(key),
                    repeat,
                    ..
                } => key_event(hid_usage(key), true, repeat),
                Event::KeyUp {
                    scancode: Some(key),
                    repeat,
                    ..
                } => key_event(hid_usage(key), false, repeat),
                Event::Quit { .. }
                | Event::Window {
                    win_event: WindowEvent::CloseRequested,
                    ..
                } => {
                    self.quit = true;
                    None
                }
                _ => None,
            };
            out.extend(mapped);
        }
    }
}

impl std::fmt::Debug for KeyboardPad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyboardPad")
            .field("quit", &self.quit)
            .finish_non_exhaustive()
    }
}

/// The USB HID usage id of a key. SDL scancodes are these ids.
pub(crate) fn hid_usage(key: Scancode) -> u32 {
    u32::try_from(key as i32).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use neptron_types::Control;

    use super::*;
    use crate::keymap::control_of;

    #[test]
    fn the_key_map_matches_the_sdl_scancodes() {
        let expected = [
            (Scancode::Up, Control::Up),
            (Scancode::Down, Control::Down),
            (Scancode::Left, Control::Left),
            (Scancode::Right, Control::Right),
            (Scancode::X, Control::A),
            (Scancode::Z, Control::B),
            (Scancode::S, Control::X),
            (Scancode::A, Control::Y),
            (Scancode::Q, Control::L),
            (Scancode::W, Control::R),
            (Scancode::Return, Control::Start),
            (Scancode::RShift, Control::Select),
        ];
        for (key, control) in expected {
            assert_eq!(control_of(hid_usage(key)), Some(control), "{key:?}");
        }
        assert_eq!(control_of(hid_usage(Scancode::Escape)), None);
    }
}
