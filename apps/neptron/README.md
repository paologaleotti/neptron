# neptron

The frontend binary. It holds only wiring, no logic.

## Public API sketch

- `main`: build the backend, the session and the UI, run the frame loop

## Must never depend on

- Logic of its own: logic belongs in the crates

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `kms`, `window`. No feature is on by default.

## Status

Stub.
