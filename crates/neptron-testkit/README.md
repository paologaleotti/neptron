# neptron-testkit

Test fakes: a display that records every mode and scene, a fake audio sink, scripted input, a simulated clock and golden helpers.

## Public API sketch

- `struct FakeDisplay: Display`, `struct FakeAudio: AudioSink`
- `struct ScriptedInput: InputSource`, `struct SimClock: Clock`

## Must never depend on

- Any hardware crate
- Threads or sleeps: the simulated clock is the only time source

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
