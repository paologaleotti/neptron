# neptronctl

Command-line client: print a modeline, test a mode, self-test, diagnostics. It talks to the frontend on the control socket.

## Public API sketch

- `neptronctl modeline ...`, `neptronctl selftest video`

## Must never depend on

- `neptron-ui`
- Any async runtime

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
