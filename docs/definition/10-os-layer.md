## OS layer

The OS is a Buildroot image defined by a [br2-external tree](https://buildroot.org/downloads/manual/customize-outside-br.txt) inside the monorepo, with two targets: `x86_64` and one unified `rpi-arm64` image for Pi 3, 4, 5 and Zero 2 W. The Rust workspace is built through Buildroot's [cargo-package infrastructure](https://www.buildroot.org/downloads/manual/adding-packages-cargo.txt).

Buildroot ships only per-board Pi configurations, so the unified Pi image is NEPTRON's own defconfig: one 4K-page `kernel8.img` (the Pi 5 firmware falls back to it when `kernel_2712.img` is absent) and a userland built for Cortex-A53.

**Boot path**

1. Firmware loads the kernel directly. On Pi the GPU firmware does it, with no U-Boot. On x86 GRUB with zero timeout does it, for both UEFI and legacy BIOS.
2. The kernel has storage and DRM drivers built in, so no initramfs is needed to reach the root filesystem. The kernel sets no display mode of its own; see the safety model.
3. `BusyBox init` mounts filesystems and starts the launcher first; the launcher starts the frontend and respawns it if it exits. Network, Bluetooth and file sharing start afterwards, in the background.
4. The frontend shows the game list from a cached library index. Rescans happen after the UI is up.

| Stage | Budget, Pi 4/5 from SD | Budget, x86 from SSD |
| --- | --- | --- |
| Firmware to kernel start | Outside our control | Outside our control (POST) |
| Kernel start to PID 1 | 1.5 s | 1.0 s |
| PID 1 to first UI frame | 1.5 s | 1.0 s |
| Total we own | 3 s | 2 s |

These are targets to measure against from M0, not measured results. A boot-time regression test runs in CI under QEMU.

**Disk layout**

| Partition | Filesystem | Contents |
| --- | --- | --- |
| boot | FAT32 | Firmware, kernels, `neptron.toml` first-boot presets (monitor profile, Wi-Fi) |
| root A / root B | EROFS or SquashFS, read-only | The OS, frontend, cores |
| state | ext4 | Settings, saves, save states, last-known-good video config |
| share | exFAT | ROMs, BIOS files, skins. Readable when the card or stick is plugged into Windows or macOS |

Saves live on ext4 rather than exFAT because exFAT does not survive power loss well, and people switch CRT setups off at the wall.

### Configuration: one schema, one folder

Every setting belongs to one typed schema and is stored in one folder, `config/`. Anything else that looks like configuration is either a read-only default inside the image or a one-way inbox into that folder.

| What | Where it lives | Who writes it | How you reach it |
| --- | --- | --- | --- |
| Your settings | `config/` on the state partition | The menus, or you by hand | Menus; SMB share `config`; SFTP |
| First-boot and rescue file | `neptron.toml` on the FAT32 boot partition | You | Plug the disk into any PC; SFTP |
| Built-in defaults, monitor presets, system and core definitions | Inside the read-only image | Updates only | Read-only over SFTP; shown as "default" in the menus |
| Saves and save states | `saves/` and `states/` on the state partition | NEPTRON | SMB shares `saves` and `states`; SFTP |
| Secrets and machine state: Wi-Fi password, SSH password hash, last-known-good video | Internal part of the state partition | NEPTRON | Not shared; root over SSH only |
| ROMs, BIOS, skins, screenshots | Share partition (exFAT) | You | Plug the disk into any PC; SMB; SFTP |

```text
config/
├── global.toml                   what applies everywhere
├── systems/<system>.toml         only what differs for one system
├── games/<system>/<game>.toml    only what differs for one game
├── input/<controller>.toml       button mappings from the guided mapper
├── monitors/custom.toml          your own monitor profiles
└── reference.toml                generated, read-only: every key, default, allowed values
```

**How it behaves**

- **Order.** Game beats system, system beats global, global beats the built-in default. Files are sparse: a key appears only where you changed it. Deleting a key returns to the inherited value, and deleting the folder is a factory reset of settings.
- **Core options are ordinary keys** in the same three levels, under `[core.<name>]`. There are no separate per-core config files.
- **Editable by default.** The files are plain TOML, and a hand edit has the same standing as a menu change. NEPTRON keeps your comments and ordering when it writes, and writes atomically.
- **When a hand edit takes effect.** At start, and on "Reload settings" in the menu or `neptronctl settings reload`. NEPTRON re-reads a file before writing to it, so a menu change to one key never discards your edit to another.
- **Bad input never blocks boot.** An unknown or invalid key is ignored on its own and listed on the diagnostics page with file and line. Display keys still pass the safety validator, whoever wrote them.
- **`neptron.toml` is an inbox, not a second system.** It uses the same keys as `global.toml`. When it has changed since the last boot, its values are copied into `config/global.toml` once. It exists because the state partition is ext4 and cannot be read by plugging the disk into a Windows or Mac PC.
- **No secrets in `config/`.** The share is open on the LAN, so passwords are kept in the internal area.
- **Firmware files are generated.** `config.txt` on a Pi and the GRUB configuration are written from the settings. Hand edits to them are overwritten.

**Folder layout.** The share partition uses the folder names of Batocera and RetroPie, so an existing `roms` and `bios` collection copies over unchanged.

```text
share/                      exFAT; also the SMB share
├── roms/<system>/          one folder per system, EmulationStation names: nes, snes, megadrive, psx, arcade, ...
│   └── gamelist.xml        optional; names, favourites and image paths are imported
├── bios/                   BIOS files, with the subfolders some cores expect; cores get it as their system directory
├── skins/<name>/
└── screenshots/
state/                      ext4; reached over SMB and SFTP, not by plugging into a PC
├── config/                 global.toml, systems/<system>.toml, games/<system>/<game>.toml
├── saves/<system>/         <game>.srm, the same names as RetroArch
├── states/<system>/
└── video/                  last-known-good display configuration
```

- Each system definition in `data/systems` names its folder and its aliases (for example `genesis` for `megadrive`). An unknown folder is ignored and listed on the diagnostics page.
- The first boot creates an empty folder for every supported system, as Batocera does. A system shows in the UI only when its folder has games.
- exFAT ignores case, so RetroPie's `BIOS` folder works as `bios`. Each core's metadata lists its BIOS files with checksums for the presence check.
- Saves stay on the state partition. A `saves` folder found on the share (the Batocera layout) is imported once, on request.

**Updates.** A/B image updates through RAUC, built without its D-Bus service: the new signed bundle is written to the inactive slot, booted once, and kept only if the frontend reaches a confirmed picture. Otherwise the bootloader falls back. On x86, RAUC's GRUB backend does this with a one-shot try flag in `grubenv`. On Pi, RAUC has no firmware backend yet ([PR #1599](https://github.com/rauc/rauc/pull/1599) is open), so NEPTRON supplies a RAUC custom-backend script that edits `autoboot.txt` and reboots with `tryboot`. Bundles can also be installed from a file on USB, with no network.

**Network services.** NEPTRON follows Batocera's habits so the box is easy to reach on a home network:

- The SMB share is open to the LAN by default, with no password, under a default hostname (`neptron`).
- SSH and SFTP are on by default with a documented default login. The password can be changed, and each service switched off, in settings.
- These services are meant for a home LAN only. The first-boot wizard and the documentation say plainly not to forward them to the internet.
- The clock is set from network time once at startup when a network is up. With no network it keeps the last saved time and can be set by hand.

**Footprint and determinism**

- No systemd, no udev daemon, no display server, no PulseAudio or PipeWire. Audio goes through SDL3 to ALSA.
- BusyBox provides init and a debug shell. glibc is the C library, for Mesa and for core compatibility and speed.
- Targets: root image under 300 MB without cores, under 80 MB of RAM used at the game list, no periodic background work while a game runs.
- The emulation thread is pinned to a core kept free of other work, the audio thread runs with real-time priority, and the CPU governor is fixed at performance during play.

**Kernel**

- x86: current LTS (6.18 line) plus the 15 kHz patch set as Buildroot kernel patches, and out-of-tree input modules as Buildroot packages.
- Pi: the Raspberry Pi downstream kernel tree, plus the small DPI interlace patch for Pi 3/4 and custom device-tree overlays for the supported GPIO DACs.
- One kernel config fragment per concern (`video.config`, `input.config`, `fastboot.config`) so each choice is reviewable. The release kernel has no virtual terminal (CONFIG\_VT off) and no getty on a screen, so keys from keyboard encoders cannot reach a console. NEPTRON also grabs each keyboard device it reads.

**Cores** are Buildroot packages pinned to exact commits. Each core has a metadata file in the repo: systems, file extensions, required BIOS files, default options per platform, and whether it reports resolution changes reliably.
