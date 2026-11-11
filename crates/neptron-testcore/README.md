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

The parser checks every value. A size must fit the maximum size in force at that line. Limits: sizes 1 to 4096 pixels, frame rate 1,000 to 1,000,000 mHz, sample rate 1 to 384,000 Hz, aspect up to 100,000, save RAM up to 16 MiB. An error names the line. A script with an error makes `retro_load_game` return false.

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

## The core

The crate builds as a dynamic library (`cdylib`: `libneptron_testcore.so` on Linux, `libneptron_testcore.dylib` on macOS) that exports all 25 libretro entry points, and as a Rust library that exports the script parser and the frame and audio generators for tests.

| Call | What the core does |
| --- | --- |
| `retro_set_environment` | Calls `SET_SUPPORT_NO_GAME` (true) and `GET_LOG_INTERFACE` |
| `retro_load_game` | Reads the script, calls `SET_PIXEL_FORMAT` (not for `0rgb1555`) and `GET_CAN_DUPE`. The content is not used: `game` may be null. Returns false and logs the reason on a script error or a refused pixel format |
| `retro_get_system_av_info` | The geometry and timing in force |
| `retro_run` | Calls `input_poll`, reads the 16 joypad buttons of port 0, makes the environment calls of the frame's events, sends the frame (or `NULL` for `dupe`), then the audio |
| `retro_reset` | Starts the script again at frame 0 and keeps the save RAM. If the geometry or timing changed since load, the next frame calls `SET_SYSTEM_AV_INFO` with the load values |
| `retro_get_memory_data`, `retro_get_memory_size` | The save RAM for `RETRO_MEMORY_SAVE_RAM`, zeros at load |
| `retro_serialize_size` | 0: no save states |

The frame buffer has the largest maximum size of the script, and the pitch is its full row, so the pitch is larger than the width in most frames. `dupe` sends the previous frame again when the frontend does not allow duplicate frames. The core allocates no buffer after load.

Global state: libretro has no instance handle, so the state is one static behind a mutex. The frontend calls the core from one thread, as libretro.h says. The core holds no lock while it calls into the frontend, so a callback can call back into the core. A call from a second thread while the lock is taken does nothing. One process holds one test core.

Panics: no panic crosses the C boundary. Each entry point runs in `catch_unwind`. After a panic the core does nothing for the rest of the process, and each call returns its default value.

## Must never depend on

- Any NEPTRON crate except `neptron-libretro-sys`

## Rules

- `unsafe` is allowed only on the exported libretro functions, in `src/ffi.rs`: the exports and the calls into the frontend callbacks. Each block has a `// SAFETY:` comment. Every other module carries `#![forbid(unsafe_code)]`. The test core is never in the image.
- No features.
