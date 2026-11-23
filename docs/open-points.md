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
- Decision: <empty while open; the decision, its date and a link to its ADR>
```

The status has one of three values:

- `open`: no decision, or a decision that leaves work to do. The default holds.
- `decided`: the owner decided. The Decision line gives the decision and its date. The code follows it, or the entry names what is not built.
- `closed`: a task or a commit settled the point. The Status line names it.

An entry is never deleted and its id is never used again. An entry that a later one replaces stays, with a line that names its successor.

## Index

The state column is one line. The full entry below is the reference.

### Open

| Id | Title | State |
| --- | --- | --- |
| OP-001 | Dependency kinds of the modeline and safety rules | Normal dependencies only |
| OP-002 | `cargo-deny` licences and advisories | No advisory check in T0 |
| OP-003 | macOS CI job and the machine-load limits | The limits apply in CI |
| OP-004 | Tier time budgets | Reported, not enforced |
| OP-006 | Char literal inside an attribute fools the forbid check | Accepted |
| OP-008 | The four `vesa_*` presets | Not converted |
| OP-009 | No hotplug event, no device other than a RetroPad | Absent |
| OP-010 | Transparent UI canvas | Opaque |
| OP-014 | One sync and porch set per EDID range | Largest GTF porches |
| OP-015 | Real EDID of the SyncMaster 740s | Bench check |
| OP-016 | A flat panel in a custom file skips the rails | Accepted |
| OP-017 | No hard rail above 40 kHz | The profile is the only limit |
| OP-018 | Sync polarity and GPU size in `validate` | Not checked |
| OP-019 | Rails are the union of a class | No rails per monitor type |
| OP-020 | Bindings and test core on Linux | Real cores load on Linux; test core not run on Linux |
| OP-021 | Test core has no save states | Absent |
| OP-022 | Core options as schema entries | Kept as unknown keys |
| OP-023 | `gamelist.xml` import | Absent |
| OP-027 | Window backend is always 4:3 | 4:3 |
| OP-029 | No tier compiles the SDL3 code | Manual checks |
| OP-030 | `vkms` vblanks at twice the field rate in interlace | Kernel behaviour |
| OP-033 | Switchres goldens on Linux | Not recorded |
| OP-034 | No doublescan field in `GpuCaps` | Doublescan off |
| OP-035 | A guest command can forge the end marker | Accepted for the MVP |
| OP-036 | `neptron-vkms-probe` divides by zero | Callers give good modes |
| OP-039 | Input limits that hardware must show | Listed in the input README |
| OP-041 | Libretro host defaults | As listed |
| OP-042 | No pause in `AudioSink`; long run with Polynomial | As listed |
| OP-043 | Limits of the OpenGL ES renderer | As listed |
| OP-045 | What `kms-spike` has not shown | Bench check |
| OP-047 | Limits of the ODREN policy | Rotation, aspect, fixed-clock CRT |
| OP-049 | Screen behaviour for the owner to confirm | Defaults as built |
| OP-051 | No help text field in the schema | Help is in doc comments |
| OP-055 | Real core reports and full-picture options | Options set; launch settle |
| OP-057 | Crop at the bound on arcade boards | Known limit |
| OP-060 | Smaller points of the KMS backend | Low priority |
| OP-062 | Engine behaviour around the blank | As built |
| OP-063 | Timing check 6 with late flips | Weaker than the plan text |
| OP-064 | Session choices for the app | As built |
| OP-065 | Guard choices | Points 1 to 4 for the owner |
| OP-066 | User override of the safety limits | After the MVP |
| OP-067 | GPU cores and which emulators | After the MVP |
| OP-068 | 16:9 content | After the MVP |
| OP-069 | App and launcher choices | As built; points for the owner |
| OP-072 | Build cache strategy | Local part exists |
| OP-073 | Whole-kHz pixel clocks on a tube | Bench check |
| OP-074 | The hardware self-test | Bench check |
| OP-075 | Flat panels: limits | Bench and after the MVP |
| OP-077 | The PC image: what is not proven | Release image from CI only |
| OP-080 | Variable refresh rate | After the MVP |
| OP-081 | Menu and game modes differ on a panel | No change now |
| OP-087 | HDMI or DisplayPort output with the monitor | Mapping missing with more outputs than monitors |
| OP-088 | Later steps of the audio settings | After the MVP |
| OP-089 | Stale cross-build cache and warm image builds | Touch the sources; name the rebuild targets |
| OP-091 | `os build` image copy on a Linux host not uid 1000 | Fails |
| OP-092 | Later steps of the network share | Second share in C19 |
| OP-093 | No row kind in the control protocol | Protocol 2 exists, no `kind` field |
| OP-097 | Picture size and position on a TV-class CRT | First tube test done; no setting |
| OP-098 | A TV standard that follows the game | After the first composite image |
| OP-099 | A picture width per system on composite | Neo Geo 11 % narrow |
| OP-100 | Keep the video rules few | Accepted as built; checks to do |
| OP-101 | An old share at another place | No share, data safe |
| OP-102 | First composite picture: open points | Menu width, HDMI PCM label |
| OP-103 | Frame budget on the Pi 3 | Dialog rate fixed; standard change underruns |
| OP-104 | CI limits | aarch64 timing checks not in CI; two paths not proven |
| OP-106 | Open questions of the arcade spec | Defaults in `docs/spec/arcade.md`, section 19 |
| OP-107 | TATE: a monitor on its side, with the UI and the games turned | After the MVP; design notes in the arcade spec |
| OP-108 | Arcade leftovers after M07 | No diagnostics page; core default for the service menu |
| OP-109 | Video policy: questions for the owner and leftovers | Built defaults stay; the app does not set the arcade flag yet |
| OP-110 | Input for four players: owner decisions and leftovers | Plug order; a picker in Settings > Input is task M05b; no multitap |
| OP-111 | PC findings of 2026-11-17 | Governor fixed; the PS3 arcade stick works with the old USB enumeration scheme; cold build with the 15 kHz patch fails |
| OP-112 | A setting to turn off the old USB enumeration scheme | The images boot with `usbcore.old_scheme_first=1`; no setting yet |
| OP-113 | Leftovers of the start failure and quick menu work of 2026-11-18 | `os push --data` carries no arcade index and no controller database; no game with a disk image is tested |
| OP-114 | Decisions and findings of 2026-11-19 | M05b dropped; M12's gate met in CI; Killer Instinct is slow on the bench PC; the Pi has no HDMI entry in the hardware matrix |
| OP-115 | Decisions and findings of 2026-11-23 | The last sound round of M16 is dropped; the cost of the picture answers in a menu has no limit per frame; the screensaver is a bouncing wordmark |

### Decided

| Id | Title | State |
| --- | --- | --- |
| OP-007 | Switchres preset data in the tree | `data/monitors` is GPL-2.0-or-later |
| OP-046 | Readings of the Definition in the ODREN policy | Points 1 and 2 by C02; 3 to 7 as built |
| OP-052 | Scanline defaults | 75 percent; panel shaders after the MVP |
| OP-053 | Test ROMs with art of other owners | Outside the repository |
| OP-054 | Non-commercial cores in an image | Allowed, never sold |
| OP-056 | Policy choices of C02 | Accepted as built |
| OP-086 | Intel display firmware (DMC) | Left out |

### Closed

| Id | Title | Closed by |
| --- | --- | --- |
| OP-070 | Licence of the 15 kHz kernel patch set | M12 (ADR 0018) |
| OP-011 | UI placement on a super-width mode | C01 (ADR 0005) |
| OP-005 | The CI workflow on `main` | First green run 2026-11-14 |
| OP-012 | What the GLES renderer must do to match | T19 |
| OP-013 | EDID pixel clock limit | T22 (`Monitor::caps_for`) |
| OP-024 | Control commands in a release image | T28 (no `control` feature) |
| OP-025 | `xtask clippy -p` and test code | `--all-targets` |
| OP-026 | 224-line source padded to 500 lines on 31 kHz | T15 and C02 |
| OP-028 | One SDL event pump per process | T18 |
| OP-031 | Data per option and page names | T20 |
| OP-032 | `present` or `set_mode` while a flip is pending | C07 (ADR 0007) |
| OP-037 | The x86_64 image never built or booted | T28 and the first PC bench session |
| OP-038 | Blind recovery combination and the Start hold | C03 |
| OP-040 | Zip path with a real core | T24 |
| OP-044 | GPL presets in a binary | T27 (presets loaded at run time) |
| OP-048 | App guard for the confirm dialog | T27 |
| OP-050 | Black scanlines for line-doubled 480p | C04 (ADR 0006) |
| OP-058 | Layout and alert of the video status block | C06 |
| OP-059 | Bounded flip wait in the contract | C07 (ADR 0007) |
| OP-061 | Engine points for the session and the app | T27 and C07 |
| OP-071 | Owner decisions on flat panels | C08 (ADR 0010) |
| OP-078 | Rescan without a restart | C14 |
| OP-079 | Copy games over the network | C13 |
| OP-082 | Quick menu rows a flat panel ignores | C11 |
| OP-083 | Frontend starts before a monitor answers | C11 |
| OP-084 | Small findings of the first hardware session | C11; firmware in OP-086 |
| OP-085 | Audio volume and output device | C12 (ADR 0011) |
| OP-090 | Settings row, submenu arrow, breadcrumb | C15 |
| OP-094 | Reboot and shut down from the UI | C17 |
| OP-095 | First boot did not attach the share | C16 and C18 |
| OP-096 | The Raspberry Pi image | P01 to P06; P07 to P09 left |
| OP-076 | Boot prompt when nothing would change | M01: no prompt for the safe default |
| OP-105 | The keep or revert dialog of the display | M01, ADR 0013 |

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
- Status: closed by the CI task: the workflow runs on every push of `main`. The first green run on `main` is run 43637921848 of 2026-11-14 (commit `4b9f3c9`, 16 min 16 s, warm): lint, tier 2 on Linux and macOS, and the x86_64, aarch64 and Raspberry Pi image jobs with their checks. OP-104 holds what CI does not cover.
- Default: `docs/ci.md` describes the jobs, the caches and the measured times.
- Where: `.github/workflows/ci.yml`, `.github/workflows/images.yml`, `.github/image-targets.json`, `docs/ci.md`
- How to change: see `docs/ci.md`.
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
- Decision: the owner (2026-11-11): the presets stay in `data/monitors`, and that directory is licensed GPL-2.0-or-later with its own `LICENSE` file. The same holds for `vendor/switchres`.

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
- Status: closed by C01 (ADR 0005)
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
- Decision: done in T19. P10 and P11 changed the means on the Raspberry Pi 3 and kept the rule (exact for nearest and UI pixels, within 1 for sharp bilinear): one texel per pixel in BGRA bytes with the 16-bit formats expanded on the CPU, the destination index from a varying, and the quad limited to the pixels inside the frame (`crates/neptron-gfx/src/gles`). Successor for the limits: OP-043.

### OP-013: Nothing forces a caller to apply the pixel clock limit of an EDID profile.

- Raised by: T11
- Status: closed by T22: `Monitor::caps_for` in `crates/neptron-video/src/monitor.rs` applies `EdidProfile::limit_caps` for every EDID monitor, so the engine never calls the policy without the limit
- Default: `EdidProfile` carries the limit next to the profile, and the caller must call `limit_caps`.
- Where: `crates/neptron-platform/src/profile.rs`, `crates/neptron-video/src/monitor.rs`
- How to change: carry the limit in `MonitorProfile` (a contract change).
- Decision: the engine applies the limit; no contract change.

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
- Status: open, for the test core only
- Default: the bindings are generated for LP64 targets and give the same output for x86_64 Linux, aarch64 Linux and aarch64 macOS. The frontend loads the three real cores through them on aarch64 Linux (`core-trace` in QEMU, the Raspberry Pi 3) and on x86_64 Linux (the PC bench, 2026-11-13). No document records a build or a load of the test core's Linux `.so` (UNVERIFIED).
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
- Status: open
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
- Status: closed by T28: only the development fragments and the QEMU image set `BR2_PACKAGE_NEPTRON_CONTROL`, so a release image builds the frontend without `control`
- Default: the Unix socket code is always compiled in `neptron-ipc`. The application serves `input_send`, `input_script`, `session_launch` and `settings_set` only when it is built with its `control` feature. A scripted client can press "confirm", so these commands must not be in a release image.
- Where: `crates/neptron-ipc/README.md`, the application wiring, `os/package/neptron/neptron.mk`, `os/configs/*_dev.fragment`
- How to change: a read-only subset (`hello`, `video_state`, `stats`, `log_tail`) on a socket with owner-only permissions in a release image.
- Decision: a release image has no `control` feature. The read-only subset is a later step.

### OP-025: `cargo xtask clippy -p` does not check test code.

- Raised by: T10
- Status: closed by T10
- Default: only `cargo xtask verify` finds a lint in test code.
- Where: `xtask/src/main.rs`
- How to change: pass `--all-targets` in the `clippy` command.
- Decision: done. The command passes `--all-targets`.

### OP-026: On a 31 kHz monitor, Switchres pads a 224-line source to 500 lines.

- Raised by: T05
- Status: closed by T15 and C02
- Default: the mode is valid, but the picture is small and high on the screen. The policy must prefer 240p at double refresh or a line-doubled mode on a 31 kHz profile.
- Where: `crates/neptron-modeline/tests/integration/safety.rs` records the mode; `crates/neptron-modeline/README.md` ("Padding (OP-026)"); `tests/integration/policy_table.rs`
- How to change: the ODREN policy.
- Decision: the policy ranks a padded mode after the exact results when the game fills less than `PADDED_FILL_MIN_PERCENT` (75 %) of its picture lines. A table row on a 30 kHz profile proves it.

### OP-027: The window backend shows every mode as a 4:3 picture.

- Raised by: T13
- Status: open
- Default: 4:3 for all modes, also a 16:9 flat-panel mode.
- Where: `crates/neptron-window/src/picture.rs`, `CRT_ASPECT`
- How to change: pass the aspect of the monitor profile into `picture_size`.
- Decision:

### OP-028: SDL has one event pump for each process, and the keyboard pad of the window owns it.

- Raised by: T13
- Status: closed by T18
- Default: the window's `KeyboardPad` owns the pump. Its device id is 0, which can be the id of a gamepad too.
- Where: `crates/neptron-window/src/keymap.rs`, `src/sdl`, `crates/neptron-input/src/sdl.rs`, `crates/neptron-input/src/device.rs`
- How to change: T18 shares the pump in a window build, and the ids get separate ranges.
- Decision: the SDL3 gamepad source reads the gamepads with no event pump (`SDL_UpdateGamepads`), so the window keeps the one pump of the process. Gamepads and keyboards have separate device id ranges in `neptron-input`.

### OP-029: No tier compiles the SDL3 code.

- Raised by: T13 review
- Status: open
- Default: the SDL3 feature is checked by hand with `cargo xtask clippy -p neptron-window --features sdl3` and by `cargo xtask window-smoke`.
- Where: `xtask/src/verify.rs`
- How to change: a tier 2 step that builds the hardware features on a host that has the libraries.
- Decision:

### OP-030: `vkms` gives vblank events at twice the field rate for an interlaced mode.

- Raised by: T14, confirmed in review against the kernel source
- Status: open
- Default: on `vkms`, interlaced modes are tested for the commit, the readback, the CRC and the writeback only. Field pacing is tested with the simulated clock and on hardware.
- Where: `os/`, the `vkms` probe
- How to change: none on kernel 6.18. It is how the kernel computes the frame duration for the vblank timer.
- Decision:

### OP-031: The UI needs data per option and names for its pages.

- Raised by: T07 review
- Status: closed by T20
- Default: a choice option is a text label only, so the display panel cannot show the line rate of a picture style or grey out a style the monitor cannot show. The page ids for the display panel and the quick menu are not named. No intent dismisses a notice.
- Where: `crates/neptron-ui/src/snapshot.rs`, `intent.rs`
- How to change: T20 adds an option type with a label, a detail text and an enabled flag, named page ids and a dismiss intent.
- Decision: done. An option has a label, a detail and a disabled reason (`is_enabled`), and `Intent::DismissNotice` exists.

### OP-032: `present` or `set_mode` while a flip is pending.

- Raised by: T03 review
- Status: closed by C07 (ADR 0007)
- Default: the fake display returns a device error, as a nonblocking atomic commit returns EBUSY. The engine must call `wait_flip` first.
- Where: `crates/neptron-testkit/README.md`, section "What the real backend must match"; `crates/neptron-backend/src/display.rs`
- How to change: the KMS backend returns the same error, or the `Display` contract states the rule.
- Decision: the `Display` contract states the rule: `present`, `set_mode` and `blank` return `DisplayError::Device` as busy while a flip is queued, and a backend never drops a queued flip. The engine waits for the flip first.

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
- Status: closed by T28 and the first PC bench session (2026-11-13)
- Default: all tier 3 work of the MVP runs on the aarch64 QEMU image. The x86_64 target has a configuration and no result. The Debian packages of the development container are not pinned. CI does not fetch a base image, and the `--hot` overlay does not exist.
- Where: `os/README.md`, section "Not verified"
- How to change: build and boot the x86_64 image in T28 (the image for the real PC), and repair what fails.
- Decision: done in T28: the x86_64 PC image builds and boots in QEMU on BIOS and UEFI. The development image ran on the owner's PC on 2026-11-13 (OP-078 to OP-084 record that session). OP-077 lists what is still not proven.

### OP-038: Which combination is the blind recovery, and the arcade quick-menu rule.

- Raised by: T18
- Status: closed by C03 (decided by the owner)
- Default: Select+Start opens the quick menu. The same combination held for 5 s is the blind recovery. The arcade rule "hold Start for 2 s" is not implemented, because nothing tells a pad from a panel with no Select.
- Where: `QUICK_MENU_COMBO`, `BLIND_RECOVERY_COMBO` and `BLIND_RECOVERY_HOLD_NS` in `crates/neptron-input/src/hotkeys.rs`
- How to change: edit the constants. The arcade rule needs a second hold timer.
- Decision: the owner (2026-11-11): Select+Start opens the quick menu, and the same combination held for 5 s is the blind recovery. "Hold Start for N seconds" becomes a setting (off by default, 2 s on an arcade panel). C03 adds it as `input.menu_hold_start_seconds`.

### OP-039: Input limits that hardware must show.

- Raised by: T18
- Status: open
- Default: a gamepad is identified by its SDL GUID only, so two identical pads can swap ports on reconnect. A gamepad refused because all ports are full gets no port until it reconnects. The device rescans run on the frame thread (every 30 polls for SDL, every 120 for evdev). The evdev crate prints one line to stderr when a grabbed keyboard goes away. The keyboard table of `neptron-window` is not tested against the shared fixture. SDL3 hotplug without libudev is not verified.
- Where: `crates/neptron-input/README.md`
- How to change: see the README for each item.
- Decision:

### OP-040: The zip path of the libretro host is not tested with a real core library.

- Raised by: T16
- Status: closed by T24
- Default: `neptron-testcore` reports `need_fullpath = true`, so the test with the real library checks only the typed `ZipNeedsFullPath` error. A fake core inside the crate tests the zip success path through the same dispatch code. The real cores (FCEUmm, Snes9x, Genesis Plus GX) are not checked.
- Where: `crates/neptron-libretro/tests`, `crates/neptron-testcore`
- How to change: add a `content-data` script keyword to the test core. With it the core sends `SET_CONTENT_INFO_OVERRIDE` for its extension and reads the data with `GET_GAME_INFO_EXT`. T24 runs the real cores.
- Decision: done in T24. All three real cores load a zipped ROM, with the same frame and audio hashes as the plain ROM.

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
- Default: a scene whose crop side times dest side on one axis is above 2^23 is refused (`ScaleTooLarge`); the reference has no such limit. The renderer has a headless constructor, a pbuffer constructor for the window path of `gles-compare`, and the GBM window target of the KMS backend. A big-endian host is a compile error. Without `EGL_KHR_display_reference` (Mesa 26.1.8 lacks it) a renderer never calls `eglTerminate`. `glow` can still panic when a driver gives a null or non-UTF-8 extension or renderer string. `khronos-egl` brings a second `libloading` version. `gles-compare --arch x86_64` was never run. On the Raspberry Pi 3 (vc4, Mesa 26.1.8) every scene of `gles-compare` passes, through the framebuffer object and the window path, except the two axis product cases: their 4096-pixel output is above the 2048 texture limit of vc4 (commit `e03f6f7`). A source texture grows at the first frame of a larger size (OP-103 point 4).
- Where: `crates/neptron-gfx/src/gles`, `crates/neptron-gfx/README.md`, `xtask/src/os/cross.rs`
- How to change: see the README for each item. Read `GL_RENDERER` and the extension list through the checked `glGetString` path to close the panic paths.
- Decision:

### OP-044: A binary that embeds the monitor presets carries their GPL licence.

- Raised by: T21
- Status: closed by T27 (decided by the owner): `apps/neptron/src/presets.rs` loads the presets at run time
- Default: `kms-spike` embeds `data/monitors` and states `(MIT OR Apache-2.0) AND GPL-2.0-or-later`. It is a bench tool and is not shipped. The same question comes back for the main binary: if it embeds the presets, the binary is GPL-2.0-or-later as a whole.
- Where: `tools/kms-spike/Cargo.toml`, `data/monitors/LICENSE`
- How to change: load the presets from files on the image at run time, so that the binary embeds no GPL data.
- Decision: the owner (2026-11-11): the main binary loads the monitor presets from files on the image at run time and embeds no GPL data. T27 and T28 follow this. `kms-spike` stays as it is.

### OP-045: What `kms-spike` has not shown.

- Raised by: T21
- Status: open, the hardware run decides
- Default: the tool ran on `vkms` only. `vkms` has one fixed connector and no console to restore, takes interlace and any pixel clock, and gives vblanks at twice the field rate for an interlaced mode (OP-030). The derived 480p mode of `pc_31_120` has vertical porches of about 14, 3 and 27 lines (VGA has 10, 2 and 33). After a kill of the worker the test mode can stay on the output with a black picture.
- Where: `tools/kms-spike/README.md`, `docs/hardware-checklist.md`
- How to change: run the check list on the bench and record the results.
- Decision:

### OP-046: Readings of the Definition in the ODREN policy that the owner must confirm.

- Raised by: T15 and its reviews
- Status: decided; C02 built points 1 and 2
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
- Decision: the owner (2026-11-11): (1) `native` means the lines of the game 1:1 and never a multiple; `auto` takes the best exact result for the monitor, which is native on 15 kHz and line doubled on 31 kHz. (2) Pixel exact has priority: a border mode can have any height that the monitor accepts, and a filtered fit is the last choice. Change C02 does both. Points 3 to 7 keep their defaults.

### OP-047: Limits of the ODREN policy.

- Raised by: T15
- Status: open
- Default: a TV profile (`MonitorKind::Tv`, ADR 0012) gets its modes from the fixed table of its standard (`policy/tv.rs`), not from Switchres. A CRT profile on a GPU with a fixed pixel clock (`pi_vc4_composite`, 13.5 MHz) still gets no mode, because Switchres does not calculate for a fixed clock (`tests/golden/policy_gpus.toml`); the composite output of that GPU takes a TV profile. The policy swaps the axes of a rotated game, but `Scene` has no rotation and the renderer does not rotate, so the engine must not show a rotated game. With a Switchres monitor aspect other than 4:3 every exact rung drops; no preset sets one, and the default is 4:3. A non-finite Switchres score stops the plan. The engine must apply `EdidProfile::limit_caps` before it calls the policy (OP-013), and it must commit `token.mode()` of the winner.
- Where: `crates/neptron-modeline/README.md`, section "Known limits"
- How to change: a fixed-clock strategy for a CRT profile (after the MVP); a rotation field in `Scene` (a contract change); pass 4:3 to Switchres and use the monitor aspect for border widths only.
- Decision:

### OP-048: What the app must do for the confirm dialog and the screens.

- Raised by: T20 and its reviews
- Status: closed by T27: `apps/neptron/src/nav.rs` stops every repeat on a prompt and ignores held keys until release, with a first repeat delay of 400 ms (`REPEAT_DELAY_NS`); the `ConfirmId` is the guard's prompt id; a late Keep gets `ConfirmError::Expired` from the guard and does nothing; `DismissNotice` removes the notice (`apps/neptron/src/shell/intents.rs`)
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
- Status: closed by C04 (ADR 0006), decided by the owner
- Default: black scanlines are in the MVP for a whole vertical multiple, with a strength setting. The type in `Scene` is built so that other scanline kinds can follow (for example shaped scanlines at 3x and 4x, and different kinds for different resolutions) with no second contract change.
- Where: `crates/neptron-types/src/scene.rs`, `crates/neptron-gfx`
- How to change: a scanline strength in `Scene` (a contract change), in both renderers, with goldens.
- Decision: the owner (2026-11-11): in the MVP. It must look right, like other setups and hardware line doublers, and it must be built so that more scanline kinds and their own settings can follow.

### OP-051: What the app must do for the Start hold, and settings help text.

- Raised by: C03 and its review
- Status: open for the help text field only; T27 built the app part (`crates/neptron-session/README.md`, `apps/neptron/README.md`)
- Default: the app reads `input.menu_hold_start_seconds`, passes it to `Hotkeys::set_start_hold_ns`, and treats a second `QuickMenu` while the menu is open as no action (the quick menu is never a toggle). When the hold fires, the app gives the core Start up on that port until the release. The game has already seen the Start press, so most games are in their own pause screen on resume. A game that needs Start held opens the menu by accident; the hold is off by default and the first-boot setup must say this. A schema entry has no help text field: the help of a setting is in the doc comment of its key and in the README.
- Where: `crates/neptron-input/README.md`, `crates/neptron-settings/src/builtin.rs`
- How to change: a `help` field in the schema entry (a change to the settings model, with an ADR).
- Decision:

### OP-052: Scanline defaults and what follows later.

- Raised by: C04 and its review
- Status: decided
- Default: the strength default is 75 percent. MiSTer offers 25, 50 and 75 for VGA monitors and no 100; users on PC CRTs mostly prefer 75; black lines on every second line halve the light. The dark line is the second of each pair. At 3x one line in three is dark, at 4x two in four: black lines stand in for the "shaped scanlines" of the Definition until that kind exists. `auto` is off on a flat panel and on 15 kHz. The 15/31 kHz boundary is 20 kHz by the line rate of the mode. The UI can offer presets (off, light 25, medium 50, strong 75, black 100). No test links the mode names in `neptron-settings` with `ScanlineMode` in `neptron-types`; a crate that uses both must add it.
- Where: `crates/neptron-types/src/scanline.rs`, `crates/neptron-settings/src/builtin.rs`, `docs/adr/0006-scanlines-in-scene.md`
- How to change: edit the named constants. A new scanline kind is a new variant of `ScanlineKind`. CRT shaders for flat panels (mask, glow, curvature) need one more field in `Scene`, a post-process stage in the GLES renderer, and a tolerance comparison in place of the byte-equal one.
- Decision: the owner (2026-11-11) asked for a researched default for VGA monitors; 75 percent. Shaders for flat panels come after the MVP.

### OP-053: Two test ROMs hold art of other owners, so they are outside the repository.

- Raised by: T24 review
- Status: decided
- Default: the SNES and Mega Drive builds of the 240p Test Suite contain backgrounds from Sonic (Sega) and Kiki Kaikai (Taito) and "Donna" art with no stated licence. They are in no commit. `cargo xtask core-trace` reads them from `~/.cache/neptron/test-roms` and checks their SHA-256. A missing file fails the run with a line that names it. The NES build (240p-test-mini v0.23, GPL-2.0-or-later, own art) is in the repository. A machine without the two files, a CI runner too, cannot pass tier T4.
- Where: `tests/roms/README.md`, `xtask/src/os/core_trace.rs`
- How to change: (a) accept the two files in the repository with an ADR; (b) keep them outside, as now; (c) build the suite from its GPL source with that art removed or replaced, which needs the SNES and Mega Drive toolchains and new traces.
- Decision: the owner (2026-11-12): a ROM with a clear open licence for all its content can be in the repository; any other ROM stays outside and is downloaded to the development machine for the tests. The NES build (GPL-2.0-or-later, own art) stays in. The SNES and Mega Drive builds hold art with no licence from its owners, so they stay outside, as now.

### OP-054: Non-commercial cores in a distributed image.

- Raised by: T24
- Status: decided
- Default: the Buildroot packages for Snes9x and Genesis Plus GX exist and only the two QEMU test images enable them. Those images are not distributed. Both licences forbid sale and commercial use. Genesis Plus GX also links LGPL code (Nuked OPN2). FCEUmm is GPL-2.0-or-later.
- Where: `os/package/libretro-*`, `data/cores`
- How to change: ship the cores in a free image with the licence texts and the `make legal-info` output; or download them at first boot; or use other cores (bsnes, BlastEm).
- Decision: the owner (2026-11-12): the product is open source and not commercial, so an image can hold Snes9x and Genesis Plus GX. The image must carry the licence texts and the `make legal-info` output, and must never be sold. T28 does this.

### OP-055: What the real cores report, and the options for the full picture.

- Raised by: T24
- Status: open; the core options of the full picture are set (commit `9053872`)
- Default: `data/cores` sets the options of the full picture for each core: `fceumm_overscan_v_top` and `fceumm_overscan_v_bottom` at 0, `snes9x_overscan` at `auto`, `genesis_plus_gx_overscan` at `disabled`. With their own defaults the cores give this: FCEUmm crops NES to 256x224 (8 lines at the top and at the bottom), Snes9x gives 224 lines always. Genesis Plus GX reports 256x192 at load and changes to 320x224 at frame 4; the session waits for the source to settle before the first mode (OP-064 point 1). No real core used save RAM, because no test ROM has it. The host serves no `SET_SERIALIZATION_QUIRKS`, which matters when save states come. The traces are expected to be equal on x86_64; nobody has run it.
- Where: `tests/traces/README.md`, `data/cores`
- How to change: default options per system in the core metadata (it has no field for them yet): `fceumm_overscan_v_top=0` and `fceumm_overscan_v_bottom=0` give 256x240; `snes9x_overscan=auto` gives 224 or 239 lines as the game asks; `genesis_plus_gx_overscan=disabled` gives 320x224 and `top/bottom` gives 320x240 with the border. The session starts from the system's default geometry, or waits for the first frames before it commits a mode.
- Decision:

### OP-056: Policy choices of change C02 that the owner has not confirmed.

- Raised by: C02 and its review
- Status: decided
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
- Status: closed by C06
- Default: (1) the result words sit on their own two lines under the mode line, because the mode text already uses 32 to 37 of the 37 characters at 320 pixels. The block always has five lines, so the menu rows never move. (2) An alert result (filtered, speed changed) shows in the focus colour of the skin, amber in the default skin. The skin has no warning colour, and the alert is not in the accessibility tree. (3) The display panel drops its Mode line during a game, because the block shows the mode read back from the kernel. (4) A screen shows the block only when at least 2 rows of its page fit under it; with 24-pixel rows the display panel shows no block and the quick menu scrolls. (5) The UI part is merged; the formatter that fills the text is in `neptron-video` (T22), and the app (T27) copies it into the snapshot each frame.
- Where: `crates/neptron-ui/src/screens/video.rs`, `crates/neptron-ui/README.md` (the contract of the text)
- How to change: a warning colour is a new field of the skin and a new version of the skin format. A cheaper mark is a text prefix from the formatter, for example `! filtered`, with no UI change.
- Decision: the owner (2026-11-12): the layout stays. An alert gets a text mark `[!]` from the formatter and its own warning colour. The palette also gets a red error colour for errors. Change C06 does this with a new version of the skin format. Done in C06: the mark is `[!] `, the warning colour is `#FF8020`, the error colour is `#FF6060`, and the speed word is `speed (X to Y Hz)` so the longest result fits two lines. In a version 2 skin file both colour keys are required. A warning notice uses the warning colour on its title bar.

### OP-059: The bounded flip wait is not in the `Display` contract or the fake.

- Raised by: T23 and its review
- Status: closed by C07 (ADR 0007)
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
- Status: closed by T27 and C07: the app calls `set_scanlines` with the settings (`apps/neptron/src/shell/boot.rs`, `shell/intents.rs`), `Display::blank()` exists (ADR 0007), and points 3 to 7 describe the labels as built
- Default: (1) the engine draws no scanlines until the session calls `set_scanlines` with the settings. (2) When no valid mode exists, the engine reports `current_refused()` and cannot turn the output off, because the `Display` contract has no `blank()` (see OP-059). (3) The video status text comes from `Odren::video_status` (the read back); a status built from a proposal is for the Display panel preview only and must not go in the running game's snapshot. (4) On the mode line an interlaced 1:1 mode shows `native`, the fitted rung shows `fit`, a flat panel shows `panel`, and a multiple shows `Nx`. (5) `exact` shows when there is no fit, no crop, no padding and no speed change; a fractional horizontal scale under a fixed super width still counts as exact. (6) `double refresh` shows also when the user asked for it. (7) A field rate of 100 Hz or more makes the mode line 38 characters, and the UI cuts it.
- Where: `crates/neptron-video/src/odren.rs`, `crates/neptron-video/src/status.rs`, `crates/neptron-video/README.md`
- How to change: the labels and words are in `rung_label` and `write_result`. Scanline settings can move into `VideoEngine::new`.
- Decision:

### OP-062: Engine behaviour around the blank, for the session and the guard.

- Raised by: C07 and its review
- Status: open
- Default: (1) a flip that never completes stops the output: each frame waits up to 500 ms on KMS and returns `PendingFlip`, and the engine does not recover alone. (2) After device errors block a source, the blank frame clears the block, so one more `set_mode` attempt follows (`MODESET_ATTEMPTS + 1` in all). (3) The engine also blanks when a valid mode exists but device errors stop it from replacing a refused mode on screen. (4) A blank that fails is tried up to `MODESET_ATTEMPTS` times; an input change or `retry()` starts a new count. (5) While a refused mode is on screen and a geometry change waits for its debounce, the refused mode stays for up to `DEBOUNCE_FRAMES - 1` frames, with `current_refused()` on.
- Where: `crates/neptron-video/src/odren.rs`, `crates/neptron-video/src/engine.rs`, `docs/adr/0007-bounded-flip-wait-and-blank.md`
- How to change: for (1), the session or the app opens the display again after N `PendingFlip` errors in a row. For (5), skip the debounce when `refused_on_screen` is on.
- Decision:

### OP-063: Timing check 6 with late flips is weaker than the plan text.

- Raised by: T25 review
- Status: open, needs a look from the owner
- Default: the plan asks that the audio buffer stays inside its bounds over 100,000 frames, late vblanks included, in well under a second. With the 32 ms audio target, two late flips a few seconds apart run the queue dry, because the rate control recovers with a 6.4 s time constant. The long run with late flips asserts that every underrun follows two late flips in the 10 s before it. The runs without late flips assert no underrun. The two 100,000-frame runs take about 67 s each in the dev profile, so they are ignored tests: `cargo xtask test -p neptron-session --run-ignored only -E 'test(hundred_thousand)'`. The tier runs cover 30 simulated seconds per case with the strict assertions.
- Where: `crates/neptron-session/tests/integration/timing.rs`, `docs/spec/pacing-audio.md`
- How to change: a larger audio target level (more latency, more margin), or a faster recovery after an underrun.
- Decision:

### OP-064: Session choices for the app.

- Raised by: T25 and its review
- Status: open
- Default: (1) at launch the core runs up to 30 frames with no display until its source is the same for 6 frames, so the first mode is right (Genesis Plus GX). (2) A frame with more than 288 lines counts as interlaced. (3) Save RAM is flushed every 30 s when it changed, when the menu opens and at exit; a save file longer than the core's save RAM keeps its tail. (4) After 10 frames in a row with no flip the session ends with `DisplayStalled`; with wait timeouts that is about 5 s. (5) While the output is off, `run_frame` returns at once and the app paces the loop. (6) A core option that the core refuses is counted and not reported per key. (7) Two contents with the same file stem share one `.srm`. (8) "No allocation in a steady frame" holds by construction and has no test. (9) The frame after a resume goes out as silence.
- Where: `crates/neptron-session/README.md` (the list "what the app calls")
- How to change: the limits are named constants in `crates/neptron-session`. A default geometry per system in the core data removes the launch settle.
- Decision:

### OP-065: Guard choices and what the app must do.

- Raised by: T26 and its three reviews
- Status: open; points 1 to 4 need a look from the owner
- Default: (1) the guard covers the monitor and the policy settings the user changes. The connector (`display.output`) and geometry offsets are not under it; the app must put a connector change under the same prompt. (2) Last known good holds the monitor only, because the policy settings resolve per game. It is recorded only on a confirmation. (3) Blind recovery persists safe mode until the user confirms a configuration. (4) Limits: confirm window 10 s; a chain of changes with no confirmation ends 30 s after its first change (`PENDING_LIMIT_NS`); 1 s of no valid picture reverts a pending change or goes to the safe default (`NO_PICTURE_LIMIT_NS`); a boot is good after 10 s of picture. (5) A confirmation counts only after the engine placed a mode for the current inputs and a frame showed it (`Odren::settled`). A mode left on screen by device errors can never be confirmed. (6) When a decline or a game setting meets persistent device errors, a valid but unconfirmed mode can stay on screen with no good boot recorded; repeated boots like this go to safe mode. (7) In safe mode with no picture the guard does nothing more: no signal is the safe state. (8) The app gives every guard call the same clock read of the frame; an earlier time reverts a pending change. (9) A revert of a boot prompt carries the configuration the app must write to the global settings (`rewrite`); the app never writes `Guard::active()`. (10) The app builds the safe default for the detected output and stores the record (profile id and kind) with temp file, fsync, rename, before the first modeset. (11) Recovery before `Guard::boot` belongs to the launcher. (12) T27 needs one end-to-end test from the 5 s hold in `neptron-input` to `Guard::recover`.
- Where: `crates/neptron-video/src/guard`, `crates/neptron-video/README.md` (The guard; Narrower than the safety model)
- How to change: the limits are named constants in `guard.rs`. A connector field in `DisplayConfig` puts the output under the guard.
- Decision:

### OP-066: User override of the safety limits, with warnings.

- Raised by: the owner (2026-11-12), in a side session
- Status: open, after the MVP
- Default: no switch turns the validator off. The override is the monitor profile, which is a file loaded at run time.
- Where: `crates/neptron-safety`, `crates/neptron-modeline`, `data/monitors`, the display panel
- How to change: (1) a "custom monitor" editor in the UI for the line rate, field rate and sync ranges, with a warning text, under confirm-or-revert; (2) a tolerance setting for small misses outside the range, with the `[!]` alert; (3) a per-game override of the mode choice, saved by the user.
- Decision:

### OP-067: Hardware-rendered (GPU) cores, and which emulators are supported.

- Raised by: the owner (2026-11-12), in a side session
- Status: open, after the MVP
- Default: the MVP host takes software frames only. Direction: libretro cores only, and no standalone emulators, because they open the display themselves and go around the mode control and the safety path.
- Where: `crates/neptron-libretro`, `crates/neptron-gfx`, `data/cores`
- How to change: add the libretro hardware render interface to `neptron-libretro` (context create, reset, destroy; protect the GL state of the renderer), a desktop OpenGL context through EGL next to GLES in `neptron-gfx`, the internal resolution per core in the core files (1x for a CRT so nothing is filtered), and Vulkan later. Size: about one wave. The target is the cores that Batocera and RePlayOS offer, for example Dreamcast and GameCube.
- Decision:

### OP-068: 16:9 content with no per-game list.

- Raised by: the owner (2026-11-12), in a side session
- Status: open, after the MVP
- Default: the frame fills the 4:3 tube. The policy assumes a 4:3 monitor. The project ships no per-game patch; files per system and per core are acceptable.
- Where: `crates/neptron-modeline` (policy), `crates/neptron-types` (the destination in `Scene`), `crates/neptron-settings`, the quick menu
- How to change: a monitor aspect setting (4:3 or 16:9 tube); read the aspect ratio that the core reports in its geometry, also when it changes while the game runs; letterbox or fill as picture options; for GPU cores the core renders at the letterbox size so the picture stays exact. An anamorphic game (16:9 chosen in the game's own menu, output still a 4:3 frame) sends no signal, so the user sets "Aspect: 16:9" in the quick menu, and it is saved for that game as a user setting.
- Decision:

### OP-069: App and launcher choices, and what is not done.

- Raised by: T27 and its two reviews
- Status: open; points 6 to 12 need a look from the owner
- Default: (1) one run is one process: the game list or one game. A launch or the end of a game writes `run/next.toml` on a RAM file system and exits with code 75; the launcher starts the next run at once. More than 10 relaunches in 10 s count as crashes with the backoff (250 ms doubling to 8 s). (2) A planned relaunch gives back the failed-boot count of the run before (`record::for_continued_boot` in the app). A `Guard::resume` in `neptron-video` is the cleaner home. (3) A start that fails before its record write (the display does not open, no safe preset) is not a counted boot; it sets no mode, and the launcher keeps the last mode on the wire. (4) The KMS build of the app runs on the PC bench (2026-11-13) and on a Raspberry Pi 3 B (2026-11-14). The `sdl3` crate adds `-L /usr/local/lib`, which the Buildroot compiler wrapper refuses; the image packages and `os app-check` link through `os/package/neptron/link-filter.sh`, which drops only that path. (5) KMS defaults: buffers up to 2560x1200, the connector rule for a CRT, GPU caps from the first connected output, safe start profile `generic_15`. The launcher does not blank a 15 kHz output before the first commit and does not open the device again after many restarts. (6) The output connector has no menu: it changes in the settings file and takes effect at the next start, under the boot prompt. (7) The recovery tone plays only in the game list. (8) The control protocol does not serve `screenshot` or `clock_step`, and `video_state` gives no validator verdicts. `input_send`, `input_script`, `session_launch` and `settings_set` need the `control` build feature. (9) Favourites and recent games are not kept. (10) The menu is a 320x240 source at 60 Hz on every profile, 50 Hz profiles too. (11) In the window backend, Back on the home screen does nothing; the run ends when the window closes. (12) Logic that sits in the app and can move later: key repeat, the pages, preset loading, display detection.
- Where: `apps/neptron`, `apps/neptron-launch`, their READMEs, `xtask/src/os`
- How to change: see each README.
- Decision:

### OP-070: Licence of the 15 kHz kernel patch set. Must be closed before any image with the set is distributed.

- Raised by: T28
- Status: closed by M12 (ADR 0018, 2026-11-19). The author set the kernel patches to GPL-2.0-only in the tree (README section, commits `c57e6508` and `7abc1ed3`; GPL v2 LICENSE, `e106247a`). NEPTRON pins `d85a343379f7f54155f69ef56dc544b666635894`; the nine `linux-6.18` files are byte-identical to the old pin, so the hashes did not change. Every PC image applies the set, `LICENCES.txt` names the set, its commit and its licence, and each image carries a written offer for its sources. The owner accepted the remaining risk on 2026-11-19: the earlier contributors Ansa89, MaKoTo, substring, Calamity and amxcs did not state GPL-2.0 themselves in that repository; if one objects, the own-patch plan below applies. `--no-kernel-15khz` stays for the bench comparison. Earlier state: The author answered on 2026-11-15 in issue 22: the patches are GPL-2.0-only (upstream commits `c57e6508` and `e106247a`). The pinned commit is older than the licence change, so no image with the set is given to anybody until M12 pins a commit that carries the licence and records it
- Default: the repository `D0023R/linux_kernel_15khz` has a GPL-3.0 LICENSE file, and its patches carry no licence line. The Linux kernel is GPL-2.0-only, and GPL-3.0-only code cannot be combined with it. The PC images (release and development) apply the nine patches of the pinned commit. No image with these patches is given to anybody until this point is closed. `cargo xtask os build --arch x86_64 --no-kernel-15khz` builds the PC image without them, in its own output (`os/configs/neptron_pc_x86_64_no15khz.fragment`). The Raspberry Pi image never takes a kernel patch: `os/board/rpi/check-kernel-patches.sh` fails its build when `BR2_LINUX_KERNEL_PATCH` is set or its patch directory holds a patch, and a test in `xtask/src/os/rpi.rs` pins this. So this point does not block the Pi image. The files of the set are not committed to this repository; the pin is a commit id and the SHA-256 values in `os/patches/linux/linux.hash`. The owner asked the author to state the licence of the patches: https://github.com/D0023R/linux_kernel_15khz/issues/22. The last recorded check (2026-11-13) found no answer.
- Where: `os/patches/linux`, `os/configs/neptron_pc_x86_64_defconfig`, `os/README.md`, `os/board/pc/LICENCES.txt`, `os/board/rpi/check-kernel-patches.sh`
- How to change: when the author states GPL-2.0 for the patches, record the answer here and in `os/README.md` and close the point. With no answer or a GPL-3.0-only answer, write NEPTRON's own patch (below) and remove the pinned set.
- Decision: the owner (2026-11-12): include the patches in the images for now, ask the author, keep this reminder. Plan agreed with the owner: (1) keep the pinned set in private images for now. (2) On the bench, run the self-test also on a kernel built without the patches, and record which modes the unpatched driver refuses (pixel clock floor, interlace). NEPTRON sets every mode through atomic KMS and uses super-width modes, so it needs only that the driver accepts its modes, not the console and mode-table parts of the set. (3) Write only the needed change as our own patch, from the kernel source and documentation and without the text of the set, keep it in `os/patches` under GPL-2.0, and remove the pinned set. A copy or an adapted copy of the set does not clear the licence. If the author states GPL-2.0 first, the pinned set can stay. The owner (2026-11-13): every PC release image includes the 15 kHz patches; if no answer comes, NEPTRON writes its own patches and keeps them in its repository; until the licence is clear, CI uploads only the image without the patches (the CI workflow is not on `main`; UNVERIFIED).

### OP-071: Owner decisions on flat panels.

- Raised by: the owner (2026-11-12)
- Status: closed by C08 (ADR 0010), decided by the owner
- Default: flat panels are in the MVP and must be usable. The system detects a flat panel, uses its native mode, and shows the game at its correct shape with a whole vertical factor, centred, with black around it. Pixel exactness matters only on a CRT. On a flat panel a sharp filter is accepted and gives no alert; a blurry picture is never accepted. CRT shaders for flat panels come after the MVP. A VGA output behind a DAC is a CRT: an EDID that says analog input, or a DisplayPort adapter that reports VGA, selects the CRT path. An adapter that presents its own digital EDID cannot be detected; the owner accepts this limit, the user's preset in the settings wins over detection, and the first boot on a new output is under the confirm prompt.
- Where: `apps/neptron`, `crates/neptron-platform`, `crates/neptron-modeline` (the flat panel path)
- How to change: see the C08 report and the README of `apps/neptron`.
- Decision: as above. Added by the owner (2026-11-12): CRT limits apply to an analog output only; for a flat panel that is detected with certainty on a digital link they make no sense, and the type system must tell digital from analog. Done in C08 with `OutputKind` and ADR 0010.

### OP-072: Build cache strategy, locally and in CI.

- Raised by: the owner (2026-11-12)
- Status: open, the local part exists, the CI part is a task for after the MVP
- Default: what exists locally: Docker volumes hold the Buildroot downloads (`neptron-br-dl`), the compiler cache (`neptron-br-ccache`, `BR2_CCACHE=y`), the Buildroot output per target (`neptron-br-out-<arch>`) and the Cargo registry and target directory of the guest builds (`neptron-cross-<arch>`). All worktrees share them. A second image build reuses the output volume and builds only what changed, so the long first build (toolchain, LLVM, Mesa, kernel) happens once per target and per version pin. Each task worktree gets its `target/` as a clone of main's. The development container is tagged by the hash of its Dockerfile. What does not exist: CI itself; a published base image or toolchain for CI; a cache key rule; a check that a change of one package does not rebuild the world.
- Where: `xtask/src/os/build.rs`, `os/README.md` (the table of volumes), `docs/plan/09-...` (CI)
- How to change: (1) CI on x86_64 Linux runners with hardware virtualisation, so the x86_64 guest checks run fast. (2) Restore and save the four caches in CI with keys from the pins: the downloads by the hash of the `.hash` files, the output volume by the hash of the defconfig, the kernel and Mesa pins and the Dockerfile, the compiler cache by branch with a fallback to main. (3) Publish the built toolchain and the base image (everything except the frontend) as a CI artifact; a normal change then builds only the frontend package and the image step. `--images` and `NEPTRON_OS_IMAGES` already boot an images directory from elsewhere. (4) Build the frontend package from the Cargo cache volume so a Rust change compiles only changed crates in the release profile. (5) Never delete the volumes in a routine clean; give `cargo xtask os` a command that reports their sizes and a separate, explicit one that prunes. (6) Measure and record the time of a cold build, a warm build and a frontend-only build per target.
- Decision:

### OP-073: Pixel clocks in whole kHz, and what follows.

- Raised by: T29 (found on vkms), C09 and its review
- Status: open, points 1 and 2 need the bench
- Default: the kernel mode record holds the pixel clock in kHz, and a backend refuses any other clock and never rounds (ADR 0009). The policy gives every CRT mode a whole-kHz clock: it changes the horizontal total by up to 8 pixels in the back porch (`HTOTAL_CHANGE_MAX`) and takes the first timing that keeps the game's field rate. Every exact result stays within 3 mHz of the game's rate, so no new speed alert fires and no tolerance changed. `FakeDisplay` refuses the same modes as the KMS backend. (1) A changed back porch shifts the picture by at most about 1.3 microseconds, 1.9 percent of a 15 kHz line for a 320-wide mode and up to 3.2 percent for a 160-wide 31 kHz mode, and changes the active width by the same share. Nobody has seen it on a tube. (2) The sync pulse of a changed timing stays between 90 and 110 percent of nominal (`HSYNC_MAX_PERCENT` is the implementer's bound; the validator allows 150). (3) `neptron_safety::validate` does not check the kHz step; the reviewer agrees: it is a rule of the kernel interface, and a refused mode never reaches the wire. (4) `tools/kms-spike` takes raw Switchres modes and refuses a clock that is not whole kHz, so it shows refusals where the frontend sets a mode. (5) `cargo xtask kms-check` did not run on this change before the merge, because the image build held the lock; run it when the lock is free.
- Where: `crates/neptron-modeline/src/policy/clock.rs`, `docs/adr/0009-pixel-clock-in-whole-khz.md`
- How to change: the bounds are named constants in `clock.rs`.
- Decision:

### OP-074: The hardware self-test: choices and limits.

- Raised by: T29 and its review
- Status: open, for the bench
- Default: (1) `neptronctl selftest video` runs with the frontend and the launcher stopped and opens the DRM node itself. The dry run is the default; the real run needs `--i-have-checked-the-monitor <profile id>`. (2) Every run ends with the output off, because the backend gives no way to read the earlier state and validate it. (3) `tools/kms-spike` stays until the owner has run both tools on the bench and compared the reports (OP-060); then the spike and its rule in `xtask/src/rules.rs` can go. (4) The test pattern is at most 512 pixels wide (the texture limit of the smallest supported GPU, vc4) and is scaled by a whole factor on wider modes. A `KmsDisplay` accessor for the real texture size would lift this. (5) Missed vblanks are recorded and do not fail a mode. (6) For an unknown GPU the interlace timing unit is assumed and the report says so (`unit_assumed`). (7) The vkms gate `cargo xtask os selftest-check` passes: 2 profiles, 4 modes each, 0 failures. (8) The images hold `neptronctl` with the `kms` feature (`os/package/neptronctl`).
- Where: `apps/neptronctl/src/selftest`, `apps/neptronctl/README.md` (the report schema), `docs/hardware-checklist.md`
- How to change: the pass rules are named constants in the self-test.
- Decision:

### OP-075: Flat panels: what is built, and the limits.

- Raised by: C08 and its two reviews
- Status: open, for the bench and for after the MVP
- Default: (1) Class rule: an EDID that says analog input, a `subconnector` of VGA or DVI-A, or an analog connector type is a CRT; every other digital EDID on a digital connector is a flat panel. A named or driven output uses only its own EDID. A preset in `display.monitor` wins; `flat` forces the panel path. (2) An adapter that shows its own digital EDID with a CRT behind it looks like a panel (accepted by the owner). The protection: the 15 kHz safe start is the fallback until a panel is confirmed on its output; a run opens at most two panel windows of 10 s, the boot prompt and one offer on a fresh button press; never after a blind recovery; never over an explicit preset. An unconfirmed first boot on a real panel shows no picture until that press. After two unconfirmed boots the start is in safe mode. (3) Limits by output kind (ADR 0010): analog keeps the old values; digital on Intel gen9: HDMI 300 MHz, DVI-D 165 MHz, DisplayPort 533.25 MHz, size 4096x2160. Other GPUs have no digital limits yet, so their panels run at 1080p or less. After the display opens on an analog output, the caps are cut to the analog values. (4) When the native mode does not fit, a mode of 50 Hz or more comes before a larger slower one: a 4K panel on HDMI 1.4 runs at 1080p60 with a notice. (5) Picture: the native mode, the largest whole vertical factor, the game's shape, centred, black around it. The horizontal axis uses sharp bilinear; an axis with a scale between 1 and 2 uses nearest; the status says `exact` or `sharp`, never an alert. A true downscale (a source taller than the panel mode) keeps sharp bilinear and can blur; no MVP source does that at 720p or more. (6) Not parsed from the EDID: interlaced modes, 59.94 Hz variants of most CTA codes, other standard timings, DisplayID. EDID 1.0 to 1.2 or a bad extension checksum gives the safe start, so such a panel shows no picture. (7) When the large buffers of a 4K panel cannot be allocated, the start fails; a retry at the smaller size needs an optional fallback size in `KmsConfig`. (8) The pre-open connector choice does not know the subconnector. (9) No pixel-exact option on a panel, and no CRT shaders (after the MVP). (10) `docs/definition` says "EDID from any connector" and "a flat panel gets built-in scanlines"; the code is stricter on the first and follows OP-052 on the second.
- Where: `crates/neptron-platform` (class, panel profile, matrix), `apps/neptron` (display, shell), `crates/neptron-modeline/src/policy` (panel placement), `data/hardware/matrix.toml`, `docs/adr/0010-output-kind-picks-the-gpu-limits.md`
- How to change: digital limits per GPU go in `[[gpu.digital]]` of the matrix with their source. The offer count is `PANEL_WINDOWS_PER_RUN`.
- Decision:

### OP-076: The first boot shows a confirm prompt also when nothing would change.

- Raised by: T28 review
- Status: closed by M01 (ADR 0013): a boot whose wanted monitor is the safe default, is trusted by its class, or is confirmed at this place opens no prompt and shows no notice. The text below describes the state before M01
- Default: with no confirmed record, `Guard::boot` opens the boot prompt for the wanted configuration and reverts to the safe default after 10 s. When the wanted and the safe configuration are the same (a monitor with EDID on first boot, or vkms with `generic_15`), the prompt "reverts" to the same mode and a notice says "The display change went back: it was not kept in time." until the user presses OK. Nothing is wrong on the wire, but the text confuses. The boot-check golden shows this notice.
- Where: `crates/neptron-video/src/guard/machine.rs` (`Guard::boot`), `apps/neptron` (the notice), `os/tests/golden/boot-game-list.ppm`
- How to change: when the wanted configuration equals the safe default, open no prompt and record nothing as confirmed; or keep the prompt and give the notice another text. A change in the guard needs a review, and the golden changes with it.
- Decision:

### OP-077: The PC image: what exists, what is not proven, and what must happen before the bench.

- Raised by: T28 and its three reviews
- Status: open; points 1 to 3 come before the first release image
- Default: (1) No release image holds the final code. The release image that passed the gate (SHA-256 fb6dce6c..., kept in `~/.cache/neptron/buildroot/images/x86_64-release-fb6dce6c`) is older than the last fixes. The x86_64 output volume holds the development packages, so the release build refuses it. A release image needs a clean build in an empty output: `cargo xtask os build --arch x86_64 --output x86_64-clean`, about 5 hours and about 45 GB in the Docker disk, which has about 9 GB free. Then `os boot-check` on BIOS and UEFI, the size and the SHA-256. (2) The development image is the one for the first bench session: `images/x86_64-dev/neptron-x86_64.img`, SHA-256 4602b70dfd0234658268a219d097556782037f535f589e2acdc2873dca602e6c, with SSH for one public key, the hot folder `/state/hot`, `cargo xtask os push`, `os logs` and `os ssh`, the log under `/state/log`, and, until M02, the address in `neptron-ip.txt` on the share partition (later images write `/run/neptron/network.toml` and nothing to the share). It was built step by step, not clean. (3) The 15 kHz patch set is in both images (OP-070): nothing is given to anybody. (4) Mesa has the Intel `iris` driver; no firmware files are in the image, so AMD GPUs do not work and the Intel DMC firmware is not loaded. The i915 and amdgpu firmware licences allow redistribution of the binaries only. (5) About 104 MiB of clang libraries that nothing uses are in the root. (6) Before the frontend's first commit a CRT gets the firmware and GRUB picture at 31 kHz or more; the launcher does not blank the output. A 15 kHz-only tube needs a sync-protecting adapter or a late power-on. (7) The share partition is made at first boot. The format rule is strict (a marker, a clean read, no file system and no exFAT backup record). Not run: a real read error, a re-flash with a different layout, a 4096-byte sector disk. A power cut during mkfs leaves a share that needs a format on another computer. (8) A quit in the UI restarts the frontend; nothing powers the PC off. (9) Each variant has a fixed disk signature, so two sticks of one variant in one PC share a PARTUUID. (10) Test tools and QEMU hooks are in the image; they refuse to act unless the DRM driver is vkms. (11) The crates of the frontend are pinned by Cargo.lock checksums through `cargo vendor --locked`. (12) The development image waits for DHCP at boot, so its first frame comes later. (13) The boot check runs only in QEMU; the x86_64 times are emulated.
- Where: `os/`, `os/README.md`, `xtask/src/os`, `xtask/README.md`
- How to change: see `os/README.md`, sections "The PC image" and "Working on the bench".
- Decision: Owner, 2026-11-13: the release image is built in CI only and is uploaded as a workflow artifact; no local release build.

### OP-078: The game list does not see new files until the frontend starts again.

- Raised by: the owner, on the first hardware session (2026-11-13)
- Status: closed by C14; the hardware checks and the later steps stay in this entry
- Default: the library is scanned when the game list run starts. Files copied to the share partition while the list is on screen do not appear. The way out today is a restart of the frontend (quit a game, or `kill -TERM` of the frontend over SSH).
- Where: `apps/neptron` (the library and its background scan), `crates/neptron-library`
- How to change: a "Rescan games" row in the menu; a rescan when the list gets the focus again after a game; a watch on the `roms` folders (inotify) with a debounce, so files copied over the network appear by themselves; a progress line while a large scan runs. The scan must stay off the frame thread and check for cancellation.
- Decision: Owner, 2026-11-13: do it now (task C14). Built by C14: a `Rescan games` row reads every folder on a thread; each time the game list opens after a game, only the folders whose time changed are read, and only when a full scan ran in this power cycle; a notice gives the games added and removed. Later steps, not built: a `library rescan` control command; a watch of the folders (inotify) with its limits on exFAT and SMB; a progress count on the row; the home focus kept by system after a scan; a visible report of entries that cannot be read; removal of the unused `ScanWorker` type.

### OP-079: No way to copy games over the network.

- Raised by: the owner, on the first hardware session (2026-11-13)
- Status: closed by C13; the later steps are OP-092
- Default: the image has no SMB server, though `docs/definition` describes one. Games reach the share partition by moving the stick to another computer, or on the development image over SSH (a file is piped through `ssh`, because the SSH server has no SFTP).
- Where: `os/` (packages, init), `os/README.md`
- How to change: add Samba (or a small SMB server) with one guest share for `/share`, off by default on the release image or on with a setting; or add SFTP to the development image; and a `cargo xtask os put` command for the bench. A network service on the release image needs a look at its attack surface, and it needs the network (DHCP) on the release image, which today only the development image has.
- Decision: Owner, 2026-11-13: an SMB share in every image, development and release, on by default, as Batocera does (task C13). Built by C13 (`docs/spec/network-share.md`): the in-kernel server `ksmbd`, one share of `/share`, guest read-write, plus the published account `neptron` with password `neptron` and the guest's rights (owner, 2026-11-13: Windows 11 24H2 refuses guest sessions); wired DHCP in the background in every image; mDNS name `neptron.local`; the off switch is the file `/state/network-share-off`. Any host on the local network can write files that the scan and the cores then read as root; the spec states the exposure.

### OP-080: Variable refresh rate on flat panels.

- Raised by: the owner, on the first hardware session (2026-11-13)
- Status: open, after the MVP unless the owner asks sooner
- Default: on a flat panel the policy takes a fixed mode from the panel's list, and the audio rate control absorbs a small rate difference (59.923 Hz game on a 59.951 Hz mode). The owner's monitor (Dell U2724D, 2560x1440) reports a range of 48 to 120 Hz. NEPTRON does not use it.
- Where: `crates/neptron-backend` (the contract), `crates/neptron-kms` (the `VRR_ENABLED` CRTC property and the `vrr_capable` connector property), `crates/neptron-modeline/src/policy/panel.rs`, `crates/neptron-video/src/status.rs`
- How to change: a contract change with an ADR: the display reports its variable range; the panel policy takes a "variable" target when the game's rate is inside the range (frame doubling below it); the session presents at the game's own pace; the Video block gets a word for it. Check first what the GPU driver offers: the Intel driver is believed to support variable refresh only on newer generations than the HD 530 of the bench, which is not verified.
- Decision: Owner, 2026-11-13: after the MVP; stays open.

### OP-081: The menu and a game use different modes on a flat panel.

- Raised by: the orchestrator, from the log of the first hardware session (2026-11-13)
- Status: open, needs a look from the owner
- Default: on the Dell U2724D the menu runs at 2560x1440 and 120 Hz with each frame shown twice, and a 59.923 Hz game switches to the 59.951 Hz mode. So the panel changes mode when a game starts and ends, with a moment of black each time.
- Where: `crates/neptron-modeline/src/policy/panel.rs` (the order of the panel proposals), `apps/neptron` (`MENU_SOURCE`)
- How to change: prefer one mode for both when a listed rate is a whole multiple of the game's rate within the audio control's reach (120 Hz for 60 Hz games); or give the menu the game's mode.
- Decision: Owner, 2026-11-13: no change now; the mode change at a game start did not disturb on the bench panel. Stays open for a later look.


### OP-082: The quick menu shows rows that a flat panel ignores.

- Raised by: the orchestrator, from the owner's photos of the first hardware session (2026-11-13)
- Status: closed by C11
- Default: (state before task M08e) on a flat panel the quick menu showed "Picture style" and "Refresh policy" as active rows that had no effect. M08e makes the picture mode act on a flat panel and disables the refresh rate row there with a reason.
- Where: `crates/neptron-ui` (the quick menu), the settings schema (a condition on the display class)
- How to change: show these rows as disabled on a panel, with one line that gives the reason; or hide them. The schema needs a "applies to" field so the crawler can check it.
- Decision: each setting says on which display class it has an effect; a row that a flat panel ignores is disabled with the reason "Not used on a flat panel". The scanline rows stay active, because scanlines work on a panel.

### OP-083: The frontend starts before a monitor answers.

- Raised by: the orchestrator, from the log of the first hardware session (2026-11-13)
- Status: closed by C11; the hardware check is in `docs/hardware-checklist.md`
- Default: the log shows three starts that end with "cannot open the display: no connected display found" before the DisplayPort monitor is found. The frontend exits and the launcher starts it again, so the system recovers, but each failed start delays the first picture and counts toward the relaunch window.
- Where: `apps/neptron` (display open), `apps/neptron-launch` (the relaunch window), `crates/neptron-kms` (hotplug)
- How to change: at start, wait a bounded time for a connected connector (hotplug events, not a sleep) before the frontend reports the error. Do not count "no display" as a failed boot for the safe-mode counter.
- Decision: with output `auto` the frontend waits at most 15 s for a display that the connector rule accepts, then exits with code 69; the launcher restarts it after a fixed 1 s, counts no crash, and logs at most one line per minute. An analog connector with no EDID still needs a named output.

### OP-084: Small findings of the first hardware session.

- Raised by: the orchestrator, from the log and the state partition (2026-11-13)
- Status: closed by C11; point 3 for the Intel display firmware continues in OP-086
- Default: (1) Mesa prints at each start that it cannot create its shader cache, because the root file system is read-only. (2) The app wrote a game settings file that holds only an empty `[display]` table. (3) The kernel log reports missing firmware for the Intel display (DMC) and for the Realtek network chip (rtl8168h); both devices work without it.
- Where: `os/` (the service environment, the firmware packages), `crates/neptron-settings` (the writer)
- How to change: (1) set `MESA_SHADER_CACHE_DIR` to a directory on the state partition, or disable the cache. (2) Do not write a settings file with no value; remove the file when its last value goes. (3) Add the two firmware files, after a check of the `linux-firmware` licence terms for each.
- Decision: (1) the Mesa cache is on the state partition, or off when that partition is in RAM or read-only. (2) A settings file with no value is not written, and is removed when its last value goes, unless it held a comment or a newer schema version. (3) The Realtek firmware is in the development image only.

### OP-085: Audio settings: volume and output device.

- Raised by: the owner, after the first hardware session (2026-11-13)
- Status: closed by C12 (ADR 0011); the later steps are OP-087 and OP-088
- Default: `audio.volume` (percent, applied in software after the resampler) and `audio.output` (`auto` or the name of a playback device). A named device that is absent falls back to the default device with a notice, and the setting stays as it is. The image sets the hardware mixer to a known level at boot, so the software volume has the full range.
- Where: `docs/spec/audio-settings.md`, `crates/neptron-audio`, `crates/neptron-settings`, `crates/neptron-ui`, `apps/neptron`, `os/`
- How to change: later steps, not in C12: a volume hotkey in a game, a test tone on a device change, a volume per system or per game, a follow of the HDMI device when the monitor changes.
- Decision: the owner asked for volume and output device in the MVP (2026-11-13). As built (`docs/spec/audio-settings.md`, ADR 0011): `audio.volume` defaults to 80, because the boot sets the hardware mixer to 0 dB; 100 is bit-exact. The volume has the global, system and game levels. For ALSA, NEPTRON lists the playback PCMs itself (analog, S/PDIF, HDMI/DP) and stores a name such as `hw:CARD=PCH,DEV=3`. A new device takes effect at the next start of the frontend. `auto` takes the one HDMI/DP output with a monitor, else the first analog output, else the default device.

### OP-086: The Intel display firmware (DMC) is not in the image.

- Raised by: C11, from the kernel log of the first hardware session (2026-11-13)
- Status: decided
- Default: not added. The display works without it; only the low-power states of the display are lost. The i915 driver is built into the kernel and asks for the file before the root file system mounts, so a file in `/lib/firmware` does not help.
- Where: `os/board/pc` (kernel config), `os/board/pc/LICENCES.txt`
- How to change: (a) put the file inside the kernel image with `CONFIG_EXTRA_FIRMWARE`, which places an Intel binary inside the GPL-2.0 kernel image and needs a licence check; (b) build i915 as a module, which changes a recorded design choice; (c) keep it out.
- Decision: Owner, 2026-11-13: leave it out.

### OP-087: Which HDMI or DisplayPort audio output has the monitor.

- Raised by: C12 (2026-11-13)
- Status: open, a fix wanted before the MVP is called done
- Default: the files in `/proc/asound` do not map a monitor (ELD) to its PCM when a card has more HDMI/DP outputs than monitors. Then the list shows `HDMI/DP 1` to `n` with no monitor name, and `auto` takes the analog output. On the bench (one monitor, three outputs) the owner chooses `HDMI/DP 1` once.
- Where: `crates/neptron-audio/src/alsa.rs`, `docs/spec/audio-settings.md`
- How to change: read the `ELD` control of each PCM through `/dev/snd/controlC<N>` (an ioctl in the unsafe module of `neptron-audio`, with a new dependency such as `rustix`), or read the HDMI jack switches of the input devices. Both give the exact mapping. Then `auto` can follow the monitor in use.
- Decision: Owner, 2026-11-13: do it after the Raspberry Pi work.

### OP-088: Later steps of the audio settings.

- Raised by: C12 (2026-11-13)
- Status: open, after the MVP
- Default: not built: a volume hotkey in a game; a test tone when the output changes; a notice when the device is lost during a game (SDL's device-removed event); a `neptronctl` command that lists the playback devices (today the start log line lists them); Bluetooth audio; a sound server. The mixer step sets a fixed list of control names, so a card with other names keeps its reset state. The `sdl3` crate 0.22.1 can panic on a device name that is not UTF-8; only the window build on a desktop uses that path.
- Where: `docs/spec/audio-settings.md`, `os/board/pc/rootfs-overlay/etc/init.d/S08neptron-mixer`
- How to change: one task per step.
- Decision:

### OP-089: The shared cross-build cache gives stale results between worktrees.

- Raised by: C10 and C12 (2026-11-13)
- Status: open, a fix wanted soon
- Default: two caches can give a stale build with no warning.
  1. `cargo xtask os app-check` and `os push` mount every worktree at `/src/repo` and share one target folder in the volume `neptron-cross-<arch>`. When a different worktree built later than the source files of this one changed, cargo takes the old outputs as fresh, and the check fails or, worse, passes on code of a different branch. The workaround is to touch the source files first.
  2. A warm `cargo xtask os build` does not rebuild the NEPTRON packages when their source changed, so the image keeps an old frontend. The targets must be named by hand (`-- neptron-rebuild neptron-launch-rebuild neptronctl-rebuild neptron-capture-rebuild neptron-vkms-probe-rebuild`). On 2026-11-14 a Buildroot output kept an old `neptron-types` in the frontend until the source files were touched.
- Where: `xtask/src/os` (the cross build and the image build), OP-072 (the cache strategy)
- How to change: one target folder per worktree inside the volume (keyed by the worktree path or the branch), with the dependency outputs shared; or mount each worktree at its own path. `os build` adds the rebuild targets of the NEPTRON packages itself.
- Decision:

### OP-090: Settings under one home row, a submenu arrow and a breadcrumb.

- Raised by: the owner (2026-11-13)
- Status: closed by C15 (`docs/spec/ui-navigation.md`)
- Default: the question: the home screen listed each settings panel (Display, Audio) as its own row, a row that opens a page looked like a row that changes a value, and no screen said where the player is.
- Where: `crates/neptron-ui`, `apps/neptron`, `docs/flows/07-screens.md`
- How to change: C15: one `Settings` row on home that opens a page with Display, Audio and the other panels; an arrow at the right of every row that opens a page; a breadcrumb in the title bar; the same rules in the quick menu of a game.
- Decision: the owner (2026-11-13): in the MVP. Built by C15 as described under How to change.

### OP-091: `os build` fails to copy the images on a Linux host whose user is not uid 1000.

- Raised by: the CI work (2026-11-13)
- Status: open
- Default: the last step of `cargo xtask os build` copies the images as the container user (uid 1000) into the images directory of the host. On a Linux host with another uid the copy fails with "Permission denied" after a complete build. macOS is not affected. The CI workflow gives the directory to uid 1000 before the build.
- Where: `xtask/src/os` (the image copy), `.github/workflows/images.yml`
- How to change: run the container with the uid of the caller, or copy the images out with `docker cp`.
- Decision:

### OP-092: Later steps of the network share.

- Raised by: C13 (2026-11-13)
- Status: open, after the MVP unless the owner asks sooner
- Default: not built: a settings row "Network share" (the hook is the off switch file and `S45neptron-share restart`); WS-Discovery, so the machine shows in the Windows network view (needs a new package); Wi-Fi; shares for saves, states and config, which `docs/definition/10-os-layer.md` describes; DHCP for a USB network adapter plugged in after boot; a clean power-off on the release image. On Windows 11 a plain open of the share in Explorer probably shows an error and no prompt; the ways that work are in the spec. The check for `inet dhcp` in `os/board/common/post-build.sh` can never fail after the fixed write above it and can go.
- Where: `os/`, `docs/spec/network-share.md`, `crates/neptron-ui`
- How to change: one task per step.
- Built since: M03 (2026-11-16) shows the address, the host name and the state of the share in Settings > Network (`docs/spec/network-panel.md`). The panel shows the switch as text; the settings row for the switch is still a later step.
- Decision: Owner, 2026-11-13: a second share for saves and states is wanted (task C19, after the Raspberry Pi work); config stays local.

### OP-093: The control protocol does not give the kind of a UI row.

- Raised by: C15 (2026-11-13)
- Status: open
- Default: `neptronctl ui tree` shows no row kind (page, value, action, disabled). The crawler reads the kind in the process. Protocol version 2 exists (`PROTOCOL_VERSION`, `crates/neptron-ipc/src/envelope.rs`): it adds `fields_missed` to the stats reply, `Reply::for_version` removes a newer field from a reply to an older request, and `Client::call` falls back to the version of an older server. No `kind` field exists.
- Where: `crates/neptron-ipc`, `apps/neptronctl`
- How to change: a small contract branch with an ADR: the `kind` field in a new protocol version, removed by `Reply::for_version` for older requests. The `library rescan` command of OP-078 can go in the same change.
- Decision:

### OP-094: Reboot and shut down from the UI.

- Raised by: the owner (2026-11-13)
- Status: closed by C17 (`docs/spec/ui-navigation.md`)
- Default: the question: the UI could not reboot or power off the machine. Later steps, not built: a handler for the power button of a PC, the version and the network address in the panel, a sleep timer, a tier 3 check of a clean file system after a power-off.
- Where: `crates/neptron-ui` (a System panel under Settings), `apps/neptron`, `apps/neptron-launch`, `os/`
- How to change: C17: a `System` panel with `Reboot` and `Shut down`, each behind a confirm; the frontend ends cleanly, and the launcher runs the stop step and then the reboot or the power-off.
- Decision: the owner (2026-11-13): build it. Built by C17 as described under How to change.

### OP-095: The first boot did not attach the new share partition on real hardware.

- Raised by: the owner's bench, after a reflash over an older NEPTRON stick (2026-11-13)
- Status: closed by C16 and C18. The root cause is likely but not proven (a stick that rejects the cache flush); the next flash of the bench stick shows it in `/state/log/storage.log`. C18 adds the user-confirmed format for the states the script does not fix. OP-101 continues the limits
- Default: the first boot wrote partition 4 into the table, but the script stopped with no message after the partition tool reported an error, so the kernel did not see the partition and `/share` was a RAM folder until the next boot. The old share in that space, with its files, was not harmed. The boot checks did not catch it, because their disk has empty space there.
- Where: `os/board/common/rootfs-overlay/etc/init.d/S06neptron-storage`, `xtask/src/os/boot_check.rs`
- How to change: C16: judge the partition step by its result, attach the partition in the same boot, adopt an existing share as it is, no silent exit, messages to the system log, and a tier 3 case with an old share on the disk.
- Decision: the owner: the share partition must always work (2026-11-13).

### OP-096: The Raspberry Pi image: composite first, GPIO DACs later.

- Raised by: the owner (2026-11-13)
- Status: closed by P01 to P06: the spec (`docs/spec/raspberry-pi.md`), ADR 0012, the TV policy, the `TV mode` property in `neptron-kms`, the `rpi-arm64` image and the app wiring. P07 (`neptron.toml`), P08 (CI) and P09 (the bench session) are left. OP-097 to OP-103 hold the points that the first tests raised
- Default: the owner's decisions for the first image: composite video is the default output, NTSC at 60 Hz, and the user can change the standard (PAL at 50 Hz, PAL at 60 Hz and the others the hardware gives); the picture must be as sharp as composite allows; it must work on a 15 kHz tube through composite. The GPIO DACs (VGA666, RGB-Pi, Pi2SCART) are not in the first image; they need a line in the boot configuration on the card, as `docs/definition/05-crt-safety-model.md` describes.
- Where: `docs/spec/raspberry-pi.md`, `os/` (a new board), `crates/neptron-modeline` and `crates/neptron-video` (an output with fixed timings), `data/hardware/matrix.toml`
- How to change: P01 writes the spec and the plan; the build tasks follow it.
- Decision: the owner asked for it after the PC round (2026-11-13). Owner, 2026-11-13: the test board is a Pi 3 B+ (the tests of 2026-11-14 ran on a Pi 3 B); HDMI on the Pi is in the first image as a flat panel; the default standard stays NTSC; the default width rule is `aspect`, because the picture must have the right size and shape; the names NTSC and PAL are used only for composite, other outputs show lines and rates.

### OP-097: Picture size and position on a TV-class CRT (overscan).

- Raised by: the owner (2026-11-13)
- Status: open, a design is wanted before the MVP is called done
- Default: a CRT TV hides part of the picture at its edges, on composite and on RGB alike (a PC monitor does not). The only allowance is the 5 % safe area of the UI theme, on every output. On composite the menu is 640 of the 720 samples wide by design (the UI rectangle of `policy/tv.rs`), which keeps its text 10 % from the side edges; the top and the bottom have only the theme's margin. A game is placed as its console placed it. No setting moves or sizes the picture. The first test on a tube (Samsung, composite, Pi 3 B, 2026-11-14): the game picture reached the edges of the visible area with correct geometry; the menu shows a black band at each side (OP-102 point 1).
- Where: `crates/neptron-ui` (the theme's safe area), `crates/neptron-modeline` (on RGB the porches of the modeline set the size and the position at no cost in sharpness; on composite only the placement inside the fixed 720 samples can change), the settings schema
- How to change: one setting for the menu's safe area on a 15 kHz profile, and picture position and size settings: through the timings on RGB, through the placement on composite. Check on more tubes what is cut.
- Decision:

### OP-098: A TV standard that follows the game on composite.

- Raised by: the Raspberry Pi spec (2026-11-13)
- Status: open, after the first composite image
- Default: the TV standard is one setting; a 50 Hz game on a 60 Hz standard runs with the speed alert, and the reverse. The signal does not change by itself.
- Where: `crates/neptron-video` (the guard), `crates/neptron-modeline/src/policy/tv.rs`, the settings
- How to change: a second standard for 50 Hz games that the player has confirmed once on this TV; the guard then allows the change between the two confirmed standards with no prompt. It needs its own safety rule and an ADR.
- Decision: the owner: keep it as an open point (2026-11-13).

### OP-099: A picture width per system on composite.

- Raised by: the review of the TV policy (2026-11-13)
- Status: open
- Default: a 4:3 console picture of 224 to 240 lines gets the width that the common consoles had (644 samples at 525 lines). The Neo Geo draws its 320 pixels at 6 MHz, which is 720 samples, so it comes out about 11 % narrow.
- Where: `data/systems`, `crates/neptron-modeline/src/policy/tv/width.rs`
- How to change: a pixel clock or an active picture time per system in the systems data, used by the width rule.
- Decision:


### OP-100: Keep the video rules few, and check them against a real library.

- Raised by: the owner (2026-11-13)
- Status: open; the owner accepts the rules as they are today
- Default: the project has three independent rule sets, one for each kind of display (CRT, flat panel, TV), and no rule for a single game. Each rule works from the geometry and the rate that the core reports. The last proposal of every plan always fits, so a game always gets a mode. The points that carry risk:
  1. The width rule for composite has the most special cases: a general rule, a console case for 4:3 pictures of 224 to 240 lines, a snap to whole and half factors, and a 3 % tolerance. On the first tube test (2026-11-14) the game picture reached the edges of the visible area with correct geometry; one TV and a few games are not a proof for every source.
  2. The session takes a source of 289 lines or more as interlaced. This is a guess from the line count.
  3. Thresholds change the behaviour at their edges: 224 to 240 lines, 3 %, at most 8 lines of crop per edge, 5000 ppm.
  4. A fact of one system (the Neo Geo's wider picture, OP-099) pulls toward a rule per system.
  5. The sweeps use made-up sources. They show that nothing fails and that each source gets a mode; they do not show that each real game looks right.
- Where: `crates/neptron-modeline/src/policy` (`crt.rs`, `panel.rs`, `tv.rs`, `tv/width.rs`), `crates/neptron-session` (`INTERLACE_MIN_LINES`), `data/systems`, `docs/plan/10-code-standards.md`
- How to change:
  1. Run the policy on a list of real machines (the size, rate and shape that MAME and the cores publish), for each kind of display, and read the outliers.
  2. Add a rule to the code standards: no rule names a game or a system; a fact of a system goes into the systems data; a new threshold has a test on each side and falls back to the general rule.
  3. Look at the composite width rule again after more tests on tubes; if the general rule looks right, remove the console case.
  4. Replace the 289-line guess with the interlace report of the core.
- Decision: the owner: it is acceptable as it is; keep these points open (2026-11-13).

### OP-101: An old share at another place, and the other limits of the format checks.

- Raised by: C18 (fourth review, 2026-11-14)
- Status: open
- Default: the first boot protects an older exFAT share that it finds in the scan window. When that share does not start at the planned start of partition 4, the first boot makes no partition and formats nothing. The data is safe, but the run has no share, and the UI gives no way out. Example: the PC image written on a stick that held the Pi image, whose share starts later. The other limits:
  1. The scan finds only a share with a good main boot sector. A damaged old share at another start is not seen, and the format of the new partition 4 can erase its head.
  2. A request does not write its scan result to the cache. After a refused request, a stale cache can show the offer again. No format follows.
  3. When a request is refused for a read error and a marker exists, the marker rules format in the same boot. The status then says that the format was refused.
  4. A signature that blkid finds exactly at the planned start, from an older layout, gives no share and no offer.
  5. No test covers a read error on the request path with a cached result, an NTFS header with a sector size of 0, or a FAT32 header with other hidden sectors.
- Where: `os/board/common/rootfs-overlay/etc/init.d/S06neptron-storage`, `xtask/src/os/storage_script.rs`, `os/README.md` (storage)
- How to change:
  1. Add a row that adopts an old share where it lies: partition 4 is made at the start of the old share, after the user confirms. This closes the main case.
  2. Check the backup boot region (sector + 12) at each scan step.
  3. Let the request write the cache, or remove the cache when a request is refused.
  4. Make no marker format in a boot that refused a request.
  5. Add the missing tests.
- Decision:

### OP-102: First composite picture on a Pi 3 B, and what it showed.

- Raised by: the owner's first test on a tube (Samsung, composite, Pi 3 B, 2026-11-14)
- Status: open for points 1 and 2; point 3 is closed by commit `fd713d9`
- Default: the owner reports on 2026-11-15, with the image of commit `7f2e31e` on the Pi 3 B: SNES and Mega Drive games on composite are very sharp, and the audio is fine. Earlier on 2026-11-14: the picture locks in NTSC 240p, the menu is sharp, and the grid pattern of the 240p test suite fills the tube with correct geometry. The points:
  1. The menu is 640 samples wide in the 720 samples of the line, so the tube shows a black band at each side of the menu. This is the UI rectangle of `policy/tv.rs` by design, not the TV. A wider menu needs the overscan decision of OP-097.
  2. The audio list labels the PCM of the `vc4-hdmi` card "Analog", because the kind of a PCM comes from its name only, and its name (`MAI PCM i2s-hifi-0`) has no HDMI word (`crates/neptron-audio/src/alsa.rs`). With no jack card, `auto` then took the HDMI card as an analog output, and the open failed with error 524 while HDMI was off.
  3. `dtparam=audio=on` after the overlay line gave no jack card. `os/board/rpi/config.txt` puts it before the overlay line, and a test pins the order.
- Where: `crates/neptron-modeline/src/policy/tv.rs` (UI rectangle), `crates/neptron-audio` (the output list and its labels), `os/board/rpi/config.txt`
- How to change: for 1, a setting for the menu width on a TV, or the result of OP-097. For 2, take the kind of a PCM from its card driver (`vc4-hdmi` is HDMI) as well as from its name, and do not offer an HDMI output while its connector is off.
- Decision:

### OP-103: Frame budget on the Raspberry Pi 3, and what still costs fields.

- Raised by: P10 and P11 (measured on a Pi 3 B, composite, 2026-11-14)
- Status: open
- Default: NES, SNES and Mega Drive games hold the field rate with no missed field. The CPU governor of the Pi image is `performance`. The points that stay open:
  1. Fixed by P12. The dialog over a running game drew the whole UI canvas again in every frame (5.5 ms of CPU for the rectangle fills on the A53), and drew the game quad under the opaque canvas (GPU 4.9 ms instead of 3.4 ms). The render ended 16.7 ms after the flip event, so on NTSC the flip phase held at every second field. Now the canvas fill uses memory copies, the canvas is drawn only when its scene changes, the GPU gets only the changed canvas rows, and the game is drawn only outside the UI. A dialog frame ends 8.4 ms after the flip event (a game frame alone: 9.9 ms). Measured: 0 missed fields in 36 NTSC dialogs over the SNES 240p Suite. Still open: an unchanged canvas costs a comparison of its rows in the renderer (about 0.35 ms a frame), because the `Display` contract does not say that the UI did not change. A UI generation number in `Scene` would remove it (a contract change, so an ADR).
  2. A change of the TV standard takes about 75 ms from the last flip of the old mode to the first flip of the new one. The muted audio queue covers about 32 ms, so it runs dry in silence and the underrun count goes up by one.
  3. Super Mario World leaves 1.66 ms in its worst frame with the development build (`os push`, no LTO). The release image is not measured yet.
  4. The source texture grows at the first frame of a larger size, because the `Display` contract does not carry the maximum geometry of the core. A game that changes its size in play makes one allocation at that moment.
  5. The DRM vblank sequence of vc4 in 480i is not checked: if it counts frames and not fields, `fields_missed` hides real misses in interlace.
  6. `neptronctl screenshot` answers `unavailable` on the image, because the build has no PNG encoder.
- Where: `crates/neptron-session` (frame loop), `crates/neptron-ui` and `apps/neptron` (the dialog path), `crates/neptron-audio` (mute cover), `crates/neptron-gfx/src/gles`, `os/board/rpi/system.config`
- How to change: for 1, add a UI generation number to `Scene` so the renderer skips the row comparison (an ADR). For 2, a longer muted cover, which adds delay after a short modeset. For 3, measure with the release image. For 4, pass the maximum geometry through the contract (an ADR). For 5, the hardware check in the checklist.
- Decision:

### OP-104: What the CI does not cover.

- Raised by: the CI task (2026-11-14)
- Status: open
- Default: the runners for arm64 have no KVM, so the aarch64 guest runs under emulation, and the checks that measure time cannot pass there. `kms-check`, `selftest-check` and `core-trace` run in CI on x86_64 under KVM only. For aarch64 they run on a developer machine with a hypervisor. The Raspberry Pi image is built and its layout is checked, but no guest boots it. Two paths of the workflow are not proven: the resume of a build from a partial base, and the downloads cache, which only a run on `main` saves. The check `ci-ok` is not yet a required check on `main`; the owner sets it. The storage cost of the base artifacts (about 5 to 9 GB each) on a public repository is not confirmed.
- Where: `.github/image-targets.json`, `.github/workflows/images.yml`, `docs/ci.md`
- How to change: add a self-hosted arm64 runner with KVM and add `kms`, `selftest` and `core` to the aarch64 entry of `.github/image-targets.json`. Force a partial base once and read the run. Read the billing page after some days.
- Decision: the owner: the three timing checks run on x86_64 only (2026-11-14).

### OP-105: The keep or revert dialog of the display behaves wrongly.

- Raised by: the owner, on the Raspberry Pi 3 with a composite TV (2026-11-15)
- Status: closed by M01 (2026-11-16): the spec `docs/spec/display-guard.md` and ADR 0013. The owner accepted trust by display class (an EDID flat panel and a composite TV get no boot prompt), a 15 s window from the first settled picture, a failed boot only for a run that ends abnormally before its first picture, one record per place, and two rows to leave the safe mode. The causes found: the boot prompt asked to confirm the safe default itself, a return from a game was a new boot, and the record held one place only. The hardware checks are in `docs/hardware-checklist.md`
- Default: the dialog stays as it is. The owner saw three faults:
  1. The dialog shows in too many places.
  2. After the settings were kept, the dialog shows again at the next restart.
  3. A press of Keep sometimes gives the notice that the settings were reverted, although they were kept.
  Known facts that can be part of the cause: the first boot of an image has nothing confirmed, so the guard opens the boot prompt (OP-076); a frontend that restarts several times goes to the safe mode for failed boots; on the Pi the record of the guard is kept per connector. None of these is proven as the cause.
- Where: `crates/neptron-video` (the guard and its record), `apps/neptron/src/shell` (the boot prompt, the dialog, the notices), `docs/spec`
- How to change: reproduce each fault on the Pi with the log of the guard, write the rules for when a prompt is needed (a mode that the display never confirmed) and when it is not (a kept mode at a restart), and agree them with the owner in a spec before the code.
- Decision:

### OP-106: Open questions of the arcade spec.

- Raised by: M06 (2026-11-16)
- Status: open; each question has a default that the build tasks use
- Default: the questions A1 to A6 of `docs/spec/arcade.md`, section 19: the XML reader for the DAT in xtask (`roxmltree`, xtask only), how the image build runs the index generator (a hook of the package), which Neo Geo BIOS files and optional ROMs are mandatory (block only when no BIOS file matches), where one-time notices are stored (`state/config/notices.toml`), PAL composite runs arcade boards 9 to 16 % slow (accepted with the speed alert), and the shape of the TATE setting (decided by the TATE task). The owner decided on 2026-11-16: FinalBurn Neo on every board, no donations, 480i as the default for a vertical game on a horizontal 15 kHz CRT, the speed change on composite accepted. Measured on the Pi 3 B on 2026-11-16 (aarch64, composite NTSC 240p): DoDonPachi and Metal Slug X run with 0 missed fields and 0 underruns, worst field 11.83 ms of 16.65 ms; late stages and HDMI are not measured. TATE (the monitor on its side with a turned UI) is after the MVP and important to the owner.
- Where: `docs/spec/arcade.md`, ADR 0014, 0015 and 0016
- How to change: each build task (M07a to M08b) closes its questions in its report.
- Decision:

### OP-107: TATE mode: a monitor on its side, with the UI and the games turned.

- Raised by: the owner (2026-11-16); important to him, planned after the MVP
- Status: open; not on the taskboard of the MVP, which holds only the tasks of version 0.1
- Default: version 0.1 shows a vertical game upright on a horizontal display (rotation in the renderer, task M08a). It has no mode for a monitor that stands on its side: the menu is not turned, and a vertical game is not shown at its native lines on such a monitor.
- Where: `docs/spec/arcade.md` (the TATE section has the design notes), ADR 0015, `docs/definition/06-flat-panels-shaders-and-vertical-games.md` (`screen_orientation`)
- How to change: a task of the first taskboard after the MVP. It covers a setting for the screen orientation with a direction (left or right), the UI drawn turned, a vertical game at its native lines with no rotation (240p at the game's own rate on 15 kHz), a horizontal game turned or refused on such a screen, the input directions turned in the menu, and the boot picture. The setting shape is question A6 of the arcade spec (OP-106).
- Decision: Owner, 2026-11-19: TATE is the first task after version 0.1. One system setting gives the screen orientation: 0 (YOKO), 90, 180 or 270 degrees, and the UI follows it. In Auto a vertical game runs at its own lines, full screen; a horizontal game shows upright and small, with a notice. Each game has an orientation override (Auto, 0, 90, 180, 270) in its display settings.

### OP-108: Arcade leftovers after the library tasks (M07a, M07b, M07c).

- Raised by: M07b, M07c and their reviews (2026-11-16, 2026-11-17)
- Status: open
- Default: no diagnostics page exists, so the set check writes the list of wrong files to the log, and no line names a `mame` folder, an unknown set or the state of the index. `data/cores/fbneo.toml` does not set `fbneo-diagnostic-input`, because the free test set does not declare that option; the service menu stays on the core default, hold Start. The session does not read `always_progressive` yet (M08a). A launch through the control protocol skips the set check. A local build after a new pin of the core keeps the old index until the `neptron` package is built again (CI always does); the image check proves only that the index exists. On a boot with the state folder in RAM, the warning about it comes again in each game run. No x86_64 trace of FinalBurn Neo and no trace of a set with a missing ROM exist. The test of the Back key in the versions list asserts only the screen name. The texts of messages 4, 4b and 5 of the set check are the owner's (2026-11-17) and are in `docs/spec/arcade.md` section 10.
- Where: `apps/neptron`, `crates/neptron-ui`, `crates/neptron-library`, `crates/neptron-session`, `data/cores/fbneo.toml`, `os/package/neptron`, `tests/traces`
- How to change: a diagnostics page as its own task; M08a for `always_progressive`; a trace of a Neo Geo set that declares the diagnostic input before the key goes in the data file; a version compare in `check-arcade-index.sh`.
- Decision:

### OP-109: Video policy: questions for the owner and leftovers of the policy task.

- Raised by: the policy task of M08 and its reviews (2026-11-16, 2026-11-17)
- Status: open
- Default: the built result stays until the owner decides. V6: an exact rung at a slower rate that changes the speed more than the fit (DOS 640x400 on a TV) keeps the fit. V7: WonderSwan stays at 1x where 2x would squeeze it by 5 %. V8: a game wider than the face with too few lines (Virtual Boy on `ntsc`, a 3:2 source of 224 lines on a 15 kHz CRT) goes to interlaced 1:1 at about 50 % of the height. The overscan crop is bound by 8 source lines and by 8 of 240 lines of the screen per edge. The fill rule of a flat panel applies to every source, not only to a vertical game. The app sets `PolicySettings::arcade` for no system yet, so `arcade` and `neogeo` use the console rules until M08a. The app does not show the new notice reasons yet. Shaders for flat panels would replace the hard scanlines; they are planned after the MVP. Low findings of the last review: the continuity exception of the sweep covers every plan that has a window rung; `onto_one_field_rate` does not try the GPU floor for a wider multiple before the validator; the diagnostic of `interlaced_rung` gives the source lines, not the shaped lines.
- Where: `crates/neptron-modeline`, `crates/neptron-video`, `apps/neptron`, `docs/spec/video-policy-cases.md`, `docs/spec/arcade.md` sections 11 and 12
- How to change: an owner decision per question, then a golden change with its reason; M08a sets the arcade flag and shows the notice reasons.
- Decision: Owner, 2026-11-16: with scanlines on, a flat panel uses the largest whole multiple and never the fill; every picture that is not exact shows a notice. Owner, 2026-11-17: V6 takes the exact lines (the slower exact rung wins over the fit); V7 takes 2x with the squeeze of 5 %; V8 stays interlaced 1:1; the crop bound stays scaled to the screen; the fill of a flat panel stays for every source. V6 and V7 change the policy and its goldens: task M08d.

### OP-110: Input for four players: owner decisions and leftovers.

- Raised by: M04 (2026-11-17)
- Status: open
- Default: a pad gets its port by plug order, and a replug in the menus changes the order. Every pad and every keyboard controls the menus. Pads 5 to 8 drive only the menus and the hotkeys, and a ninth pad is refused until it is plugged again. Every port is a plain joypad: no multitap and no Four Score, so a game that needs one sees two pads. Every keyboard is player 1. Two pads of one model that both come back at new paths in a game can swap ports. The service menu of FinalBurn Neo stays on the core default (OP-108). M04b (2026-11-17) wires the notices of the input backend to the log and to the player. Left by M04b: the mapping file `data/input/gamecontrollerdb.txt` has no entry, and the community SDL_GameControllerDB is not shipped, because its entries carry no licence that can be cited (a licence question for the owner); a generic pad with fewer than 10 buttons has no Start until M05; a second pad that nobody touches gets the silent pad notice once per power cycle; a pad notice that is pushed out of a full notice list does not come back in that run; `usbhid.jspoll=1` is not on the kernel command line. Since 2026-11-17 the image ships the community gamepad database (ADR 0017, owner decision), and M05 gives the guided mapping and the stored layouts (`config/input/pads.toml`). Left by M05: no remapping during a game (the page is read only there); an input used twice is refused, not swapped; two pads of one model with an empty path share a key; the crawler does not reach the review dialog; no per-game or per-system layout (the file leaves room); nothing of the input work of this day ran on hardware.
- Where: `docs/spec/input-players.md`, `crates/neptron-input`, `apps/neptron`
- How to change: task M05b for the picker; a task of its own for four-player adapters, with the device type per system in `data/systems`; M04b wires the input notices to the log.
- Decision: Owner, 2026-11-17: a way in Settings > Input to pick the pad of each player is wanted, with a good UI (task M05b). Every pad controls the menus. More than four pads as built, for now. Four-player adapters stay open. The service menu key keeps the core default.

### OP-111: Findings on the PC of 2026-11-17.

- Raised by: the bench session on the i3-6100T (2026-11-17)
- Status: open
- Default: the PC image booted with the `powersave` governor and FinalBurn Neo missed fields (42 frames per second against 60.2); the kernel fragment `os/board/pc/system.config` now sets `performance`, and the result on the PC is a line of the hardware checklist. A Mad Catz FightStick Tournament Edition for PS3 (`0738:8838`) enumerates on a direct port of the PC and sends only neutral reports; it works on a Mac behind a USB hub. The PS3 start request (feature report, reply `21 26 01 07`), the two Sony requests and an LED output report do not change that. The cause is the default enumeration scheme of Linux: its first read of 64 bytes at address 0 leaves the firmware of the stick in a state with no button data until the stick loses power. With `usbcore.old_scheme_first=1` the stick gives input in each trial that starts with a physical replug (owner trials on the PC, 2026-11-18); the images now boot with that parameter, see OP-112. A test that enumerates a device again without a cut of its power does not show this. A cold build of the PC image with the 15 kHz patch set stopped at "No hash found for 01_linux_15khz.patch" in the kernel headers step: the linux-headers package checks the remote kernel patches against its own hash file only. M12 made `linux-headers.hash` a link to `linux.hash`; the hash check step passes in an empty output, and the first CI run with the patched image is the proof of the whole cold build. The base key of the CI image cache now hashes the kernel and BusyBox configuration fragments of each target. The image builds SDL3 without HIDAPI and without libudev.
- Where: `os/board/pc`, `os/patches/linux`, `.github/workflows/images.yml`, `crates/neptron-input`
- How to change: SDL3 with HIDAPI and libudev is a build choice of the image.
- Decision: Owner, 2026-11-17: NEPTRON's own driver code for third-party PS3 pads is allowed if a host-side cause is found.

### OP-112: A setting to turn off the old USB enumeration scheme.

- Raised by: the fix of the Mad Catz PS3 FightStick (`0738:8838`) on the PC (2026-11-18)
- Status: open
- Default: every player image boots with `usbcore.old_scheme_first=1`, and there is no setting. `use_both_schemes` keeps its default, so a device that refuses the old scheme enumerates with the other scheme after a short delay. The owner saw XInput pads work with the parameter on. A controller that enumerates with the old scheme and then works badly has no remedy in the UI.
- Where: `os/board/pc/grub.cfg`, `os/board/rpi/cmdline.txt`; a setting belongs in Settings > Input, global layer only, with a backend that writes `Y` or `N` to `/sys/module/usbcore/parameters/old_scheme_first` at the start of the session and at a change.
- How to change: add the setting (schema entry, backend write with a test against a temporary directory). The value applies to a controller that is plugged in after the write. The setting cannot cover a controller that is plugged in at power-on without a write to the boot partition.
- Decision: Owner, 2026-11-18: ship the parameter now and keep the setting as an open point.

### OP-113: Leftovers of the start failure and quick menu work of 2026-11-18.

- Raised by: the test of Killer Instinct in a QEMU guest and the reviews of the two tasks (2026-11-18)
- Status: open
- Default: `cargo xtask os push --data` replaces the data folder of the hot folder with `monitors`, `cores`, `systems`, `skins` and `hardware` only. The arcade index exists only in an image build and `input/` is not in the list, so an app pushed with `--data` runs with no arcade index (no set check, file names in the list) and with no controller database. A push without `--data` keeps the data of the image. The notice of a game that did not start is written to the log two times, by the game run and by the menu run. A set with a disk image passes the set check when the file is present; NEPTRON does not read it, and no game with a disk image is a tested board family: in the guest, FinalBurn Neo ran Killer Instinct for 5 minutes on a file of zero bytes with no stop. In a game, a blind recovery hold opens the quick menu at 0.5 s and the recovery fires at 5 s under it; the owner was told. The hold time of the quick menu is a constant.
- Where: `xtask/src/os` (`DATA_DIRS`), `apps/neptron/src/start_failure.rs`, `crates/neptron-library`, `crates/neptron-input/src/hotkeys.rs`
- How to change: M11 for the data push (build the index in the push, add `input/`); a SHA-1 check of a disk image as a diagnostics job; a setting for the hold time if the owner wants one.
- Decision: Owner, 2026-11-18: the texts of message 8 and of the notices S1, S2 and S3 are approved; the quick menu opens after a hold of 0.5 seconds.

### OP-114: Decisions and findings of 2026-11-19.

- Raised by: the owner and the bench session on the i3-6100T (2026-11-19)
- Status: open
- Default: (1) M05b, the picker of the pad of each player, is dropped: the plug order stays. (2) The kms-check of the M12 gate did not run on the development Mac, because the local release image of 2026-11-13 is older than the test tooling and its guest does not mount the test folder. The first CI run with the 15 kHz patch set (commit `62029d3e`) built the PC image from an empty output and passed the patch check, the boot with BIOS and with UEFI, kms-check, the GLES comparison, the video self-test, the core trace and the share check; it uploaded the image and its sources. So the gate of M12 is met and the cold build fault of OP-111 is proven fixed. (3) Killer Instinct (FinalBurn Neo, a MIPS interpreter with no recompiler at the pinned commit) needs about 1.0 to 1.5 field periods for one frame on the i3-6100T at 3.2 GHz. Before task M16 the game ran at 30 to 38 frames per second with 9 to 13 audio underruns per second; the first build of M16 gave 48 to 50 frames per second on average, 40 in the heavy scenes, with under 1 underrun per second. The bench image of that day was older than the governor fix, so the PC ran at 2.7 GHz until the governor was set by hand. (4) `data/hardware/matrix.toml` has no entry for the HDMI output of the Raspberry Pi; the flat panel path checks the panel's own mode list, so the effect is small. (5) A setting file can still name `display.vertical` and `display.panel_scale`; the menus no longer show them after M08e.
- Where: `docs/taskboard.toml`, `.github/workflows/images.yml`, `crates/neptron-session`, `data/hardware/matrix.toml`
- How to change: a new release image for the local tier 3 commands; M16 for the pacing; an entry for the Pi HDMI output when its limits are measured on the bench.
- Decision: Owner, 2026-11-19: the notice texts of M16 are approved ("This game runs at {p}% speed. This machine is too slow for it." and "Sound off: the game runs at less than half speed."); when Auto finds no picture for a game, the Fill picture is the default (built in M08e).

### OP-115: Decisions and findings of 2026-11-23.

- Raised by: the measurement of M16 on the bench PC and the closing review of M08e (2026-11-23)
- Status: open
- Default: (1) M16 is merged at the build that the owner tested. On the i3-6100T at 3.2 GHz, Killer Instinct in its attract mode gives 58.6 frames per second over 120 s with 165 missed fields and 27 audio underruns. A later round restored a bound of 4 underruns in the test `a_fast_core_with_a_spike_every_10_frames_is_measured` (the merged test allows 15) and changed the speed measure at the start of a heavy scene. On the same run it gave 48.3 frames per second with 1402 missed fields and 76 underruns, and the simulation did not show the loss. That round is not merged. The few audio underruns of a heavy scene stay. (2) The picture answers of a menu page are made at 2 decisions per frame. One decision that falls back to Fill on a 31 kHz CRT walks three plans; the estimate for a Pi 3 from the dev profile is 11 ms for that decision, which is more than a frame can give. The work runs only while a menu shows, never during play. (3) The screensaver of M15 is a black screen with the NEPTRON wordmark that moves and takes the next colour of a fixed palette at each wall.
- Where: `crates/neptron-session/src/pacing.rs`, `crates/neptron-session/src/speed.rs`, `crates/neptron-video/src/answers.rs`, `docs/hardware-checklist.md`
- How to change: for (1), first give the fake audio sink the period and the blocking of the real device so the loss shows in a test, then take up the speed measure again; for (2), measure `policy_cost` on a Pi 3, and if needed give the step a budget in plans and make the fallback of `decide_shown` resumable.
- Decision: Owner, 2026-11-23: merge the tested build of M16 and drop the last round; the screensaver starts after 5 minutes in the menu, a paused game and the quick menu, never in a running game, with the settings On or Off and 1, 5, 10 or 30 minutes.
