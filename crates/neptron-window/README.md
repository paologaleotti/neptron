# neptron-window

Desktop window backend for development. It shows the reference renderer's output in an SDL3 window and maps the keyboard to a gamepad. Tests never use it.

## Must never depend on

- `neptron-kms`
- Any test crate: tests use `neptron-testkit`

`unsafe` is forbidden (`#![forbid(unsafe_code)]`). Features: `sdl3`, off by default.

## Public API

All items exist only with the `sdl3` feature. No SDL3 type is in the public API.

- `open(title) -> Result<(WindowDisplay, KeyboardPad), WindowError>`
- `WindowDisplay: Display`, with `clock() -> WindowClock`
- `WindowClock: Clock`, the time source of the flip events
- `KeyboardPad: InputSource`, with `quit_requested() -> bool`
- `WindowError`, `WINDOW_CAPS`, `WINDOW_MAX_SIDE_PX`, `KEYBOARD_DEVICE_ID`

## Rules for the app

- Call `open` on the main thread, and keep both values on it. SDL video runs only on the main thread on macOS.
- Poll the `KeyboardPad` once per frame. The poll also reads the window events, so the window stays responsive.
- Stop when `quit_requested()` is true: the user closed the window.
- Use `WindowDisplay::clock()` as the frontend clock, so that flip times and the clock have the same origin.
- Pace audio on the flip times that this backend reports, not on the field rate of the mode. The two rates differ (see Pacing).
- After a failed `set_mode` at a new active size, the old mode is gone. `present` and `wait_flip` return `NoMode` until a mode is set.

## Picture

The window shows the active area of the mode as a 4:3 picture, as a CRT does.

- The vertical axis scales by the largest whole factor at which the picture fits the desktop, less 64 points on each axis for the window frame. The horizontal axis scales on its own to `height * 4 / 3`: a 320x240 and a 2560x240 mode both show as 960x720 on a 1440x900 desktop. A mode that does not fit at factor 1 shrinks to the largest 4:3 box.
- Nearest sampling when both factors are whole, else linear. The check uses window points, so with a fractional desktop scale (for example 1.5) nearest can give columns or lines of uneven width.
- `set_size` is asynchronous on some platforms, so the window can reach its new size a few frames after `set_mode`.

## Pacing

The window presents with the desktop's vsync. `wait_flip` shows the frame for `round(desktop rate / mode field rate)` desktop refreshes, from 1 to 8, and blocks in SDL. It never sleeps.

- The flip rate is the desktop rate divided by that number: near the mode rate, not equal to it. A 50 Hz mode flips at 60 Hz on a 60 Hz desktop.
- `FlipEvent::time_ns` is the time at which SDL returned from the last present. The compositor adds latency that the window cannot see.
- `FlipEvent::sequence` counts one flip per call. The window cannot see a missed desktop refresh, so it never reports a gap.
- On macOS an occluded or minimised window can flip faster than the desktop rate, or without a wait.

## Display rules

- `set_mode` takes a `ValidatedMode` and returns it unchanged: a window has no timings to read back. It refuses an active area above `WINDOW_MAX_SIDE_PX` (4096) on either axis.
- `WINDOW_CAPS` takes every pixel clock and interlaced modes. An interlaced mode shows as one progressive picture and flips once per field.
- `present` renders with one `ReferenceRenderer` into a buffer made at `set_mode`, and uploads it to a streaming texture. A new active size makes a new renderer, texture and buffer.
- `blank` shows black once and drops the renderer and the buffer. `present` and `wait_flip` then return `NoMode` until the next `set_mode`.
- No busy rule: `present` draws at once and `wait_flip` shows the frame before it returns, so no flip stays queued. Test code that must work on KMS on the fake display.
- When SDL cannot make a renderer, it closes the window, because it takes the window by value. That `set_mode` returns `DisplayError::Device`, and the next `set_mode` opens a new window.

## Keyboard

Physical keys (SDL scancodes, which are USB HID usage ids), so the map does not change with the layout. The defaults are those of the RetroArch keyboard driver.

| Key | Control |
| --- | --- |
| Arrows | D-pad |
| X | A |
| Z | B |
| S | X |
| A | Y |
| Q | L |
| W | R |
| Return | Start |
| Right Shift | Select |

A key repeat gives no event. Every event has the device id `KEYBOARD_DEVICE_ID` (0).

## SDL3

- The `sdl3` crate is pinned to 0.22.1 and links the system SDL3 dynamically through pkg-config. On macOS: `brew install sdl3`. On Debian or Ubuntu: `libsdl3-dev`. Buildroot has an SDL3 package.
- The crate uses the `unsafe_textures` feature of `sdl3`, so that one struct owns the renderer and its texture, and calls no unsafe function. A `Texture` is then a raw pointer that its renderer frees on drop. The private `Gpu` type owns both and never hands the texture out, so no texture outlives its renderer.

## Tests

The picture rule, the pacing rule and the key map are plain functions with unit tests, run with and without the feature. With the feature, a test also checks the key map against the SDL scancodes.

`cargo xtask window-smoke` builds the crate with `sdl3`, opens the window, shows a 240p and a 2560x240 mode validated against `generic_15`, injects a key press and checks it. It needs a display, so no tier runs it. No tier compiles the `sdl3` code either: after a change to this crate or to a contract it uses, run `cargo xtask clippy -p neptron-window --features sdl3`.
