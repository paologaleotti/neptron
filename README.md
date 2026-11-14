# NEPTRON

NEPTRON is an open-source, Buildroot-based retro-gaming OS. Its first job is to drive a CRT correctly: native resolution and refresh for each game, on x86 PCs and Raspberry Pi, with every setting reachable from a controller. On flat panels it keeps the panel's native mode and fits games with integer scaling. It is one Rust monorepo. A single Rust process hosts libretro cores, owns the display through DRM/KMS and draws its own UI. No modeline reaches the hardware unless it passes validation against the active monitor profile.

## Status

Early development. There is no release yet. The current milestone is the MVP: one playable path on an x86_64 PC. A 64-bit Raspberry Pi 3 image with composite video also exists. The PC development image ran on an i3-6100T on 2026-11-13: a game played, with audio and the SMB share. A Pi 3 B on a composite TV showed NTSC 240p on 2026-11-14, and NES, SNES and Mega Drive games held 60.054 frames per second.

## Build and verify

You need `rustup`, `cargo-nextest` and `cargo-deny`. The toolchain is pinned in `rust-toolchain.toml`.

```sh
cargo xtask verify --tier 1
```

This runs formatting, clippy, the dependency rules, the licence check and all tests. It writes `target/verify/report.json`. Run `cargo xtask help` for the other commands.

## Continuous integration

GitHub Actions runs the same `cargo xtask verify` on every pull request, on Linux and macOS. It builds three OS images: the PC image and the aarch64 test image, which it boots in QEMU for the tier 3 checks, and the Raspberry Pi image, whose SD card layout it checks. It builds them for a pull request that changes the OS or the device crates, on `main`, and every night. Make the check `ci-ok` required. No image, kernel or cache that CI uploads holds the 15 kHz kernel patch set, because its licence is not clear (OP-070). [docs/ci.md](docs/ci.md) describes the jobs, the caches, the times and the artifacts.

## Repository layout

- `crates/`: libraries (types, safety validator, modeline solver, backends, UI, session).
- `apps/`: binaries (`neptron`, `neptron-launch`, `neptronctl`, `neptron-update`).
- `tools/`: standalone hardware tools.
- `xtask/`: build, verification and worktree automation.
- `docs/`: definition, implementation plan, ADRs and taskboard.

## Documents

Start at [docs/README.md](docs/README.md).

## Licence

NEPTRON source is licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Two directories are the exception: `data/monitors` (presets derived from Switchres) and `vendor/switchres` are licensed GPL-2.0-or-later, and each holds its licence text. The `neptron` binary and the OS images link Switchres, so they are distributed under GPL-3.0-or-later.
