# neptron-shader

Slang shader presets through librashader. Milestone M4: this crate stays a stub in the MVP.

## Public API sketch

- `struct ShaderChain` with `load(preset)` and `apply(...)`

## Must never depend on

- `neptron-kms`, `neptron-video`, `neptron-ui`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
