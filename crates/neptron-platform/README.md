# neptron-platform

GPU and board detection from sysfs, the capability table, and a monitor profile built from the range limits in an EDID.

## Public API sketch

- `fn detect(sysfs_root) -> Platform`
- `fn caps_for(&Gpu) -> GpuCaps`
- `fn profile_from_edid(&[u8]) -> MonitorProfile`

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
