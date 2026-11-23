# neptron-shader

Slang shader presets through librashader. Milestone M4: this crate stays a stub in the MVP.

## Must never depend on

- `neptron-kms`, `neptron-video`, `neptron-ui`

`unsafe` is forbidden (`#![forbid(unsafe_code)]`). No features.

## Public API sketch

- Planned, not in the code: `struct ShaderChain` with `load(preset)` and `apply(...)`
