# neptron-testkit

Test fakes: a display that records every mode and scene, a fake audio sink, scripted input, a simulated clock and golden helpers.

Other crates use it as a dev-dependency. It is library code: typed errors, no `unwrap` outside its own tests.

## Public API sketch

- `SimClock: Clock`: the only time source. Clones share one time. Time moves only on `advance`, `advance_to`, or when a fake display waits for a flip.
- `FakeDisplay: Display`: vertical blanks at the field rate of the mode read back, on the simulated clock. `present` queues one flip for the first blank after the call where a flip can land; `wait_flip` moves the clock to that blank. Options: `Render` mode, `Jitter` (scripted or seeded), seeded `LateFlips`, a programmed refresh rate, a modeset duration, a flip timeout. Faults: `ModeFault` (refused, device error, other mode read back) by call number or by the requested mode (`fault_requested_mode`), and `FlipFault` (late, lost, slow, failed `present`, device failure in one `wait_flip`; each keeps the flip queued) by present number. Queries: `mode_calls`, `mode_switches`, `active_mode`, `frames`, `flips`, `flip_times`, `frame_hashes`, `late_frames`, `dropped_frames`, `repeated_vblanks`, `screen`, `capture` and `captured`.
- `FakeAudio: AudioSink`: a device that plays its queue at its rate on the simulated clock, from the first write. Queries: `writes` (time, offered, taken, depth after), `overruns`, `underruns`, `played_frames`, `written_frames`, `sample_hash`, `samples` (on request). The device rate can differ from the nominal rate.
- `ScriptedInput: InputSource`: timed RetroPad events. `press`, `release`, `tap`, `set`, `push`; `delivered` and `pending`.
- `golden`: `RgbImage`, `encode_ppm`, `decode_ppm`, `difference`, and `Golden`, which compares an image with `tests/golden/<name>.ppm` at the workspace root.
- `preset`: `monitor_preset(id)` reads `data/monitors/<id>.toml`; `validated_mode` runs `validate` against it. `MODE_240P`, `MODE_288P`, `MODE_480I` and `CAPS_LOW_CLOCK` pass `generic_15`.
- `frame_hash`: FNV-1a 64 over little-endian 64-bit words, the same on every host.

Clones of each fake share one state behind a mutex, so a test keeps a clone to read the records and gives another to the code under test. The mutex only makes the fakes `Send`; nothing here starts a thread.

## Determinism

No fake reads the wall clock, sleeps, starts a thread, iterates a hash map or reads an address. Every map is a `BTreeMap`. Jitter and late flips come from a script or from a stateless SplitMix64 of a seed and an index (vertical blank sequence or present number), so the order of calls never changes a draw. The test `a_thousand_frame_session_gives_the_same_output_twice` is the gate.

## Display contract choices

- `present` and `wait_flip` before the first successful `set_mode` return `NoMode`.
- One flip at a time. A second `present`, a `set_mode` with a queued flip, or a `wait_flip` with nothing queued returns `Device`, as KMS returns `EBUSY` or never sends an event.
- A failed `set_mode` keeps the active mode. A wait that times out moves the clock by the flip timeout (100 ms by default), returns `Device` and keeps the flip queued.
- Jitter of half a period or more is refused at `set_mode`, so blanks stay in order.
- Vertical blanks come at the field rate, and the sequence counts fields. For an interlaced mode on caps without `flip_per_field`, a flip lands only on the blank that starts a frame, every second field: 480i gives 30 flips per second. With `flip_per_field`, every field takes a flip. A programmed refresh rate overrides both: every blank of that rate takes a flip.
- `repeated_vblanks` counts the blanks beyond the flip interval of each frame (`FrameRecord::flip_interval`), so 480i without `flip_per_field` repeats nothing when each frame starts a new picture.
- A late flip in the records means a flip that landed after the first blank following its `present`. A frame presented after its blank has passed lands on the next one and shows as a repeated blank, not as a late flip.

## What the real backend must match

Code that passes on the fake display must also work on the KMS backend. The KMS backend must keep these choices, or the fake must change with it:

- `present` or `set_mode` while a flip is pending returns `DisplayError::Device`, as a nonblocking atomic commit returns `EBUSY`. The engine must call `wait_flip` first.
- `wait_flip` with no flip queued returns `DisplayError::Device`. It does not block.
- `wait_flip` is bounded. On a timeout or another wait error it returns `DisplayError::Device` and the flip stays queued: a later `wait_flip` returns its event, and until then `present` and `set_mode` are busy. `FlipFault::Lost` never lands, `FlipFault::Slow { timeouts }` lands after `timeouts` timed-out waits, and `FlipFault::WaitFails` fails one wait at once. A timed-out wait moves the simulated clock by `DisplayOptions::flip_timeout_ns`.
- A failed `set_mode` keeps the old mode active.
- Interlaced flips follow `GpuCaps::flip_per_field`: one flip per field with it, one flip per frame without it.

## Render modes and memory per frame

| `Render` | Per frame | Kept for the run | Cost in the dev profile | Use it for |
| --- | --- | --- | --- | --- |
| `Scene` (default) | Hashes the scene (mode size, crop, destination, UI destination, filter, rows without padding) | Two copies of a scene; pixels only for `screen` and captures | Well under 1 ms | Sessions and scripted scenarios |
| `Pixels` | Renders at the mode size and hashes the pixels | Two XRGB8888 buffers of the mode size | About 10 ms for 320x240 | Goldens of every frame; a capture in `Scene` mode is enough for one frame |
| `Off` | Checks the scene only | Nothing | Least | Long timing runs, such as the 100,000-frame pacing check |

Each frame also adds one `FrameRecord` (a few hundred bytes). A capture keeps one RGB image. Each audio write adds one `AudioWrite`.

## Golden helpers

The PPM format (`P6`, maxval 255, one whitespace byte after each header field) and the difference report are the same as in the `neptron-gfx` goldens. Those helpers are private to that crate's test binary, so this crate has its own copy. A check never writes a golden: on a difference or a missing file it writes the actual image to the directory the caller names and returns an error that says to look at it first.

## Must never depend on

- Any hardware crate, SDL3, or a hardware feature of `neptron-gfx`
- Threads or sleeps: the simulated clock is the only time source

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
