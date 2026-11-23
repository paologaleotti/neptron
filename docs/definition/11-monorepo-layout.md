## Monorepo layout

One Cargo workspace holds libraries in `crates/`, binaries in `apps/`, the Buildroot tree in `os/` and all declarative data in `data/`.

```text
neptron/
├── Cargo.toml                  workspace, shared lints, build profiles (toolchain pinned in rust-toolchain.toml)
├── crates/
│   ├── neptron-types           shared data: modeline, monitor profile, GPU capabilities, source geometry
│   ├── neptron-backend         display, audio, input and clock traits
│   ├── neptron-modeline        safe Switchres wrapper, profiles, ODREN policy (no I/O)
│   ├── neptron-switchres-sys   Switchres C++ build and raw FFI calls, nothing else
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
│   └── neptronctl              CLI of the control API, and the video self-test
├── tools/kms-spike             standalone hardware spike (WP 1.1)
├── os/                         Buildroot br2-external
│   ├── configs/                neptron_pc_x86_64_defconfig, neptron_rpi_arm64_defconfig, neptron_qemu_aarch64_defconfig, dev fragments
│   ├── board/                  per target: kernel fragments, overlays, boot files, kernel patch hashes
│   ├── package/                neptron apps, libretro cores, the gamepad database
│   ├── patches/linux/          hashes of the pinned 15 kHz kernel patch set
│   └── tests/                  goldens of the boot check
├── data/
│   ├── monitors/               monitor profiles
│   ├── systems/  cores/        system definitions, per-core metadata
│   ├── hardware/matrix.toml    support matrix + GPU capabilities
│   ├── input/                  community gamepad database
│   └── skins/default/
├── tests/                      free test ROMs, core traces
├── xtask/                      cargo xtask: verify, rules, shot, gallery, golden, os (build, checks, push), qemu, kms-check, core-trace
└── docs/                       definition/, plan/, flows/, adr/, spec/, taskboard.toml, open-points.md, hardware-checklist.md
```

**Dependency rules**

`cargo xtask rules` (`xtask/src/rules.rs`) checks these rules.

- `neptron-types` depends on no other workspace crate.
- `neptron-modeline` and `neptron-safety` depend only on the shared types crate (normal dependencies). The one exception is `neptron-switchres-sys`: only `neptron-modeline` depends on it, and it depends on no workspace crate. They build and test on any laptop, with no hardware.
- `neptron-kms` accepts `ValidatedMode` only. `neptron-video` is the single crate that joins solver, safety and display: it is the only library crate that depends on `neptron-modeline`, and it uses the `Display` trait, never `neptron-kms`.
- `neptron-ui` depends on no other NEPTRON crate, and only the app binary (and the screenshot tooling in `xtask`) depends on it: state snapshot in, scene and intents out. `neptron-session` mediates between UI, core host and video.
- `neptron-libretro-sys` depends on no workspace crate. `tools/kms-spike` depends only on `drm`, `neptron-types` and `neptron-safety`.
- `unsafe` is allowed only in `-sys`, `libretro`, `kms`, `gfx`, `audio`, `input` and `testcore`. Every other crate carries `#![forbid(unsafe_code)]`. `unsafe` is used only where no safe way exists, in the smallest possible block, with a `// SAFETY:` comment that states the invariant. `neptron-testcore` uses it only for the exported libretro functions, and it is never in the image.
- No async runtime in the frontend. One frame loop on the main thread, plus dedicated audio and I/O threads talking over channels. Only `xtask` may pull in an async runtime.
- Display, audio and input sit behind traits. A windowed desktop backend lets UI work happen on a laptop; `neptron-testkit` fakes let CI run a full launch-play-exit session with no GPU.
- Every workspace crate has a `README.md`.

**Build profiles**

The development profile is tuned for compile and verify time, the release profile for speed on the device.

| Profile | Settings | Why |
| --- | --- | --- |
| `dev` (and tests) | No optimisation for any crate, dependencies included (`opt-level = 0`); line-table debug info; incremental builds | Compile time has priority over run-time speed: the edit-to-verdict loop stays short |
| `release` | `opt-level = 3`, fat LTO, one codegen unit, `panic = "abort"`, symbols stripped | Maximum speed and a small binary; abort is also the safe behaviour across the libretro FFI |
| `profiling` | `release` plus debug info | Benchmarks and flame graphs |

- Hardware backends are Cargo features (`kms`, `gles`, `sdl3`, `evdev`, `window`). The headless build that T0 to T2 use turns none of them on, contains no SDL3, EGL, GL, KMS, evdev or librashader crate, and links no C library except Switchres.
- Generated code is committed, not built: the libretro bindings come from `xtask bindgen`, so no build needs libclang.
- Each crate has one integration-test binary, to cut link time. Tests run with `cargo nextest`.
- `.cargo/config.toml` sets no linker: Linux links with rust-lld by default since Rust 1.90, and the macOS default linker is already fast. CI caches the Cargo target directory and the Buildroot download and compiler caches.
- Release builds target `x86-64-v2` *(after 0.1)* on PCs and Cortex-A53 on Pi, so one image runs on every supported machine.
- A new dependency must justify its compile time in its commit. `xtask verify` prints the build time, and the tier budgets in the Implementation plan are limits, not hopes.

**Working with Claude Code**

- A root `CLAUDE.md` states these rules, the milestone in progress and the commands (`cargo xtask verify`, `cargo xtask check`, `cargo xtask test`).
- Each crate has a short `README.md` with its purpose, public API sketch and what it must never depend on.
- Architecture decisions are numbered files in `docs/adr/`.
