# neptron-backend

The traits that hide display, audio, input and clock behind one interface, so that the frontend runs on KMS, in a window or on test fakes. A backend executes what it gets and decides nothing. The implementations are in `neptron-kms`, `neptron-window`, `neptron-audio`, `neptron-input` and `neptron-testkit`.

## Must never depend on

- Any NEPTRON crate except `neptron-types` and `neptron-safety`
- Any hardware crate (SDL3, EGL, DRM, evdev)

`unsafe` is forbidden (`#![forbid(unsafe_code)]`). No features.

## Public API

- `trait Display`:
  - `caps() -> &GpuCaps`
  - `set_mode(&ValidatedMode) -> Result<Modeline, DisplayError>`: returns the mode the driver reports back. For a mode with a TV standard it sets the standard in the same commit, or refuses with `ModeRefused` when the connector cannot take it (ADR 0012).
  - `present(&Scene)`: queues one flip.
  - `wait_flip() -> Result<FlipEvent, DisplayError>`: bounded. On a timeout the flip stays queued, and `present`, `set_mode` and `blank` are busy until a later wait returns its event.
  - `blank() -> Result<(), DisplayError>`: the output is off until the next `set_mode`, which is then always a real modeset. It is the one display change without a `ValidatedMode`.
- `enum DisplayError` (`#[non_exhaustive]`): `ModeRefused`, `SceneRefused`, `NoMode`, `Device`. Every variant but `NoMode` carries what the driver or the device reported.
- `trait AudioSink`: `rate_hz()`, `queued_frames()`, `write(&[i16]) -> usize`. Interleaved 16-bit stereo; `write` never blocks and takes only what fits.
- `trait InputSource`: `poll(&mut Vec<InputEvent>)` appends the events since the last call, already mapped to the RetroPad.
- `trait Clock`: `now_ns()`, monotonic nanoseconds.

`FlipEvent`, `InputEvent` and `Control` are plain data in `neptron-types`, so that the control protocol can send them. Every trait works as a trait object.

## Tests

`tests/integration` checks that the traits work as trait objects and that the errors carry their context.
