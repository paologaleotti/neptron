# neptron-testcore

A libretro core that follows a script, for tests. It reads its script from the file that the environment variable `NEPTRON_TESTCORE_SCRIPT` names.

## Script format

A script is a UTF-8 text file. A byte sequence that is not UTF-8 is an error, reported with its line. Each line holds one keyword and its arguments, separated by spaces. A `#` starts a comment to the end of the line. Blank lines do not count. Every number is an unsigned decimal integer, ASCII digits only (no sign, no separators): frequencies in millihertz or hertz, sizes in pixels, aspect ratios in thousandths.

The first line is the header `neptron-testcore 1`. The number is the script version.

The setup lines come next. Each one is optional and can appear once.

| Line | Meaning | Default |
| --- | --- | --- |
| `pixel_format <xrgb8888\|rgb565\|0rgb1555>` | The pixel format. For `0rgb1555`, the libretro default, the core does not call `SET_PIXEL_FORMAT` | `xrgb8888` |
| `geometry <width> <height> <aspect_milli>` | The base geometry at load. An aspect of 0 means width over height | `320 240 0` |
| `max_size <width> <height>` | The maximum frame size at load | the geometry size |
| `timing <fps_mhz> <sample_rate_hz>` | Frame rate and audio sample rate at load | `60000 48000` |
| `audio <batch\|single\|off>` | One `audio_sample_batch` call per frame, one `audio_sample` call per stereo frame, or no audio | `batch` |
| `save_ram <bytes>` | Size of the save RAM. 0 means none | `0` |

Event lines follow: `at <frame> <action>`. Frame 0 is the first `retro_run` after `retro_load_game` or `retro_reset`. The frame numbers must not go down. Events of one frame run in file order, before the core sends that frame.

| Action | What the core does |
| --- | --- |
| `geometry <width> <height> <aspect_milli>` | Calls `SET_GEOMETRY`. Frames take the new size |
| `av_info <width> <height> <aspect_milli> <max_width> <max_height> <fps_mhz> <sample_rate_hz>` | Calls `SET_SYSTEM_AV_INFO`. Frames take the new size, audio the new rate |
| `size <width> <height>` | Sends frames of a new size and tells the frontend nothing, as some real cores do |
| `dupe` | Sends no frame data (`NULL`) for this frame, if the frontend allows duplicate frames |
| `sram_write <offset> <byte>` | Writes one byte of save RAM |
| `shutdown` | Calls `RETRO_ENVIRONMENT_SHUTDOWN` |

The parser checks every value. A size must fit the maximum size in force at that line. Limits: sizes 1 to 4096 pixels, frame rate 1 to 1,000,000 mHz, sample rate 1 to 384,000 Hz, aspect up to 100,000, save RAM up to 16 MiB. An error names the line. A script with an error makes `retro_load_game` return false.

The format is append-only. A new keyword or action can come in a later version. A keyword never changes its meaning or its arguments.

Example:

```text
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 60099 48000
save_ram 8192

at 120 geometry 320 224 1333      # width change under the same maximum
at 300 av_info 320 240 1333 640 480 50000 48000
at 301 sram_write 0 42
at 600 shutdown
```

## Frames

Each pixel is a fixed function of the frame number, the joypad buttons of port 0 that the core read for that frame, and the pixel position: `pixel(format, frame, buttons, x, y)`. The core writes it in native byte order, as a `u32` for `xrgb8888` and a `u16` for the 16-bit formats. Only the colour bits of the format are set.

The frame hash is FNV-1a 64 over the visible bytes: `width` pixels of each row, rows `pitch` bytes apart. `expected_hash(size, format, frame, buttons)` gives the hash of the frame that the core sends. A test that injects a button state can check that the core read it, because the hash changes with each button.

## Audio

The stereo sample with index `n` (from 0 at load or reset) is `[n as i16, -(n as i16)]`, with wrapping. After `k` video frames at a constant timing, the core has sent `k * sample_rate_hz * 1000 / fps_mhz` stereo frames, rounded down (`frames_after`). Each frame sends the difference, so the count never drifts. An `av_info` event starts the count again from that frame, at the new rate.

## Must never depend on

- Any NEPTRON crate except `neptron-libretro-sys`

## Rules

- `unsafe` is allowed only on the exported libretro functions. Each has a `// SAFETY:` comment. The test core is never in the image.
- No features.
