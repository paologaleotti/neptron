# neptron-launch

The launcher. It holds the DRM descriptor, starts the frontend, passes the descriptor in `NEPTRON_DRM_FD` and respawns the frontend with a backoff.

## Public API sketch

- `main`

## Must never depend on

- `neptron-ui`
- Any frontend logic

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
