## Decisions and open points

Twelve decisions are made. The open points below are a checklist: tick one when it is answered, and write the answer under it. Claude Code records new points in `docs/open-points.md` and copies them here.

### Decided on 2026-11-10

| Topic | Decision |
| --- | --- |
| Licence | All NEPTRON source is `MIT OR Apache-2.0`. The `neptron` binary and the images link Switchres (GPL-2.0-or-later), so they are distributed under GPL-3.0-or-later |
| MVP rendering | OpenGL ES and GBM buffers from the start, with no CPU-only stage to rework. A CPU reference renderer exists only for tests and the desktop window |
| Non-commercial cores | Accepted (Snes9x, Genesis Plus GX, FBNeo). Each core's licence is in its metadata, in the image's licence manifest, and on the About screen |
| Kernel | 6.18 LTS on x86 and the `rpi-6.18.y` tree on Pi. The 15 kHz patch set and the Pi tree both support it. Move to the next LTS when AMD analog outputs need it |
| Autonomy | Claude Code builds the whole MVP without human review. You answer only open points and architecture decisions |
| Safety limits | Generated from the shipped Switchres presets, not typed by hand; see the safety model |
| Taskboard | Every task has a binding brief, and the shared contracts are written in the Implementation plan tab |
| Test bench | i3-6100T on VGA with a Samsung SyncMaster 740s CRT; a Raspberry Pi 3B on composite; a European 15 kHz SCART TV. There is no VGA to SCART adapter yet, so 15 kHz from x86 is proven on the virtual display only until the community hardware matrix covers it. A VGA666 board for the Pi comes later |
| UI rendering | The UI emits a scene of four primitive kinds. GLES draws it on hardware; the CPU reference renderer draws it in tests and the desktop window |
| Burn-in | Screensaver after idle, dim by default |
| Network services | SMB open on the LAN under a default hostname; SSH and SFTP on with a default password; clock from network time at startup |
| Code and comments | Doc comments on every public item; comments only for limits, invariants and reasons; safety-critical code kept small and plain. Rules are in the Implementation plan tab |

### You decide

- [ ] **Update signing keys.** Default: you hold one offline release key; CI signs development builds with a separate key. Needed by WP 4.1.
- [ ] **Systems in the alpha** beyond the five reference ones.
- [ ] **32-bit targets** (old PCs, Pi 2, original Zero). Default: no.
- [ ] **Sinden guns** need a closed-source driver. Default: leave out.
- [ ] **Web companion** for phone control and ROM upload. Default: after alpha.
- [ ] **Dual screen** (CRT plus LCD). Default: after alpha.
- [ ] **Online system updates.** Default: keep signed downloads, plus USB-file updates.

### Claude Code answers during the build

The default applies until the point is ticked. A different answer needs your approval.

- [ ] **Text entry.** Wi-Fi passwords and search need an on-screen keyboard. Default: a grid keyboard widget in `neptron-ui`. WP 3.2.
- [ ] **Share partition size.** Default: the image has no share partition; the first boot creates it to fill the disk and formats it exFAT. WP 1.8.
- [ ] **Audio on a Pi with a GPIO DAC.** DPI uses most GPIO pins and the Pi 5 has no jack. Default: HDMI or USB audio; check each DAC board's free pins. Pi image task.
- [ ] **Arcade.** Core, ROM-set version and orientation data. Default: FBNeo with its own DAT file. WP 2.7.
- [ ] **Save compatibility.** Default: read RetroArch `.srm` names; save states are not compatible; `.m3u` and CHD through the cores. WP 3.3 and 3.4.
- [ ] **Rewind, run-ahead, cheats.** Default: non-goals until after the alpha.
- [ ] **Per-board performance list.** Default: `neptronctl bench` measures each system on each board in M2, and the table is committed.
- [ ] **Custom monitor profiles.** Default: a `custom.toml` file, validated under the hard rails; no editor screen in the alpha.
- [ ] **Fonts and languages.** Default: Latin scripts only in the alpha, with one permissively licensed pixel font.
- [ ] **Test ROMs.** Default: the 240p Test Suite builds (GPL-2.0) for NES, SNES and Mega Drive. MVP task T24.

### Answered by a task, not by a decision

- [ ] Does `vkms` accept interlaced user modes? (T14)
- [ ] Does the i3-6100T's DP-to-VGA bridge pass a 6.5 MHz pixel clock, and does 480i work with linear buffers? (T21, on your machine)
- [ ] Does librashader's OpenGL ES 3 path work on Pi 4 and 5? (WP 4.6)
- [ ] Does 480i work on Pi 3, 4 and Zero 2 W over GPIO DPI with the kernel patch? (when the VGA666 arrives)
