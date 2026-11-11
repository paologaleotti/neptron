# neptron-session

Game lifecycle: launch, the display-locked loop, pause, quick menu, exit and save RAM flush. It mediates between UI, core host and video.

## Public API sketch

- `struct Session` with `launch(game)`, `run_frame()`, `quit()`

## Must never depend on

- `neptron-ui` (only the app binary depends on it)
- `neptron-kms`, `neptron-window`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
