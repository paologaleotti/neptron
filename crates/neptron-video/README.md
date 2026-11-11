# neptron-video

The ODREN engine: core geometry to policy to safety to display. Debounce, commit, mute and the switch trace. The `guard` module holds confirm-or-revert, last known good and safe mode.

## Public API sketch

- `struct VideoEngine<D: Display>`
- `fn on_geometry(&mut self, SourceGeometry)`
- `guard` module

## Must never depend on

- `neptron-kms` (it uses the `Display` trait only)
- `neptron-ui`
- Any hardware crate

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
