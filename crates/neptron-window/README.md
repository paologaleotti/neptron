# neptron-window

Desktop window backend for development. It shows the reference renderer's output in an SDL3 window and maps the keyboard to a gamepad. Tests never use it.

## Public API sketch

- `struct WindowDisplay: Display`
- `struct KeyboardPad: InputSource`

## Must never depend on

- `neptron-kms`
- Any test crate: tests use `neptron-testkit`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `sdl3`. No feature is on by default.

## Status

Stub.
