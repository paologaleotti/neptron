# neptron-audio

Audio out: rubato resampler, dynamic rate control and the SDL3 sink.

`docs/spec/pacing-audio.md` is the specification: the control law and its source, the rates, the latency, the threads and the behaviour at start, mute, pause, mode switch, underrun, overrun and forced 50/60 Hz.

## Public API

- `AudioPipeline<S: AudioSink>`: `new(sink, CoreTiming, Resampling)`, `set_run_rate_mhz`, `push_samples` (from the core's audio callbacks), `end_frame` (once per core frame), `mute`, `unmute`, `stats`, `sink`. It takes interleaved `i16` stereo at the core's rate and writes to any `AudioSink`.
- `CoreTiming { sample_rate_mhz, frame_rate_mhz }`: the core's AV timing, in millihertz.
- `Resampling::{Sinc, Polynomial}`: the interpolation. `Sinc` is the default.
- `RateControl`: the law alone, `factor(queued_frames)`.
- `FlipRateMeter`: the run rate from flip times, for the desktop window.
- `AudioStats`, `AudioError`, and the limits as named constants.
- With the `sdl3` feature: `Sdl3Sink: AudioSink`, `SinkError`.

## How the session uses it

1. At launch, and when the core's AV info changes: `AudioPipeline::new` with the core's timing. This is the only allocation.
2. Give the run rate (core frames per second of real time): on KMS, the field rate of the mode read back (half of it for double refresh); in the desktop window, `FlipRateMeter::rate_mhz` fed with every `FlipEvent::time_ns`.
3. Each frame: the core's audio goes to `push_samples`; after `retro_run`, call `end_frame`.
4. Before a modeset or a pause: `mute`. After the first flip in the new mode, or on resume: `unmute`.

## Choices

- The law is RetroArch's: `ratio = nominal * (1 + d * clamp((A_B - 2q) / A_B, -1, 1))`, `d` = 0.5%, `A_B` = 64 ms, target 32 ms.
- rubato 5.0.1 `Async`, fixed input, with the chunk size set to each frame's input: no input waits for a full chunk. Sinc: 128 taps, linear interpolation, Blackman-Harris window, 95 output frames of delay from 32 kHz to 48 kHz. Measured on an M1 Pro core: 0.33% of real time in release, 21% in the dev profile. The polynomial mode costs 0.08% and 0.64%.
- SDL3 push model: the frame thread puts samples into an SDL audio stream, and SDL's device thread plays them. No code of this crate runs on the audio thread, so there is no ring buffer and no callback. The stream runs at the device's rate, so SDL does not resample. The sink limits its queue to 128 ms.
- The sdl3 crate's safe API covers the sink. The crate is on the `unsafe` allow-list but uses none: every target root carries `#![forbid(unsafe_code)]`.

## Tests

- Unit tests: the law, the rates and their limits, conversions and fades, the pipeline on `FakeAudio` (priming, restart, mute, unmute, overrun, dropped input), the flip rate meter.
- `tests/integration`: the audio part of timing check 6 on `FakeAudio` and `SimClock`, with no sleep. Ten-minute runs with the polynomial mode; the sinc runs last 30 simulated seconds, because the filter costs 21% of real time in the dev profile.
- `tests/sdl_smoke.rs`: a quiet 440 Hz tone through `Sdl3Sink` for one second. It needs a sound device, so it is ignored by default:

```sh
cargo xtask test -p neptron-audio --features sdl3 --run-ignored only
```

No tier compiles the `sdl3` code. After a change, check it with:

```sh
cargo xtask clippy -p neptron-audio --features sdl3
```

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`
- SDL3 outside the `sdl3` feature

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it, but the crate uses none.
- Features: `sdl3`. No feature is on by default.
- `push_samples` and `end_frame` do not allocate, do no I/O and take no lock of their own.

## Status

Complete for the MVP. The SDL3 sink has run on macOS only; the target hardware is not checked.
