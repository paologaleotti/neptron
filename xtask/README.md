# xtask

Build, verification and worktree automation for the NEPTRON workspace. Run it as `cargo xtask <command>`. All automation lives here, not in shell scripts.

## Commands

- `cargo xtask check [cargo args]`: `cargo check` under the machine-load rules.
- `cargo xtask test [nextest args]`: `cargo nextest run` under the machine-load rules.
- `cargo xtask clippy [cargo args]`: `cargo clippy` under the machine-load rules.
- `cargo xtask rules`: checks the crate boundaries (see below).

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

## Dependencies

- `serde_json`: reads `cargo metadata` and Cargo build messages, and writes the report.
- `thiserror`: each module owns its error enum, as the code standards ask.

Arguments are parsed by hand, and the slot lock uses `std::fs::File::try_lock`, to keep the compile time low.

## Must never depend on

- Any NEPTRON crate except `neptron-ui` (for `shot`, `gallery` and `golden`).
- Heavy crates: every dependency must justify its compile time.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
