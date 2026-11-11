# neptron-gfx

Renderers for a `Scene`. `ReferenceRenderer` (CPU, pure Rust) defines the correct pixels. `GlesRenderer` (EGL, OpenGL ES 2) must match it.

## Public API sketch

- `struct ReferenceRenderer` with `render(&Scene, &mut [u8])`
- `gles::GlesRenderer` (feature `gles`)

## Must never depend on

- `neptron-kms`, `neptron-video`, `neptron-ui`
- EGL or GL crates outside the `gles` feature

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it. Every `unsafe` block has a `// SAFETY:` comment.
- Features: `gles`. No feature is on by default.

## Status

Stub.
