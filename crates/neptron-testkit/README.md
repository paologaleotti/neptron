# neptron-testkit

Test fakes: a display that records every mode and scene, a fake audio sink, scripted input, a simulated clock and golden helpers. Other crates use it as a dev-dependency. It is library code: typed errors, no `unwrap` outside its own tests.

## Must never depend on

- Any hardware crate, SDL3, or a hardware feature of `neptron-gfx`
- Threads or sleeps: the simulated clock is the only time source
- `neptron-modeline`: only `neptron-video` may depend on it. The screen simulator reaches the policy through `neptron-video`.
- `neptron-ui`: only the app and xtask may depend on it. The screen simulator leaves the text of its labels to xtask.

## Rules

- `#![forbid(unsafe_code)]` at the crate root. No features.
- Clones of each fake share one state behind a mutex, so a test keeps a clone to read the records and gives another to the code under test. The mutex only makes the fakes `Send`. Nothing here starts a thread.

## Public API sketch

- `SimClock: Clock`: the only time source. Clones share one time. Time moves only on `advance`, `advance_to`, or when a fake display waits for a flip.
- `FakeDisplay: Display`: vertical blanks at the field rate of the mode read back, on the simulated clock. `present` queues one flip for the first blank after the call where a flip can land. `wait_flip` moves the clock to that blank. Options (`DisplayOptions`): `Render` mode, `Jitter` (scripted or seeded), seeded `LateFlips`, a programmed refresh rate, a modeset duration, a flip timeout, `tv_standards`, `gpu_ns`, `frozen_vblank_count`. Faults: `ModeFault` (refused, device error, other mode read back) by call number or by requested mode (`fault_requested_mode`); `FlipFault` (late, lost, slow, failed `present`, failed `wait_flip`) by present number; `fault_blank` for one `blank` call. Queries: `mode_calls`, `blank_calls`, `mode_switches`, `active_mode`, `active_tv_standard`, `frames`, `flips`, `flip_times`, `frame_hashes`, `late_frames`, `dropped_frames`, `repeated_vblanks`, `screen`, `capture`, `captured`.
- `FakeDisplay: CoreRender` (`docs/spec/hw-render.md`, section 7.2): with `DisplayOptions::core_offer`, `core_render` hosts the context of a GL core on the reference core GL; without it, the default, `core_render` returns `None`. `create` checks the offer and the size (`CORE_MAX_SIDE_PX`) and makes an image and its framebuffer; `begin` and `end` make the context current and report the GL errors raised in between; `read_back` copies the picture upright by an origin. `present` reads a `GamePicture::Gpu` picture from its image into a memory frame by the origin rule, then renders it as any frame; an image that the display did not make, or that is gone, gives `SceneRefused`. The scene hash adds the kind of the game picture and its origin, so a GPU picture and the same frame in memory hash apart; a memory frame hashes as before. `SceneSummary::gpu_origin` names a GPU picture. Faults: `fault_core_create` (the next `create` fails with `Driver`), `fault_core_loss(end_call)` (that `end` reports a lost context and the image goes), `fault_core_begin_loss(begin_call)` (that `begin` finds the context lost: the image goes and it returns `NoContext`, as the KMS display does), `fault_core_begin(begin_call, reason)` (that `begin` fails with `Driver`). Query: `core_calls` (each call with its result and the presents before it).
- `gl`: the reference core GL, the eight GL functions of the test card as `extern "C"` functions on images in memory, with thread-local state (one thread runs the core and the display). `gl_lookup` is the lookup of the `CoreTarget`. A call while no context is current changes nothing and counts in `gl_calls_without_context`. `gl_left_state` shows the framebuffer, viewport, scissor and blending that a core left. Framebuffer 0 is no surface, as for a surfaceless context; a clear follows the scissor box and not the viewport, as in GL.
- `FakeAudio: AudioSink`: a device that plays its queue at its rate on the simulated clock, from the first write. The device rate can differ from the nominal rate. Queries: `writes`, `overruns`, `underruns`, `played_frames`, `written_frames`, `sample_hash`, `samples` (on request).
- `ScriptedInput: InputSource`: timed RetroPad events. `press`, `release`, `tap`, `set`, `push`; `delivered`, `pending`.
- `golden`: `RgbImage`, `encode_ppm`, `decode_ppm`, `difference`, and `Golden`, which compares an image with `tests/golden/<name>.ppm` at the workspace root.
- `preset`: `monitor_preset(id)` reads `data/monitors/<id>.toml`. `validated_mode` runs `validate` against it. `MODE_240P`, `MODE_288P`, `MODE_480I` and `CAPS_LOW_CLOCK` pass `generic_15`.
- `screen`: the screen simulator (`docs/spec/screen-simulator.md`). It runs one core frame through `neptron_video::VideoEngine` on a `FakeDisplay` with the settings functions of `neptron-video`, then models the glass and measures the picture. `simulate` runs one `Case`; `measure_card` reads the test card from the light of the glass (`GlassShape`, against the core and against the console of `consoles`); `run_gate` gives each stored case its `Verdict`; `survey_rows` runs the survey matrix and flags the rows (`Flag`, `Severity`); `cli::run` is the command line that the example `screen` runs and `cargo xtask screen` starts. It writes PPM and JSON files under `target/screen/`; its goldens are in `tests/golden/screen/`. It depends on `neptron-video`, `neptron-platform` (the GPU table), `neptron-settings` (the schema defaults) and `serde_json`.
- `frame_hash`: FNV-1a 64 over little-endian 64-bit words, the same on every host.

## Determinism

No fake reads the wall clock, sleeps, starts a thread, iterates a hash map or reads an address. Every map is a `BTreeMap`. Jitter and late flips come from a script or from a stateless SplitMix64 of a seed and an index (vertical blank sequence or present number), so the order of calls never changes a draw. The test `a_thousand_frame_session_gives_the_same_output_twice` is the gate.

## Display contract choices

- `present` and `wait_flip` before the first successful `set_mode` return `NoMode`.
- Jitter of half a period or more is refused at `set_mode`, so blanks stay in order.
- Vertical blanks come at the field rate, and the sequence counts fields. A programmed refresh rate overrides the flip rule below: every blank of that rate takes a flip.
- `repeated_vblanks` counts the blanks beyond the flip interval of each frame (`FrameRecord::flip_interval`), so 480i without `flip_per_field` repeats nothing when each frame starts a new picture.
- A late flip is a flip that landed after the first blank following its `present`. A frame presented after its blank has passed lands on the next one and counts as a repeated blank, not as a late flip.

## What the real backend must match

Code that passes on the fake display must also work on the KMS backend. The KMS backend must keep these choices, or the fake must change with it:

- One flip at a time. `present` or `set_mode` while a flip is pending returns `DisplayError::Device`, as a nonblocking atomic commit returns `EBUSY`. The engine must call `wait_flip` first.
- `wait_flip` with no flip queued returns `DisplayError::Device`. It does not block.
- `blank` with a flip pending returns `DisplayError::Device` as busy. After a blank, `present` and `wait_flip` return `NoMode` and the next `set_mode` is a real modeset. The KMS backend never takes its same-mode shortcut after a blank.
- `DisplayOptions::gpu_ns` is the time the GPU draws a presented scene: the flip lands only on a blank after the present plus that time, as a KMS flip waits for the fence of the frame. The clock does not move at the present.
- `DisplayOptions::frozen_vblank_count` makes every flip event carry a vertical blank count of 0, as from a driver that does not count blanks.
- `wait_flip` is bounded. On a timeout or another wait error it returns `DisplayError::Device` and the flip stays queued: a later `wait_flip` returns its event, and until then `present` and `set_mode` are busy. `FlipFault::Lost` never lands, `FlipFault::Slow { timeouts }` lands after `timeouts` timed-out waits, and `FlipFault::WaitFails` fails one wait at once. A timed-out wait moves the simulated clock by `DisplayOptions::flip_timeout_ns` (`DEFAULT_FLIP_TIMEOUT_NS`, 100 ms).
- A failed `set_mode` keeps the old mode active.
- `set_mode` refuses a mode whose pixel clock is not a whole number of kHz with `DisplayError::ModeRefused`, after the busy check, and never rounds it. The kernel's mode record holds the clock in kHz, and the KMS backend refuses such a mode in `ModeRecord::from_validated`. Both use `Modeline::pclock_is_whole_khz`.
- Interlaced flips follow `GpuCaps::flip_per_field`: one flip per field with it, one flip per frame (every second field) without it. So 480i without it gives 30 flips per second.
- A mode with a TV standard (`ValidatedMode::tv_standard`) sets the standard with the mode. A connector without the `TV mode` property (`DisplayOptions::tv_standards` empty, the default, as HDMI or vkms), or without the value of the standard, refuses it with `DisplayError::ModeRefused` and keeps the active mode. A mode that is not one of the two modes of its standard is refused the same way. The check comes after the pixel clock check, and no fault applies to such a mode. A change of the standard alone is a modeset (ADR 0012).

## Render modes and memory per frame

| `Render` | Per frame | Kept for the run | Cost in the dev profile | Use it for |
| --- | --- | --- | --- | --- |
| `Scene` (default) | Hashes the scene (mode size, crop, destination, UI destination, filter, rows without padding) | Two copies of a scene; pixels only for `screen` and captures | Well under 1 ms | Sessions and scripted scenarios |
| `Pixels` | Renders at the mode size and hashes the pixels | Two XRGB8888 buffers of the mode size | About 10 ms for 320x240 | Goldens of every frame; a capture in `Scene` mode is enough for one frame |
| `Off` | Checks the scene only | Nothing | Least | Long timing runs, such as the 100,000-frame pacing check |

Each frame also adds one `FrameRecord` (a few hundred bytes). A capture keeps one RGB image. Each audio write adds one `AudioWrite`.

## Golden helpers

The PPM format (`P6`, maxval 255, one whitespace byte after each header field) and the difference report are the same as in the `neptron-gfx` goldens. Those helpers are private to the `neptron-gfx` test binary, so this crate has its own copy. A check never writes a golden: on a difference or a missing file it writes the actual image to the directory the caller names, and returns an error that says to look at it first.

## Tests

`cargo xtask test -p neptron-testkit`.
