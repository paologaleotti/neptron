# NEPTRON

NEPTRON is an open-source, Buildroot-based retro-gaming OS. Its first job is to drive a CRT correctly: native resolution and refresh for each game, on x86 PCs and Raspberry Pi, with every setting reachable from a controller. On flat panels it keeps the panel's native mode and fits games with integer scaling. It is one Rust monorepo. A single Rust process hosts libretro cores, owns the display through DRM/KMS and draws its own UI. No modeline that NEPTRON sets reaches the hardware unless it passes validation against the active monitor profile. Before NEPTRON starts, the computer itself drives the output: read "Safety" before you connect a 15 kHz monitor.

## Status

Early development. Release 0.1.0 is the first version, an early milestone: its images for a 64-bit PC and for the Raspberry Pi 3 are on the releases page, with the notes in `docs/release-notes/0.1.0.md`. Versions 0.x work and anyone can use them, but they are not complete; the first release for the public is 1.0. The PC development image ran on an i3-6100T on 2026-11-13: a game played, with audio and the SMB share. A Pi 3 B on a composite TV showed NTSC 240p on 2026-11-14, and NES, SNES and Mega Drive games held 60.054 frames per second.

## Hardware

Release 0.1 is tested on Intel graphics (PC) and on the Raspberry Pi 3. AMD and other graphics cards are not tested and can show no picture: the PC image holds no firmware file, and the AMD driver needs its firmware. `docs/hardware-checklist.md` lists the checks that still need hardware.

## Safety

NEPTRON checks every video mode that it sets against the limits of the monitor profile (`docs/definition/05-crt-safety-model.md`). It cannot control what the computer shows before NEPTRON starts:

- **PC.** From power-on until the NEPTRON menu appears, the firmware of the PC (BIOS or UEFI) and the boot loader drive the output in their own mode, at 31 kHz or more. That lasts many seconds; the time on a real PC is not measured. NEPTRON does not blank the output in that time. A monitor that takes 15 kHz only (an arcade monitor, or a TV on a VGA-to-SCART cable) gets a signal out of its range for all of that time. Switch such a monitor on, or connect it, only after the NEPTRON menu appears, or connect it through an adapter that blocks a signal above 15 kHz (a sync-protecting adapter), unless you know that the output of your PC at power-on is safe for it.
- **Raspberry Pi.** The composite output carries a TV standard (NTSC or PAL) from the start, which a TV takes.
- **The first boot of NEPTRON.** With no monitor set and no EDID from the monitor, NEPTRON starts at 15 kHz (`generic_15`). A 31 kHz PC monitor then shows "out of range". Set `monitor` in `neptron.toml` on the boot partition before the first boot (`os/README.md`).

## Build and verify

You need `rustup`, `cargo-nextest` and `cargo-deny`. The toolchain is pinned in `rust-toolchain.toml`.

```sh
cargo xtask verify --tier 1
```

This runs formatting, clippy, the dependency rules, the licence check and all tests. It writes `target/verify/report.json`. Run `cargo xtask help` for the other commands.

## Continuous integration

GitHub Actions runs the same `cargo xtask verify` on Linux and macOS. It builds three OS images on `main`, every night, and for a pull request that changes the OS or the device crates: the PC image and the aarch64 test image, which it boots in QEMU for the tier 3 checks, and the Raspberry Pi image, whose SD card layout it checks. Make the check `ci-ok` required. The PC image that CI uploads applies the 15 kHz kernel patch set, which is GPL-2.0-only (ADR 0018); CI uploads the sources archive of each image beside it. A version tag makes a draft GitHub Release with the checked images and their sources archives. [docs/ci.md](docs/ci.md) describes the jobs, the caches, the times and the artifacts.

## Repository layout

- `crates/`: libraries (types, safety validator, modeline solver, backends, UI, session).
- `apps/`: binaries (`neptron`, `neptron-launch`, `neptronctl`, `neptron-update`).
- `tools/`: standalone hardware tools.
- `data/`: monitor presets, cores, systems, input mappings, skins and hardware data.
- `os/`: the Buildroot tree of the OS images ([os/README.md](os/README.md)).
- `vendor/`: vendored third-party source (Switchres).
- `xtask/`: build, verification and worktree automation ([xtask/README.md](xtask/README.md)).
- `docs/`: definition, implementation plan, specs, flows, ADRs and taskboard.

## Documents

Start at [docs/README.md](docs/README.md).

## Licence

The NEPTRON source is licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. These parts are the exceptions, and each holds its licence text:

- `vendor/switchres` (Switchres) and `data/monitors` (monitor presets derived from Switchres): GPL-2.0-or-later.
- `crates/neptron-ui/assets/fonts/spleen` (the Spleen font) and the glyph table generated from it, `crates/neptron-ui/src/font/spleen_6x12.rs`: BSD-2-Clause (ADR 0004).
- `tests/roms`: the NES test ROM `240pee.nes`, GPL-2.0-or-later. `tests/roms/README.md` names its source. No image contains it.
- `tools/kms-spike`: `(MIT OR Apache-2.0) AND GPL-2.0-or-later`.

The `neptron` and `neptronctl` binaries link Switchres, so they are distributed under GPL-3.0-or-later ([LICENSE-GPL-3.0](LICENSE-GPL-3.0), ADR 0002). `neptron-launch` links no Switchres code and stays `MIT OR Apache-2.0`.

An OS image is an aggregate of programs under different licences: the Linux kernel (GPL-2.0-only), GRUB (GPL-3.0-or-later, PC), BusyBox, the NEPTRON programs, the cores and others. Each program keeps its own licence. FinalBurn Neo, Snes9x and Genesis Plus GX may not be sold or used commercially under their own licences, so an image that contains them may not be sold. Every other program keeps the rights of its own licence; remove those three cores to redistribute the rest under its own terms (ADR 0019).

Where to find the licences of an image: `LICENCES.txt` on the boot partition (a FAT partition that any computer can read), with the folder `legal/` beside it (the licence manifest, the licence texts, and the notices of the Rust crates in `legal/neptron-rust/`). Inside the image the same files are in `/usr/share/licenses`. The head of `LICENCES.txt` names the NEPTRON commit of the image. NEPTRON has no About screen yet. The source code of each released image is its `-sources.tar` file on the same release page.
