## Roadmap

Five milestones lead to a public alpha, each closed by a test on real hardware rather than by a date. The order puts the riskiest part, a correct and safe analog signal, before any UI work.

| Milestone | Delivers | Exit test |
| --- | --- | --- |
| M0 Skeleton | Workspace, lints, CI, backend fakes, scene renderers (CPU reference first), the agent control API and verification tiers T0 to T2 | A scripted session drives the app through the control socket, with identical results on macOS and Linux |
| M1 Signal | `modeline`, `safety`, `kms`, `platform`, `neptronctl`, monitor profiles, confirm-or-revert, built-in test patterns, Buildroot images for both targets, the QEMU tier, boot-time measurement | Stable 240p60, 288p50 and 480i on two setups: i3-6100T onboard VGA and Pi 4 with a GPIO DAC. Property tests green |
| M2 Play | Libretro host for software and GLES cores, audio with rate control, gamepads, ODREN switching, 50/60 forcing, flat-panel mode with integer scaling | Five reference systems (NES, SNES, Mega Drive, PlayStation, arcade) run 30 minutes with zero dropped frames; PlayStation mid-game resolution changes switch cleanly |
| M3 Console | UI, library, settings schema with three levels, quick menu, first-boot wizard, calibration, built-in scanline shaders, vertical-game rotation, network file transfer, diagnostics | A new user goes from a freshly flashed image to playing a game using only a gamepad |
| M4 Alpha | A/B updates, x86 installer, light guns, wheels, composite out, slang shader presets, HD upscaling, box-art scraping, validated hardware matrix, user docs | Public alpha images for `x86_64` and `rpi-arm64` |
| Later | Vulkan cores, bezels, more skins, local-network web companion over `neptron-ipc`, more boards |  |

### Described but not built: after 0.1

These pages describe the items below, but version 0.1 does not build them. Each place carries the mark *(after 0.1)*.

| Item | Where | State in 0.1 |
| --- | --- | --- |
| SSH and SFTP on by default with a password | 02, 10, 15 | The release image has no SSH server; the development image has Dropbear with one public key (`os/configs/*_dev.fragment`) |
| A/B updates with RAUC, and GRUB and `config.txt` written from the settings | 02, 10, 12 | One root partition; `os/board/pc/grub.cfg` and `os/board/rpi/config.txt` are fixed files; `neptron-update` is a stub |
| `tracing` into a RAM ring buffer | 12 | The programs log to stderr; `svlogd` keeps the log in `/state/log/neptron` (`os/README.md`) |
| `neptronctl bench` | 15 | No such command (`apps/neptronctl`) |
| The first-boot wizard | 05, 08, 10 | No wizard screen (`crates/neptron-ui`) |
| The game page; sort, filter and search in the game list | 02, 08 | The game list is sorted by name (`compare_names`, `crates/neptron-library/src/index.rs`) and has no search |
| The blind probe | 05 | With no EDID and no named profile, the start uses the safe default under the guard (`docs/spec/display-guard.md`) |
| The launcher blanks the analog output before the frontend starts | 05 | The launcher sets no mode (`apps/neptron-launch/README.md`, OP-077) |
| Release builds for `x86-64-v2` | 11 | The PC image builds for generic `x86_64` (`BR2_x86_64`, no target CPU); the Pi image builds for Cortex-A53 (`BR2_cortex_a53`) |
