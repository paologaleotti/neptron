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

**Where to start with Claude Code.** Build `neptron-modeline` and `neptron-safety` straight after the M0 verification tooling described in the Implementation plan tab: they are pure Rust, need no hardware, and can be tested against known Switchres outputs. In parallel, run `neptronctl` from a text console on any stock Linux install on the i3-6100T to prove modesetting and Intel interlace before the Buildroot image exists. The MVP taskboard in the Implementation plan tab gives the exact order.
