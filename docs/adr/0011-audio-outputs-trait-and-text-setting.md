# 0011: An audio outputs trait, and a text kind of setting

- Status: accepted
- Date: 2026-11-13
- Task: C12

## Context

The owner wants a choice of the playback device (OP-085). The frontend opens the default device through `Sdl3Sink::open()`. The contract trait `AudioSink` in `neptron-backend` plays samples and knows nothing of devices. The choice must stay behind a trait, so that the fallback rules run on the fakes and no SDL type leaves the backend module.

SDL 3.4's ALSA backend lists one device per PCM, but opens the card's default PCM for each of them, so an HDMI or DisplayPort output of an HDA card cannot be opened from SDL's list. SDL opens any ALSA PCM name as its default device when the hint `SDL_AUDIO_ALSA_DEFAULT_PLAYBACK_DEVICE` names it.

The setting holds a device name: for ALSA a PCM name such as `hw:CARD=PCH,DEV=3`, for another driver SDL's device name, which can have spaces and more than 32 bytes. The `name` kind of `neptron-settings` takes 1 to 32 bytes of `[A-Za-z0-9_-]`, so it cannot hold these.

## Decision

- `neptron-audio` gets the trait `AudioOutputs`: the driver name, the list of playback devices (`OutputDevice`: the stored name, the label, the detail, and what is known of a monitor), and an open of the default device or of a named one, which returns an `AudioSink`. `Sdl3Outputs` implements it behind the `sdl3` feature. `AudioSink` and `neptron-backend` do not change.
- For the `alsa` driver the list comes from `/proc/asound` (cards, playback PCMs, ELD files), parsed in pure code in `neptron-audio`, and the open sets the SDL hint to the `plughw:` form of the name, then opens SDL's default device. For another driver the list is SDL's.
- The decision (which device to open, the rule of `auto`, the fallback, the notices) is a pure function of the trait's answers, in `neptron-audio`.
- `neptron-settings` gets `Kind::Text { max_len, default }`: 1 to `max_len` bytes of UTF-8 with no control character, stored as a TOML string. `audio.output` uses it with a limit of 128 bytes.

## Consequences

- The fallback and the notices are tested on a fake `AudioOutputs` over `FakeAudio`, with no device.
- The trait lives beside the pipeline, not with the contract traits. A second audio backend implements it in its own crate or feature.
- The ALSA list depends on the text format of `/proc/asound`, which the kernel has kept for many years. `/proc` does not say which HDMI PCM a monitor uses, so the list names a monitor only when the counts settle it.
- `Kind` has one more variant. Code that matches on `Kind` must handle it; in the workspace that is the settings crate and the app's settings pages.
- The text kind is append-only in the file format: an older build that does not know `audio.output` reports it as unknown and keeps it.

## Alternatives

- Add `devices` and `open_named` to `AudioSink` in `neptron-backend`: a change to a contract trait that every sink and fake must follow, for a step that runs once per process. Not taken.
- Store a sanitised name in the `name` kind: two devices can map to the same name, and the stored value would not be the name that the backend lists. Not taken.
- Store an index into the device list: the order changes when a USB card is plugged in. Not taken.
- Use SDL's ALSA list: it cannot reach an HDMI or DisplayPort PCM of an HDA card. Not taken.
- Read the ELD control or the HDMI jack through the ALSA control or input interface: exact for the monitor of each PCM, but it needs an `ioctl` dependency edge. Left as an open point of the spec.
