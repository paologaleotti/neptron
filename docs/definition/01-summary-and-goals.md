## Summary and goals

NEPTRON is an open-source, Buildroot-based retro gaming OS whose first job is to drive a CRT correctly: native resolution and refresh per game, on x86 PCs and Raspberry Pi, with every setting reachable from a controller. On flat panels it keeps the panel's native mode and fits games with integer scaling and scanline or CRT shaders.

It is one Rust monorepo. A single Rust process hosts libretro cores directly, owns the display through DRM/KMS, and draws its own UI. There is no RetroArch, no EmulationStation, no X11, no Wayland and no systemd in the image.

**Design principles**

- **CRT first.** Every timing decision is judged on a 15 kHz tube first. Flat panels over HDMI and DisplayPort are a fully supported second mode, not an afterthought.
- **Never emit an unsafe signal.** No modeline reaches the hardware unless it passes validation against the active monitor profile.
- **Appliance, not a desktop.** Read-only root, atomic updates, power-loss safe, boots to the game list in seconds.
- **Everything from the couch.** No config files to edit, no SSH needed, no keyboard required after first flash.
- **Small and deterministic.** Minimal userspace so cores get the CPU, the cache and a quiet scheduler.
- **Open and modular.** Permissive or copyleft licences only, one crate per concern, strict dependency direction.

**Non-goals for the first releases**

- Bezel artwork and decorative overlays, for now. Shaders and HD upscaling are in scope.
- Standalone (non-libretro) emulators. Everything runs as a libretro core. Also no Steam, Kodi or media playback.
- Online functionality: no netplay, achievements, cloud saves or accounts. The exceptions are box-art scraping, run only when the user asks, and setting the clock from network time at startup.
- ARM boards other than Raspberry Pi.
