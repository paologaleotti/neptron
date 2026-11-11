//! Smoke test of the window backend, for a desktop with a display.
//!
//! It opens the window, sets a 240p mode and a 2560x240 super-width mode,
//! both validated against the `generic_15` preset, presents a test scene
//! in each, injects one key press and checks the event, and closes. Run it
//! with `cargo xtask window-smoke`.

#![forbid(unsafe_code)]

use std::error::Error;
use std::path::Path;

use neptron_backend::{Display, InputSource};
use neptron_types::{
    Control, Filter, Frame, InputEvent, Modeline, MonitorProfile, PixelFormat, Rect, Scanlines,
    Scene,
};
use neptron_window::{KEYBOARD_DEVICE_ID, KeyboardPad, WindowDisplay};
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Mod, Scancode};

const FRAMES_PER_MODE: u32 = 120;
const CANVAS_W: u16 = 320;
const CANVAS_H: u16 = 240;

/// 320x240 at 60.03 Hz and 15.728 kHz, inside `generic_15`.
const MODE_240P: Modeline = Modeline {
    pclock_hz: 6_700_000,
    hactive: 320,
    hsync_start: 334,
    hsync_end: 366,
    htotal: 426,
    vactive: 240,
    vsync_start: 241,
    vsync_end: 244,
    vtotal: 262,
    interlace: false,
    doublescan: false,
    hsync_positive: false,
    vsync_positive: false,
};

/// 2560x240 at 59.96 Hz and 15.708 kHz, inside `generic_15`.
const MODE_SUPER_WIDTH: Modeline = Modeline {
    pclock_hz: 52_560_000,
    hactive: 2560,
    hsync_start: 2667,
    hsync_end: 2919,
    htotal: 3346,
    ..MODE_240P
};

fn main() -> Result<(), Box<dyn Error>> {
    let profile = load_preset("generic_15")?;
    let (mut display, mut pad) = neptron_window::open("NEPTRON window smoke test")?;
    let canvas = test_canvas();
    for mode in [MODE_240P, MODE_SUPER_WIDTH] {
        let validated = neptron_safety::validate(&mode, &profile, display.caps())
            .map_err(|e| format!("{}x{}: {e}", mode.hactive, mode.vactive))?;
        let read_back = display.set_mode(&validated)?;
        if read_back != mode {
            return Err(format!("mode read back differs: {read_back:?}").into());
        }
        run_frames(&mut display, &mut pad, &canvas, &mode)?;
    }
    check_key(&mut pad)?;
    println!("smoke test passed");
    Ok(())
}

fn load_preset(id: &str) -> Result<MonitorProfile, Box<dyn Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/monitors")
        .join(format!("{id}.toml"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(toml::from_str(&text)?)
}

/// Colour bars at half brightness with a white 16-pixel grid, as
/// XRGB8888. The mask halves each channel on its own.
fn test_canvas() -> Vec<u8> {
    const BARS: [u32; 8] = [
        0xFFFFFF, 0xFFFF00, 0x00FFFF, 0x00FF00, 0xFF00FF, 0xFF0000, 0x0000FF, 0x000000,
    ];
    let (w, h) = (usize::from(CANVAS_W), usize::from(CANVAS_H));
    let mut data = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let grid = x % 16 == 0 || y % 16 == 0 || x == w - 1 || y == h - 1;
            let pixel = if grid {
                0xFFFFFF
            } else {
                (BARS[x * 8 / w] >> 1) & 0x7F7F7F
            };
            data.extend_from_slice(&pixel.to_ne_bytes());
        }
    }
    data
}

fn run_frames(
    display: &mut WindowDisplay,
    pad: &mut KeyboardPad,
    canvas: &[u8],
    mode: &Modeline,
) -> Result<(), Box<dyn Error>> {
    let frame = Frame {
        data: canvas,
        width: CANVAS_W,
        height: CANVAS_H,
        pitch: usize::from(CANVAS_W) * 4,
        format: PixelFormat::Xrgb8888,
    };
    let mut events = Vec::new();
    let mut times = Vec::new();
    for i in 0..FRAMES_PER_MODE {
        // The crop moves by one pixel per frame, so every flip shows a new
        // picture.
        let shift = i32::try_from(i % 16)?;
        let scene = Scene {
            game: Some(frame),
            crop: Rect {
                x: shift,
                y: 0,
                w: u32::from(CANVAS_W),
                h: u32::from(CANVAS_H),
            },
            dest: Rect {
                x: 0,
                y: 0,
                w: u32::from(mode.hactive),
                h: u32::from(mode.vactive),
            },
            filter: Filter::Nearest,
            scanlines: Scanlines::NONE,
            ui: None,
            ui_dest: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
        };
        display.present(&scene)?;
        let flip = display.wait_flip()?;
        times.push(flip.time_ns);
        pad.poll(&mut events);
        if pad.quit_requested() {
            return Err("the window was closed".into());
        }
    }
    report(mode, &times);
    Ok(())
}

fn report(mode: &Modeline, times: &[u64]) {
    let gaps: Vec<u64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let (Some(min), Some(max)) = (gaps.iter().min(), gaps.iter().max()) else {
        return;
    };
    let mean = gaps.iter().sum::<u64>() / gaps.len() as u64;
    println!(
        "{}x{} at {} mHz: {} flips, interval min {} us, mean {} us, max {} us",
        mode.hactive,
        mode.vactive,
        mode.field_rate_mhz(),
        times.len(),
        min / 1_000,
        mean / 1_000,
        max / 1_000
    );
}

/// Pushes a press and a release of Z into the SDL event queue and checks
/// that the pad reads them as button B.
fn check_key(pad: &mut KeyboardPad) -> Result<(), Box<dyn Error>> {
    let sdl = sdl3::init()?;
    let queue = sdl.event()?;
    for down in [true, false] {
        let event = if down {
            Event::KeyDown {
                timestamp: 0,
                window_id: 0,
                keycode: Some(Keycode::Z),
                scancode: Some(Scancode::Z),
                keymod: Mod::NOMOD,
                repeat: false,
                which: 0,
                raw: 0,
            }
        } else {
            Event::KeyUp {
                timestamp: 0,
                window_id: 0,
                keycode: Some(Keycode::Z),
                scancode: Some(Scancode::Z),
                keymod: Mod::NOMOD,
                repeat: false,
                which: 0,
                raw: 0,
            }
        };
        queue.push_event(event)?;
    }
    let mut events = Vec::new();
    pad.poll(&mut events);
    let b = |value| InputEvent {
        device: KEYBOARD_DEVICE_ID,
        control: Control::B,
        value,
    };
    if events != [b(1), b(0)] {
        return Err(format!("injected Z press and release, read {events:?}").into());
    }
    println!("key injection: {events:?}");
    Ok(())
}
