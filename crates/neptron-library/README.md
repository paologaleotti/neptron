# neptron-library

ROM scan, index cache, core and system metadata, gamelist import.

## Public API sketch

- `struct Library` with `scan(root)` and `query(filter)`
- `struct System`, `struct Game`

## Must never depend on

- Any hardware crate
- `neptron-ui`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
