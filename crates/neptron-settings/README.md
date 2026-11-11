# neptron-settings

Typed settings schema, three-level resolution (game, system, global, default) and TOML persistence.

## Public API sketch

- `struct Schema`, `struct Settings`
- `fn resolve(key, game, system) -> Value`
- `fn write(...)`: temp file, fsync, rename

## Must never depend on

- Any hardware crate
- `neptron-ui`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
