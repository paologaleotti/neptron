# neptron-safety

The CRT safety validator. `validate` is the only way to make a `ValidatedMode`, and every backend that programs a display takes a `ValidatedMode` only. Range checks and hard rails, integer maths only.

## Public API

- `struct ValidatedMode`: private field, no `Default`, no public constructor, no `Deserialize`. `mode()` gives the `Modeline` that passed.
- `fn validate(&Modeline, &MonitorProfile, &GpuCaps) -> Result<ValidatedMode, Rejection>`: judges the modeline it gets and changes nothing.
- `enum Rejection`: one variant per check. Each variant carries the value it found and the limit: `Malformed`, `LineRate`, `FieldRate`, `LineCount`, `SyncWidth`, `Porch`, `PixelClock`, `Interlace`, `HardRail`, `NotAPanelMode`.
- `enum Axis`, `enum PorchEdge`, `enum InterlaceLimit`: the details of some rejections.

`Rejection` implements `Display` and `std::error::Error` by hand, because the crate cannot depend on `thiserror`.

## Must never depend on

- Any crate except `neptron-types` (normal dependencies). `xtask rules` checks this.
- Floating point maths
- Macros or generics that hide a limit

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
- Every limit is a named constant with its unit in the name.
- The crate stays near 500 lines, tests excluded.

## Status

The API is in place. `validate` panics: the checks are not implemented yet.
