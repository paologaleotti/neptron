## Monorepo layout

One Cargo workspace holds libraries in `crates/`, binaries in `apps/`, the Buildroot tree in `os/` and all declarative data in `data/`.

```text
neptron/
├── Cargo.toml                  workspace, shared lints, pinned toolchain
├── crates/
│   ├── neptron-types           shared data: modeline, monitor profile, GPU capabilities, source geometry
│   ├── neptron-backend         display, audio, input and clock traits
│   ├── neptron-modeline        Switchres wrapper, profiles, ODREN policy (no I/O)
│   ├── neptron-safety          ValidatedMode, range checks, hard rails
│   ├── neptron-kms             DRM/KMS: connectors, atomic commits, flip events, buffers
│   ├── neptron-window          desktop window backend for development
│   ├── neptron-gfx             EGL/GLES renderer; CPU reference renderer for tests; rotation, shaders
│   ├── neptron-shader          slang presets via librashader (M4)
│   ├── neptron-video           ODREN engine: core geometry -> policy -> safety -> display
│   ├── neptron-libretro-sys    raw FFI bindings to libretro.h
│   ├── neptron-libretro        safe core host: load, env callbacks, HW render, states
│   ├── neptron-testcore        scripted libretro core for tests
│   ├── neptron-audio           SDL3 audio out, rubato resampler, rate control
│   ├── neptron-input           SDL3 gamepads, evdev keyboards and pointers, RetroPad mapping
│   ├── neptron-settings        typed schema, 3-level resolution, TOML persistence
│   ├── neptron-library         ROM scan, index, core and system metadata, gamelist import
│   ├── neptron-ui              sealed: snapshot in, scene + intents out
│   ├── neptron-session         game lifecycle: launch, pause, quick menu, exit
│   ├── neptron-platform        GPU and board detection, capability table
│   ├── neptron-ipc             control + test protocol: screenshot, UI tree, input, clock
│   └── neptron-testkit         fake display, audio, input and clock; goldens
├── apps/
│   ├── neptron                 the frontend binary; only wiring, no logic
│   ├── neptron-launch          holds the DRM descriptor, starts and respawns the frontend
│   ├── neptron-update          thin glue around RAUC (M4)
│   └── neptronctl              CLI: print a modeline, test a mode, self-test, diagnostics
├── tools/kms-spike             standalone hardware spike (WP 1.1)
├── os/                         Buildroot br2-external
│   ├── configs/                neptron_x86_64_defconfig, neptron_rpi_arm64_defconfig, QEMU configs
│   ├── board/                  per target: kernel fragments, overlays, boot files
│   ├── package/                neptron apps, libretro cores, out-of-tree kernel modules
│   └── patches/linux/          15 kHz set, Pi DPI patches
├── data/
│   ├── monitors/               monitor profiles
│   ├── systems/  cores/        system definitions, per-core metadata
│   ├── hardware/matrix.toml    support matrix + GPU capabilities
│   └── skins/default/
├── tests/                      goldens, free test ROMs, core traces
├── xtask/                      cargo xtask: verify, shot, gallery, golden, image, qemu, deploy, bench
└── docs/                       adr/, spec/, taskboard.toml, hardware-checklist.md
```

**Dependency rules**

- `neptron-modeline` and `neptron-safety` depend only on the shared types crate. They build and test on any laptop, with no hardware.
- `neptron-kms` accepts `ValidatedMode` only. `neptron-video` is the single crate that joins solver, safety and KMS.
- `neptron-ui` depends on no other NEPTRON crate, and only the app binary depends on it: state snapshot in, scene and intents out. `neptron-session` mediates between UI, core host and video.
- `unsafe` is allowed only in `-sys`, `libretro`, `kms`, `gfx`, `audio` and `input`. Every other crate carries `#![forbid(unsafe_code)]`.
- No async runtime in the frontend. One frame loop on the main thread, plus dedicated audio and I/O threads talking over channels.
- Display, audio and input sit behind traits. A windowed desktop backend lets UI work happen on a laptop; `neptron-testkit` fakes let CI run a full launch-play-exit session with no GPU.

**Build profiles**

The development profile is tuned for compile and verify time, the release profile for speed on the device.

| Profile | Settings | Why |
| --- | --- | --- |
| `dev` (and tests) | No optimisation for any crate, dependencies included (`opt-level = 0`); line-table debug info; incremental builds. Compile time has priority over run-time speed | The edit-to-verdict loop stays short, and heavy dependencies (resampler, image code) still run fast in tests |
| `release` | `opt-level = 3`, fat LTO, one codegen unit, `panic = "abort"`, symbols stripped | Maximum speed and a small binary; abort is also the safe behaviour across the libretro FFI |
| `profiling` | `release` plus debug info | Benchmarks and flame graphs |

- Hardware backends are Cargo features. The headless build that T0 to T2 use compiles no SDL3, EGL or KMS code and links no C library except Switchres.
- Generated code is committed, not built: the libretro bindings come from `xtask bindgen`, so no build needs libclang.
- Each crate has one integration-test binary, to cut link time. Tests run with `cargo nextest`.
- The fastest available linker is set in `.cargo/config.toml` (lld or mold on Linux). CI caches the Cargo target directory and the Buildroot download and compiler caches.
- Release builds target `x86-64-v2` on PCs and Cortex-A53 on Pi, so one image runs on every supported machine.
- A new dependency must justify its compile time in the pull request. `xtask verify` prints the build time, and the tier budgets in the Implementation plan are limits, not hopes.

**Working with Claude Code**

- A root `CLAUDE.md` states these rules, the milestone in progress and the commands (`cargo xtask check`, `cargo xtask qemu`, `cargo xtask deploy <host>`).
- Each crate has a short `README.md` with its purpose, public API sketch and what it must never depend on.
- Architecture decisions are recorded as numbered files in `docs/adr/`, starting with the ones in this document.
