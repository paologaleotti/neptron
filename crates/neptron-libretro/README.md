# neptron-libretro

Safe libretro core host: load a core, environment callbacks, save RAM, core options.

## Public API sketch

- `struct Core` with `load(path)`, `run_frame()`, `save_ram()`
- `trait HostCallbacks`

## Must never depend on

- `neptron-ui`, `neptron-kms`, `neptron-video`

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it. Every `unsafe` block has a `// SAFETY:` comment.
- No features.

## Status

Stub.
