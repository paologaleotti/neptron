# neptron-switchres-sys

The Switchres C++ build and the raw FFI calls, and nothing else. `neptron-modeline` is its safe wrapper.

## Public API sketch

- Raw `extern "C"` declarations of the Switchres calculation functions
- A build script that compiles the calculation sources with the `cc` crate and `-ffp-contract=off`

## Must never depend on

- Any NEPTRON crate
- Any crate except the build tools for Switchres

## Rules

- `unsafe` is allowed: the crate holds raw FFI declarations. The safe wrapper is in another crate.
- No features.

## Status

Stub.
