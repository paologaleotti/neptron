# neptron-libretro-sys

Raw FFI bindings to `libretro.h`. `cargo xtask bindgen` generates them from `vendor/libretro.h`, and the generated code is committed. `vendor/README.md` names the source commit and the licence of the header.

## Public API

- Every `retro_*` type and every `RETRO_*` constant of the header, with the names of the header. The enums are plain integer types with one constant per value (`RETRO_PIXEL_FORMAT_XRGB8888: retro_pixel_format`), because a core can send a value that the enum does not list.
- A function pointer type for each core entry point (`RetroInitFn`, `RetroRunFn`, `RetroLoadGameFn`, ...), and `CORE_API_SYMBOLS`, the names of all 25 entry points. A frontend looks the entry points up in a loaded library and calls them through these types.

The crate declares no entry point as an external function. A call to such a declaration fails to link, because a frontend loads the core at run time.

## Files

| File | Source |
| --- | --- |
| `src/bindings.rs` | Generated: types and constants |
| `src/core_api_decls.rs` | Generated: the entry points as the header declares them. Only the tests use it |
| `src/core_api.rs` | Written by hand: the function pointer types. A test proves that each one equals the generated declaration |
| `tests/layout.rs` | Sizes and offsets of the structs that the host and the test core exchange, worked out by hand |

## Updating the header

1. Copy the new `libretro.h` into `vendor/` and update `vendor/README.md`.
2. Run `cargo xtask bindgen` (it needs the `bindgen` tool: `brew install bindgen` on macOS).
3. Run `cargo xtask test -p neptron-libretro-sys`. If the header adds an entry point, add its type to `src/core_api.rs`.

## Targets

The generated code checks the size, the alignment and the field offsets of every struct at compile time, against the layout of `x86_64-unknown-linux-gnu`. The crate builds for every 64-bit target with the LP64 data model: x86_64 and aarch64, Linux and macOS. A 32-bit target needs a second generated file.

## Must never depend on

- Any NEPTRON crate
- libclang at build time

## Rules

- `unsafe` is allowed: the crate holds raw FFI declarations. The safe wrapper is in another crate.
- No features.
