# Libretro host

This spec defines `crates/neptron-libretro`: the safe host for one libretro core in the frontend process. It turns the C callbacks of `libretro.h` into a typed Rust API for the session.

The host loads a core, loads content, runs one frame and returns what that frame produced. It reads and writes save RAM and keeps the core options. It does not own the display, the audio device, the input devices or the clock. It produces data, and the session decides what to do with it.

## Scope

In scope:

- Software-rendered cores: FCEUmm, Snes9x, Genesis Plus GX and `neptron-testcore`.
- The "Must" environment calls below, and the calls that the three MVP cores need to load content and to declare options.
- Content from a plain file, or from one entry of a `.zip` file (stored or deflate).
- Save RAM in memory and in a `.srm` file.
- Core options with the version 0 API (`SET_VARIABLES`).

Out of scope for the MVP:

- Hardware rendering (`SET_HW_RENDER` and its interfaces).
- Save states, disk control, VFS, rumble, the frame-time callback, audio buffer status, the software framebuffer, memory maps, achievements, subsystems.
- Rotation. The scope table puts it after the MVP.
- `.7z` content. Zip content for a core that needs a path on disk.

## Trust boundary

A core is native code in the frontend process. It can corrupt any memory, and no check in the host can see that. The host trusts one thing: the file is a libretro core that keeps the memory rules of `libretro.h` (a pointer that the core gives is valid for the extent that the header defines, for the time that the header defines).

The host checks every value that it can check, before use:

- A null pointer, where the header allows one or not.
- A size or a count against a named limit and against the maximum size that the core announced.
- A length calculation, with checked arithmetic.
- A C string: the host reads at most `MAX_CORE_STRING_BYTES` bytes and stops at the first NUL. A string with no NUL in that range is refused.
- A floating-point value from the core: finite, in range, then converted to an integer one time.

A value that fails a check gives a typed error, a refused environment call or a refused frame. It never gives undefined behaviour in the host's own code and never a panic.

`Host::open` is a safe function. Loading a library runs its initialisers, so `libloading` marks the load `unsafe`. The host accepts that risk for the caller, because in-process cores are the design (the risk table of the Definition names it). The session and the application stay free of `unsafe` code.

## Public API

```rust
pub struct HostConfig {
    pub system_dir: PathBuf,   // BIOS folder, for GET_SYSTEM_DIRECTORY
    pub save_dir: PathBuf,     // state/saves/<system>/, for GET_SAVE_DIRECTORY
}

pub struct Host { /* private; !Send and !Sync */ }

impl Host {
    pub fn open(core: &Path, config: HostConfig) -> Result<Host, OpenError>;
    pub fn system_info(&self) -> &SystemInfo;
    pub fn options(&self) -> &[CoreOption];
    pub fn set_option(&mut self, key: &str, value: &str) -> Result<(), OptionError>;
    pub fn load_game(&mut self, content: Option<ContentRequest<'_>>) -> Result<AvInfo, LoadError>;
    pub fn av_info(&self) -> Option<AvInfo>;
    pub fn pixel_format(&self) -> PixelFormat;
    pub fn set_pad(&mut self, port: usize, pad: Pad) -> Result<(), InputError>;
    pub fn run_frame(&mut self) -> Result<FrameOutput<'_>, RunError>;
    pub fn reset(&mut self) -> Result<(), RunError>;
    pub fn logs(&self) -> &[LogRecord];
    pub fn copy_save_ram(&mut self, out: &mut Vec<u8>) -> Result<usize, SaveRamError>;
    pub fn load_save_ram(&mut self, bytes: &[u8]) -> Result<SaveRamLoad, SaveRamError>;
}

pub struct ContentRequest<'a> {
    pub path: &'a Path,           // a plain file or a .zip file
    pub extensions: &'a [String], // the extensions that the system accepts, without dots
}

pub struct FrameOutput<'a> {
    pub video: Video<'a>,
    pub audio: &'a [i16],        // interleaved stereo, left first
    pub events: &'a [Event],
    pub logs: &'a [LogRecord],
    pub dropped: Dropped,         // audio frames, events and log records that did not fit
}

pub enum Video<'a> {
    Frame(Frame<'a>),             // a new frame, packed rows
    Duplicate(Option<Frame<'a>>), // the core sent NULL; the last frame, if one exists
    Missing,                      // the core did not call video_refresh this frame
    Rejected(FrameFault),         // the core sent a frame that failed a check
}

pub enum Event {
    Geometry(GameGeometry),       // SET_GEOMETRY: new base size or aspect, same maximum
    AvInfo(AvInfo),               // SET_SYSTEM_AV_INFO: geometry and timing
    PixelFormat(PixelFormat),     // SET_PIXEL_FORMAT during a frame
    Shutdown,                     // the core asks to stop
    Message { text: String, frames: u32 }, // SET_MESSAGE: a notice for the UI
    Refused(Refusal),             // the host refused a request of the core, with the reason
}

pub struct GameGeometry { pub base_width: u16, pub base_height: u16,
                          pub max_width: u16, pub max_height: u16, pub aspect_milli: u32 }
pub struct Timing { pub fps_mhz: u32, pub sample_rate_mhz: u32 }
pub struct AvInfo { pub geometry: GameGeometry, pub timing: Timing }
```

Free functions for save files (they do file I/O, so the session calls them off the frame thread):

```rust
pub fn save_file_path(save_dir: &Path, content: &Path) -> Option<PathBuf>;
pub fn read_save_file(path: &Path) -> Result<Option<Vec<u8>>, SaveFileError>;
pub fn write_save_file(path: &Path, bytes: &[u8]) -> Result<(), SaveFileError>;
```

`Pad` holds the 16 RetroPad buttons and the two analog sticks of one port. `Pad::apply(control, value)` takes one `Control` and value from an `InputEvent`. The session maps input devices to ports.

The session does not get a `SourceGeometry` from the host. The engine builds it from `AvInfo`, the frame sizes and its own settings (interlace, rotation).

### The order of calls

1. `Host::open`: claims the process, loads the library, checks the symbols and the API version, sets the six callbacks, calls `retro_init`, reads `retro_get_system_info`. The core declares its options here in most cases.
2. The session reads `options()` and calls `set_option` with the stored values.
3. `load_game`: prepares the content, calls `retro_load_game`, reads `retro_get_system_av_info`, makes the buffers, calls `retro_set_controller_port_device(port, RETRO_DEVICE_JOYPAD)` for ports 0 and 1.
4. The session calls `read_save_file` and `load_save_ram`.
5. Each frame: `set_pad` for each port, then `run_frame`. The session uses the output before the next call.
6. On a timer and on exit: `copy_save_ram`, then `write_save_file` on a worker thread or at exit.
7. Drop: `retro_unload_game` (if a game is loaded), `retro_deinit`.

## One core per process

libretro callbacks carry no user pointer, so the host keeps process-global state. The Definition makes the frontend re-execute itself between games, because some cores do not unload cleanly. The host enforces this:

- A process-wide `AtomicBool` is set by the first `Host::open`, before the library is opened. It is never cleared. A second `Host::open` in the same process returns `OpenError::ProcessAlreadyUsed`, also after the first host is dropped or failed.
- The library is never closed. After `retro_deinit` the host keeps it mapped until the process exits. A core can leave atexit handlers, thread-local destructors or threads that point into its code; closing the library would make them crash at exit. The process re-executes for the next game, so the mapping costs nothing.
- Drop calls `retro_unload_game` and `retro_deinit` once each, in that order, and is safe to run after any failed step.

## Global state and the thread rule

`libretro.h` says that the frontend drives a core from one thread. The host makes that a type rule: `Host` is `!Send` and `!Sync`, so it stays on the thread that opened it.

The callback state (`State`) lives in a `Box` that the host owns. It holds the pixel format, the geometry and timing, the frame buffer, the audio buffer, the event and log queues, the options, the pads, the directory strings and the content strings.

For each call into the core, the host lends the box to a thread-local slot:

1. Move the box out of the host into the thread-local slot.
2. Call the core.
3. Move the box back.

Moving a box moves one pointer: no allocation, no lock. No lock is held across a call into the core.

A callback finds the state through the thread-local slot, with `try_with` and `RefCell::try_borrow_mut`, which do not panic. The invariants:

- The slot holds a state only during a call from the host into the core, on the host's thread.
- A callback on another thread, or a callback outside a call from the host (for example from a core thread after `retro_run` returned), finds no state. It then does nothing and returns the default: `false` for the environment, `0` for input, the full count for audio (the samples are dropped, so a core that loops until its audio is taken does not hang).
- A callback that finds the slot borrowed (a nested callback) also gets the default.
- Pointers that the host gives to the core (directory strings, option values, content data and strings, the log function) point into heap memory of the state. That memory does not move when the box moves. Each pointer stays valid as long as the header requires: directories for the life of the host, an option value until that option changes, content until `retro_unload_game`.

## Panics

No panic crosses an `extern "C"` callback. The callback code has no panic path: checked arithmetic, `get` instead of indexing, `try_with` and `try_borrow_mut`, `try_reserve` for the rare allocation. As a second line, each callback body runs in `catch_unwind` and returns the default on a panic. The dev profile unwinds, so this matters in tests. The release profile aborts, which is also safe.

## Unsafe code

`unsafe` is only in the module `ffi` and its child modules. The crate root carries `#![deny(unsafe_code)]` and allows it on `ffi` alone. `ffi` loads the library, calls the core, defines the callbacks, and reads the pointers that the core gives. It turns each raw value into a plain Rust value and passes it to safe code (`State` and the validators), which decides. Every `unsafe` block has a `// SAFETY:` comment that states the invariant.

## Frames

The video pointer is valid only during the callback. The host copies the frame into its own buffer during the callback, row by row, `width * bytes_per_pixel` bytes per row. The output frame is packed: its pitch is `width * bytes_per_pixel`.

`run_frame` returns `FrameOutput<'_>`, which borrows the host. The borrow checker ends the borrow before the next call into the core, so the data cannot change under the caller.

Why a copy, not a closure inside the callback: the session must present the frame, write audio and run the engine after `retro_run` returns. A closure inside the callback would run session code inside the core's call stack. The copy is one `memcpy` per row. A 320x240 XRGB8888 frame is 300 KiB, well under 0.1 ms on the target CPUs, against a frame budget of 16.6 ms.

The frame buffer holds `max_width * max_height * 4` bytes. The host makes it at `load_game` and makes it larger at a `SET_SYSTEM_AV_INFO` with a larger maximum (with `try_reserve`: a failed allocation refuses the change). It never allocates for a frame.

Checks on each frame, in order. A failed check gives `Video::Rejected(fault)` and the buffer keeps the last good frame:

| Check | Fault |
| --- | --- |
| `data` is null | not a fault: `Video::Duplicate(last frame)` |
| `data` is `RETRO_HW_FRAME_BUFFER_VALID` (`(void*)-1`) | `HardwareFrame` |
| no AV info yet (before `load_game` returned) | `NoGeometry` |
| `width` and `height` are 1 or more | `ZeroSize` |
| `width <= max_width`, `height <= max_height` (the current maximum) | `OverMaximum` |
| `pitch >= width * bytes_per_pixel` | `PitchTooSmall` |
| `pitch <= MAX_PITCH_BYTES` (32,768) | `PitchTooLarge` |

The last good frame stays in the buffer, so a duplicate gives the caller the pixels to show again. A core that calls `video_refresh` more than once in one `retro_run` gives the last call's result.

The host does not check the alignment of the video pointer, because it copies bytes.

## Audio

The host copies the samples of `audio_sample_batch` and `audio_sample` into one buffer of interleaved stereo `i16`. The buffer holds `max(4 * frames_per_run, 4096)` stereo frames, where `frames_per_run = ceil(sample_rate / fps)`. The host makes it at `load_game` and at a timing change. Samples beyond it are dropped and counted in `Dropped::audio_frames`. A null pointer with a count above zero is a dropped batch. A count whose sample total overflows is dropped.

`audio_sample_batch` returns the count that it got, also for dropped samples.

## Geometry and timing

The core gives floating-point values. The host converts each one time, with the stated rounding, and refuses what an integer contract type cannot hold.

| Value | Rule | Limit |
| --- | --- | --- |
| `base_width`, `base_height` | 1 or more | `MAX_DIMENSION_PX` (4,096) |
| `max_width`, `max_height` | at least the base size | `MAX_DIMENSION_PX` |
| `aspect_ratio` (`f32`) | `<= 0`, NaN or infinite means "width over height", as `libretro.h` says for `<= 0`. Otherwise `round(aspect * 1000)`, half away from zero | 1 to `MAX_ASPECT_MILLI` (100,000) thousandths |
| `fps` (`f64`) | finite, then `round(fps * 1000)` mHz | 1,000 to 1,000,000 mHz |
| `sample_rate` (`f64`) | finite, then `round(rate * 1000)` mHz | 1,000 to 384,000,000 mHz |

The derived aspect is `round(base_width * 1000 / base_height)`, in integers.

The sample rate is in millihertz, not hertz, because cores report fractions (32,040.5 Hz for some SNES cores).

- `retro_get_system_av_info` after `retro_load_game`: a value that fails a check fails the load with `LoadError::InvalidAvInfo`. The host unloads the game first.
- `SET_GEOMETRY`: `libretro.h` says the maximum does not change, so the host keeps the current maximum and checks the new base size against it. On success: `true` and `Event::Geometry`. On failure: `false` and `Event::Refused`.
- `SET_SYSTEM_AV_INFO`: the full check. On success the host grows the buffers if the maximum grew, then answers `true` and adds `Event::AvInfo`. On failure: `false` and `Event::Refused`.
- Either call before `load_game` has read the AV info: the host answers `true` and ignores it, because the AV info read after the load supersedes it. It logs this once.

The engine gets the event in the output of the frame where the core made the call. The frames after the call in the same `retro_run` already use the new size.

## Pixel formats

| libretro | `PixelFormat` |
| --- | --- |
| `RETRO_PIXEL_FORMAT_0RGB1555` (0, the default) | `Xrgb1555` |
| `RETRO_PIXEL_FORMAT_XRGB8888` (1) | `Xrgb8888` |
| `RETRO_PIXEL_FORMAT_RGB565` (2) | `Rgb565` |

Another value: `false` and `Event::Refused`. A change during `run_frame` also adds `Event::PixelFormat`. A change during `load_game` is in `pixel_format()`.

## Input

`set_pad(port, pad)` sets the state that the core reads in the next `run_frame`. `MAX_PORTS` is 4; a higher port is `InputError::NoSuchPort`.

`input_state(port, device, index, id)`:

- `RETRO_DEVICE_JOYPAD`, `id` 0 to 15: the button bit.
- `RETRO_DEVICE_JOYPAD`, `id == RETRO_DEVICE_ID_JOYPAD_MASK`: all 16 bits.
- `RETRO_DEVICE_ANALOG`, index 0 (left) or 1 (right), id 0 (x) or 1 (y): the axis.
- Anything else, and any port at or above `MAX_PORTS`: 0.

`input_poll` does nothing: the state is already set.

## Environment calls

An unknown call returns `false` and gets one log record (level warn) per command number. The host remembers up to 64 command numbers; after that it stops logging new ones. A known call that the host refuses also gets one log record per command number (level info).

| Call | Answer |
| --- | --- |
| `GET_CAN_DUPE` | `true`, writes `true` |
| `SET_PIXEL_FORMAT` | see Pixel formats |
| `GET_SYSTEM_DIRECTORY` | `true`, writes `HostConfig::system_dir` |
| `GET_SAVE_DIRECTORY` | `true`, writes `HostConfig::save_dir` |
| `SET_VARIABLES` | `true`, declares the options (see Core options) |
| `GET_VARIABLE` | `true` with the current value when the key is declared; otherwise `false` |
| `GET_VARIABLE_UPDATE` | `true`, writes whether a value changed since the last call |
| `GET_CORE_OPTIONS_VERSION` | `true`, writes 0 |
| `SET_CORE_OPTIONS`, `_INTL`, `_V2`, `_V2_INTL`, `SET_CORE_OPTIONS_DISPLAY`, `SET_CORE_OPTIONS_UPDATE_DISPLAY_CALLBACK` | `false`, logged once |
| `GET_LOG_INTERFACE` | `true`, writes the log function (see Log) |
| `SET_INPUT_DESCRIPTORS` | `true`, ignored |
| `SET_CONTROLLER_INFO` | `true`, ignored |
| `GET_INPUT_BITMASKS` | `true` |
| `SHUTDOWN` | `true`, `Event::Shutdown` |
| `SET_GEOMETRY`, `SET_SYSTEM_AV_INFO` | see Geometry and timing |
| `SET_SUPPORT_NO_GAME` | `true`, recorded: `load_game(None)` is then allowed |
| `SET_MEMORY_MAPS` | `false`, logged once. Nothing in the MVP reads core memory maps |
| `GET_LANGUAGE` | `true`, writes `RETRO_LANGUAGE_ENGLISH` |
| `SET_MESSAGE` | `true`, `Event::Message` with the text (at most 1,024 bytes) and the frame count |
| `GET_AUDIO_VIDEO_ENABLE` | `true`, writes video and audio enabled (3) |
| `SET_ROTATION` | `false`, logged once. Rotation is after the MVP |
| `SET_HW_RENDER`, `GET_HW_RENDER_INTERFACE`, `SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE`, `GET_PREFERRED_HW_RENDER` | `false`, logged once |
| `SET_CONTENT_INFO_OVERRIDE` | `true`, records `need_fullpath` per extension (see Content) |
| `GET_GAME_INFO_EXT` | `true` during and after `retro_load_game`, writes a pointer to the extended content information; `false` with no content |
| any other | `false`, logged once |

A call whose data pointer is null where the header needs one returns `false`.

### Which option API the MVP cores need

All three cores use the `libretro_core_options.h` template of libretro-common (checked in the upstream sources of libretro-fceumm, snes9x and Genesis-Plus-GX). The template asks `GET_CORE_OPTIONS_VERSION`. With version 0 it builds the version 0 strings from its v2 definitions, with the default value first, and calls `SET_VARIABLES`. The template then calls `SET_CORE_OPTIONS_DISPLAY`, whose answer it ignores. So version 0 is enough for the MVP, and the v1 and v2 calls return `false`.

### Calls that the MVP cores need for content

FCEUmm and Genesis Plus GX report `need_fullpath = true` in `retro_get_system_info`. They call `SET_CONTENT_INFO_OVERRIDE` with `need_fullpath = false` for their ROM extensions, and read the data through `GET_GAME_INFO_EXT`. Without these two calls they read the file from its path, and zip content would be impossible for NES and Mega Drive. The host supports both calls for this reason. Snes9x reports `need_fullpath = false`.

## Log

The log callback of `libretro.h` is variadic (`printf` style). Stable Rust cannot define a variadic function. The host answers `GET_LOG_INTERFACE` with a function written in C, in `src/log_shim.c`, compiled with the `cc` crate:

```c
void neptron_libretro_log_printf(unsigned level, const char *fmt, ...);
```

It formats the message with `vsnprintf` into a stack buffer of 1,024 bytes (longer messages are cut) and calls one Rust function through a function pointer, which the host sets once in `Host::open`, before the core can get the log function. The Rust function finds the state through the thread-local slot, as the other callbacks do. A message from another thread has no state and is dropped.

Why not answer `false`: Snes9x calls its log pointer without a null check on some paths (an unknown controller port, a failed multi-cart load), so `false` can crash it. FCEUmm and Genesis Plus GX use their own logger when the call fails, and lose nothing. A core log is also the best way to see why a ROM does not load. The shim is 30 lines of C with no state except the function pointer, and `cc` is already the build tool of the Switchres crate.

`LogRecord { level, source, text }`: the source is the core or the host. The log queue holds 64 records of at most 1,024 bytes each, made once at `Host::open`. A record beyond that is counted in `Dropped::log_records`. `run_frame` clears the queue at its start; `FrameOutput::logs` and `Host::logs` show the records of the last call. The host writes no log itself: the session forwards the records to its own log.

## Content

`load_game(Some(request))`:

1. A game is loaded already: `LoadError::GameAlreadyLoaded`.
2. The path must be UTF-8 (`libretro.h` says paths are UTF-8): otherwise `LoadError::InvalidPath`.
3. The path ends in `.zip` (any case) and the core did not set `block_extract`: zip content (below). Otherwise: plain content.
4. `need_fullpath` for the content is the override for its extension (from `SET_CONTENT_INFO_OVERRIDE`), else the value from `retro_system_info`.

Plain content: with `need_fullpath`, the core gets the path and no data. Without it, the host reads the file (at most `MAX_CONTENT_BYTES`, 64 MiB; more is `LoadError::ContentTooLarge`) and the core gets the path and the data.

Zip content:

- The host opens the archive read-only and never changes it.
- It takes the first entry in central-directory order that is not a directory and whose extension is in `request.extensions` (compared without case).
- No such entry: `LoadError::Zip(ZipError::NoMatchingEntry)`.
- `need_fullpath` for the entry's extension: `LoadError::ZipNeedsFullPath`. The MVP does not extract to a file.
- The entry is read into memory: stored or deflate only, at most `MAX_CONTENT_BYTES`, CRC-32 checked. A damaged archive, an encrypted entry, another method, ZIP64, a multi-disk archive or an entry above the limit is a typed `ZipError`.
- The core gets the data and the path `<archive path>#<entry name>`, as RetroArch names it.

`GET_GAME_INFO_EXT` gives: `full_path` (the plain path, or the archive path with the entry), `archive_path` and `archive_file` (zip only), `dir` (the folder of the file or archive), `name` (the file or entry name without folder and extension), `ext` (lower case), `data` and `size` (null and 0 with `need_fullpath`), `file_in_archive`, and `persistent_data = true`: the host keeps the data until `retro_unload_game`.

`load_game(None)`: allowed only after `SET_SUPPORT_NO_GAME` with `true`; otherwise `LoadError::NoContent`. The core gets a null `retro_game_info`.

`retro_load_game` returns false: `LoadError::Refused`. The game is not loaded, the host stays open, and `Host::logs` holds what the core logged.

ROMs and BIOS files are user data. The host opens content read-only and never writes, renames or deletes anything under a content folder or the system folder.

### The zip reader

The host reads zip files with `miniz_oxide` (raw deflate) and its own central-directory reader of about 250 lines.

- `miniz_oxide` is pure Rust, licensed MIT, Zlib or Apache-2.0, and has one dependency (`adler2`, 0BSD, MIT or Apache-2.0). It compiles in about one second. It is the deflate code that `flate2` and the Rust standard library's backtrace code use.
- The `zip` crate with only deflate pulls in `flate2`, `crc32fast`, `indexmap` and more, and also writes archives. The host needs one read path.
- The reader works on `Read + Seek`, so the tests build archives in memory, damaged ones too. CRC-32 is a 256-entry table in the crate.

## Save RAM

`copy_save_ram` reads `retro_get_memory_data(RETRO_MEMORY_SAVE_RAM)` and `retro_get_memory_size` and copies the bytes into the caller's buffer. `load_save_ram` copies bytes into the core's memory. Both run between frames, never during a call into the core.

- Null data or size 0: the core has no save RAM. `copy_save_ram` gives 0 bytes; `load_save_ram` reports 0 bytes loaded.
- Size above `MAX_SAVE_RAM_BYTES` (16 MiB): `SaveRamError::TooLarge`.
- A file of a different size than the core's save RAM: the host loads the smaller of the two lengths, as RetroArch does, and leaves the rest of the core's memory as the core set it. `SaveRamLoad { loaded, file_len, ram_len }` reports this, so the session can log it. The next write has the core's size.

`save_file_path(save_dir, content)` is `<save_dir>/<content file stem>.srm`, the RetroArch name. For a zip it is the stem of the archive.

`write_save_file(path, bytes)` writes atomically and keeps one backup:

1. Remove a stale `<name>.srm.tmp` (a missing file is fine), then create it with `create_new`, so a symlink at that path is never followed. Write, `fsync`.
2. If `<name>.srm` exists: copy it the same way to `<name>.srm.bak.tmp`, `fsync`, rename it to `<name>.srm.bak`.
3. Rename `<name>.srm.tmp` to `<name>.srm`. `fsync` the folder.

At every moment `<name>.srm` is the old file or the new file. A crash leaves a `.tmp` file, which the next write removes.

`read_save_file(path)`: a missing file is `Ok(None)`. A file above `MAX_SAVE_RAM_BYTES` is an error.

## Core options

`SET_VARIABLES` gives an array of `{ key, "Description; default|value2|value3" }` that ends with a null key. The host reads at most `MAX_OPTIONS` (1,024) entries. An entry with no `"; "`, an empty value list or an empty value is skipped and logged. A second `SET_VARIABLES` replaces the list and keeps the current value of each key that is still declared with that value.

`CoreOption` holds the key, the description, the values and the current value. The current value starts as the default (the first value). `set_option(key, value)`: an undeclared key is `OptionError::UnknownKey`, a value not in the list is `OptionError::UnknownValue`. A change sets the flag that `GET_VARIABLE_UPDATE` reads.

The settings crate keeps option values as unknown keys under `[core.<name>]` until open point OP-022 is decided. The session reads them there and calls `set_option`.

## Errors

Each module owns its error enum, made with `thiserror`. Each variant carries the value, the limit or the path.

- `OpenError`: `ProcessAlreadyUsed`, `InvalidPath` (a folder path that is not UTF-8), `Library` (the file is not a loadable library), `MissingSymbol { name }`, `ApiVersion { found, expected }`, `InvalidSystemInfo`, `HostPoisoned`.
- `LoadError`: `GameAlreadyLoaded`, `NoContent`, `InvalidPath`, `ReadContent`, `ContentTooLarge`, `Zip { path, source: ZipError }`, `ZipNeedsFullPath`, `Refused`, `InvalidAvInfo`, `OutOfMemory`, `HostPoisoned`.
- `ZipError`: `Io`, `NoEndOfCentralDirectory`, `MultiDisk`, `Zip64`, `CentralDirectory`, `NoMatchingEntry`, `Encrypted`, `Method`, `TooLarge`, `LocalHeader`, `Inflate`, `SizeMismatch`, `Crc`.
- `RunError`: `NoGame`, `HostPoisoned`.
- `SaveRamError`: `NoGame`, `TooLarge`, `OutOfMemory`, `HostPoisoned`.
- `SaveFileError`: `Read`, `Write`, `TooLarge`, `NoFileName`.
- `OptionError`: `UnknownKey`, `UnknownValue`. `InputError`: `NoSuchPort`.

`HostPoisoned` means the state box did not come back from a call into the core. The design makes this impossible; the error replaces an `expect`.

## Limits

| Constant | Value |
| --- | --- |
| `MAX_DIMENSION_PX` | 4,096 |
| `MAX_PITCH_BYTES` | 32,768 |
| `MAX_ASPECT_MILLI` | 100,000 |
| `MIN_FPS_MHZ`, `MAX_FPS_MHZ` | 1,000, 1,000,000 |
| `MIN_SAMPLE_RATE_MHZ`, `MAX_SAMPLE_RATE_MHZ` | 1,000, 384,000,000 |
| `MAX_CONTENT_BYTES` | 64 MiB |
| `MAX_SAVE_RAM_BYTES` | 16 MiB |
| `MAX_CORE_STRING_BYTES` | 16,384 |
| `MAX_OPTIONS` | 1,024 |
| `MAX_CONTENT_OVERRIDES` | 64 |
| `MAX_PORTS` | 4 |
| `EVENT_CAPACITY` | 32 per frame |
| `LOG_CAPACITY`, `LOG_TEXT_MAX_BYTES` | 64 records, 1,024 bytes |
| `MAX_LOGGED_COMMANDS` | 64 |

## No allocation in the frame path

`run_frame` allocates nothing in the steady state. The frame buffer, the audio buffer, the event queue and the log queue (with the text buffer of each record) are made at `Host::open` or `load_game`, or at a timing change. The exceptions are rare and bounded: `Event::Message` allocates its text, a `SET_SYSTEM_AV_INFO` with a larger maximum grows the buffers, a `SET_VARIABLES` allocates the option list.

## Tests

`cargo nextest` runs each test in its own process. The host allows one core per process, so the tests depend on that; plain `cargo test` would fail the second test of a binary.

- Unit tests of each safe module: geometry conversion and every refusal, option parsing, the zip reader (stored, deflate, damaged archives of each kind, no entry, too large, ZIP64, encrypted, CRC), save files (round trip, backup, stale temp file, a symlink at the temp path, wrong size), pads, the log queue.
- Unit tests in `ffi` with a fake core made of Rust `extern "C"` functions: a wrong API version, zip content for a core without `need_fullpath`, `SET_CONTENT_INFO_OVERRIDE` with `GET_GAME_INFO_EXT`, hostile frames, audio and environment data, callbacks with no host active.
- Integration tests with `neptron-testcore` (the built `cdylib`, found next to the test binary as in the test core's own load test) and `neptron-testkit` (scripted input, fake audio, simulated clock): the frame hash of every frame equals `expected_hash`, audio counts and samples match, geometry and AV changes arrive at their frames, a duplicate frame, input read-back, save RAM through a file, core options, the unknown call logged once, a refused load with the core's log, a zip for a core that needs a path, a library that is not a core, a missing symbol and a second host.
