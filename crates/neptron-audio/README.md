# neptron-audio

Audio out: rubato resampler, dynamic rate control and the SDL3 sink.

## Public API sketch

- `struct RateControl`
- `struct Sdl3Sink: AudioSink` (feature `sdl3`)

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`
- SDL3 outside the `sdl3` feature

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it. Every `unsafe` block has a `// SAFETY:` comment.
- Features: `sdl3`. No feature is on by default.

## Status

Stub.
