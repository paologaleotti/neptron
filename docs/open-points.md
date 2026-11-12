# Open points

A question that the documents do not answer becomes an open point (run rule 8). The task takes a safe default, records it in its task report, and continues. The orchestrator copies it here.

A `TODO` in the code names the id of its open point, for example `TODO(OP-003)`.

## Format

Each open point is one section:

```markdown
### OP-NNN: <the question, in one line>

- Raised by: <task id>
- Status: open | decided | closed
- Default: <the safe default that the code uses now>
- Where: <files or modules that hold the default>
- How to change: <what to edit when the decision changes>
- Decision: <empty while open; the decision and a link to its ADR>
```

## List

### OP-001: Which dependency kinds do the modeline and safety dependency rules restrict?

- Raised by: T01
- Status: open
- Default: normal dependencies only. Build dependencies (`cc`) and test dependencies stay allowed.
- Where: `xtask/src/rules.rs`
- How to change: add the other dependency kinds to the rule.
- Decision:

### OP-002: Which licences does `cargo-deny` allow, and do advisories run in T0?

- Raised by: T01
- Status: open
- Default: permissive licences only. The advisory check is not in T0, because it needs the network.
- Where: `deny.toml`, `xtask/src/verify.rs`
- How to change: edit the allow-list; add an advisory step to a tier that can use the network.
- Decision:

### OP-003: Does the macOS CI job run under the machine-load limits?

- Raised by: T01
- Status: open
- Default: yes. `xtask` cannot tell a CI runner from a local machine.
- Where: `.github/workflows/ci.yml`, `xtask/src/load.rs`
- How to change: set `NEPTRON_BUILD_SLOTS` and `NEPTRON_BUILD_JOBS` in the CI job.
- Decision:

### OP-004: Are the tier time budgets enforced?

- Raised by: T01
- Status: open
- Default: no. `verify` reports the time against the budget and does not fail on it.
- Where: `xtask/src/verify.rs`
- How to change: make a step over budget a failure.
- Decision:

### OP-005: The CI workflow was never run.

- Raised by: T01
- Status: open
- Default: the workflow is checked by inspection only. The first push of `main` runs it.
- Where: `.github/workflows/ci.yml`
- How to change: repair the workflow after the first run.
- Decision:

### OP-006: The forbid check can be fooled by a char literal inside an attribute.

- Raised by: T01 review
- Status: open
- Default: accepted. Only a file made on purpose can do this.
- Where: `xtask/src/rules.rs`, `bracket_body`
- How to change: handle char literals and comments inside attributes.
- Decision:

### OP-007: Can preset data derived from Switchres stay in the permissively licensed tree?

- Raised by: T02
- Status: decided
- Default: the 25 files in `data/monitors` stay, and `data/monitors/README.md` states the origin (Switchres, GPL-2.0-or-later, commit 09272ac).
- Where: `data/monitors`
- How to change: move the presets to a directory with its own GPL licence file, or write the values again from the monitor specifications.
- Decision: the presets stay in `data/monitors`, and that directory is licensed GPL-2.0-or-later with its own `LICENSE` file. The same holds for `vendor/switchres`.

### OP-008: The four `vesa_*` presets of Switchres are not converted.

- Raised by: T02
- Status: open
- Default: absent. Switchres computes them at run time with floating-point GTF maths, so there is no table to convert.
- Where: `data/monitors/README.md`
- How to change: T05 can produce them through Switchres.
- Decision:

### OP-009: `InputEvent` has no hotplug event and no device that is not a RetroPad.

- Raised by: T02
- Status: open
- Default: absent. A later addition does not change the existing wire format.
- Where: `crates/neptron-types/src/event.rs`
- How to change: add variants.
- Decision:

### OP-010: Is the UI canvas transparent over the game?

- Raised by: T06
- Status: open
- Default: opaque. The X byte of the canvas is ignored, so the quick menu hides the game where it is drawn.
- Where: `crates/neptron-gfx/src/reference/overlay.rs`
- How to change: a contract change (an alpha or colour-key rule for the canvas), with an ADR, then the overlay, its tests and one golden.
- Decision:

### OP-011: How is the UI placed on a super-width mode?

- Raised by: T06
- Status: decided
- Default: one uniform integer scale, centred. On a 1920x240 mode the 320-pixel menu fills one sixth of the width.
- Where: `Scene::ui_dest` in `crates/neptron-types/src/scene.rs`; `docs/adr/0005-ui-destination-in-scene.md`
- How to change: a contract change. `Scene` carries a UI destination rectangle or a scale per axis, which the video side chooses.
- Decision: `Scene` carries a destination rectangle for the UI canvas, `ui_dest`. The video side sets it: the full active area on a CRT, a centred box with one scale for both axes on a flat panel. The renderer draws the canvas into it with nearest scaling, each axis on its own. Done as contract change C01, before T19 and T25.

### OP-012: What the OpenGL ES renderer must do to match the reference renderer.

- Raised by: T06 review
- Status: closed by T19
- Default: the OpenGL ES renderer follows the rules below. It matches the reference under Mesa software GL: exact for nearest and UI pixels, within 1 per channel for sharp bilinear.
- Where: `crates/neptron-gfx/src/reference.rs`
- How to change: T19 computes the source index from `gl_FragCoord` with a small bias and `highp`, expands 5 and 6 bit channels itself (rounded, not bit replication), converts XRGB1555, and clamps the taps to the crop in the shader.
- Decision:

### OP-013: Nothing forces a caller to apply the pixel clock limit of an EDID profile.

- Raised by: T11
- Status: open, input for T15 and T27
- Default: `EdidProfile` carries the limit next to the profile, and the caller must call `limit_caps`.
- Where: `crates/neptron-platform/src/profile.rs`
- How to change: carry the limit in `MonitorProfile` (a contract change).
- Decision:

### OP-014: An EDID profile has one sync and porch set for the full line-rate range.

- Raised by: T11
- Status: open
- Default: the porches are the largest GTF values in the range. Wide modes near the top line rate then need more pixel clock than the monitor allows (for example 1024x768 at 85 Hz), and DMT modes with short sync pulses are refused. The project's own modes (480p, 240p at 120 Hz) fit.
- Where: `crates/neptron-platform/src/gtf.rs`, `profile.rs`
- How to change: one `CrtRange` per line-rate band.
- Decision:

### OP-015: The real EDID of the SyncMaster 740s is not checked.

- Raised by: T11
- Status: open, bench check
- Default: none. If the monitor does not set the GTF bit or is not EDID 1.3 or 1.4, it gets no profile from EDID and the user selects a preset.
- Where: `crates/neptron-platform/src/edid.rs`
- How to change: relax one named check after the real EDID is read.
- Decision:

### OP-016: A flat-panel profile in a custom file bypasses the hard rails.

- Raised by: T04 review
- Status: open
- Default: a flat-panel profile is matched exactly against its own listed modes and skips the rails.
- Where: `crates/neptron-safety/src/validated.rs`
- How to change: record where a profile comes from (EDID or custom file) and refuse `flat_panel` in a custom file. This is a contract change.
- Decision:

### OP-017: No hard rail applies above 40 kHz.

- Raised by: T04
- Status: open
- Default: above 40 kHz the monitor profile is the only limit.
- Where: `crates/neptron-safety/src/rails.rs`
- How to change: add one compiled absolute ceiling for line rate and field rate.
- Decision:

### OP-018: Sync polarity and the largest size of the GPU are not checked by `validate`.

- Raised by: T04
- Status: open
- Default: not checked. The KMS test-only commit refuses a mode that is too large.
- Where: `crates/neptron-safety`
- How to change: add `Rejection` variants (a contract change).
- Decision:

### OP-019: The hard rails are the union of all presets of a line-rate class.

- Raised by: T04 review
- Status: open
- Default: the rails guard against a bad custom profile, and not against the choice of the wrong preset for a monitor.
- Where: `crates/neptron-safety/build.rs`
- How to change: rails per monitor type.
- Decision:

### OP-020: The libretro bindings and the test core are checked on macOS only.

- Raised by: T09
- Status: open, the first CI run decides
- Default: the bindings are generated for LP64 targets and give the same output for x86_64 Linux, aarch64 Linux and aarch64 macOS. Nobody has built or loaded the Linux `.so`.
- Where: `crates/neptron-libretro-sys`, `crates/neptron-testcore`
- How to change: repair after the first Linux run. A 32-bit target needs a second generated file.
- Decision:

### OP-021: The test core has no save states.

- Raised by: T09
- Status: open
- Default: `retro_serialize_size` returns 0. No MVP task needs save states.
- Where: `crates/neptron-testcore/src/ffi.rs`
- How to change: add them to the core model when the save-state work starts.
- Decision:

### OP-022: Core options cannot be schema entries.

- Raised by: T08
- Status: open, input for T16
- Default: keys under `[core.<name>]` are kept in the file and reported as unknown keys.
- Where: `crates/neptron-settings/src/schema.rs` (`Entry` and `Kind` use `&'static str`)
- How to change: a namespace for options that a core declares at run time.
- Decision:

### OP-023: `gamelist.xml` import is not done.

- Raised by: T10
- Status: open
- Default: absent. It needs an XML reader.
- Where: `crates/neptron-library`
- How to change: a module that fills names and favourites after the scan.
- Decision:

### OP-024: Which control commands does a release image serve?

- Raised by: T12 review
- Status: open, to settle in T27 and T28
- Default: the Unix socket code is always compiled in `neptron-ipc`. The application serves `input_send`, `input_script`, `clock_step`, `session_launch` and `settings_set` only when it is built with its `control` feature. A scripted client can press "confirm", so these commands must not be in a release image.
- Where: `crates/neptron-ipc/README.md`, the application wiring
- How to change: a read-only subset (`hello`, `video_state`, `stats`, `log_tail`) on a socket with owner-only permissions in a release image.
- Decision:

### OP-025: `cargo xtask clippy -p` does not check test code.

- Raised by: T10
- Status: closed
- Default: only `cargo xtask verify` finds a lint in test code.
- Where: `xtask/src/main.rs`
- How to change: pass `--all-targets` in the `clippy` command.
- Decision: done. The command passes `--all-targets`.

### OP-026: On a 31 kHz monitor, Switchres pads a 224-line source to 500 lines.

- Raised by: T05
- Status: open, input for T15
- Default: the mode is valid, but the picture is small and high on the screen. The policy must prefer 240p at double refresh or a line-doubled mode on a 31 kHz profile.
- Where: `crates/neptron-modeline/tests/integration/safety.rs` records the mode
- How to change: the ODREN policy.
- Decision:

### OP-027: The window backend shows every mode as a 4:3 picture.

- Raised by: T13
- Status: open
- Default: 4:3 for all modes, also a 16:9 flat-panel mode.
- Where: `crates/neptron-window/src/picture.rs`, `CRT_ASPECT`
- How to change: pass the aspect of the monitor profile into `picture_size`.
- Decision:

### OP-028: SDL has one event pump for each process, and the keyboard pad of the window owns it.

- Raised by: T13
- Status: open, input for T18 and T27
- Default: the window's `KeyboardPad` owns the pump. Its device id is 0, which can be the id of a gamepad too.
- Where: `crates/neptron-window/src/keymap.rs`, `src/sdl`
- How to change: T18 shares the pump in a window build, and the ids get separate ranges.
- Decision:

### OP-029: No tier compiles the SDL3 code.

- Raised by: T13 review
- Status: open, to settle in T27
- Default: the SDL3 feature is checked by hand with `cargo xtask clippy -p neptron-window --features sdl3` and by `cargo xtask window-smoke`.
- Where: `xtask/src/verify.rs`
- How to change: a tier 2 step that builds the hardware features on a host that has the libraries.
- Decision:

### OP-030: `vkms` gives vblank events at twice the field rate for an interlaced mode.

- Raised by: T14, confirmed in review against the kernel source
- Status: open, input for T23
- Default: on `vkms`, interlaced modes are tested for the commit, the readback, the CRC and the writeback only. Field pacing is tested with the simulated clock and on hardware.
- Where: `os/`, the `vkms` probe
- How to change: none on kernel 6.18. It is how the kernel computes the frame duration for the vblank timer.
- Decision:

### OP-031: The UI needs data per option and names for its pages.

- Raised by: T07 review
- Status: open, part of T20
- Default: a choice option is a text label only, so the display panel cannot show the line rate of a picture style or grey out a style the monitor cannot show. The page ids for the display panel and the quick menu are not named. No intent dismisses a notice.
- Where: `crates/neptron-ui/src/snapshot.rs`, `intent.rs`
- How to change: T20 adds an option type with a label, a detail text and an enabled flag, named page ids and a dismiss intent.
- Decision:

### OP-032: `present` or `set_mode` while a flip is pending.

- Raised by: T03 review
- Status: open, input for T23
- Default: the fake display returns a device error, as a nonblocking atomic commit returns EBUSY. The engine must call `wait_flip` first.
- Where: `crates/neptron-testkit/README.md`, section "What the real backend must match"
- How to change: the KMS backend returns the same error, or the `Display` contract states the rule.
- Decision:

### OP-033: The Switchres wrapper and its goldens are checked on macOS only.

- Raised by: T05
- Status: open, the first CI run decides
- Default: the build uses `-ffp-contract=off` and the sources use exact library calls, so the 61 goldens are expected to be equal on Linux. Nobody has run it.
- Where: `crates/neptron-switchres-sys`, `crates/neptron-modeline/tests/golden`
- How to change: repair after the first Linux run.
- Decision:

### OP-034: `GpuCaps` has no doublescan field.

- Raised by: T05
- Status: open
- Default: doublescan is off in the solver settings, because i915 refuses doublescan modes.
- Where: `crates/neptron-modeline/src/settings.rs`
- How to change: a capability field in `GpuCaps` (a contract change).
- Decision:

### OP-035: A guest command can read the run nonce and forge the end marker.

- Raised by: T14
- Status: open, accepted for the MVP
- Default: the nonce stops marker text that a command prints by accident. It does not stop a command that reads the nonce from `fw_cfg` as root. The markers give honest test results; they are not a security boundary.
- Where: `xtask/src/os/transcript.rs`, `os/README.md`
- How to change: send the exit status through a channel that the guest command cannot write, for example a second serial port that only the init script opens.
- Decision:

### OP-036: `neptron-vkms-probe` divides by zero for a mode with a zero clock or a zero total.

- Raised by: T14 (review)
- Status: open
- Default: the probe is a test tool and its callers give it correct modes.
- Where: `os/` package `neptron-vkms-probe`, `parse_mode`
- How to change: refuse such a mode in `parse_mode`. Count a refused test-only commit as a failure.
- Decision:

### OP-037: The x86_64 image was never built or booted.

- Raised by: T14
- Status: open, the first x86_64 build decides
- Default: all tier 3 work of the MVP runs on the aarch64 QEMU image. The x86_64 target has a configuration and no result. The Debian packages of the development container are not pinned. CI does not fetch a base image, and the `--hot` overlay does not exist.
- Where: `os/README.md`, section "Not verified"
- How to change: build and boot the x86_64 image in T28 (the image for the real PC), and repair what fails.
- Decision:

### OP-038: Which combination is the blind recovery, and the arcade quick-menu rule.

- Raised by: T18
- Status: decided by the owner
- Default: Select+Start opens the quick menu. The same combination held for 5 s is the blind recovery. The arcade rule "hold Start for 2 s" is not implemented, because nothing tells a pad from a panel with no Select.
- Where: `QUICK_MENU_COMBO`, `BLIND_RECOVERY_COMBO` and `BLIND_RECOVERY_HOLD_NS` in `crates/neptron-input/src/hotkeys.rs`
- How to change: edit the constants. The arcade rule needs a second hold timer.
- Decision: Select+Start opens the quick menu, and the same combination held for 5 s is the blind recovery. "Hold Start for N seconds" becomes a setting (off by default, 2 s on an arcade panel). Change C03 adds it.

### OP-039: Input limits that hardware must show.

- Raised by: T18
- Status: open
- Default: a gamepad is identified by its SDL GUID only, so two identical pads can swap ports on reconnect. A gamepad refused because all ports are full gets no port until it reconnects. The device rescans run on the frame thread (every 30 polls for SDL, every 120 for evdev). The evdev crate prints one line to stderr when a grabbed keyboard goes away. The keyboard table of `neptron-window` is not tested against the shared fixture. SDL3 hotplug without libudev is not verified.
- Where: `crates/neptron-input/README.md`
- How to change: see the README for each item.
- Decision:

### OP-040: The zip path of the libretro host is not tested with a real core library.

- Raised by: T16
- Status: closed by T24. All three real cores load a zipped ROM, with the same frame and audio hashes as the plain ROM
- Default: `neptron-testcore` reports `need_fullpath = true`, so the test with the real library checks only the typed `ZipNeedsFullPath` error. A fake core inside the crate tests the zip success path through the same dispatch code. The real cores (FCEUmm, Snes9x, Genesis Plus GX) are not checked.
- Where: `crates/neptron-libretro/tests`, `crates/neptron-testcore`
- How to change: add a `content-data` script keyword to the test core. With it the core sends `SET_CONTENT_INFO_OVERRIDE` for its extension and reads the data with `GET_GAME_INFO_EXT`. T24 runs the real cores.
- Decision:

### OP-041: Libretro host defaults.

- Raised by: T16
- Status: open
- Default: a frame larger than the announced maximum is refused. The core library is never closed. `GET_LANGUAGE` reports English. Only version 0 of the core options API is served. A zip load holds up to about 148 MiB for a short time.
- Where: `docs/spec/libretro-host.md`, `crates/neptron-libretro/README.md`
- How to change: see the spec for each item. T24 traces of real cores decide the frame rule.
- Decision:

### OP-042: `AudioSink` has no pause, and the long audio run uses the cheaper resampler.

- Raised by: T17
- Status: open
- Default: a pause mutes, lets the device run dry on silence and fills the queue with silence at the next frame. The ten-minute part of timing check 6 is proven with the Polynomial mode; the Sinc default is proven for 30 s runs, because the sinc filter is slow at `opt-level = 0`. The rate re-estimation of Arntzen's paper (equation 8) is not implemented: the session gives the run rate.
- Where: `docs/spec/pacing-audio.md`, `crates/neptron-audio`
- How to change: add `pause` and `resume` to the trait (a contract change). Set an `opt-level` for `rubato` in the dev profile of the root `Cargo.toml`, then run the long tests with Sinc.
- Decision:

### OP-043: Limits of the OpenGL ES renderer.

- Raised by: T19
- Status: open
- Default: a scene whose crop side times dest side on one axis is above 2^23 is refused (`ScaleTooLarge`); the reference has no such limit. Only a headless constructor exists; T23 adds the one for its GBM surface. A big-endian host is a compile error. Without `EGL_KHR_display_reference` (Mesa 26.1.8 lacks it) a renderer never calls `eglTerminate`. `glow` can still panic when a driver gives a null or non-UTF-8 extension or renderer string. `khronos-egl` brings a second `libloading` version. `gles-compare --arch x86_64` was never run.
- Where: `crates/neptron-gfx/src/gles`, `crates/neptron-gfx/README.md`, `xtask/src/os/cross.rs`
- How to change: see the README for each item. Read `GL_RENDERER` and the extension list through the checked `glGetString` path to close the panic paths.
- Decision:

### OP-044: A binary that embeds the monitor presets carries their GPL licence.

- Raised by: T21
- Status: decided by the owner
- Default: `kms-spike` embeds `data/monitors` and states `(MIT OR Apache-2.0) AND GPL-2.0-or-later`. It is a bench tool and is not shipped. The same question comes back for the main binary: if it embeds the presets, the binary is GPL-2.0-or-later as a whole.
- Where: `tools/kms-spike/Cargo.toml`, `data/monitors/LICENSE`
- How to change: load the presets from files on the image at run time, so that the binary embeds no GPL data.
- Decision: the main binary loads the monitor presets from files on the image at run time and embeds no GPL data. T27 and T28 follow this. `kms-spike` stays as it is.

### OP-045: What `kms-spike` has not shown.

- Raised by: T21
- Status: open, the hardware run decides
- Default: the tool ran on `vkms` only. `vkms` has one fixed connector and no console to restore, takes interlace and any pixel clock, and gives vblanks at twice the field rate for an interlaced mode (OP-030). The derived 480p mode of `pc_31_120` has vertical porches of about 14, 3 and 27 lines (VGA has 10, 2 and 33). After a kill of the worker the test mode can stay on the output with a black picture.
- Where: `tools/kms-spike/README.md`, `docs/hardware-checklist.md`
- How to change: run the check list on the bench and record the results.
- Decision:

### OP-046: Readings of the Definition in the ODREN policy that the owner must confirm.

- Raised by: T15 and its reviews
- Status: decided in part by the owner
- Default: each reading is one named constant or function, listed in the README under "Readings".
  1. The `native` picture style walks the same rungs as `auto`, so both give the same plan today. The other reading: `native` falls only to interlaced 1:1, then to the fit, and never to a multiple.
  2. Borders exist only in 224 or 240 line modes (`BORDER_LINES`). On a 31 kHz monitor, Master System, Atari 2600 and ZX Spectrum get a filtered fit, where a pixel-exact 2x in a 400-line mode is possible. WonderSwan on `generic_15` gets a 144 to 221 line stretch at 65 Hz.
  3. A mode is padded when the game fills less than 75 % of its picture lines (`PADDED_FILL_MIN_PERCENT`). The Definition gives no number.
  4. With `exact_x2`, a padded mode at the doubled rate ranks before an unpadded exact multiple at the game's rate (Game Boy on `pc_31_120`: 144 lines in a 224-line border mode at 119.456 Hz wins over 3x at 59.728 Hz).
  5. A medium-resolution source on 15 kHz is shown interlaced 1:1 before the fit.
  6. "Matches to two decimals" means a difference under 5 mHz (`REFRESH_MATCH_MHZ`). `force_60` is 60.000 Hz exactly.
  7. A flat panel gives no notice for its refresh rate.
- Where: `crates/neptron-modeline/README.md`, `crates/neptron-modeline/src/policy`
- How to change: edit the named constant or function, and update the table rows and the goldens.
- Decision: (1) `native` means the lines of the game 1:1 and never a multiple; `auto` takes the best exact result for the monitor, which is native on 15 kHz and line doubled on 31 kHz. (2) Pixel exact has priority: a border mode can have any height that the monitor accepts, and a filtered fit is the last choice. Change C02 does both. Points 3 to 7 keep their defaults.

### OP-047: Limits of the ODREN policy.

- Raised by: T15
- Status: open
- Default: `pi_vc4_composite` gets no mode, because its pixel clock is fixed at 13.5 MHz and Switchres does not calculate for a fixed clock. The policy swaps the axes of a rotated game, but `Scene` has no rotation and the renderer does not rotate, so the engine must not show a rotated game. With a Switchres monitor aspect other than 4:3 every exact rung drops; no preset sets one, and the default is 4:3. A non-finite Switchres score stops the plan. The engine must apply `EdidProfile::limit_caps` before it calls the policy (OP-013), and it must commit `token.mode()` of the winner.
- Where: `crates/neptron-modeline/README.md`, section "Known limits"
- How to change: a fixed-clock strategy for composite (after the MVP); a rotation field in `Scene` (a contract change); pass 4:3 to Switchres and use the monitor aspect for border widths only.
- Decision:

### OP-048: What the app must do for the confirm dialog and the screens.

- Raised by: T20 and its reviews
- Status: open, input for T25 to T27
- Default: the dialog ignores Left and Right until 60 frames in a row pass with no direction action (`QUIET_FRAMES_BEFORE_MOVE`). The app must also guard: when a confirmation appears it stops every key repeat and sends nothing for a key that is held at that moment. Its first repeat delay must stay under 60 frames at the highest refresh rate (500 ms at 120 Hz). It calls `update` one time for each displayed frame. It gives each confirmation a new `ConfirmId`. It drops a `KeepMode` that arrives after its own timeout, because the UI still sends one at 0 s. It removes dismissed notices.
- Where: `crates/neptron-ui/README.md`, `crates/neptron-ui/src/screens/overlays.rs`
- How to change: the UI can also require elapsed time from `remaining_ns`, and can ignore Keep at 0 s.
- Decision:

### OP-049: Screen behaviour that the owner must confirm.

- Raised by: T20
- Status: open, needs a decision from the owner
- Default: only the Menu button resets a settings row to "inherited", so a two-button pad cannot do it. Notices wait until a menu is open (the canvas is opaque, OP-010). "All games" in the quick menu writes the global level only, so a value set for the system still wins. The display panel always writes the global level. More than 16 settings changed in one quick-menu visit: the extra ones stay at the game level. Favourites and Recent always show on the home screen when systems exist. The skin file refuses unknown keys.
- Where: `crates/neptron-ui/README.md`, `crates/neptron-ui/src/screens`
- How to change: see the README for each item. An "Inherit" entry in the option list solves the first.
- Decision:

### OP-050: Black scanlines for line-doubled 480p.

- Raised by: the owner's review of wave 2
- Status: decided by the owner, change C04
- Default: black scanlines are in the MVP for a whole vertical multiple, with a strength setting. The type in `Scene` is built so that other scanline kinds can follow (for example shaped scanlines at 3x and 4x, and different kinds for different resolutions) with no second contract change.
- Where: `crates/neptron-types/src/scene.rs`, `crates/neptron-gfx`
- How to change: a scanline strength in `Scene` (a contract change), in both renderers, with goldens.
- Decision: in the MVP. The owner: it must look right, like other setups and hardware line doublers, and it must be built so that more scanline kinds and their own settings can follow.

### OP-051: What the app must do for the Start hold, and settings help text.

- Raised by: C03 and its review
- Status: open, input for T27
- Default: the app reads `input.menu_hold_start_seconds`, passes it to `Hotkeys::set_start_hold_ns`, and treats a second `QuickMenu` while the menu is open as no action (the quick menu is never a toggle). When the hold fires, the app gives the core Start up on that port until the release. The game has already seen the Start press, so most games are in their own pause screen on resume. A game that needs Start held opens the menu by accident; the hold is off by default and the first-boot setup must say this. A schema entry has no help text field: the help of a setting is in the doc comment of its key and in the README.
- Where: `crates/neptron-input/README.md`, `crates/neptron-settings/src/builtin.rs`
- How to change: a `help` field in the schema entry (a change to the settings model, with an ADR).
- Decision:

### OP-052: Scanline defaults and what follows later.

- Raised by: C04 and its review
- Status: decided in part by the owner
- Default: the strength default is 75 percent. MiSTer offers 25, 50 and 75 for VGA monitors and no 100; users on PC CRTs mostly prefer 75; black lines on every second line halve the light. The dark line is the second of each pair. At 3x one line in three is dark, at 4x two in four: black lines stand in for the "shaped scanlines" of the Definition until that kind exists. `auto` is off on a flat panel and on 15 kHz. The 15/31 kHz boundary is 20 kHz by the line rate of the mode. The UI can offer presets (off, light 25, medium 50, strong 75, black 100). No test links the mode names in `neptron-settings` with `ScanlineMode` in `neptron-types`; a crate that uses both must add it.
- Where: `crates/neptron-types/src/scanline.rs`, `crates/neptron-settings/src/builtin.rs`, `docs/adr/0006-scanlines-in-scene.md`
- How to change: edit the named constants. A new scanline kind is a new variant of `ScanlineKind`. CRT shaders for flat panels (mask, glow, curvature) need one more field in `Scene`, a post-process stage in the GLES renderer, and a tolerance comparison in place of the byte-equal one.
- Decision: the owner asked for a researched default for VGA monitors; 75 percent. Shaders for flat panels come after the MVP.

### OP-053: Two test ROMs hold art of other owners, so they are outside the repository.

- Raised by: T24 review
- Status: open, needs a decision from the owner
- Default: the SNES and Mega Drive builds of the 240p Test Suite contain backgrounds from Sonic (Sega) and Kiki Kaikai (Taito) and "Donna" art with no stated licence. They are in no commit. `cargo xtask core-trace` reads them from `~/.cache/neptron/test-roms` and checks their SHA-256. A missing file fails the run with a line that names it. The NES build (240p-test-mini v0.23, GPL-2.0-or-later, own art) is in the repository. A machine without the two files, a CI runner too, cannot pass tier T4.
- Where: `tests/roms/README.md`, `xtask/src/os/core_trace.rs`
- How to change: (a) accept the two files in the repository with an ADR; (b) keep them outside, as now; (c) build the suite from its GPL source with that art removed or replaced, which needs the SNES and Mega Drive toolchains and new traces.
- Decision: the owner (2026-11-12): a ROM with a clear open licence for all its content can be in the repository; any other ROM stays outside and is downloaded to the development machine for the tests. The NES build (GPL-2.0-or-later, own art) stays in. The SNES and Mega Drive builds hold art with no licence from its owners, so they stay outside, as now.

### OP-054: Non-commercial cores in a distributed image.

- Raised by: T24
- Status: open, needs a decision from the owner before the first release
- Default: the Buildroot packages for Snes9x and Genesis Plus GX exist and only the two QEMU test images enable them. Those images are not distributed. Both licences forbid sale and commercial use. Genesis Plus GX also links LGPL code (Nuked OPN2). FCEUmm is GPL-2.0-or-later.
- Where: `os/package/libretro-*`, `data/cores`
- How to change: ship the cores in a free image with the licence texts and the `make legal-info` output; or download them at first boot; or use other cores (bsnes, BlastEm).
- Decision: the owner (2026-11-12): the product is open source and not commercial, so an image can hold Snes9x and Genesis Plus GX. The image must carry the licence texts and the `make legal-info` output, and must never be sold. T28 does this.

### OP-055: What the real cores report, and the options for the full picture.

- Raised by: T24
- Status: open, input for T25 and T27
- Default: the cores run with their default options. FCEUmm crops NES to 256x224 (8 lines at the top and at the bottom). Snes9x gives 224 lines always. Genesis Plus GX reports 256x192 at load and changes to 320x224 at frame 4, so the first mode choice at launch can be wrong for 4 frames. No real core used save RAM, because no test ROM has it. The host serves no `SET_SERIALIZATION_QUIRKS`, which matters when save states come. The traces are expected to be equal on x86_64; nobody has run it.
- Where: `tests/traces/README.md`, `data/cores`
- How to change: default options per system in the core metadata (it has no field for them yet): `fceumm_overscan_v_top=0` and `fceumm_overscan_v_bottom=0` give 256x240; `snes9x_overscan=auto` gives 224 or 239 lines as the game asks; `genesis_plus_gx_overscan=disabled` gives 320x224 and `top/bottom` gives 320x240 with the border. The session starts from the system's default geometry, or waits for the first frames before it commits a mode.
- Decision:

### OP-056: Policy choices of change C02 that the owner has not confirmed.

- Raised by: C02 and its review
- Status: open, needs a look from the owner
- Default: (1) on a 31 kHz monitor the `native` style takes double refresh on its own when the game's lines at the game's rate do not fill the range, and gives a notice. (2) The overscan crop bound is 8 lines per edge, the crop is symmetric, and an odd miss costs one extra line. (3) A padded exact mode is used only when the game fills at least 75 percent of the picture lines; a smaller fill ranks after it. (4) The standard border heights 224 and 240 come first, so a 192-line game gets 448 lines before 400. (5) Donkey Kong with `native` on `pc_31_120` shows at full height at 121.212 Hz with a crop of 8 lines per edge. (6) A double rate that the user asks for (`exact_x2`) ranks like a forced rate, before any fill comparison.
- Where: `crates/neptron-modeline/src/policy/crt.rs`, `crates/neptron-modeline/src/policy/choose.rs`, `crates/neptron-modeline/README.md` (Order, Known limits)
- How to change: the bounds are named constants (`OVERSCAN_CROP_MAX_LINES_PER_EDGE`, `PADDED_FILL_MIN_PERCENT`, `BORDER_LINES`). The order is in `native_key` and `entry`. Each rule has a table row in `tests/integration/policy_table.rs`.
- Decision: the owner (2026-11-12): accepted as built.

### OP-057: A crop at the bound can hide game content on arcade boards.

- Raised by: C02 review
- Status: open, known limit
- Default: arcade boards draw to the frame edge, so a crop of up to 8 lines per edge can hide real content. A vertical game shown unrotated loses columns of its upright picture (Donkey Kong on `arcade_15` loses 5 at each side). The bound is not checked against real game pictures. The notice reports the hidden lines.
- Where: `crates/neptron-modeline/README.md` (Known limits)
- How to change: a per-system or per-game crop bound in the core or game metadata; or a setting that turns the crop off, at the cost of a padded or filtered picture.
- Decision:

### OP-058: Layout and alert of the video status block.

- Raised by: C05 and its review
- Status: closed
- Default: (1) the result words sit on their own two lines under the mode line, because the mode text already uses 32 to 37 of the 37 characters at 320 pixels. The block always has five lines, so the menu rows never move. (2) An alert result (filtered, speed changed) shows in the focus colour of the skin, amber in the default skin. The skin has no warning colour, and the alert is not in the accessibility tree. (3) The display panel drops its Mode line during a game, because the block shows the mode read back from the kernel. (4) A screen shows the block only when at least 2 rows of its page fit under it; with 24-pixel rows the display panel shows no block and the quick menu scrolls. (5) The UI part is merged; the formatter that fills the text is in `neptron-video` (T22), and the app (T27) copies it into the snapshot each frame.
- Where: `crates/neptron-ui/src/screens/video.rs`, `crates/neptron-ui/README.md` (the contract of the text)
- How to change: a warning colour is a new field of the skin and a new version of the skin format. A cheaper mark is a text prefix from the formatter, for example `! filtered`, with no UI change.
- Decision: the owner (2026-11-12): the layout stays. An alert gets a text mark `[!]` from the formatter and its own warning colour. The palette also gets a red error colour for errors. Change C06 does this with a new version of the skin format. Done in C06: the mark is `[!] `, the warning colour is `#FF8020`, the error colour is `#FF6060`, and the speed word is `speed (X to Y Hz)` so the longest result fits two lines. In a version 2 skin file both colour keys are required. A warning notice uses the warning colour on its title bar.

### OP-059: The bounded flip wait is not in the `Display` contract or the fake.

- Raised by: T23 and its review
- Status: closed
- Default: `KmsDisplay::wait_flip` waits at most `FLIP_WAIT_LIMIT_MS` (500 ms). On a timeout it returns `DisplayError::Device` and the flip stays queued: it can still complete, and until then `present` and `set_mode` return `Device` as busy. `FakeDisplay` drops the pending flip on `FlipFault::Lost`, so the next `present` works. The two differ.
- Where: `crates/neptron-backend/src/display.rs`, `crates/neptron-testkit/src/display.rs`, `docs/spec/` (the KMS backend spec, steps 2 and 8)
- How to change: one small branch with an ADR. Add the sentence to the `wait_flip` doc. In the fake, `Lost` keeps the flip pending and returns `Device`; add `FlipFault::Slow { timeouts }`. Update the fake's README section "What the real backend must match". The same branch can add `Display::blank()` (output off: CRTC ACTIVE 0, MODE_ID 0), which the engine and T26 need when no valid mode exists.
- Decision: done in C07 with ADR 0007. The fake keeps a flip queued after any failed wait, `FlipFault::Slow` exists, and `Display::blank()` is in the contract, the fake, the KMS backend, the window and the engine.

### OP-060: Smaller points of the KMS backend.

- Raised by: T23 review
- Status: open, low priority
- Default: (1) `tools/kms-spike` and `neptron-kms` each hold a copy of the mode conversion; the records are equal except the name text. (2) `draw_to_window` with a zero side has no test, because EGL does not run on the host. (3) Another thread can close and reuse the launcher descriptor number between the device check and the take; the launcher contract excludes it. (4) The task also changed `neptron-gfx` (one shared `draw`, the GBM window target) and `xtask` (`kms-check`, one `RUSTFLAGS` for all guest builds); the orchestrator accepted both.
- Where: `crates/neptron-kms`, `crates/neptron-gfx`, `tools/kms-spike`, `xtask/src/os`
- How to change: make the spike depend on `neptron-kms` or remove it after the hardware tests.
- Decision:

### OP-061: Engine points for the session and the app.

- Raised by: T22 and its reviews
- Status: open, input for T25, T26 and T27
- Default: (1) the engine draws no scanlines until the session calls `set_scanlines` with the settings. (2) When no valid mode exists, the engine reports `current_refused()` and cannot turn the output off, because the `Display` contract has no `blank()` (see OP-059). (3) The video status text comes from `Odren::video_status` (the read back); a status built from a proposal is for the Display panel preview only and must not go in the running game's snapshot. (4) On the mode line an interlaced 1:1 mode shows `native`, the fitted rung shows `fit`, a flat panel shows `panel`, and a multiple shows `Nx`. (5) `exact` shows when there is no fit, no crop, no padding and no speed change; a fractional horizontal scale under a fixed super width still counts as exact. (6) `double refresh` shows also when the user asked for it. (7) A field rate of 100 Hz or more makes the mode line 38 characters, and the UI cuts it.
- Where: `crates/neptron-video/src/odren.rs`, `crates/neptron-video/src/status.rs`, `crates/neptron-video/README.md`
- How to change: the labels and words are in `rung_label` and `write_result`. Scanline settings can move into `VideoEngine::new`.
- Decision:

### OP-062: Engine behaviour around the blank, for the session and the guard.

- Raised by: C07 and its review
- Status: open, input for T25, T26 and T27
- Default: (1) a flip that never completes stops the output: each frame waits up to 500 ms on KMS and returns `PendingFlip`, and the engine does not recover alone. (2) After device errors block a source, the blank frame clears the block, so one more `set_mode` attempt follows (`MODESET_ATTEMPTS + 1` in all). (3) The engine also blanks when a valid mode exists but device errors stop it from replacing a refused mode on screen. (4) A blank that fails is tried up to `MODESET_ATTEMPTS` times; an input change or `retry()` starts a new count. (5) While a refused mode is on screen and a geometry change waits for its debounce, the refused mode stays for up to `DEBOUNCE_FRAMES - 1` frames, with `current_refused()` on.
- Where: `crates/neptron-video/src/odren.rs`, `crates/neptron-video/src/engine.rs`, `docs/adr/0007-bounded-flip-wait-and-blank.md`
- How to change: for (1), the session or the app opens the display again after N `PendingFlip` errors in a row. For (5), skip the debounce when `refused_on_screen` is on.
- Decision:
