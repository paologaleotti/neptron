# kms-spike

Standalone hardware spike (WP 1.1). It shows 240p, 480i and 480p test modes for a named preset and prints what the driver programmed.

## Public API sketch

- `kms-spike <preset>`

## Must never depend on

- Any crate except `drm`, `neptron-types` and `neptron-safety`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `kms`. No feature is on by default.

## Status

Stub.
