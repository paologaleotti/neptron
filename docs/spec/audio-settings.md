# Audio settings: volume and output device

This spec settles two settings for the MVP (OP-085): the volume, and the playback device. Before this work the sink opens the default device at full level, and no audio setting exists. `docs/spec/pacing-audio.md` stays the spec of the pipeline; this spec adds a gain stage to it and a way to choose the device.

## Facts that drive the design

Read on the bench PC (i3-6100T) on 2026-11-13, read-only:

- One card: `0 [PCH]: HDA-Intel - HDA Intel PCH`. Its PCM devices: `00-00: ALC221 Analog`, `00-03: HDMI 0`, `00-07: HDMI 1`, `00-08: HDMI 2`.
- The image has no `amixer` and no `alsactl`. The codec state in `/proc/asound/card0/codec#0` after boot: the analog DAC amplifiers (nodes `0x02`, `0x03`) are at step 0 of their range, and the output pins (`0x14`, `0x21`) have their mute bit set (`0x80`). The kernel leaves the mixer at its reset state, and nothing in the image sets it. So the analog output is silent today, whatever the frontend plays.
- SDL is 3.4.12 (`libSDL3.so.0.4.12`), with the ALSA backend and no sound server.

What SDL 3.4's ALSA backend does (`src/audio/alsa/SDL_alsa_audio.c`, tag `release-3.4.12`):

- It lists one playback device for each PCM device of each card, with the name `<card name>:<PCM name>`, for example `HDA Intel PCH:ALC221 Analog` and `HDA Intel PCH:HDMI 0`. The default device is a separate entry, `ALSA default playback device`, which opens the ALSA PCM `default`.
- It keeps only the card id of a listed device. To open it, it builds the PCM name `<prefix>CARD=<card id>` (`get_pcm_str`), where the prefix is `hw:`, `sysdefault:` or `default:`, from the first ALSA name hint that has one. The PCM device number is not in that name. So every entry of one card opens the same PCM: the card's default, which is device 0 on an HDA card, the analog output.

The consequence: through SDL 3.4, NEPTRON can choose a card, but not a PCM device of a card. On the bench the HDMI outputs of the HDA card cannot be chosen at all; an entry `HDA Intel PCH:HDMI 0` would play on the analog output. A USB sound card is a second card, so it can be chosen.

## Settings

| Key | Kind | Levels | Default | Live | Safety | Label |
| --- | --- | --- | --- | --- | --- | --- |
| `audio.volume` | integer, 0 to 100 (`VOLUME_MAX_PERCENT`), percent | game, system, global | 100 | yes | none | `Volume` |
| `audio.output` | text, 1 to 128 bytes (`AUDIO_OUTPUT_MAX_LEN`) | global | `auto` | no | none | `Audio output` |

`tests/fixtures/keys.txt` gets two lines at its end: `audio.volume game,system,global int` and `audio.output global text`.

**Volume levels.** The volume follows the layer rules of the scanline strength: a game, a system or the global file can hold it. A per-game volume does not make the design harder: the session takes the resolved value at launch, as it takes the picture settings. The quick menu writes at the game level, then asks where to keep the change, as for every row. The Audio panel writes the global level. Step 5 in the menus; a file can hold any whole percent.

**Default 100.** 100 is unity gain, a bit-exact pass-through. With the mixer at 0 dB (see "The image") this is the level of the core's own samples, as in RetroArch. No reason for a lower default was found. The hashes of the existing audio tests stay valid, because their pipelines run at 100.

**Output.** `auto` is the system default device. Any other value is the raw name of a device as the backend lists it, for example `HDA Intel PCH:ALC221 Analog`. The value is never rewritten by the frontend: a device that is absent at start falls back, and the file keeps the name. A global setting, because the device belongs to the machine, as `display.output` does.

**A new kind of value.** An ALSA device name has spaces, a colon and more than 32 bytes, so the `name` kind (`[A-Za-z0-9_-]`, 32 bytes) cannot hold it. `neptron-settings` gets `Kind::Text { max_len, default }`: 1 to `max_len` bytes of UTF-8 with no control character. In a file it is a TOML string, as a name is. An older build that does not know the key reports it as unknown and keeps it. ADR 0011 records the kind.

## Volume

### The curve

The gain of a volume `v` (whole percent, 0 to 100) is a cubic curve:

```
g(v) = (v / 100)^3
gain_q24(v) = floor(v^3 * 2^24 / 10^6)        // integer, Q24 fixed point
```

| `v` | Gain | dB |
| --- | --- | --- |
| 100 | 1.0 (`GAIN_UNITY_Q24` = 16 777 216) | 0 |
| 75 | 0.42 | -7.5 |
| 50 | 0.125 | -18.1 |
| 25 | 0.0156 | -36.1 |
| 10 | 0.001 | -60.0 |
| 5 | 0.000125 | -78.1 |
| 0 | 0 | silence |

In dB the curve is `60 * log10(v / 100)`: equal steps of the setting are close to equal steps of loudness over the useful range, and the low end reaches silence. It is the cubic volume mapping that PulseAudio and ALSA's `alsamixer` use for a control with no dB scale. It needs no table and no float: `v^3 * 2^24` is at most `2^44`, which fits a `u64`. `v` above 100 is taken as 100.

### Where it applies

In `AudioPipeline::end_frame`, after the conversion to 16-bit and the fade-in, and before the write to the sink. The gain multiplies each sample:

```
out = (sample * g_q24 + 2^23) >> 24          // i64; rounds half up
```

- At 100 the stage is skipped: the output is bit for bit the input.
- Below 100, `|g| < 1`, so `|out| <= |sample|`, and a full-scale input (`-32768`, `32767`) gives no overflow and no clip.
- At 0 every sample is 0. The pipeline still writes the frames, so the rate control and the queue level do not change.
- No allocation: the stage works in place on the buffer that `new` made.
- The fade-out of a mute starts from the last sample written, after the gain, so a mute after a volume change has no step.

### A change takes effect at once, with no click

`AudioPipeline::set_volume_percent(v)` sets a new target. The gain moves from its current value to the target in a straight line over `VOLUME_RAMP_MS` (20 ms) of output frames, 960 frames at 48 kHz, one gain value for each frame. A change during a ramp starts a new ramp from the gain reached. The ramp counts output frames, so it also advances while muted. No device is opened again, and no buffer is made.

`AudioPipeline::with_volume_percent(v)` sets the gain with no ramp, for a pipeline that has not played yet: the session makes the pipeline with the resolved volume at launch, and makes it again with the same volume at a core timing change.

### Mute stays separate

The mute of a mode switch and of the quick menu (`mute`, `unmute`) is a different state. The volume does not change the mute, and the mute does not change the volume. While muted, the frames are silence, and the gain of silence is silence.

The blind recovery cue of the game list (`CUE_MS`, `CUE_HZ` in the app) is not scaled by the volume: it confirms a safety action, and a volume of 0 must not hide it.

### Session and app

- `SessionOptions::volume_percent` (default 100) goes to the pipeline at launch, with no ramp.
- `Session::set_volume_percent(v)` sets it live, with the ramp. The app calls it when `audio.volume` changes in the quick menu or in the Audio panel while a game runs. It does nothing while the source settles at launch, when there is no pipeline yet.
- At a core timing change the new pipeline gets the old target, with no ramp.
- The game list plays no game audio, so a volume change there only reaches the file. The next game resolves it.

## Output device

### The trait

The device list and the open by name are behind a new trait in `neptron-audio` (ADR 0011). `AudioSink` does not change.

```rust
pub trait AudioOutputs {
    type Sink: AudioSink;
    /// The backend's driver, for example `alsa`. The labels depend on it.
    fn driver(&self) -> &str;
    /// The playback devices, in the backend's order, as raw names. The
    /// default device is not in the list.
    fn devices(&mut self) -> Result<Vec<String>, OutputError>;
    /// Opens the default device (`None`) or the device with this raw name.
    fn open(&mut self, device: Option<&str>) -> Result<Self::Sink, OutputError>;
}
```

`Sdl3Outputs` (feature `sdl3`) implements it. It holds the SDL context and the audio subsystem; every SDL type stays in `src/sdl.rs`. `Sdl3Sink::open()` stays, as the default device.

### The choice, in pure code

`open_output(outputs, setting) -> OpenedOutput<S>` decides and returns the sink, the menu list and an optional notice. It is pure apart from the two trait calls.

| Setting | The device is listed | The open | Result | Notice |
| --- | --- | --- | --- | --- |
| `auto` | | default opens | default device | none |
| `auto` | | default fails | no sound | `No sound: the audio device did not open (<error>).` |
| a name | yes | it opens | that device | none |
| a name | yes | it fails, default opens | default device | `The audio output "<label>" did not open (<error>). The default output plays.` |
| a name | no (or the list fails) | default opens | default device | `The audio output "<label>" is not connected. The default output plays.` |
| a name | either | default fails too | no sound | the notice of the row above, then the no-sound notice |

- The setting is never written by this code. The next start tries the named device again, so a USB card that is plugged in later is used again with no change in the menu.
- The first row's notice is the text that the app shows today, so the boot flow keeps its wording.
- The notice uses the label, not the raw name.

### The list the user sees

`menu_outputs(driver, raw) -> Vec<OutputChoice { name, label }>`, pure:

- For the driver `alsa`: entries whose raw names share the card part (the text before the first `:`) are one entry, the first of them, because SDL opens the same PCM for all of them (see the facts). The label is the card part, for example `HDA Intel PCH`. The stored value stays the raw name of that first entry.
- For any other driver (the desktop window on macOS, for example): one entry for each device, labelled with its raw name.
- An empty card part, or a raw name with no `:`, keeps the raw name as the label.

The menu shows `auto` first, then the list. A stored name that is not in the list stays an option, labelled with its raw name and the detail `Not connected`, so the row shows what the file holds. The detail of a listed option is its raw name.

On the bench this gives two options: `auto` and `HDA Intel PCH`. Both play on the analog output. HDMI audio on the HDA card is not reachable (open point below).

### When the device opens: the safe point

The device opens once, when the frontend process starts, before the first frame: in `kms::run` and `window::run`, where `Sdl3Sink::open` runs today. Every launch of a game and every return to the game list starts a new frontend process (`neptron-launch`), so a new process is the safe point: no core runs, and no queue holds audio.

`audio.output` is therefore not live. A change in the Audio panel goes to the file at once. The game list plays no game audio, so the next sound, the next game, already comes from the new device. The frame loop never opens or closes a device.

### Device loss while in use

The pipeline never blocks on the sink: `write` takes what fits and returns.

- With SDL3, a lost device (a USB card unplugged, an ALSA write error) becomes a zombie device inside SDL: SDL's device thread keeps taking the queue at the device rate and drops it. The queue level keeps its normal values, the rate control sees nothing, and the game goes on with no sound. SDL reports the loss as an event; the frontend does not read audio events in the MVP.
- If a backend stops taking data (the queue stays full) or reports an error (`Sdl3Sink` then acts as full), each `end_frame` counts the frames it could not write as an overrun, and the frame loop goes on, paced by the display.
- At the next start, the named device is absent: the fallback row above applies, with its notice.

Tests on the fakes cover both cases: a sink that stops draining, and one that drains and drops.

### Bench

Each start logs one line with the driver, the raw names, and the device that opened:

```
neptron: audio: driver alsa; devices "HDA Intel PCH:ALC221 Analog", "HDA Intel PCH:HDMI 0"; opened "default"
```

`neptronctl log tail` and `cargo xtask os logs` show it. A `neptronctl` command that lists the devices needs a new request in `neptron-ipc` and a command in `apps/neptronctl`, which this task does not own; the log line gives the same facts.

## Menus

- Home gets an `Audio` row after `Display`, when systems exist. The empty home keeps its one `Display` row, so the boot picture of the image check does not change.
- `Audio` opens the Audio panel (`screens::AUDIO_PAGE`, screen id `audio`): a framed page with the rows `Volume` and `Audio output`, written at the global level.
- `Volume` is a number row: left and right step it by 5, and it shows `< 75 % >`. This is the slider control of the settings pages, the same as the scanline strength. The standalone `Slider` template draws a track over two rows and no settings page uses it; putting it in a settings row would change every number row and its goldens, so it is not done here.
- `Audio output` is a choice row: accept opens the list of its options with `auto` first, each with its detail.
- The quick menu gets a `Volume` row after the scanline rows. It writes the game level with effect at once, and the keep prompt offers this game, the system or all games.
- The UI crawler finds both keys through their labels, and the screen `audio`.

## The image

The image sets the mixer of each card to a known state at boot, so that the software volume has the full range:

- `BR2_PACKAGE_ALSA_UTILS=y` with `amixer` only (no `alsactl`, no `alsamixer`, no `aplay`). alsa-utils is GPL-2.0, as BusyBox and the kernel; `make legal-info` lists it.
- `S08neptron-mixer` (PC image) runs at start, in the background so it never delays the boot. It waits up to 10 s for a card in `/proc/asound`. For each card it sets each playback control of a fixed list to 0 dB and unmutes it: `Master`, `PCM`, `Front`, `Surround`, `Center`, `LFE`, `Side`, `Headphone`, `Speaker`, `Line Out`, and every `IEC958` switch. A control the card does not have is skipped. 0 dB, not 100%: on a control whose range goes above 0 dB (some USB cards), the maximum would add gain and could clip.
- Input controls (`Mic`, `Line`, `CD`, `Beep`, the boosts and the loop-back mixers) are not touched: on an HDA codec, `Line` and `Mic` playback volumes mix an input into the output, which adds noise.
- A card that is plugged in after the boot keeps its own state. The frontend does not change the mixer.

## Tests

| Test | Where | Asserts |
| --- | --- | --- |
| Curve | `neptron-audio` gain | `gain_q24(100)` is unity, `gain_q24(0)` is 0, the curve does not fall, it rises on every step of 5, values above 100 are 100, the table values above |
| Stage | gain | unity is bit-exact on full-scale input; full-scale input at every volume stays in `i16` with `|out| <= |in|`; volume 0 gives zeros |
| Ramp | gain | a change ramps in `VOLUME_RAMP_MS` of frames, each step at most the full change over the ramp frames, ends at the target; a change during a ramp starts from the reached gain; `with` has no ramp |
| Pipeline | pipeline | the default pipeline is unchanged (same hashes as before); a pipeline at 50 writes the samples times the gain; a change mid-run has no step larger than the ramp allows; mute and volume are separate |
| Outputs | `neptron-audio` outputs, on a fake `AudioOutputs` over `FakeAudio` | the list and its labels (ALSA collapse, other drivers, odd names); `auto`; a named device that opens; one that is absent (fallback, notice, setting untouched); one that fails to open; the default failing too |
| Device loss | pipeline and session | a sink that stops taking data, and one that drops what it takes: the frames go on, no panic, no hang, overruns counted |
| Settings | `neptron-settings` | schema check, the key list, defaults, parse of good and bad values (out of range, wrong type, empty text, control characters, too long), the levels, an edit and a parse back |
| Session | `neptron-session` | the resolved volume reaches the pipeline at launch; a live change ramps; a core timing change keeps it |
| UI | `neptron-ui` scenes and goldens | `screens/audio`, `screens/audio-output`, `screens/quick-menu-volume`, and `screens/home` with the new row |
| App | `apps/neptron` | the Audio page rows; the quick menu Volume row; a volume change reaches the session; the crawler reaches `audio` and both labels |

No test opens a real device or sleeps.

## Later steps

Not in this task; the orchestrator records them:

- A volume hotkey in a game.
- A test tone when the output changes.
- Follow the HDMI or DisplayPort audio device when the monitor changes.
- Bluetooth audio.
- A sound server.
- A `neptronctl` command that lists the playback devices (needs `neptron-ipc` and `apps/neptronctl`).
- Read SDL's device-removed event and show a notice when the device is lost during a game.

## Open points

- **HDMI and DisplayPort audio on an HDA card.** SDL 3.4's ALSA backend opens only the card's default PCM. Default: the list shows one entry for each card, and HDMI on the HDA card is not offered. How to change: open the device through the SDL hint `SDL_AUDIO_ALSA_DEFAULT_PLAYBACK_DEVICE` with an ALSA PCM name (`hdmi:CARD=PCH,DEV=0`), from a list that NEPTRON reads from ALSA itself, or a newer SDL whose ALSA backend keeps the device number.
- **The mixer list.** The fixed list of control names covers HDA codecs and the usual USB cards. A card with other names keeps its reset state. How to change: add the name to `S08neptron-mixer`.
- **`sdl3` 0.22.1 panics on a device name that is not UTF-8** (`AudioDeviceID::name` unwraps). ALSA names come from the kernel and USB strings, which the kernel gives as UTF-8. Default: accept. How to change: read the names through `sdl3-sys` in an `ffi` module of this crate.
