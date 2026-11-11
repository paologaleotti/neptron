# neptron-ui

The sealed UI: snapshot in, scene and intents out. A 320x240 XRGB canvas, a bitmap font, widget templates and screens.

## Public API sketch

- `trait Canvas` and `CpuCanvas`
- `fn frame(&Snapshot, &[UiInput]) -> (UiScene, Vec<Intent>)`
- `screens` module

## Must never depend on

- Any other NEPTRON crate
- Any hardware crate

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
