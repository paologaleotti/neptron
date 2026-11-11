# neptron-backend

The traits that hide display, audio, input and clock behind one interface, so that the frontend runs on KMS, in a window or on test fakes.

## Public API sketch

- `trait Display` (`caps`, `set_mode(&ValidatedMode)`, `present(&Scene)`, `wait_flip`)
- `trait AudioSink`, `trait InputSource`, `trait Clock`
- `FlipEvent`, `InputEvent`, `DisplayError`

## Must never depend on

- Any NEPTRON crate except `neptron-types` and `neptron-safety`
- Any hardware crate (SDL3, EGL, DRM, evdev)

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
