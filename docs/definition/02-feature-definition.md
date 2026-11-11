## Feature definition

The table is the full scope for the first public alpha. Milestones M0 to M4 are defined in the roadmap. The MVP in the Implementation plan tab is a subset of M0 to M3.

| Area | Feature | Milestone |
| --- | --- | --- |
| Video | ODREN engine: native resolution and refresh per game, re-evaluated whenever the core changes geometry mid-game | M2 |
| Video | Monitor profiles: 15 kHz TV (NTSC/PAL), arcade 15/25/31 kHz, multisync, PC VGA 31 kHz, flat panel | M1 |
| Video | Safety validator, confirm-or-revert on every signal change, boot-time safe mode | M1 |
| Video | Three horizontal strategies: native width, dynamic super width, fixed super width (for example 2560x240) | M1 |
| Video | Force 50 Hz or 60 Hz, globally, per system or per game | M2 |
| Video | Interlaced modes (480i/576i) with automatic choice between progressive and interlaced | M2 |
| Video | 31 kHz VGA modes: 240p at 120 Hz with real scanlines, line doubling, 480i shown as 480p, native 70 Hz for DOS | M2 |
| Video | Picture style setting: native, native at double refresh, 2x, 3x, 4x or a fixed mode, each at the game's exact refresh, with scanline strength; per game, system or global | M2, menu M3 |
| Video | Sources that do not fit: handheld centring, medium-resolution arcade, out-of-range refresh; launch prediction from region and core traces | M2 |
| Video | Flat-panel mode: panel-native resolution up to 4K, integer scaling, aspect options, nearest refresh; variable refresh after alpha | M2 |
| Video | Signal geometry (H/V position and size) per monitor profile and mode family; software image position per game | M3 |
| Video | Sync type selection: separate H/V, composite sync where the hardware allows | M3 |
| Video | Monitor profile from EDID on PC CRTs; colour range, gain, gamma; dithering for 18-bit GPIO DACs | M3 |
| Video | Vertical games on horizontal screens, rotated and fitted; full rotation for a physically turned monitor | M3 |
| Video | Built-in shaders: scanlines, aperture grille and shadow mask, sharp-bilinear, CRT preset, handheld LCD grid; selectable per game, system or globally | M3 |
| Video | RetroArch slang shader presets through librashader | M4 |
| Video | HD upscaling on flat panels: higher internal resolution in cores that support it, plus upscaling shader presets for 2D games | M4 |
| Calibration | Built-in test patterns: grid, overscan, colour bars, convergence, geometry, 50/60 Hz, plus the 240p Test Suite ROM | M3 |
| Emulation | Multi-core libretro host: software, OpenGL ES and later Vulkan cores | M2 |
| Emulation | Low-latency frame pacing and audio resampling locked to the display's real refresh | M2 |
| Emulation | Save states, SRAM flush on exit, per-core options editor, BIOS presence check | M3 |
| Emulation | ROMs in `.zip` files, read into memory at launch | MVP |
| Emulation | Fast-forward; `.7z` ROMs; zipped ROMs unpacked to a file for cores that need a path | M3 |
| UI | Rust frontend: system list, game list, favourites, recents, search, launch, exit | M3 |
| UI | Settings at three levels: global, per system, per game, with visible inheritance | M3 |
| UI | In-game quick menu on a button combination: video, timing, core options, save states, input, quit | M3 |
| UI | Skinnable through data files; one built-in skin readable at 240p | M3, skins M4+ |
| Library | Simple box-art scraping, on request, matched by file name; the only online feature | M4 |
| Input | Gamepads and arcade sticks with automatic mapping; guided mapping for unknown pads | M2 |
| Input | USB steering wheels, with rumble passed to the wheel motor where the core supports it | M4 |
| Input | Light guns: GunCon 2, GunCon 3 and HID pointer guns (Gun4IR class), with a calibration screen. Sinden is an open question | M4 |
| Platform | x86-64 PCs (UEFI and legacy BIOS) and Raspberry Pi 3, 4, 5, Zero 2 W | M0 onward |
| Platform | Analog out: VGA and DVI-I on x86; GPIO DPI (VGA666, RGB-Pi, Pi2SCART class) and composite on Pi | M1, composite M4 |
| System | Game transfer over the network (SMB share, open on the LAN by default under a default hostname), USB storage, exFAT data partition; web uploader after alpha | M3 |
| System | Network setup and language from the UI; clock set from network time at startup when a network is there, by hand otherwise; Bluetooth pairing after alpha | M3 |
| System | Diagnostics page and one-button log export; SSH and SFTP on by default with a documented default password, changeable in settings | M3 |
| System | Safe shutdown from the power button (ACPI on x86, GPIO on Pi) with saves flushed first | M3 |
| System | Burn-in protection: screensaver after idle in menus and while paused; dims by default, with black-screen and off as options | M3 |
| System | Read-only root, A/B signed updates with rollback | M4 |
| System | Install to internal disk from the live USB image on x86 | M4 |
| Developer | Agent control API and headless verification: screenshots, UI tree, input injection, simulated clock, local emulation on macOS and Linux. See the Implementation plan tab | M0 onward |
