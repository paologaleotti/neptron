# xtask

Build, verification and worktree automation for the NEPTRON workspace. Run it as `cargo xtask <command>`. All automation lives here, not in shell scripts.

## Commands

- `cargo xtask verify --tier <N>`: runs tier N of the verification ladder and every tier below it. Tier 0: `fmt`, `rules`, `cargo deny check licenses bans sources`, `clippy` with warnings denied. Tier 1: `cargo nextest run --workspace --no-run` (timed as build time), `cargo nextest run --workspace --no-tests=warn` and `cargo test --workspace --doc`. Tier 2: `cargo nextest run -p neptron --run-ignored only`, the frontend on the fakes of `neptron-testkit` with the built test core: the end-to-end runs (game list, launch, mode switches, quick menu, quit), the blind recovery, a stalled display and the UI crawler. It needs no display server, no QEMU and no sound device; its budget is 5 minutes, and it takes about 10 s after the tier 1 build. These tests are ignored tests of the app, so tier 1 does not run them. Every step runs, also after a failure. The command always writes `target/verify/report.json`, also when it cannot start the steps, and prints a summary with the time of each step, the build time and the tier budgets. A run over budget is reported, not failed. It exits non-zero when a step fails.
- `cargo xtask check [cargo args]`: `cargo check` under the machine-load rules.
- `cargo xtask test [nextest args]`: `cargo nextest run` under the machine-load rules.
- `cargo xtask clippy [cargo args]`: `cargo clippy --all-targets` under the machine-load rules, so that it checks test code as `verify` does.
- `cargo xtask worktree new <task>`: makes `.worktrees/<task>` in the main checkout, on branch `mvp/<task>` from `main`. Its `target/` is an APFS clone (`cp -cRp`) of the `target/` of the main checkout, so dependencies do not compile again (run rule 12). If the clone fails, the command removes the new worktree and branch again.
- `cargo xtask worktree cache-check [--from <checkout>]`: proves rule 12. It builds the source checkout (default: the main checkout) to warm it, makes a temporary worktree with a cloned `target/`, repeats the test build and clippy there, and counts the artifacts that Cargo compiles again. It fails when a dependency compiles again. It deletes the temporary worktree at the end, also on failure, and removes clones that an interrupted check left.
- `cargo xtask rules`: checks the crate boundaries (see below).
- `cargo xtask os build [--arch aarch64|x86_64] [--jobs N] [-- make targets...]`: builds the development container from `Dockerfile` (tagged `neptron-dev:<hash of the Dockerfile>`), then runs Buildroot in it for the QEMU image of the target: the defconfig, the extra make targets in order, the full build, and a copy of the images to `~/.cache/neptron/buildroot/images/<arch>`. `os/README.md` describes the images.
- `cargo xtask qemu [--arch A] [--images DIR] [--timeout SECS] [--share DIR] [--script FILE | -- command...]`: boots the QEMU image of the target. With a command or a script it runs it in the guest, prints its output and exits with its exit code. Without one it gives the serial console to the terminal. This is the base of tier T3. `os/README.md` describes how a run works.
- `cargo xtask gles-compare [--arch A] [--timeout SECS]`: the tier T3 check of the GLES renderer of `neptron-gfx`. It builds the example `gles_compare` with the `gles` feature for the guest, in the development container, then runs it in the guest with `qemu --share target/gles-compare` (default timeout 600 s) and exits with its exit code. The build uses the Rust toolchain and the C toolchain of the Buildroot output (`host-rustc` in the defconfig), so it needs a built image (`os build`). It mounts the checkout read-only, the Buildroot output volume read-only, and the volume `neptron-cross-<arch>` as the home of the build user, which holds the Cargo registry and the target directory. The build holds the Buildroot build lock, and the guest the QEMU lock. `crates/neptron-gfx/README.md` describes what the example checks.
- `cargo xtask core-trace [--arch A] [--timeout SECS] [--bless] [--dump FRAME[,FRAME...]]`: the tier T4 check of the real cores. It finds each test ROM in `tests/roms` or, when the repository cannot hold it, by its file name in `~/.cache/neptron/test-roms` (`NEPTRON_TEST_ROMS` names another directory), checks it against `tests/roms/SHA256SUMS`, copies them and a zip file of each into `target/core-trace/share`, builds the example `core_trace` of `neptron-libretro` for the guest (as `gles-compare` builds its example), and runs it in the guest once per case with `qemu --share target/core-trace/share` (default timeout 600 s). Each case loads one core from `/usr/lib/libretro` in the image, runs its ROM for 900 frames with a fixed input script, and prints a trace. The command compares each trace with `tests/traces/<case>.trace`, skipping the `arch` line, and checks that the trace of each zip case shows the same frames and audio as its plain case. It exits non-zero when a trace differs or is missing, when a ROM is missing or has a wrong hash (the case is reported as SKIPPED), or when a program fails. The trace of each run is in `target/core-trace/actual`. `--bless` writes the traces of this run to `tests/traces`; read them first (rule 11). `--dump` writes the given frames of each plain case as PNG files to `target/core-trace/frames`. The command downloads no ROM; `tests/roms/README.md` says where a person gets the ones outside the repository. `tests/traces/README.md` describes the trace lines.
- `cargo xtask kms-check [--arch A] [--timeout SECS]`: timing check 7 (tier T3) of `neptron-kms` on `vkms`. It builds the example `kms_check` with the `kms` feature for the guest, the same way as `gles-compare`, but first checks every target of `neptron-kms` (library, tests, examples) for the guest: no host gate compiles the Linux code of the feature, and the Buildroot toolchain has no clippy, so this check is the warnings gate for it. Every guest build (`gles-compare`, `kms-check`, `core-trace`) runs with `RUSTFLAGS=-D warnings`, one setting for all, so the commands share one cache in the cross volume. It then runs three steps in the guest (default timeout 900 s), each in its own process because a process takes the launcher descriptor once: `kms_check --not-drm` with a pipe on descriptor 3 and `NEPTRON_DRM_FD=3`, `kms_check --standard-fd` with `NEPTRON_DRM_FD=1`, then the main run with the device open on descriptor 3 and `NEPTRON_DRM_FD=3`, as the launcher passes it. The first two check that a wrong descriptor is refused before the process owns it. It exits with the sum of the failed checks of the three steps. `docs/spec/kms.md` lists the checks.
- `cargo xtask window-smoke`: builds `neptron-window` with its `sdl3` feature and runs its smoke example. No tier compiles the `sdl3` code, so this command and `cargo xtask clippy -p neptron-window --features sdl3` are the checks for it. The example opens a desktop window, shows a 240p and a super-width mode, injects a key press and checks it. It needs a display and the SDL3 library, so no tier runs it.
- `cargo xtask monitors <monitor.cpp> [dir]`: converts the Switchres preset table in a local copy of `monitor.cpp` into one TOML file per preset in `data/monitors` (or `dir`). Each value is converted from its decimal text with integer maths. `data/monitors/README.md` names the source commit and the units.

- `cargo xtask bindgen`: generates the libretro bindings of `neptron-libretro-sys` from `crates/neptron-libretro-sys/vendor/libretro.h`. It writes `src/bindings.rs` (every `retro_*` type and `RETRO_*` constant, no functions) and `src/core_api_decls.rs` (the core entry points, no types), then formats both with `rustfmt`. The generated files are committed, so no build needs libclang. The command needs the `bindgen` command-line tool (on macOS: `brew install bindgen`; the committed files come from bindgen 0.73.2). `NEPTRON_BINDGEN` names another executable. The clang target is always `x86_64-unknown-linux-gnu` with `-ffreestanding`, so the output does not depend on the host. The layout checks that bindgen writes are valid for every 64-bit target with the LP64 data model.
- `cargo xtask font`: generates the glyph table `crates/neptron-ui/src/font/spleen_6x12.rs` from the vendored BDF file `crates/neptron-ui/assets/fonts/spleen/spleen-6x12.bdf`, for printable ASCII and Latin-1. The parser reads the BDF text format by hand. A test fails when the committed table differs from the output of the generator.
- `cargo xtask shot <scene>`: renders one scene of the `neptron-ui` registry to `target/ui/shot/<scene>.png` and a 4x enlarged copy `<scene>@4x.png`.
- `cargo xtask gallery`: renders every scene onto one contact sheet, `target/ui/gallery.png`, each tile under its name.
- `cargo xtask golden [--bless] [scene...]`: compares each scene (default: all) with its golden image in `crates/neptron-ui/tests/golden/<scene>.ppm`, byte for byte. For a difference it prints the pixel count, the bounding box and the first differing pixels, and writes `<scene>.expected.png`, `<scene>.actual.png` and `<scene>.diff.png` (differing pixels in red over a dark copy of the golden) to `target/ui/golden`. A golden without a scene is reported as stale. It exits non-zero on any failure. `--bless` writes the golden of each scene that differs or has none; nothing blesses automatically. The format is binary PPM, described in `crates/neptron-ui/README.md`. The test `crates/neptron-ui/tests/golden.rs` runs the same comparison in tier 1.

Both worktree commands refuse to run when Cargo builds the checkout into a directory other than `<checkout>/target` (for example with `CARGO_TARGET_DIR`), because the clone would then not be the build cache.

## Rule checker

`cargo xtask rules` reads `cargo metadata` and checks the rules below. The rule tables are at the top of `src/rules.rs`. A change to them is a change to a dependency rule (run rule 9).

**Unsafe allow-list.** `unsafe` is allowed in every `-sys` crate and in `neptron-libretro`, `neptron-kms`, `neptron-gfx`, `neptron-audio`, `neptron-input` and `neptron-testcore`. Every other crate carries `#![forbid(unsafe_code)]` among the inner attributes at the head of every target root: library, binaries, tests, benches, examples and build script. The checker parses that head (comments, then inner attributes). An attribute inside a module, a comment or a string does not count. A workspace lint cannot do this job, because a crate cannot change one workspace lint.

**Dependency rules.** The layout rules become these checks:

| Rule in the layout | Check |
| --- | --- |
| `neptron-safety` depends only on the shared types | Normal dependencies of `neptron-safety`: `neptron-types` only |
| `neptron-modeline` depends only on the shared types, and wraps `neptron-switchres-sys` | Normal dependencies of `neptron-modeline`: `neptron-types` and `neptron-switchres-sys` only. Build and dev dependencies are free |
| `neptron-switchres-sys` holds the C++ build and the raw calls, nothing else | It depends on no workspace crate, and only `neptron-modeline` depends on it |
| Shared types sit at the bottom | `neptron-types` depends on no workspace crate |
| `neptron-ui` depends on no other NEPTRON crate, and only the app depends on it | `neptron-ui` depends on no workspace crate. Only `neptron` and `xtask` depend on it; `xtask` holds `shot`, `gallery` and `golden` |
| `neptron-video` is the single crate that joins solver, safety and display | Among the library crates, only `neptron-video` depends on `neptron-modeline`. `neptron-video` never depends on `neptron-kms` |
| `kms-spike` depends on `drm`, `neptron-types` and `neptron-safety` only | Normal dependencies of `kms-spike`: those three only |
| `neptron-libretro-sys` is raw bindings | It depends on no workspace crate |
| No async runtime in the frontend | No crate except `xtask` pulls in `tokio`, `async-std`, `smol` or `async-executor` |

Every crate also has a `README.md`.

**Headless build.** Hardware backends are Cargo features, off by default:

| Crate | Features |
| --- | --- |
| `neptron-kms` | `kms` |
| `neptron-gfx` | `gles` |
| `neptron-window` | `sdl3` |
| `neptron-audio` | `sdl3` |
| `neptron-input` | `sdl3`, `evdev` |
| `neptron` (app) | `kms`, `window` |
| `kms-spike` | `kms` |
| `neptron-ipc` | `control` (the TCP listener, not hardware) |

The checker reads the features that Cargo resolves for the default build. It fails when a workspace crate gets `kms`, `gles`, `sdl3`, `evdev` or `window` in any way (a `default` list, a chain of features, or `features = [...]` on a dependency entry), and when the default build contains a hardware crate (SDL3, EGL, GL, DRM, GBM, evdev, librashader).

## Machine-load rules (run rule 13)

On macOS, every Cargo command that xtask starts runs under `taskpolicy -c utility`. A build holds one of the build slots. The lock files are in `<git common dir>/neptron-build-slots/`, so all worktrees use the same slots. A build that finds no free slot waits.

| Variable | Default on macOS | Default on other hosts |
| --- | --- | --- |
| `NEPTRON_BUILD_SLOTS` | 2 | no lock |
| `NEPTRON_BUILD_JOBS` (sets `CARGO_BUILD_JOBS`) | 5 | Cargo default |

Cargo compiles `xtask` itself before xtask starts, so that one small compile runs outside `taskpolicy` and the slots.

The OS commands have their own limits, machine-wide through lock files in the cache directory (`NEPTRON_CACHE_DIR`, default `~/.cache/neptron`):

- `os build`: one Buildroot build at a time, 4 jobs by default (`--jobs`, at most 8), a container with as many CPUs as jobs and 6 GB of memory.
- `qemu`: one guest at a time, with 2 GB of memory and 2 CPUs, under `taskpolicy -c utility` on macOS. The guest stops its command at `--timeout` (default 120 s) and powers off; xtask stops QEMU 30 s after that, and at once on SIGTERM, SIGHUP or SIGINT.

## Dependencies

- `neptron-ui`: the scene registry, the CPU canvas and the golden format for `shot`, `gallery` and `golden`. The PNG files are written by a small encoder in `src/ui/png.rs` (stored deflate blocks), so no image crate is needed.
- `signal-hook`: sets a flag on SIGTERM, SIGHUP and SIGINT, so that `qemu` stops its QEMU child before it exits. It needs `unsafe` code to install a handler, which xtask forbids in its own code. It is small: it pulls in `libc` (already in the build) and `signal-hook-registry`.
- `serde_json`: reads `cargo metadata` and Cargo build messages, and writes the report.
- `thiserror`: each module owns its error enum, as the code standards ask.

Arguments are parsed by hand, and the slot lock uses `std::fs::File::try_lock`, to keep the compile time low.

## Must never depend on

- Any NEPTRON crate except `neptron-ui` (for `shot`, `gallery` and `golden`).
- Heavy crates: every dependency must justify its compile time.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
