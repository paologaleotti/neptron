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
- Status: open, input for T19
- Default: none. T19 reads this point.
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
