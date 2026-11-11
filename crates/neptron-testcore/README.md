# neptron-testcore

A libretro core that follows a script, for tests. It reads its script from a file named in an environment variable.

## Public API sketch

- The libretro entry points (`retro_init`, `retro_run`, ...)

## Must never depend on

- Any NEPTRON crate except `neptron-libretro-sys`

## Rules

- `unsafe` is allowed only on the exported libretro functions. Each has a `// SAFETY:` comment. The test core is never in the image.
- No features.

## Status

Stub.
