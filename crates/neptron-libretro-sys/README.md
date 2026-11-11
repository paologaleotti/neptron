# neptron-libretro-sys

Raw FFI bindings to `libretro.h`. `xtask bindgen` generates them, and the generated code is committed.

## Public API sketch

- Generated `retro_*` types and constants

## Must never depend on

- Any NEPTRON crate
- libclang at build time

## Rules

- `unsafe` is allowed: the crate holds raw FFI declarations. The safe wrapper is in another crate.
- No features.

## Status

Stub.
