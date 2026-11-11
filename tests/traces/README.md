# Core traces

Each `.trace` file is the golden trace of one case of the tier T4 check, `cargo xtask core-trace`. A case is one core, one 240p Test Suite ROM from `tests/roms`, and one input script. The case table is `CASES` in `xtask/src/os/trace.rs`. The program that prints the trace is `crates/neptron-libretro/examples/core_trace.rs`.

A trace is a golden (run rule 11). `cargo xtask core-trace --bless` writes new traces. Read every changed line before you commit them, and look at the frames with `--dump`.

## Cases

| Case | Core | ROM | Content |
| --- | --- | --- | --- |
| `fceumm` | FCEUmm | `nes/240pee.nes` | the plain file |
| `fceumm-zip` | FCEUmm | `nes/240pee.nes` | a zip file with one stored entry |
| `snes9x` | Snes9x | `snes/240pSuite.sfc` | the plain file |
| `snes9x-zip` | Snes9x | `snes/240pSuite.sfc` | a zip file |
| `genesis_plus_gx` | Genesis Plus GX | `megadrive/240pSuite-1.32.bin` | the plain file |
| `genesis_plus_gx-zip` | Genesis Plus GX | `megadrive/240pSuite-1.32.bin` | a zip file |

Each case runs 900 frames. The input script on port 0 leaves the start screen, goes through the menus to the audio sync test, and starts it. The frames from about 450 on show the test: a dot that falls, and a beep when it lands. A zip case runs the same script as its plain case. The check makes sure that its frame, audio, video, event and save RAM lines are equal to those of the plain case.

## The SNES and Mega Drive ROMs

The SNES and Mega Drive ROMs are not in the repository (`tests/roms/README.md` says why and where to put them). Their traces are: a trace holds hashes and text, not the art of the ROM.

## Lines

The program writes the lines in this order. Text from the core or the host is escaped, so a record is always one line.

| Line | Meaning |
| --- | --- |
| `format 1` | The version of the trace text |
| `arch <arch>` | The guest architecture of the run. The comparison skips this line, because the traces must be equal on every architecture |
| `core`, `content` | The file names of the core library and of the content |
| `run frames=<n> hash_every=<n>` | The frame count, and the interval of the hashes |
| `press <frame> <buttons> frames=<n>` | The input script: buttons held from a frame, for a number of frames |
| `system ...` | `retro_get_system_info`: name, version, `need_fullpath`, `block_extract` |
| `extensions` | The extensions that the core declares |
| `log open|load|<frame> <level> <source> <text>` | A log record of the core (`Core`) or of the host (`Host`), in `Host::open`, in `load_game` or in a frame. The host writes one record for each environment call that it does not know (`unknown environment call 0x...`, level `Warn`) or refuses (level `Info`), once per call number |
| `option <key> = "<value>" [<values>] <description>` | A core option that the core declared, with its current value (the default) |
| `av ...` | The geometry and timing that `load_game` returned: base and maximum size, aspect in thousandths, frame rate and sample rate in millihertz |
| `pixel_format` | The pixel format after the load |
| `options_after_load count=<n>` | The number of options after the load |
| `event <frame> ...` | An event of the host: geometry, AV info, pixel format, message, shutdown, refusal |
| `dropped <frame> ...` | Audio frames, events or log records that did not fit the buffers of the host |
| `hash <frame> <w>x<h> <format> <fnv>` | The frames since the last hash: FNV-1a, 64 bits, over the frame hash of each frame in turn (a frame with no picture adds 0). The frame hash is FNV-1a of the visible bytes, row by row, as the host gives them. Size and format are those of the last frame. A fault in any frame of the window changes the line |
| `audio <first>-<last> stereo_frames=<n> nonzero_samples=<n> <fnv>` | The audio of the frames since the last hash: the count of stereo frames, the count of samples that are not 0, and FNV-1a of the samples as little-endian `i16` |
| `video <first>-<last> <kind>` | A run of frames with the same video result: `frame <w>x<h>`, `duplicate`, `missing` or `rejected <fault>` |
| `save_ram size=<n> <fnv>` | The save RAM after the last frame. With a size above 0, more lines follow: the round trip through a save file, and a pattern loaded and read back |
| `end` | The host was dropped: `retro_unload_game` and `retro_deinit` ran |
| `exit <code>` | The exit code of the program, from the guest script |

## Environment call numbers

The host logs the number of each call that it does not serve. These are the numbers in the traces, with their names from `libretro.h`. `0x10000` is `RETRO_ENVIRONMENT_EXPERIMENTAL`.

| Number | Name |
| --- | --- |
| `0x8` | `SET_PERFORMANCE_LEVEL` |
| `0xd` | `SET_DISK_CONTROL_INTERFACE` |
| `0x22` | `SET_SUBSYSTEM_INFO` |
| `0x2c` | `SET_SERIALIZATION_QUIRKS` |
| `0x37` | `SET_CORE_OPTIONS_DISPLAY` (refused: only version 0 of the options API) |
| `0x3b` | `GET_MESSAGE_INTERFACE_VERSION` |
| `0x3e` | `SET_AUDIO_BUFFER_STATUS_CALLBACK` |
| `0x3f` | `SET_MINIMUM_AUDIO_LATENCY` |
| `0x45` | `SET_CORE_OPTIONS_UPDATE_DISPLAY_CALLBACK` (refused) |
| `0x46` | `SET_VARIABLE` |
| `0x10024` | `SET_MEMORY_MAPS` (refused) |
| `0x10028` | `GET_CURRENT_SOFTWARE_FRAMEBUFFER` |
| `0x1002a` | `SET_SUPPORT_ACHIEVEMENTS` |
| `0x1002d` | `GET_VFS_INTERFACE` |
| `0x1002e` | `GET_LED_INTERFACE` |
| `0x10051` | `GET_TARGET_SAMPLE_RATE` |

## Findings for later work

- Genesis Plus GX reports 256x192 at load and sends 4 frames at 256x192. At frame 4 it calls `SET_GEOMETRY` with 320x224. The first mode of a Mega Drive game must not come from the AV info of the load alone.
- Genesis Plus GX calls `SET_SERIALIZATION_QUIRKS` in `retro_init`, which the host does not serve. At the pinned commit it sets `PLATFORM_DEPENDENT` only: a state cannot be loaded on another architecture. When save states come, the host must serve this call and honour each quirk: `MUST_INITIALIZE` (`retro_serialize_size`, `retro_serialize` and `retro_unserialize` fail until the core has run for a time), `CORE_VARIABLE_SIZE` (the state size can change during a session: ask for it before each save), and `PLATFORM_DEPENDENT` (keep the architecture with the state).
- Snes9x writes `Map_LoROMMap` to standard output, past the log interface.

## Architectures

The traces were recorded on aarch64 (QEMU `virt` with the hypervisor of an Apple Silicon Mac). The comparison skips the `arch` line, so an x86\_64 run must give the same frames and audio. The three cores emulate their systems with integer code, the frames are copied byte for byte, and both targets are little-endian, so the hashes are expected to be equal. The x86\_64 image has not been built yet, so this is not verified. If an x86\_64 run differs, find the cause (for example floating-point code in a core, built with fused multiply-add on aarch64) before you change a trace.
