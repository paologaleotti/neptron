# neptron-libretro

Safe libretro core host: load a core, environment callbacks, save RAM, core options. `docs/spec/libretro-host.md` is the specification.

The host produces data and decides nothing for the session. It does not own the display, the audio device, the input devices or the clock.

## Public API

- `Host::open(core, HostConfig)`: claims the process, loads the library, checks the 25 entry points and the API version, gives the callbacks, calls `retro_init`.
- `system_info()`, `options()`, `set_option(key, value)`, `hold_options(values)`, `held_option_report()`, `input_descriptors()` (`SET_INPUT_DESCRIPTORS`, at most 256, for the remapping screen).
- `load_game(Option<ContentRequest>) -> AvInfo`: a plain file, one entry of a `.zip` file, or no content.
- `set_pad(port, Pad)`, then `run_frame() -> FrameOutput<'_>`: the video (`Frame`, `Duplicate` with the last frame, `Missing` or `Rejected`), the audio, the events (`Geometry`, `AvInfo`, `PixelFormat`, `Rotation`, `Shutdown`, `Message`, `Refused`), the log records and the drop counts.
- `reset()`, `av_info()`, `pixel_format()`, `rotation()`, `logs()`, `error_logs()` (the first `ERROR_LOG_CAPACITY` error-level records of the last call, kept also when the queue of `logs()` was full), `dropped_logs()`.
- `copy_save_ram(&mut Vec<u8>)`, `load_save_ram(&[u8])`.
- File functions for the session to call off the frame thread: `save_folder` (the folders of a game below the save folder, made with no link followed), `save_file_path`, `read_save_file`, `write_save_file` (temp file, fsync, rename; with `Backup::Refresh` it first copies the old file to one `.bak`, with `Backup::Keep` it leaves the `.bak`), `back_up_save_file` (the `.bak` step alone, which the session runs once per game session).
- `read_zip_entry`, `GameGeometry::from_core`, `Timing::from_core`: the checks, usable on their own (for example to replay core traces).
- `zip_entries`: the files of a zip archive (name, CRC-32, uncompressed size) from its central directory alone, for the arcade set check. Its own limits keep a hostile file small: a directory of at most 1 MiB and 256 KiB of names.

## Decisions

- **One core per process.** The first `Host::open` sets a process-wide flag that is never cleared. The library is never closed. The application re-executes itself for the next game.
- **One thread.** `Host` is `!Send` and `!Sync`. The callback state is a box that the host lends to a thread-local slot for each call into the core. A callback from another thread gets its default answer.
- **Frames are copied.** The video callback copies the frame, packed, into a buffer of the largest frame size, made at load and at an AV change. `FrameOutput` borrows the host until the next call. A 320x240 XRGB8888 copy is about 300 KiB.
- **Log callback.** The libretro log callback is variadic. `src/log_shim.c` formats the text with `vsnprintf` and passes it to Rust. Snes9x calls its log pointer without a null check, so the host must give one.
- **Core options.** Version 0 (`SET_VARIABLES`). `hold_options` keeps the initial values of the frontend and applies each one when the core declares its key, also inside `retro_load_game`, where FinalBurn Neo declares its options (ADR 0016). A held value that never applied is applied at a later declaration that offers it. `held_option_report` names the held values that never applied. FCEUmm, Snes9x and Genesis Plus GX use the libretro-common options template, which falls back to version 0 when the frontend reports version 0.
- **Zip content.** `miniz_oxide` for deflate and a small central-directory reader in this crate. FCEUmm and Genesis Plus GX report `need_fullpath` and lower it per extension with `SET_CONTENT_INFO_OVERRIDE`, then read the data with `GET_GAME_INFO_EXT`; the host supports both, so all three MVP cores take zip content. A core that needs a path for the zip entry gets `LoadError::ZipNeedsFullPath`. An empty file or zip entry that the core takes as data is `LoadError::EmptyContent`, so a core never gets a data pointer with size 0.
- **Memory of a zip load.** The reader holds the compressed entry (at most about 68 MiB: the 64 MiB content limit plus the deflate bound) and the inflated output (at most 64 MiB) at the same time, plus the central directory (at most 16 MiB). The peak is about 148 MiB in the worst case, during the load only. After the load the host keeps the output, at most 64 MiB, until the game is unloaded.
- **Rotation.** `SET_ROTATION` with 0 to 3 is stored as a `Rotation` (`Host::rotation`); in a frame it is also an event. The frame stays as the core draws it: the renderers turn the picture (ADR 0015).
- **Memory maps, hardware rendering, options v1 and v2** are refused and logged once. An unknown environment call returns false and is logged once.

## Tests

- Unit tests of each module: zip reader (with damaged archives), geometry and timing conversion, save files (backup, stale temp file, symlink at the temp path), options, pads, log queue, frame checks, audio buffer, state.
- `src/ffi/fake_core.rs`: a fake core of Rust `extern "C"` functions, driven through the same entry point table: a wrong API version, hostile frames, audio and environment data, callbacks from another thread, zip content for a core that takes data, content overrides with `GET_GAME_INFO_EXT`, save RAM checks.
- `examples/core_trace.rs`: the tier T4 program. It runs a real core (FCEUmm, Snes9x, Genesis Plus GX) in the QEMU guest and prints a trace of what the host saw. `cargo xtask core-trace` builds and runs it and compares the traces with `tests/traces`. The trace text only grows: an `option` line for each option that the core declares first during the load (after the `load` logs), a `rotation r90` line after the `av` line for a turn asked during the load, and an `event <frame> rotation r90` line for one asked in a frame. The cores of the committed traces cause none of them.
- `tests/test_core.rs`: the built `neptron-testcore` library with the fakes of `neptron-testkit`: frame hashes, audio, events, duplicate frames, input read-back, options, save RAM through a file, the unknown call logged once, and each error path of `Host::open` and `load_game`. The arcade cases: held values for options declared in `retro_load_game`, a 240x320 board drawn on its side with `R90` at load and `R180`, `R270`, a refused value and `R0` in frames, and an aspect of 0 with a turn.

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
