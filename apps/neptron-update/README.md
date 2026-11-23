# neptron-update

Thin glue around RAUC. Milestone M4: this app stays a stub in the MVP.

## Must never depend on

- `neptron-ui`, `neptron-kms`

## Public API sketch

- `main`, which does nothing yet

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
