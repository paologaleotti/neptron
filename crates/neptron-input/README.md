# neptron-input

Input: SDL3 gamepads, evdev keyboards, RetroPad mapping and hotplug.

## Public API sketch

- `struct RetroPadMapper`
- `struct Sdl3Pads: InputSource` (feature `sdl3`)
- `struct EvdevKeyboards: InputSource` (feature `evdev`)

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`
- SDL3 or evdev outside their features

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it. Every `unsafe` block has a `// SAFETY:` comment.
- Features: `sdl3`, `evdev`. No feature is on by default.

## Status

Stub.
