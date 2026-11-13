# Audio settings: volume and output device

This spec settles two settings for the MVP (OP-085): the volume, and the playback device. Before this work the sink opens the default device at full level, and no audio setting exists. `docs/spec/pacing-audio.md` stays the spec of the pipeline; this spec adds a gain stage to it and a way to choose the device.

## Facts that drive the design

Read on the bench PC (i3-6100T) on 2026-11-13, read-only:

- One card: `0 [PCH]: HDA-Intel - HDA Intel PCH`. Its PCM devices: `00-00: ALC221 Analog`, `00-03: HDMI 0`, `00-07: HDMI 1`, `00-08: HDMI 2`.
- The image has no `amixer` and no `alsactl`. The codec state in `/proc/asound/card0/codec#0` after boot: the analog DAC amplifiers (nodes `0x02`, `0x03`) are at step 0 of their range, and the output pins (`0x14`, `0x21`) have their mute bit set (`0x80`). The kernel leaves the mixer at its reset state, and nothing in the image sets it. So the analog output is silent today, whatever the frontend plays.
- SDL is 3.4.12 (`libSDL3.so.0.4.12`), with the ALSA backend and no sound server.
- The monitor of the bench is on DisplayPort. `/proc/asound/card0/eld#2.0` holds `monitor_present 1`, `eld_valid 1`, `monitor_name DELL U2724D`, `connection_type DisplayPort`. The files `eld#2.1` to `eld#2.8` hold `monitor_present 0`, `eld_valid 0`.
- `snd_hda_codec_hdmi` runs with `static_hdmi_pcm=N`. The kernel makes one input device per HDMI PCM, named `HDA Intel PCH HDMI/DP,pcm=3`, `...pcm=7` and `...pcm=8`.

What SDL 3.4's ALSA backend does (`src/audio/alsa/SDL_alsa_audio.c`, tag `release-3.4.12`):

- It lists one playback device for each PCM device of each card, with the name `<card name>:<PCM name>`, for example `HDA Intel PCH:ALC221 Analog` and `HDA Intel PCH:HDMI 0`. The default device is a separate entry, `ALSA default playback device`, which opens the ALSA PCM `default`.
- It keeps only the card id of a listed device. To open it, it builds the PCM name `<prefix>CARD=<card id>` (`get_pcm_str`), where the prefix is `hw:`, `sysdefault:` or `default:`, from the first ALSA name hint that has one. The PCM device number is not in that name. So every entry of one card opens the same PCM: the card's default, which is device 0 on an HDA card, the analog output.

- The canonical default device has an empty card id. For it, `get_pcm_str` reads the hint `SDL_AUDIO_ALSA_DEFAULT_PLAYBACK_DEVICE` (`SDL_HINT_AUDIO_ALSA_DEFAULT_PLAYBACK_DEVICE`), then `SDL_AUDIO_ALSA_DEFAULT_DEVICE`, then uses `default`. `ALSA_OpenDevice` calls `get_pcm_str`, so the hint is read when the device opens, not when the subsystem starts. A PCM that `snd_pcm_open` refuses makes the open fail with an error.

The consequence: SDL's list can choose a card, but not a PCM device of a card. An entry `HDA Intel PCH:HDMI 0` plays on the analog output. So for the `alsa` driver NEPTRON makes its own list of playback PCMs, and opens one through the hint.

What the kernel shows of an HDMI or DisplayPort output (`sound/hda/codecs/hdmi/hdmi.c`, Linux 6.18):

- `eld_proc_new` names the file `eld#<codec address>.<pin index>`. The second number is the index of the pin (with one entry for each device of a multi-stream pin), not a PCM device.
- With `static_pcm_mapping` off (the bench), a pin gets a PCM when its ELD becomes valid (`update_eld` calls `hdmi_attach_hda_pcm`): the pin's last PCM if it is free, else the first free one (`hdmi_find_pcm_slot`). The PCM of a pin therefore depends on the order of the connects since boot.
- No file in `/proc` maps a PCM device to its pin or its ELD. The PCM-interface control `ELD` (with the PCM device as its device number) and the jack `HDMI/DP,pcm=N` do, but both need the ALSA control or input interface, not `/proc`.

So from `/proc` the monitor of an HDMI PCM is known only in the cases that the counts settle (see "The list the user sees").

## Settings

| Key | Kind | Levels | Default | Live | Safety | Label |
| --- | --- | --- | --- | --- | --- | --- |
| `audio.volume` | integer, 0 to 100 (`VOLUME_MAX_PERCENT`), percent | game, system, global | 80 | yes | none | `Volume` |
| `audio.output` | text, 1 to 128 bytes (`AUDIO_OUTPUT_MAX_LEN`) | global | `auto` | no | none | `Audio output` |

`tests/fixtures/keys.txt` gets two lines at its end: `audio.volume game,system,global int` and `audio.output global text`.

**Volume levels.** The volume follows the layer rules of the scanline strength: a game, a system or the global file can hold it. A per-game volume does not make the design harder: the session takes the resolved value at launch, as it takes the picture settings. The quick menu writes at the game level, then asks where to keep the change, as for every row. The Audio panel writes the global level. Step 5 in the menus; a file can hold any whole percent.

**Default 80.** 100 is unity gain, a bit-exact pass-through, and with the mixer at 0 dB (see "The image") it is the full output of the codec: line level into headphones or the input of a monitor, loud for a game whose samples reach full scale. 80 is -5.8 dB, a comfortable start that leaves the player room to go up. The mixer stays at 0 dB, so 100 still reaches the full output. The pipeline and the session keep unity as their own default (`SessionOptions::volume_percent` is 100): the existing audio hashes are made at 100 and stay valid, and only the app passes the setting.

**Output.** `auto` is the automatic choice (see "`auto`"). Any other value is the name of a device as the backend lists it: on ALSA the PCM name `hw:CARD=PCH,DEV=0`, on another driver SDL's device name. The value is never rewritten by the frontend: a device that is absent at start falls back, and the file keeps the name. A global setting, because the device belongs to the machine, as `display.output` does.

**A new kind of value.** An ALSA PCM name such as `hw:CARD=PCH,DEV=0` has `:`, `=` and `,`, and an SDL device name of another driver has spaces and can pass 32 bytes, so the `name` kind (`[A-Za-z0-9_-]`, 32 bytes) cannot hold them. `neptron-settings` gets `Kind::Text { max_len, default }`: 1 to `max_len` bytes of UTF-8 with no control character. In a file it is a TOML string, as a name is. An older build that does not know the key reports it as unknown and keeps it. ADR 0011 records the kind.

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

- `SessionOptions::volume_percent` (default 100, unity; the app passes the resolved setting, default 80) goes to the pipeline at launch, with no ramp.
- `Session::set_volume_percent(v)` sets it live, with the ramp. The app calls it when `audio.volume` changes in the quick menu or in the Audio panel while a game runs. It does nothing while the source settles at launch, when there is no pipeline yet.
- At a core timing change the new pipeline gets the old target, with no ramp.
- The game list plays no game audio, so a volume change there only reaches the file. The next game resolves it.

## Output device

### The trait

The device list and the open by name are behind a trait in `neptron-audio` (ADR 0011). `AudioSink` does not change.

```rust
pub trait AudioOutputs {
    type Sink: AudioSink;
    /// The backend's driver, for example `alsa`.
    fn driver(&self) -> &str;
    /// The playback devices, in menu order. The default device is not in
    /// the list.
    fn devices(&mut self) -> Result<Vec<OutputDevice>, OutputError>;
    /// Opens the default device (`None`) or the device with this name.
    fn open(&mut self, device: Option<&str>) -> Result<Self::Sink, OutputError>;
}

pub struct OutputDevice {
    pub name: String,      // the value `audio.output` stores
    pub label: String,     // what the menu shows
    pub detail: String,    // the detail of the option
    pub monitor: Monitor,  // NotDigital, Present(Option<name>), Absent, Unknown
}
```

`Sdl3Outputs` (feature `sdl3`) implements it. Every SDL type and the hint stay in `src/sdl.rs`.

### The ALSA list

For the `alsa` driver the list comes from `/proc/asound`, not from SDL. A small reader (`AlsaText::read`) reads, once at start and off the frame loop:

- `/proc/asound/cards`: the index, the id and the name of each card;
- `/proc/asound/pcm`: the PCM devices; only the ones with `playback` count;
- `/proc/asound/card<N>/eld#*`: `monitor_present`, `eld_valid` and `monitor_name` of each HDMI or DisplayPort pin.

A file that is missing or cannot be read is empty text. The reader takes the bytes and converts them with a lossy UTF-8 conversion, so one bad byte in a monitor name does not drop its ELD. The parsing and the list (`alsa_outputs`) are pure and take the text, so the tests give them the text of the bench and of other cases.

- **The stored value** is the ALSA name with the card id, which does not change when another card is plugged in: `hw:CARD=PCH,DEV=3`. The open uses `plughw:CARD=PCH,DEV=3`, so ALSA converts the format and the rate when the hardware needs it.
- **HDMI or DisplayPort PCM**: a PCM whose id starts with `HDMI` or `DP`, or holds `DisplayPort`. Every other playback PCM is analog (a USB card's PCM is too).
- **The monitor of an HDMI PCM**, per card, from the number `v` of ELDs with `monitor_present 1` and `eld_valid 1`, and the number `h` of HDMI PCMs:
  - `v = 0`: no PCM of the card has a monitor (`Absent`): the kernel attaches a PCM only to a pin with a valid ELD.
  - `v = h`: every HDMI PCM has a monitor (`Present`). The name is known only when `h = 1`.
  - else: the monitor of each HDMI PCM is not known (`Unknown`). This is the bench: one valid ELD and three HDMI PCMs.
  An ELD with `monitor_present 1` and `eld_valid 0` is a monitor with no audio; it counts as no monitor.

### The list the user sees

| PCM | Label | Detail |
| --- | --- | --- |
| Analog | `Analog (<card name>)`: `Analog (HDA Intel PCH)` | the PCM name: `ALC221 Analog` |
| S/PDIF (a PCM whose id or name holds `Digital`, `S/PDIF`, `SPDIF` or `IEC958`) | `Digital (<card name>)` | the PCM name |
| HDMI, monitor known by name | `<monitor> (HDMI/DP)`: `DELL U2724D (HDMI/DP)` | `Monitor connected` |
| HDMI, monitor present, name not known | `HDMI/DP <k> (<card name>)` | `Monitor connected` |
| HDMI, monitor not known | `HDMI/DP <k> (<card name>)` | none |
| HDMI, no monitor | `HDMI/DP <k> (<card name>)` | `Nothing connected` |

An HDMI output shows no PCM name: the kernel counts its PCMs from 0 (`HDMI 1` is the second), and next to `HDMI/DP 2` that would confuse.

The part that tells the outputs apart comes first, because the value column of a settings row shows 14 characters. When two labels are still equal, the brackets get more, step by step, until they differ: first the card id (`Analog (USB Audio, Device)`), then the PCM name (`Analog (HDA Intel PCH, PCH, ALC892 Alt Analog)`), then the device number.

`k` counts the HDMI PCMs from 1, in device order: per card when one card has HDMI PCMs, and across all cards when two or more have them (`HDMI/DP 1 (HDA Intel PCH)`, `HDMI/DP 2 (HDA NVidia)`), so that the first 14 characters differ. The order is the cards in index order and their PCMs in device order, and the PCMs with no monitor come last. They stay selectable: a monitor can be connected later.

An S/PDIF output is not an analog output: neither `auto` nor the fallback to the first analog PCM takes it. The list of another driver (SDL's names) has no analog output in this sense either, so there `auto` is the system default device.

The menu shows `auto` first (detail `Automatic`), then the list. A stored value that is not in the list stays an option, labelled with its raw value and the detail `Not connected`.

For another driver (the desktop window on macOS), the list is SDL's, one entry per device, labelled with its name, and the `monitor` is `NotDigital`. SDL's ALSA default entry `ALSA default playback device` is never listed: `auto` stands for it.

On the bench today the list is: `Analog (HDA Intel PCH)` (`hw:CARD=PCH,DEV=0`), `HDMI/DP 1 (HDA Intel PCH)`, `HDMI/DP 2 (...)` and `HDMI/DP 3 (...)` (`DEV=3`, `7`, `8`), all three with no detail, because the counts do not settle which one the DELL monitor uses. After a boot with the monitor connected, it is `HDMI/DP 1` (`DEV=3`): the kernel gives the first valid ELD the first free PCM. This is not guaranteed after the monitor was disconnected and connected again with another pin in between, so NEPTRON does not claim it.

### `auto`

`auto` takes an HDMI or DisplayPort PCM only when exactly one PCM in the list is `Present`. Otherwise it takes the first analog PCM of the list, and when the list has none, the system default device (ALSA's `default`).

- No monitor with audio: the first analog PCM. A CRT behind a DAC gives no ELD, so it gets the analog output. The first analog PCM, not ALSA's `default`: a GPU sound card can be card 0 with no device 0, and then `default` gives no sound while the analog card is card 1.
- One PCM `Present`: that PCM. This is exact: a card with one HDMI PCM and one valid ELD, or a card whose every HDMI PCM has a monitor.
- Two or more `Present`: the first analog PCM.
- `Unknown` (the bench): the first analog PCM. The player chooses the HDMI/DP output in the menu.

The start log says which device `auto` took and why. On the bench: `auto: the first analog output "Analog (HDA Intel PCH)", because an HDMI/DP output may have a monitor but /proc does not show which one`.

### The open, through the hint

For the `alsa` driver, `Sdl3Outputs::open(Some(name))` sets the hint `SDL_AUDIO_ALSA_DEFAULT_PLAYBACK_DEVICE` to the `plughw:` form of the name, then opens SDL's default playback device. `open(None)` sets the hint to `default` first. SDL reads the hint in `ALSA_OpenDevice`, so the device that opens is the one the hint names, and a PCM that ALSA refuses makes the open fail. For another driver, a name opens SDL's device of that name.

### The choice, in pure code

`open_output(outputs, setting) -> OpenedOutput<S>` decides and returns the sink, the list, the choice of `auto`, the notices and every error. The chain, where a device already tried is not tried again:

1. A named device that is listed: open it.
2. Else, or when it fails: the device `auto` takes.
3. When that fails: the first analog PCM of the list.
4. When that fails, or the list has none: the default device.
5. When the default fails: the silent sink.

| Case | Notice |
| --- | --- |
| A named device that is not listed | `The audio output "<label>" is not connected. Another output plays.` |
| A listed device (named, `auto`'s choice or the first analog) that does not open | `The audio output "<label>" did not open (<error>). Another output plays.` |
| No device opens | The notices above without `Another output plays.`, then `No sound: the audio device did not open (<error of the default device>).` |

- Each step down gives a notice, and the log line holds each error: `; failed "<name>" (<error>)`.
- The setting is never written by this code. The next start tries the named device again, so a monitor or a USB card that comes back is used again with no change in the menu.

### When the device opens: the safe point

The device opens once, when the frontend process starts, before the first frame: in `kms::run` and `window::run`. The `/proc` reader runs there too. Every launch of a game and every return to the game list starts a new frontend process (`neptron-launch`), so a new process is the safe point: no core runs, and no queue holds audio.

`audio.output` is therefore not live. A change in the Audio panel goes to the file at once. The game list plays no game audio, so the next sound, the next game, already comes from the new device. A monitor connected after the boot is in the list of the next process. The frame loop never opens or closes a device.

### Device loss while in use

The pipeline never blocks on the sink: `write` takes what fits and returns.

- With SDL3, a lost device (a USB card unplugged, a monitor switched off, an ALSA write error) becomes a zombie device inside SDL: SDL's device thread keeps taking the queue at the device rate and drops it. The game goes on with no sound.
- If a backend stops taking data (the queue stays full) or reports an error (`Sdl3Sink` then acts as full), each `end_frame` counts the frames it could not write as an overrun, and the frame loop goes on, paced by the display.
- At the next start, the fallback chain above applies.

Tests on the fakes cover both cases.

### Bench

Each start logs one line with the driver, the devices, the choice of `auto` and the device that opened:

```
neptron: audio: driver alsa; devices "hw:CARD=PCH,DEV=0", "hw:CARD=PCH,DEV=3", "hw:CARD=PCH,DEV=7", "hw:CARD=PCH,DEV=8"; auto: the first analog output "Analog (HDA Intel PCH)", because an HDMI/DP output may have a monitor but /proc does not show which one; opened "hw:CARD=PCH,DEV=0"
```

The line goes to the log of the run before the frame loop starts, so `cargo xtask os logs` shows it; the protocol log of `neptronctl log tail` starts later and does not hold it. A `neptronctl` command that lists the devices needs a new request in `neptron-ipc` and a command in `apps/neptronctl`, which this task does not own; the log line gives the same facts.

## Menus

- The audio panel is a panel of the Settings page (`docs/spec/ui-navigation.md`): home, also with no games, has one `Settings` row, and `Settings > Audio` opens it.
- `Audio` opens the Audio panel (`screens::AUDIO_PAGE`, screen id `audio`): a framed page with the rows `Volume` and `Audio output`, written at the global level.
- `Volume` is a number row: left and right step it by 5, and it shows `< 75 % >`. This is the slider control of the settings pages, the same as the scanline strength. The standalone `Slider` template draws a track over two rows and no settings page uses it; putting it in a settings row would change every number row and its goldens, so it is not done here.
- `Audio output` is a choice row: accept opens the list of its options with `auto` first, each with its detail, as in "The list the user sees".
- The quick menu gets a `Volume` row after the scanline rows. It writes the game level with effect at once, and the keep prompt offers this game, the system or all games.
- The UI crawler finds both keys through their labels, and the screen `audio`.

## The image

The image sets the mixer of each card to a known state at boot, so that the software volume has the full range:

- `BR2_PACKAGE_ALSA_UTILS=y` with `amixer` only (no `alsactl`, no `alsamixer`, no `aplay`). alsa-utils is GPL-2.0, as BusyBox and the kernel; `make legal-info` lists it.
- `S08neptron-mixer` (PC image) runs at start, in the background so it never delays the boot. It waits up to 10 s for a card in `/proc/asound`; after the first card it waits 2 s more for later cards (a USB card, a second HDA codec), within the same 10 s. For each card it sets each playback control of a fixed list to 0 dB and unmutes it: `Master`, `PCM`, `Front`, `Surround`, `Center`, `LFE`, `Side`, `Headphone`, `Speaker`, `Line Out`, and every `IEC958` switch by its index, on every card. The HDMI codec makes one `IEC958 Playback Switch` for each HDMI PCM (`snd_hda_create_dig_out_ctls`), so the switches `'IEC958',0` to `'IEC958',N` unmute every HDMI or DisplayPort output. A control the card does not have is skipped. Many HDMI outputs give no sound until a stream opens; nothing needs to be done for that. The image on the bench has no `amixer` yet, so the control names there were not read; they come from the kernel source. 0 dB, not 100%: on a control whose range goes above 0 dB (some USB cards), the maximum would add gain and could clip.
- Input controls (`Mic`, `Line`, `CD`, `Beep`, the boosts and the loop-back mixers) are not touched: on an HDA codec, `Line` and `Mic` playback volumes mix an input into the output, which adds noise.
- A card that is plugged in after the boot keeps its own state. The frontend does not change the mixer.

## Tests

| Test | Where | Asserts |
| --- | --- | --- |
| Curve | `neptron-audio` gain | `gain_q24(100)` is unity, `gain_q24(0)` is 0, the curve does not fall, it rises on every step of 5, values above 100 are 100, the table values above |
| Stage | gain | unity is bit-exact on full-scale input; full-scale input at every volume stays in `i16` with `|out| <= |in|`; volume 0 gives zeros |
| Ramp | gain | a change ramps in `VOLUME_RAMP_MS` of frames, each step at most the full change over the ramp frames, ends at the target; a change during a ramp starts from the reached gain; `with` has no ramp |
| Pipeline | pipeline | the default pipeline is unchanged (same hashes as before); a pipeline at 50 writes the samples times the gain; a change mid-run has no step larger than the ramp allows; mute and volume are separate |
| ALSA list | `neptron-audio` alsa | parsing of `cards`, `pcm` (playback only) and ELD text; the bench text gives the four PCMs with `Unknown` HDMI; a card with one HDMI PCM and a valid ELD gives the monitor name; no valid ELD, two valid ELDs, `eld_valid 0`; missing text gives an empty list; labels, details, order and stored values |
| `auto` | outputs | none, one, two `Present`; `Unknown`; the reason text |
| Outputs | outputs, on a fake `AudioOutputs` over `FakeAudio` | `auto`; a named device that opens; one that is absent (fallback to `auto`'s choice, notice, setting untouched); one that fails to open; `auto`'s HDMI choice that fails (then the default); the default failing too; a list that fails |
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

- **The PCM of a monitor.** `/proc` does not map an HDMI PCM to its pin, so with more HDMI PCMs than monitors (the bench) the list has no monitor name and `auto` takes the first analog output. Default: list the HDMI PCMs with their PCM names; the player chooses. How to change: read the PCM-interface control `ELD` (its device number is the PCM) through the ALSA control interface (`SNDRV_CTL_IOCTL_ELEM_READ` on `/dev/snd/controlC<N>`, in the `unsafe` module of `neptron-audio`, with `rustix`), or the switch state of the jack input device `HDMI/DP,pcm=N` (`EVIOCGSW`, in `neptron-input`). Both are exact; both add a dependency edge to a crate (a `Cargo.lock` change for the orchestrator).
- **The mixer list.** The fixed list of control names covers HDA codecs and the usual USB cards. A card with other names keeps its reset state. How to change: add the name to `S08neptron-mixer`.
- **`sdl3` 0.22.1 panics on a device name that is not UTF-8** (`AudioDeviceID::name` unwraps). ALSA names come from the kernel and USB strings, which the kernel gives as UTF-8. Default: accept. How to change: read the names through `sdl3-sys` in an `ffi` module of this crate.
