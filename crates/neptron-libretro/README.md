# neptron-libretro

Safe libretro core host: load a core, environment callbacks, save RAM, core options. `docs/spec/libretro-host.md` is the specification.

The host produces data and decides nothing for the session. It does not own the display, the audio device, the input devices or the clock.

## Must never depend on

- `neptron-ui`, `neptron-kms`, `neptron-video`

Dependencies: `neptron-libretro-sys`, `neptron-types`, `neptron-backend` (the plain types of a hardware context), `libloading`, `miniz_oxide`, `thiserror`; `cc` at build time for the log shim. No features.

## Public API

- `Host::open(core, HostConfig)`: claims the process, loads the library, checks the 25 entry points and the API version, gives the callbacks, calls `retro_init`.
- `system_info()`, `options()`, `set_option(key, value)`, `hold_options(values)`, `held_option_report()`, `input_descriptors()` (`SET_INPUT_DESCRIPTORS`, at most 256, for the remapping screen).
- `load_game(Option<ContentRequest>) -> AvInfo`: a plain file, one entry of a `.zip` file, or no content.
- `set_pad(port, Pad)`, then `run_frame() -> FrameOutput<'_>`: the video (`Frame`, `Hardware` with the size of a frame in the hardware framebuffer, `Duplicate` with the last frame, `Missing` or `Rejected`), the audio, the events (`Geometry`, `AvInfo`, `PixelFormat`, `Rotation`, `Shutdown`, `Message`, `Refused`), the log records and the drop counts.
- `reset()`, `av_info()`, `pixel_format()`, `rotation()`, `logs()`, `error_logs()` (the first `ERROR_LOG_CAPACITY` error records of the last call, kept also when the `logs()` queue was full), `dropped_logs()`.
- `copy_save_ram(&mut Vec<u8>)`, `load_save_ram(&[u8])`.
- Hardware rendering (GL and GLES): `set_hw_offer(ContextOffer)` before the load, `hw_request() -> Option<ContextRequest>` and `hw_refusal() -> Option<HwRefusal>` after it, `set_hw_target(framebuffer, lookup)`, `context_reset()` and `context_destroy()` (spec, "Hardware rendering").
- File functions for the session, off the frame thread: `save_folder` (made with no link followed), `save_file_path`, `read_save_file`, `write_save_file` (temp file, fsync, rename; `Backup::Refresh` first copies the old file to one `.bak`, `Backup::Keep` leaves the `.bak`), `back_up_save_file` (the `.bak` step alone, once per game session).
- `read_zip_entry`, `GameGeometry::from_core`, `Timing::from_core`: the checks, usable alone (for example to replay core traces).
- `zip_entries`: name, CRC-32 and size of each file of a zip archive, from its central directory alone, for the arcade set check. A directory of at most 1 MiB and 256 KiB of names keeps a hostile file small.

## Rules for callers

- **One core per process.** The first `Host::open` sets a process-wide flag that is never cleared, and the library is never closed. The application runs itself again for the next game (spec, "One core per process").
- **One thread.** `Host` is `!Send` and `!Sync`. A callback from another thread gets its default answer (spec, "Global state and the thread rule").
- **Frames are copied** into a buffer of the largest frame size, made at load and at an AV change. `FrameOutput` borrows the host until the next call.
- **Core options** are version 0 (`SET_VARIABLES`). `hold_options` keeps the initial values of the frontend and applies each one when the core declares its key, also inside `retro_load_game`, where FinalBurn Neo declares its options (ADR 0016). `held_option_report` names the held values that never applied.
- **Zip content**: a core that needs a path for the entry gets `LoadError::ZipNeedsFullPath`. An empty file or entry that the core takes as data is `LoadError::EmptyContent`, so a core never gets a data pointer with size 0. A zip load holds the compressed entry (about 68 MiB at most), the inflated output (64 MiB) and the central directory (16 MiB): a peak of about 148 MiB, during the load only. After the load the host keeps the output until the game is unloaded (OP-041).
- **Rotation**: `SET_ROTATION` is stored as a `Rotation`, and in a frame it is also an event. The frame stays as the core draws it; the renderers turn it (ADR 0015).
- Memory maps, Vulkan, the hardware render interfaces, shared contexts, options v1 and v2, and unknown environment calls are refused and logged once.
- **Hardware rendering**: `SET_HW_RENDER` succeeds only inside `retro_load_game` and only for a context that the offer holds. The two callbacks of the frontend read process globals, so a core can call them on any thread. The session calls `context_destroy` before it drops the host, with the core context current.
- `unsafe` is only in `src/ffi.rs` and its child modules; the crate root denies it elsewhere. Every `unsafe` block has a `// SAFETY:` comment. The variadic log callback goes through `src/log_shim.c` (`vsnprintf`).

## Tests

The host allows one core per process, so each test needs its own process. `cargo xtask test -p neptron-libretro` runs them with `cargo nextest`, which gives that. Plain `cargo test` fails the second test of each binary.

- Unit tests of each module, also with damaged zip archives and save file faults (stale temp file, symlink at the temp path).
- `src/ffi/fake_core/hw.rs`: hardware rendering through the fake core: each answer to `SET_HW_RENDER` (offered, not offered, Vulkan, outside the load, the last call wins), `GET_PREFERRED_HW_RENDER` per offer, the refused sibling calls, hardware frames, the context callbacks, and the two frontend callbacks on another thread.
- `src/ffi/fake_core.rs`: a fake core of Rust `extern "C"` functions behind the same entry point table: a wrong API version, hostile frames, audio and environment data, callbacks from another thread, zip content, content overrides with `GET_GAME_INFO_EXT`, save RAM checks.
- `tests/test_core.rs`: the built `neptron-testcore` with the fakes of `neptron-testkit` (dev dependencies also `neptron-backend`): frame hashes, audio, events, input read-back, options, save RAM through a file, each error path of `Host::open` and `load_game`, and the arcade cases (options declared in `retro_load_game`, turns at load and in frames, an aspect of 0 with a turn). It sets the test core's script variable with one `unsafe` call.
- `examples/core_trace.rs`: the tier T4 program. It runs a real core (FCEUmm, Snes9x, Genesis Plus GX) in the QEMU guest and prints what the host saw. `cargo xtask core-trace` compares the traces with `tests/traces`. The trace text only grows: an `option` line for each option first declared during the load, a `rotation r90` line after the `av` line, and an `event <frame> rotation r90` line for a turn in a frame. The cores of the committed traces cause none of them.
