# neptron-safety

The CRT safety validator. `validate` is the only way to make a `ValidatedMode`. Range checks and hard rails, integer maths only.

## Public API sketch

- `struct ValidatedMode` (private fields) with `mode()`
- `fn validate(&Modeline, &MonitorProfile, &GpuCaps) -> Result<ValidatedMode, Rejection>`
- `enum Rejection`

## Must never depend on

- Any crate except `neptron-types` (normal dependencies)
- Floating point maths
- Macros or generics that hide a limit

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
