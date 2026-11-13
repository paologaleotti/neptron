# 0011: An audio outputs trait, and a text kind of setting

- Status: accepted
- Date: 2026-11-13
- Task: C12

## Context

The owner wants a choice of the playback device (OP-085). The frontend opens the default device through `Sdl3Sink::open()`. The contract trait `AudioSink` in `neptron-backend` plays samples and knows nothing of devices. The choice must stay behind a trait, so that the fallback rules run on the fakes and no SDL type leaves the backend module.

The setting holds the device name as the backend lists it. SDL 3.4 with ALSA names a device `<card name>:<PCM name>`, for example `HDA Intel PCH:ALC221 Analog`. The `name` kind of `neptron-settings` takes 1 to 32 bytes of `[A-Za-z0-9_-]`, so it cannot hold such a name.

## Decision

- `neptron-audio` gets the trait `AudioOutputs`: the driver name, the list of playback devices as raw names, and an open of the default device or of a named one, which returns an `AudioSink`. `Sdl3Outputs` implements it behind the `sdl3` feature. `AudioSink` and `neptron-backend` do not change.
- The decision (which device to open, the fallback, the notice, the labels of the menu) is a pure function of the trait's answers, in `neptron-audio`.
- `neptron-settings` gets `Kind::Text { max_len, default }`: 1 to `max_len` bytes of UTF-8 with no control character, stored as a TOML string. `audio.output` uses it with a limit of 128 bytes.

## Consequences

- The fallback and the notices are tested on a fake `AudioOutputs` over `FakeAudio`, with no device.
- The trait lives beside the pipeline, not with the contract traits. A second audio backend implements it in its own crate or feature.
- `Kind` has one more variant. Code that matches on `Kind` must handle it; in the workspace that is the settings crate and the app's settings pages.
- The text kind is append-only in the file format: an older build that does not know `audio.output` reports it as unknown and keeps it.

## Alternatives

- Add `devices` and `open_named` to `AudioSink` in `neptron-backend`: a change to a contract trait that every sink and fake must follow, for a step that runs once per process. Not taken.
- Store a sanitised name in the `name` kind: two devices can map to the same name, and the stored value would not be the name that the backend lists. Not taken.
- Store an index into the device list: the order changes when a USB card is plugged in. Not taken.
