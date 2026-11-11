# neptron-types

Shared plain data: modeline, CRT range, monitor profile, GPU capabilities, source geometry, frame and scene. Plain data with serde, no I/O.

## Public API sketch

- `Modeline` with `line_rate_mhz()` and `field_rate_mhz()`
- `CrtRange`, `MonitorKind`, `MonitorProfile`
- `GpuCaps`, `Rotation`, `SourceGeometry`
- `PixelFormat`, `Frame`, `Rect`, `Filter`, `Scene`

## Must never depend on

- Any other NEPTRON crate
- Any crate that does I/O or talks to hardware
- Floating point in any contract type

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
