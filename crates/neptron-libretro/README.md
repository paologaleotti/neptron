# neptron-libretro

Safe libretro core host: load a core, environment callbacks, save RAM, core options. `docs/spec/libretro-host.md` is the specification.

The host produces data and decides nothing for the session. It does not own the display, the audio device, the input devices or the clock.

## Public API

- `Host::open(core, HostConfig)`: claims the process, loads the library, checks the 25 entry points and the API version, gives the callbacks, calls `retro_init`.
- `system_info()`, `options()`, `set_option(key, value)`.
- `load_game(Option<ContentRequest>) -> AvInfo`: a plain file, one entry of a `.zip` file, or no content.
- `set_pad(port, Pad)`, then `run_frame() -> FrameOutput<'_>`: the video (`Frame`, `Duplicate` with the last frame, `Missing` or `Rejected`), the audio, the events (`Geometry`, `AvInfo`, `PixelFormat`, `Shutdown`, `Message`, `Refused`), the log records and the drop counts.
- `reset()`, `av_info()`, `pixel_format()`, `logs()`.
- `copy_save_ram(&mut Vec<u8>)`, `load_save_ram(&[u8])`.
- File functions for the session to call off the frame thread: `save_file_path`, `read_save_file`, `write_save_file` (temp file, fsync, rename, one `.bak`).
- `read_zip_entry`, `GameGeometry::from_core`, `Timing::from_core`: the checks, usable on their own (for example to replay core traces).

## Decisions

- **One core per process.** The first `Host::open` sets a process-wide flag that is never cleared. The library is never closed. The application re-executes itself for the next game.
- **One thread.** `Host` is `!Send` and `!Sync`. The callback state is a box that the host lends to a thread-local slot for each call into the core. A callback from another thread gets its default answer.
- **Frames are copied.** The video callback copies the frame, packed, into a buffer of the largest frame size, made at load and at an AV change. `FrameOutput` borrows the host until the next call. A 320x240 XRGB8888 copy is about 300 KiB.
- **Log callback.** The libretro log callback is variadic. `src/log_shim.c` formats the text with `vsnprintf` and passes it to Rust. Snes9x calls its log pointer without a null check, so the host must give one.
- **Core options.** Version 0 (`SET_VARIABLES`). FCEUmm, Snes9x and Genesis Plus GX use the libretro-common options template, which falls back to version 0 when the frontend reports version 0.
- **Zip content.** `miniz_oxide` for deflate and a small central-directory reader in this crate. FCEUmm and Genesis Plus GX report `need_fullpath` and lower it per extension with `SET_CONTENT_INFO_OVERRIDE`, then read the data with `GET_GAME_INFO_EXT`; the host supports both, so all three MVP cores take zip content. A core that needs a path for the zip entry gets `LoadError::ZipNeedsFullPath`.
- **Memory of a zip load.** The reader holds the compressed entry (at most about 68 MiB: the 64 MiB content limit plus the deflate bound) and the inflated output (at most 64 MiB) at the same time, plus the central directory (at most 16 MiB). The peak is about 148 MiB in the worst case, during the load only. After the load the host keeps the output, at most 64 MiB, until the game is unloaded.
- **Rotation, memory maps, hardware rendering, options v1 and v2** are refused and logged once. An unknown environment call returns false and is logged once.

## Tests

- Unit tests of each module: zip reader (with damaged archives), geometry and timing conversion, save files (backup, stale temp file, symlink at the temp path), options, pads, log queue, frame checks, audio buffer, state.
- `src/ffi/fake_core.rs`: a fake core of Rust `extern "C"` functions, driven through the same entry point table: a wrong API version, hostile frames, audio and environment data, callbacks from another thread, zip content for a core that takes data, content overrides with `GET_GAME_INFO_EXT`, save RAM checks.
- `tests/test_core.rs`: the built `neptron-testcore` library with the fakes of `neptron-testkit`: frame hashes, audio, events, duplicate frames, input read-back, options, save RAM through a file, the unknown call logged once, and each error path of `Host::open` and `load_game`.

The host allows one core per process, so each test needs its own process. `cargo xtask test` runs them with `cargo nextest`, which gives that. Plain `cargo test` fails the second test of each binary.

## Dependencies

- `neptron-libretro-sys`, `neptron-types`, `libloading`, `miniz_oxide`, `thiserror`.
- `cc` (build): the log shim.
- Dev: `neptron-testcore`, `neptron-testkit`, `neptron-backend`.

## Must never depend on

- `neptron-ui`, `neptron-kms`, `neptron-video`

## Rules

- `unsafe` is only in `src/ffi.rs` and its child modules. The crate root denies it elsewhere. Every `unsafe` block has a `// SAFETY:` comment. The integration test sets the test core's script variable with one `unsafe` call.
- No features.
