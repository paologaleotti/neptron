# neptron-audio

Audio out: the rubato resampler, dynamic rate control, the user volume, the choice of the output device and the SDL3 sink.

`docs/spec/pacing-audio.md` is the specification: the control law, the rates, the latency, the resampler, the threads and the behaviour at start, mute, pause, mode switch, underrun, overrun and forced 50/60 Hz. `docs/spec/audio-settings.md` adds the volume and the output device.

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`
- SDL3 outside the `sdl3` feature

The crate is on the `unsafe` allow-list but uses none: every target root carries `#![forbid(unsafe_code)]`. Features: `sdl3`, off by default.

## Public API

- `AudioPipeline<S: AudioSink>`: `new(sink, CoreTiming, run_rate_mhz, Resampling)`, `with_volume_percent` (no ramp), `set_volume_percent` (ramp of `VOLUME_RAMP_MS`), `volume_percent`, `set_run_rate_mhz`, `run_rate_mhz`, `push_samples`, `end_frame`, `idle_frame` (a frame with no core run), `write_cue` and `pending_cue` (a short tone mixed into the next frames), `mute`, `unmute`, `stats`, `sink`. Input is interleaved `i16` stereo at the core's rate.
- `CoreTiming { sample_rate_mhz, frame_rate_mhz }`. `Resampling::{Sinc, Polynomial}`, `Sinc` by default. `RateControl::factor(queued_frames)`: the law alone. `FlipRateMeter`: the run rate from flip times, for the desktop window.
- Volume: `gain_q24(percent)` (the cubic curve `(v / 100)^3` in Q24) and `scale(sample, gain)`. At 100 the stage is skipped, so the output is bit for bit the same as with no volume.
- `AudioStats`: the counters. `underruns` counts the frames that find the queue empty while the device played the frames before. The first frame and the first frame after `idle_frame` do not count; a muted queue that runs dry counts; a new pipeline starts at 0.
- `AudioError`, and the limits as named constants (`MAX_RATE_DEVIATION_PPM`, `CONTROL_SPAN_MS`, `MUTE_COVER_MS`, the rate bounds).
- Output device (spec: audio-settings.md, "Output device"): the trait `AudioOutputs` and `OutputDevice`. `open_output(outputs, setting)` follows the fallback chain of the spec, gives an `OutputNotice` at each step down, keeps every error in `failures`, and never changes the setting. `auto_choice(devices)` takes an HDMI or DisplayPort device only when exactly one has a monitor, else the first analog device, else the default device. Only an `OutputKind::Analog` device is taken by `auto` or the analog fallback.
- ALSA list: `AlsaText::read(root)` reads `cards`, `pcm` and the `eld#*` files under `/proc/asound` once. It is the only file access of the crate; a missing file is empty text. The pure `alsa_outputs(&AlsaText)` makes the devices: stored as `hw:CARD=<id>,DEV=<n>`, opened as `plughw:` (`open_name`). A PCM is HDMI by its id (`HDMI`, `DP`) or by its card (`is_hdmi_card`: the card id or driver holds `HDMI`, such as `vc4-hdmi` of a Raspberry Pi, whose PCM is `MAI PCM i2s-hifi-0`). That card has no ELD file, so its monitor is unknown. The rule reads only text: a USB device whose card id holds `HDMI` is listed as HDMI, and `auto` leaves it out. `/proc` does not map an HDMI PCM to its pin, so a monitor is named only when the counts of its card settle it. `parse_cards`, `parse_pcms`, `parse_eld` are public for the tests and the bench.
- Feature `sdl3`: `Sdl3Sink: AudioSink`, `Sdl3Outputs: AudioOutputs`, `SinkError`. With the ALSA driver, `Sdl3Outputs` lists the PCMs of `/proc/asound` and opens one as SDL's default device through the hint `SDL_AUDIO_ALSA_DEFAULT_PLAYBACK_DEVICE`; with another driver it uses SDL's list.

## How the session uses it

1. At launch, and when the core's AV info changes: `AudioPipeline::new` with the core's timing and the run rate (the core's frame rate when no better figure is known). This is the only allocation, apart from `write_cue`.
2. Give the run rate (core frames per second of real time): on KMS, the field rate of the mode read back (half of it for double refresh); in the desktop window, `FlipRateMeter::rate_mhz` fed with every `FlipEvent::time_ns`.
3. Each frame: the core's audio goes to `push_samples`; after `retro_run`, call `end_frame`.
4. Before a modeset or a pause: `mute`. It fills the queue with silence up to `MUTE_COVER_MS` (100 ms), which covers a TV standard change on the Raspberry Pi 3 (about 75 ms). After the first flip in the new mode, or on resume: `unmute`. The frames after it drop the start of their output until the queue is back at its target, so a short modeset adds no delay.

## Rules

- `push_samples` and `end_frame` do not allocate, do no I/O and take no lock of their own.
- SDL3 push model: the frame thread puts samples into an SDL audio stream, and SDL's device thread plays them. No code of this crate runs on the audio thread, so there is no ring buffer and no callback. The stream runs at the device's rate, so SDL does not resample.
- The sink limits its queue to 128 ms (`SINK_CAPACITY_MS`). It asks for a device period of 1024 frames (`DEVICE_PERIOD_FRAMES`, 21.3 ms at 48 kHz); SDL's ALSA backend makes a buffer of two periods. With 512 frames the analog output of the Pi 3 (`bcm2835`) ran short 14 times a second.
- `SDL_PutAudioStreamData` and `SDL_GetAudioStreamQueued` take SDL's stream lock, which the device thread holds while it converts one device buffer. A priority inversion is possible, and SDL's queue can allocate when it grows. Both are SDL's code; check them on the target.

## Tests

- Unit tests: the law, the rates and their limits, conversions and fades, the gain curve and its ramp, the ALSA list from `/proc` text, the rule of `auto`, the output choice on a fake `AudioOutputs` (also failures and a device lost during a run), the pipeline on `FakeAudio`, the flip rate meter.
- `tests/integration`: the audio part of timing check 6 on `FakeAudio` and `SimClock`, with no sleep (spec: pacing-audio.md, "Tests (the audio part of timing check 6)"). Polynomial runs last 120 simulated seconds; sinc runs last 30 s, because the sinc filter costs 21% of real time in the dev profile. The ten-minute run is an ignored test:

```sh
cargo xtask test -p neptron-audio --run-ignored only -E 'test(ten_minutes)'
```

- `tests/sdl_smoke.rs`: a quiet 440 Hz tone through `Sdl3Sink` for one second. It needs a sound device, so it is ignored by default:

```sh
cargo xtask test -p neptron-audio --features sdl3 --run-ignored only
```

No tier compiles the `sdl3` code. After a change, run `cargo xtask clippy -p neptron-audio --features sdl3`. The SDL3 sink runs on macOS, on the i3-6100T and on a Pi 3 B. `docs/hardware-checklist.md` lists the open hardware checks.
