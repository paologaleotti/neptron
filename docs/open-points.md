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
- Decision: <the decision, its date and a link to its ADR; left out while there is none>
```

A closed entry is short: its Status line names what closed it, one line gives the answer or the decision, and one line gives where.

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
| OP-015 | Real EDID of the SyncMaster 750s | Bench check |
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
| OP-047 | Limits of the ODREN policy | Aspect, fixed-clock CRT |
| OP-049 | Screen behaviour for the owner to confirm | Defaults as built |
| OP-051 | No help text field in the schema | Help is in doc comments |
| OP-055 | Real core reports and full-picture options | Options set; launch settle |
| OP-057 | Crop at the bound on arcade boards | Known limit |
| OP-060 | Smaller points of the KMS backend | Low priority |
| OP-062 | Engine behaviour around the blank | As built |
| OP-063 | Timing check 6 with late flips | Weaker than the plan text |
| OP-064 | Session choices for the app | As built |
| OP-065 | Guard choices | Points 1 to 3 for the owner |
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
| OP-091 | `os build` image copy on a Linux host not uid 1000 | Fails |
| OP-092 | Later steps of the network share | Second share in C19 |
| OP-093 | No row kind in the control protocol | Protocol 3 exists, no `kind` field |
| OP-097 | Picture size and position on a TV-class CRT | First tube test done; no setting |
| OP-098 | A TV standard that follows the game | After the first composite image |
| OP-099 | A picture width per system on composite | Neo Geo 11 % narrow |
| OP-100 | Keep the video rules few | Accepted as built; checks to do |
| OP-101 | An old share at another place | No share, data safe |
| OP-102 | First composite picture: open points | Menu width, after 0.1 |
| OP-103 | Frame budget on the Pi 3 | Dialog rate fixed; standard change underruns |
| OP-104 | CI limits | aarch64 timing checks not in CI; two paths not proven |
| OP-106 | Open questions of the arcade spec | Defaults in `docs/spec/arcade.md`, section 19; A1, A2 built; A6 in OP-107 |
| OP-107 | TATE: a monitor on its side, with the UI and the games turned | After the MVP; design notes in the arcade spec |
| OP-108 | Arcade leftovers after M07 | No diagnostics page; core default for the service menu |
| OP-109 | Video policy: questions for the owner and leftovers | Owner decisions built; low review findings left |
| OP-110 | Input for four players: owner decisions and leftovers | Plug order (M05b dropped); no multitap |
| OP-111 | PC findings of 2026-11-17 | Governor fixed; the PS3 arcade stick works with the old USB enumeration scheme; cold build fixed |
| OP-112 | A setting to turn off the old USB enumeration scheme | The images boot with `usbcore.old_scheme_first=1`; no setting yet |
| OP-113 | Leftovers of the start failure and quick menu work of 2026-11-18 | Start failure logged twice; no game with a disk image is tested |
| OP-114 | Decisions and findings of 2026-11-19 | M05b dropped; M12's gate met in CI; Killer Instinct is slow on the bench PC; the Pi has no HDMI entry in the hardware matrix |
| OP-115 | Decisions and findings of 2026-11-23 | The last sound round of M16 is dropped; the cost of the picture answers in a menu has no limit per frame; the screensaver is a bouncing wordmark |
| OP-116 | Release review of 2026-11-23 | Six fix tasks before 0.1; the list of what waits until after 0.1 |
| OP-117 | Decisions of 2026-11-24 | 0.1.0 is an internal milestone; the public release is 1.0; the bench monitor is a SyncMaster 750s |

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
| OP-096 | The Raspberry Pi image | P01 to P07; P09 left |
| OP-076 | Boot prompt when nothing would change | M01: no prompt for the safe default |
| OP-105 | The keep or revert dialog of the display | M01, ADR 0013 |
| OP-089 | Stale cross-build cache between worktrees | M11 |

## List

### OP-001: Which dependency kinds do the modeline and safety dependency rules restrict?

- Raised by: T01
- Status: open
- Default: normal dependencies only. Build dependencies (`cc`) and test dependencies stay allowed.
- Where: `xtask/src/rules.rs`
- How to change: add the other dependency kinds to the rule.

### OP-002: Which licences does `cargo-deny` allow, and do advisories run in T0?

- Raised by: T01
- Status: open
- Default: permissive licences only. The advisory check is not in T0, because it needs the network.
- Where: `deny.toml`, `xtask/src/verify.rs`
- How to change: edit the allow-list; add an advisory step to a tier that can use the network.

### OP-003: Does the macOS CI job run under the machine-load limits?

- Raised by: T01
- Status: open
- Default: yes. `xtask` cannot tell a CI runner from a local machine.
- Where: `.github/workflows/ci.yml`, `xtask/src/load.rs`
- How to change: set `NEPTRON_BUILD_SLOTS` and `NEPTRON_BUILD_JOBS` in the CI job.

### OP-004: Are the tier time budgets enforced?

- Raised by: T01
- Status: open
- Default: no. `verify` reports the time against the budget and does not fail on it.
- Where: `xtask/src/verify.rs`
- How to change: make a step over budget a failure.

### OP-005: The CI workflow was never run.

- Status: closed by the CI task. The workflow runs on every push of `main`; the first green run is 43637921848 of 2026-11-14 (commit `4b9f3c9`). OP-104 holds what CI does not cover.
- Where: `.github/workflows/ci.yml`, `.github/workflows/images.yml`, `.github/image-targets.json`, `docs/ci.md` (jobs, caches, times)

### OP-006: The forbid check can be fooled by a char literal inside an attribute.

- Raised by: T01 review
- Status: open
- Default: accepted. Only a file made on purpose can do this.
- Where: `xtask/src/rules.rs`, `bracket_body`
- How to change: handle char literals and comments inside attributes.

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
- How to change: produce them through Switchres.

### OP-009: `InputEvent` has no hotplug event and no device that is not a RetroPad.

- Raised by: T02
- Status: open
- Default: absent. A later addition does not change the existing wire format.
- Where: `crates/neptron-types/src/event.rs`
- How to change: add variants.

### OP-010: Is the UI canvas transparent over the game?

- Raised by: T06
- Status: open
- Default: opaque. The X byte of the canvas is ignored, so the quick menu hides the game where it is drawn.
- Where: `crates/neptron-gfx/src/reference/overlay.rs`
- How to change: a contract change (an alpha or colour-key rule for the canvas), with an ADR, then the overlay, its tests and one golden.

### OP-011: How is the UI placed on a super-width mode?

- Status: closed by C01 (ADR 0005).
- Decision: `Scene::ui_dest` is a destination rectangle that the video side sets: the full active area on a CRT, a centred box with one scale for both axes on a flat panel. The renderer scales the canvas into it with nearest, each axis on its own.
- Where: `crates/neptron-types/src/scene.rs`, `docs/adr/0005-ui-destination-in-scene.md`

### OP-012: What the OpenGL ES renderer must do to match the reference renderer.

- Status: closed by T19; P10 and P11 changed the means on the Raspberry Pi 3. Limits: OP-043.
- Rule: exact for nearest and UI pixels, within 1 per channel for sharp bilinear, under Mesa software GL. Means now: one texel per pixel in BGRA bytes with the 16-bit formats expanded on the CPU (rounded, not bit replication), the destination index from a varying, the quad limited to the pixels inside the frame, the taps clamped to the crop.
- Where: `crates/neptron-gfx/src/gles`, `crates/neptron-gfx/src/reference.rs`

### OP-013: Nothing forces a caller to apply the pixel clock limit of an EDID profile.

- Status: closed by T22; no contract change.
- Decision: `Monitor::caps_for` applies `EdidProfile::limit_caps` for every EDID monitor, so the engine never calls the policy without the limit.
- Where: `crates/neptron-video/src/monitor.rs`, `crates/neptron-platform/src/profile.rs`

### OP-014: An EDID profile has one sync and porch set for the full line-rate range.

- Raised by: T11
- Status: open
- Default: the porches are the largest GTF values in the range. Wide modes near the top line rate then need more pixel clock than the monitor allows (for example 1024x768 at 85 Hz), and DMT modes with short sync pulses are refused. The project's own modes (480p, 240p at 120 Hz) fit.
- Where: `crates/neptron-platform/src/gtf.rs`, `profile.rs`
- How to change: one `CrtRange` per line-rate band.

### OP-015: The real EDID of the SyncMaster 750s is not checked.

- Raised by: T11
- Status: open, bench check
- Default: none. If the monitor does not set the GTF bit or is not EDID 1.3 or 1.4, it gets no profile from EDID and the user selects a preset.
- Where: `crates/neptron-platform/src/edid.rs`
- How to change: relax one named check after the real EDID is read.

### OP-016: A flat-panel profile in a custom file bypasses the hard rails.

- Raised by: T04 review
- Status: open
- Default: a flat-panel profile is matched exactly against its own listed modes and skips the rails.
- Where: `crates/neptron-safety/src/validated.rs`
- How to change: record where a profile comes from (EDID or custom file) and refuse `flat_panel` in a custom file. This is a contract change.

### OP-017: No hard rail applies above 40 kHz.

- Raised by: T04
- Status: open
- Default: above 40 kHz the monitor profile is the only limit.
- Where: `crates/neptron-safety/src/rails.rs`
- How to change: add one compiled absolute ceiling for line rate and field rate.

### OP-018: Sync polarity and the largest size of the GPU are not checked by `validate`.

- Raised by: T04
- Status: open
- Default: not checked. The KMS test-only commit refuses a mode that is too large.
- Where: `crates/neptron-safety`
- How to change: add `Rejection` variants (a contract change).

### OP-019: The hard rails are the union of all presets of a line-rate class.

- Raised by: T04 review
- Status: open
- Default: the rails guard against a bad custom profile, and not against the choice of the wrong preset for a monitor.
- Where: `crates/neptron-safety/build.rs`
- How to change: rails per monitor type.

### OP-020: The libretro bindings and the test core are checked on macOS only.

- Raised by: T09
- Status: open, for the test core only
- Default: the bindings are generated for LP64 targets and give the same output for x86_64 Linux, aarch64 Linux and aarch64 macOS. The frontend loads the three real cores through them on aarch64 Linux (`core-trace` in QEMU, the Raspberry Pi 3) and on x86_64 Linux (the PC bench, 2026-11-13). No document records a build or a load of the test core's Linux `.so` (UNVERIFIED).
- Where: `crates/neptron-libretro-sys`, `crates/neptron-testcore`
- How to change: repair after the first Linux run. A 32-bit target needs a second generated file.

### OP-021: The test core has no save states.

- Raised by: T09
- Status: open
- Default: `retro_serialize_size` returns 0. No MVP task needs save states.
- Where: `crates/neptron-testcore/src/ffi.rs`
- How to change: add them to the core model when the save-state work starts.

### OP-022: Core options cannot be schema entries.

- Raised by: T08
- Status: open
- Default: keys under `[core.<name>]` are kept in the file and reported as unknown keys.
- Where: `crates/neptron-settings/src/schema.rs` (`Entry` and `Kind` use `&'static str`)
- How to change: a namespace for options that a core declares at run time.

### OP-023: `gamelist.xml` import is not done.

- Raised by: T10
- Status: open
- Default: absent. It needs an XML reader.
- Where: `crates/neptron-library`
- How to change: a module that fills names and favourites after the scan.

### OP-024: Which control commands does a release image serve?

- Status: closed by T28.
- Decision: the app serves `input_send`, `input_script`, `session_launch` and `settings_set` only with its `control` feature, because a scripted client can press "confirm". Only the development fragments and the QEMU image set `BR2_PACKAGE_NEPTRON_CONTROL`, so a release image has no `control`. A read-only subset (`hello`, `video_state`, `stats`, `log_tail`) on an owner-only socket is a later step.
- Where: `crates/neptron-ipc/README.md`, `os/package/neptron/neptron.mk`, `os/configs/*_dev.fragment`

### OP-025: `cargo xtask clippy -p` does not check test code.

- Status: closed by T10: the command passes `--all-targets` (`xtask/src/main.rs`).

### OP-026: On a 31 kHz monitor, Switchres pads a 224-line source to 500 lines.

- Status: closed by T15 and C02.
- Decision: the policy ranks a padded mode after the exact results when the game fills less than `PADDED_FILL_MIN_PERCENT` (75 %) of its picture lines. A table row on a 30 kHz profile proves it.
- Where: `crates/neptron-modeline/README.md` ("Padding (OP-026)"), `tests/integration/safety.rs`, `tests/integration/policy_table.rs`

### OP-027: The window backend shows every mode as a 4:3 picture.

- Raised by: T13
- Status: open
- Default: 4:3 for all modes, also a 16:9 flat-panel mode.
- Where: `crates/neptron-window/src/picture.rs`, `CRT_ASPECT`
- How to change: pass the aspect of the monitor profile into `picture_size`.

### OP-028: SDL has one event pump for each process, and the keyboard pad of the window owns it.

- Status: closed by T18.
- Decision: the SDL3 gamepad source reads the gamepads with no event pump (`SDL_UpdateGamepads`), so the window keeps the one pump of the process. Gamepads and keyboards have separate device id ranges.
- Where: `crates/neptron-input/src/sdl.rs`, `crates/neptron-input/src/device.rs`, `crates/neptron-window/src/keymap.rs`

### OP-029: No tier compiles the SDL3 code.

- Raised by: T13 review
- Status: open
- Default: the SDL3 feature is checked by hand with `cargo xtask clippy -p neptron-window --features sdl3` and by `cargo xtask window-smoke`.
- Where: `xtask/src/verify.rs`
- How to change: a tier 2 step that builds the hardware features on a host that has the libraries.

### OP-030: `vkms` gives vblank events at twice the field rate for an interlaced mode.

- Raised by: T14, confirmed in review against the kernel source
- Status: open
- Default: on `vkms`, interlaced modes are tested for the commit, the readback, the CRC and the writeback only. Field pacing is tested with the simulated clock and on hardware.
- Where: `os/`, the `vkms` probe
- How to change: none on kernel 6.18. It is how the kernel computes the frame duration for the vblank timer.

### OP-031: The UI needs data per option and names for its pages.

- Status: closed by T20.
- Decision: an option has a label, a detail and a disabled reason (`is_enabled`); the display panel and the quick menu have named page ids; `Intent::DismissNotice` exists.
- Where: `crates/neptron-ui/src/snapshot.rs`, `intent.rs`

### OP-032: `present` or `set_mode` while a flip is pending.

- Status: closed by C07 (ADR 0007).
- Decision: the `Display` contract states the rule: `present`, `set_mode` and `blank` return `DisplayError::Device` as busy while a flip is queued, and a backend never drops a queued flip. The engine waits for the flip first.
- Where: `crates/neptron-backend/src/display.rs`; `crates/neptron-testkit/README.md`, section "What the real backend must match"

### OP-033: The Switchres wrapper and its goldens are checked on macOS only.

- Raised by: T05
- Status: open, the first CI run decides
- Default: the build uses `-ffp-contract=off` and the sources use exact library calls, so the 61 goldens are expected to be equal on Linux. Nobody has run it.
- Where: `crates/neptron-switchres-sys`, `crates/neptron-modeline/tests/golden`
- How to change: repair after the first Linux run.

### OP-034: `GpuCaps` has no doublescan field.

- Raised by: T05
- Status: open
- Default: doublescan is off in the solver settings, because i915 refuses doublescan modes.
- Where: `crates/neptron-modeline/src/settings.rs`
- How to change: a capability field in `GpuCaps` (a contract change).

### OP-035: A guest command can read the run nonce and forge the end marker.

- Raised by: T14
- Status: open, accepted for the MVP
- Default: the nonce stops marker text that a command prints by accident. It does not stop a command that reads the nonce from `fw_cfg` as root. The markers give honest test results; they are not a security boundary.
- Where: `xtask/src/os/transcript.rs`, `os/README.md`
- How to change: send the exit status through a channel that the guest command cannot write, for example a second serial port that only the init script opens.

### OP-036: `neptron-vkms-probe` divides by zero for a mode with a zero clock or a zero total.

- Raised by: T14 (review)
- Status: open
- Default: the probe is a test tool and its callers give it correct modes.
- Where: `os/` package `neptron-vkms-probe`, `parse_mode`
- How to change: refuse such a mode in `parse_mode`. Count a refused test-only commit as a failure.

### OP-037: The x86_64 image was never built or booted.

- Status: closed by T28 and the first PC bench session (2026-11-13).
- Answer: the x86_64 PC image builds and boots in QEMU on BIOS and UEFI, and the development image ran on the owner's PC on 2026-11-13 (OP-078 to OP-084). OP-077 lists what is still not proven.
- Where: `os/README.md`, section "Not verified"

### OP-038: Which combination is the blind recovery, and the arcade quick-menu rule.

- Status: closed by C03.
- Decision: the owner (2026-11-11): Select+Start opens the quick menu, and the same combination held for 5 s is the blind recovery. "Hold Start for N seconds" is a setting, off by default, 2 s on an arcade panel: `input.menu_hold_start_seconds`.
- Where: `QUICK_MENU_COMBO`, `BLIND_RECOVERY_COMBO` and `BLIND_RECOVERY_HOLD_NS` in `crates/neptron-input/src/hotkeys.rs`

### OP-039: Input limits that hardware must show.

- Raised by: T18
- Status: open
- Default: a gamepad is identified by its SDL GUID only, so two identical pads can swap ports on reconnect. A gamepad refused because all ports are full gets no port until it reconnects. The device rescans run on the frame thread (every 30 polls for SDL, every 120 for evdev). The evdev crate prints one line to stderr when a grabbed keyboard goes away. The keyboard table of `neptron-window` is not tested against the shared fixture. SDL3 hotplug without libudev is not verified.
- Where: `crates/neptron-input/README.md`
- How to change: see the README for each item.

### OP-040: The zip path of the libretro host is not tested with a real core library.

- Status: closed by T24: all three real cores (FCEUmm, Snes9x, Genesis Plus GX) load a zipped ROM, with the same frame and audio hashes as the plain ROM.
- Where: `crates/neptron-libretro/tests`, `crates/neptron-testcore`

### OP-041: Libretro host defaults.

- Raised by: T16
- Status: open
- Default: a frame larger than the announced maximum is refused. The core library is never closed. `GET_LANGUAGE` reports English. Only version 0 of the core options API is served. A zip load holds up to about 148 MiB for a short time.
- Where: `docs/spec/libretro-host.md`, `crates/neptron-libretro/README.md`
- How to change: see the spec for each item. Traces of real cores decide the frame rule.

### OP-042: `AudioSink` has no pause, and the long audio run uses the cheaper resampler.

- Raised by: T17
- Status: open
- Default: a pause mutes, lets the device run dry on silence and fills the queue with silence at the next frame. The ten-minute part of timing check 6 is proven with the Polynomial mode; the Sinc default is proven for 30 s runs, because the sinc filter is slow at `opt-level = 0`. The rate re-estimation of Arntzen's paper (equation 8) is not implemented: the session gives the run rate.
- Where: `docs/spec/pacing-audio.md`, `crates/neptron-audio`
- How to change: add `pause` and `resume` to the trait (a contract change). Set an `opt-level` for `rubato` in the dev profile of the root `Cargo.toml`, then run the long tests with Sinc.

### OP-043: Limits of the OpenGL ES renderer.

- Raised by: T19
- Status: open
- Default: a scene whose crop side times dest side on one axis is above 2^23 is refused (`ScaleTooLarge`); the reference has no such limit. The renderer has a headless constructor, a pbuffer constructor for the window path of `gles-compare`, and the GBM window target of the KMS backend. A big-endian host is a compile error. Without `EGL_KHR_display_reference` (Mesa 26.1.8 lacks it) a renderer never calls `eglTerminate`. `glow` can still panic when a driver gives a null or non-UTF-8 extension or renderer string. `khronos-egl` brings a second `libloading` version. `gles-compare --arch x86_64` was never run. On the Raspberry Pi 3 (vc4, Mesa 26.1.8) every scene of `gles-compare` passes, through the framebuffer object and the window path, except the two axis product cases: their 4096-pixel output is above the 2048 texture limit of vc4 (commit `e03f6f7`). A source texture grows at the first frame of a larger size (OP-103 point 4).
- Where: `crates/neptron-gfx/src/gles`, `crates/neptron-gfx/README.md`, `xtask/src/os/cross.rs`
- How to change: see the README for each item. Read `GL_RENDERER` and the extension list through the checked `glGetString` path to close the panic paths.

### OP-044: A binary that embeds the monitor presets carries their GPL licence.

- Status: closed by T27: `apps/neptron/src/presets.rs` loads the presets at run time.
- Decision: the owner (2026-11-11): the main binary loads the monitor presets from files on the image at run time and embeds no GPL data. `kms-spike` embeds `data/monitors`, states `(MIT OR Apache-2.0) AND GPL-2.0-or-later`, is a bench tool, is not shipped, and stays as it is.
- Where: `tools/kms-spike/Cargo.toml`, `data/monitors/LICENSE`

### OP-045: What `kms-spike` has not shown.

- Raised by: T21
- Status: open, the hardware run decides
- Default: the tool ran on `vkms` only. `vkms` has one fixed connector and no console to restore, takes interlace and any pixel clock, and gives vblanks at twice the field rate for an interlaced mode (OP-030). The derived 480p mode of `pc_31_120` has vertical porches of about 14, 3 and 27 lines (VGA has 10, 2 and 33). After a kill of the worker the test mode can stay on the output with a black picture.
- Where: `tools/kms-spike/README.md`, `docs/hardware-checklist.md`
- How to change: run the check list on the bench and record the results.

### OP-046: Readings of the Definition in the ODREN policy that the owner must confirm.

- Raised by: T15 and its reviews
- Status: decided; C02 built points 1 and 2
- Default: each reading is one named constant or function, listed in the README under "Readings".
  1. See the decision.
  2. See the decision.
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
- Default: a TV profile (`MonitorKind::Tv`, ADR 0012) gets its modes from the fixed table of its standard (`policy/tv.rs`), not from Switchres. A CRT profile on a GPU with a fixed pixel clock (`pi_vc4_composite`, 13.5 MHz) still gets no mode, because Switchres does not calculate for a fixed clock (`tests/golden/policy_gpus.toml`); the composite output of that GPU takes a TV profile. With a Switchres monitor aspect other than 4:3 every exact rung drops; no preset sets one, and the default is 4:3. A non-finite Switchres score stops the plan. The engine must apply `EdidProfile::limit_caps` before it calls the policy (OP-013), and it must commit `token.mode()` of the winner. A turned game is shown upright: `Scene` carries the rotation (ADR 0015).
- Where: `crates/neptron-modeline/README.md`, section "Known limits"
- How to change: a fixed-clock strategy for a CRT profile (after the MVP); pass 4:3 to Switchres and use the monitor aspect for border widths only.

### OP-048: What the app must do for the confirm dialog and the screens.

- Status: closed by T27.
- Answer: `apps/neptron/src/nav.rs` stops every repeat on a prompt and ignores held keys until release, with a first repeat delay of 400 ms (`REPEAT_DELAY_NS`); the `ConfirmId` is the guard's prompt id; a late Keep gets `ConfirmError::Expired` and does nothing; `DismissNotice` removes the notice (`apps/neptron/src/shell/intents.rs`).

### OP-049: Screen behaviour that the owner must confirm.

- Raised by: T20
- Status: open, needs a decision from the owner
- Default: only the Menu button resets a settings row to "inherited", so a two-button pad cannot do it. Notices wait until a menu is open (the canvas is opaque, OP-010). "All games" in the quick menu writes the global level only, so a value set for the system still wins. The display panel always writes the global level. More than 16 settings changed in one quick-menu visit: the extra ones stay at the game level. Favourites and Recent always show on the home screen when systems exist. The skin file refuses unknown keys.
- Where: `crates/neptron-ui/README.md`, `crates/neptron-ui/src/screens`
- How to change: see the README for each item. An "Inherit" entry in the option list solves the first.

### OP-050: Black scanlines for line-doubled 480p.

- Status: closed by C04 (ADR 0006).
- Decision: the owner (2026-11-11): black scanlines are in the MVP for a whole vertical multiple, with a strength setting. They must look right, like other setups and hardware line doublers, and the `Scene` type lets more scanline kinds and their own settings follow with no second contract change.
- Where: `crates/neptron-types/src/scene.rs`, `crates/neptron-gfx`

### OP-051: What the app must do for the Start hold, and settings help text.

- Raised by: C03 and its review
- Status: open for the help text field only; T27 built the app part (`crates/neptron-session/README.md`, `apps/neptron/README.md`)
- Default: the app reads `input.menu_hold_start_seconds`, passes it to `Hotkeys::set_start_hold_ns`, and treats a second `QuickMenu` while the menu is open as no action (the quick menu is never a toggle). When the hold fires, the app gives the core Start up on that port until the release. The game has already seen the Start press, so most games are in their own pause screen on resume. A game that needs Start held opens the menu by accident; the hold is off by default and the first-boot setup must say this. A schema entry has no help text field: the help of a setting is in the doc comment of its key and in the README.
- Where: `crates/neptron-input/README.md`, `crates/neptron-settings/src/builtin.rs`
- How to change: a `help` field in the schema entry (a change to the settings model, with an ADR).

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
- How to change: accept the two files in the repository with an ADR, or build the suite from its GPL source with that art removed or replaced (needs the SNES and Mega Drive toolchains and new traces).
- Decision: the owner (2026-11-12): a ROM with a clear open licence for all its content can be in the repository; any other ROM stays outside and is downloaded to the development machine for the tests. The NES build stays in. The SNES and Mega Drive builds stay outside.

### OP-054: Non-commercial cores in a distributed image.

- Raised by: T24
- Status: decided
- Default: the PC and Pi images hold Snes9x and Genesis Plus GX. Both licences forbid sale and commercial use. Genesis Plus GX also links LGPL code (Nuked OPN2). FCEUmm is GPL-2.0-or-later. FinalBurn Neo is ADR 0014.
- Where: `os/package/libretro-*`, `data/cores`
- How to change: download the cores at first boot, or use other cores (bsnes, BlastEm).
- Decision: the owner (2026-11-12): the product is open source and not commercial, so an image can hold Snes9x and Genesis Plus GX. The image must carry the licence texts and the `make legal-info` output, and must never be sold. T28 does this.

### OP-055: What the real cores report, and the options for the full picture.

- Raised by: T24
- Status: open; the core options of the full picture are set (commit `9053872`)
- Default: `data/cores` sets the options of the full picture for each core: `fceumm_overscan_v_top` and `fceumm_overscan_v_bottom` at 0 (256x240), `snes9x_overscan` at `auto` (224 or 239 lines as the game asks), `genesis_plus_gx_overscan` at `disabled` (320x224; `top/bottom` gives 320x240 with the border). With their own defaults the cores give this: FCEUmm crops NES to 256x224 (8 lines at the top and at the bottom), Snes9x gives 224 lines always. Genesis Plus GX reports 256x192 at load and changes to 320x224 at frame 4; the session waits for the source to settle before the first mode (OP-064 point 1). No real core used save RAM, because no test ROM has it. The host serves no `SET_SERIALIZATION_QUIRKS`, which matters when save states come. The traces are expected to be equal on x86_64; nobody has run it.
- Where: `tests/traces/README.md`, `data/cores`
- How to change: a default geometry per system in the core metadata (it has no field for it yet), so the session starts from it in place of the launch settle.

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
- Default: arcade boards draw to the frame edge, so a crop of up to 8 lines per edge can hide real content. The bound is not checked against real game pictures. The notice reports the hidden lines.
- Where: `crates/neptron-modeline/README.md` (Known limits)
- How to change: a per-system or per-game crop bound in the core or game metadata; or a setting that turns the crop off, at the cost of a padded or filtered picture.

### OP-058: Layout and alert of the video status block.

- Status: closed by C06.
- Decision: the owner (2026-11-12): the layout stays (the result words on two lines under the mode line, five lines always, the block only when 2 rows of the page fit under it). An alert gets the text mark `[!] ` and the warning colour `#FF8020`; errors get `#FF6060`; the speed word is `speed (X to Y Hz)`. A version 2 skin file requires both colour keys. A warning notice uses the warning colour on its title bar.
- Where: `crates/neptron-ui/src/screens/video.rs`, `crates/neptron-ui/README.md` (the contract of the text)

### OP-059: The bounded flip wait is not in the `Display` contract or the fake.

- Status: closed by C07 (ADR 0007).
- Answer: `KmsDisplay::wait_flip` waits at most `FLIP_WAIT_LIMIT_MS` (500 ms) and a flip stays queued after any failed wait, in the fake too; `FlipFault::Slow` exists; `Display::blank()` is in the contract, the fake, the KMS backend, the window and the engine.
- Where: `crates/neptron-backend/src/display.rs`, `crates/neptron-testkit/src/display.rs`

### OP-060: Smaller points of the KMS backend.

- Raised by: T23 review
- Status: open, low priority
- Default: (1) `tools/kms-spike` and `neptron-kms` each hold a copy of the mode conversion; the records are equal except the name text. (2) `draw_to_window` with a zero side has no test, because EGL does not run on the host. (3) Another thread can close and reuse the launcher descriptor number between the device check and the take; the launcher contract excludes it.
- Where: `crates/neptron-kms`, `tools/kms-spike`
- How to change: make the spike depend on `neptron-kms` or remove it after the hardware tests.

### OP-061: Engine points for the session and the app.

- Status: closed by T27 and C07: the app calls `set_scanlines` with the settings (`apps/neptron/src/shell/boot.rs`, `shell/intents.rs`), and `Display::blank()` exists (ADR 0007).
- Labels as built: the status text comes from `Odren::video_status` (the read back); a status from a proposal is for the Display panel preview only. On the mode line an interlaced 1:1 mode shows `native`, the fitted rung `fit`, a flat panel `panel`, a multiple `Nx`. `exact` shows with no fit, crop, padding or speed change (a fractional horizontal scale under a fixed super width counts as exact). `double refresh` shows also when the user asked for it. A field rate of 100 Hz or more makes the mode line 38 characters, and the UI cuts it.
- Where: `rung_label` and `write_result` in `crates/neptron-video/src/odren.rs`, `status.rs`

### OP-062: Engine behaviour around the blank, for the session and the guard.

- Raised by: C07 and its review
- Status: open
- Default: (1) a flip that never completes stops the output: each frame waits up to 500 ms on KMS and returns `PendingFlip`, and the engine does not recover alone. (2) After device errors block a source, the blank frame clears the block, so one more `set_mode` attempt follows (`MODESET_ATTEMPTS + 1` in all). (3) The engine also blanks when a valid mode exists but device errors stop it from replacing a refused mode on screen. (4) A blank that fails is tried up to `MODESET_ATTEMPTS` times; an input change or `retry()` starts a new count. (5) While a refused mode is on screen and a geometry change waits for its debounce, the refused mode stays for up to `DEBOUNCE_FRAMES - 1` frames, with `current_refused()` on.
- Where: `crates/neptron-video/src/odren.rs`, `crates/neptron-video/src/engine.rs`, `docs/adr/0007-bounded-flip-wait-and-blank.md`
- How to change: for (1), the session or the app opens the display again after N `PendingFlip` errors in a row. For (5), skip the debounce when `refused_on_screen` is on.

### OP-063: Timing check 6 with late flips is weaker than the plan text.

- Raised by: T25 review
- Status: open, needs a look from the owner
- Default: the plan asks that the audio buffer stays inside its bounds over 100,000 frames, late vblanks included, in well under a second. With the 32 ms audio target, two late flips a few seconds apart run the queue dry, because the rate control recovers with a 6.4 s time constant. The long run with late flips asserts that every underrun follows two late flips in the 10 s before it. The runs without late flips assert no underrun. The two 100,000-frame runs take about 67 s each in the dev profile, so they are ignored tests: `cargo xtask test -p neptron-session --run-ignored only -E 'test(hundred_thousand)'`. The tier runs cover 30 simulated seconds per case with the strict assertions.
- Where: `crates/neptron-session/tests/integration/timing.rs`, `docs/spec/pacing-audio.md`
- How to change: a larger audio target level (more latency, more margin), or a faster recovery after an underrun.

### OP-064: Session choices for the app.

- Raised by: T25 and its review
- Status: open
- Default: (1) at launch the core runs up to 30 frames with no display until its source is the same for 6 frames, so the first mode is right (Genesis Plus GX). (2) A frame with more than 288 lines counts as interlaced, unless the system is `always_progressive`. (3) Save RAM is flushed every 30 s when it changed, when the menu opens and at exit; a save file longer than the core's save RAM keeps its tail. (4) After 10 frames in a row with no flip the session ends with `DisplayStalled`; with wait timeouts that is about 5 s. (5) While the output is off, `run_frame` returns at once and the app paces the loop. (6) A core option that the core refuses is counted and not reported per key. (7) Two contents with the same file stem share one `.srm`. (8) "No allocation in a steady frame" holds by construction and has no test. (9) The frame after a resume goes out as silence.
- Where: `crates/neptron-session/README.md` (the list "what the app calls")
- How to change: the limits are named constants in `crates/neptron-session`. A default geometry per system in the core data removes the launch settle.

### OP-065: Guard choices and what the app must do.

- Raised by: T26 and its three reviews
- Status: open; points 1 to 3 need a look from the owner
- Default: (1) the guard covers the monitor and the policy settings the user changes. The connector (`display.output`) and geometry offsets are not under it; the app must put a connector change under the same prompt. (2) Last known good holds the monitor only, because the policy settings resolve per game. It is recorded only on a confirmation. (3) Blind recovery persists safe mode until the user confirms a configuration. (4) Limits (ADR 0013): confirm window 15 s (`CONFIRM_WINDOW_NS`); a chain of changes with no confirmation ends 30 s after its first change (`PENDING_LIMIT_NS`); 1 s of no valid picture reverts a pending change or goes to the safe default (`NO_PICTURE_LIMIT_NS`); a boot is good after 3 s of picture (`GOOD_BOOT_PICTURE_NS`). (5) A confirmation counts only after the engine placed a mode for the current inputs and a frame showed it (`Odren::settled`). A mode left on screen by device errors can never be confirmed. (6) When a decline or a game setting meets persistent device errors, a valid but unconfirmed mode can stay on screen with no good boot recorded; repeated boots like this go to safe mode. (7) In safe mode with no picture the guard does nothing more: no signal is the safe state. (8) The app gives every guard call the same clock read of the frame; an earlier time reverts a pending change. (9) A revert of a boot prompt carries the configuration the app must write to the global settings (`rewrite`); the app never writes `Guard::active()`. (10) The app builds the safe default for the detected output and stores the record (profile id and kind) with temp file, fsync, rename, before the first modeset. (11) Recovery before `Guard::boot` belongs to the launcher. (12) The end-to-end test from the 5 s hold in `neptron-input` to `Guard::recover` is `apps/neptron/tests/app/recovery.rs`.
- Where: `crates/neptron-video/src/guard`, `crates/neptron-video/README.md` (The guard; Narrower than the safety model)
- How to change: the limits are named constants in `guard.rs`. A connector field in `DisplayConfig` puts the output under the guard.

### OP-066: User override of the safety limits, with warnings.

- Raised by: the owner (2026-11-12), in a side session
- Status: open, after the MVP
- Default: no switch turns the validator off. The override is the monitor profile, which is a file loaded at run time.
- Where: `crates/neptron-safety`, `crates/neptron-modeline`, `data/monitors`, the display panel
- How to change: (1) a "custom monitor" editor in the UI for the line rate, field rate and sync ranges, with a warning text, under confirm-or-revert; (2) a tolerance setting for small misses outside the range, with the `[!]` alert; (3) a per-game override of the mode choice, saved by the user.

### OP-067: Hardware-rendered (GPU) cores, and which emulators are supported.

- Raised by: the owner (2026-11-12), in a side session
- Status: open, after the MVP
- Default: the MVP host takes software frames only. Direction: libretro cores only, and no standalone emulators, because they open the display themselves and go around the mode control and the safety path.
- Where: `crates/neptron-libretro`, `crates/neptron-gfx`, `data/cores`
- How to change: add the libretro hardware render interface to `neptron-libretro` (context create, reset, destroy; protect the GL state of the renderer), a desktop OpenGL context through EGL next to GLES in `neptron-gfx`, the internal resolution per core in the core files (1x for a CRT so nothing is filtered), and Vulkan later. Size: about one wave. The target is the cores that Batocera and RePlayOS offer, for example Dreamcast and GameCube.

### OP-068: 16:9 content with no per-game list.

- Raised by: the owner (2026-11-12), in a side session
- Status: open, after the MVP
- Default: the frame fills the 4:3 tube. The policy assumes a 4:3 monitor. The project ships no per-game patch; files per system and per core are acceptable.
- Where: `crates/neptron-modeline` (policy), `crates/neptron-types` (the destination in `Scene`), `crates/neptron-settings`, the quick menu
- How to change: a monitor aspect setting (4:3 or 16:9 tube); read the aspect ratio that the core reports in its geometry, also when it changes while the game runs; letterbox or fill as picture options; for GPU cores the core renders at the letterbox size so the picture stays exact. An anamorphic game (16:9 chosen in the game's own menu, output still a 4:3 frame) sends no signal, so the user sets "Aspect: 16:9" in the quick menu, and it is saved for that game as a user setting.

### OP-069: App and launcher choices, and what is not done.

- Raised by: T27 and its two reviews
- Status: open; points 6 to 12 need a look from the owner
- Default:
  1. One run is one process: the game list or one game. A launch or the end of a game writes `run/next.toml` on a RAM file system and exits with code 75; the launcher starts the next run at once. More than 10 relaunches in 10 s count as crashes with the backoff (250 ms doubling to 8 s).
  2. A planned relaunch gives back the failed-boot count of the run before (`record::for_continued_boot` in the app). A `Guard::resume` in `neptron-video` is the cleaner home.
  3. A start that fails before its record write (the display does not open, no safe preset) is not a counted boot; it sets no mode, and the launcher keeps the last mode on the wire.
  4. The `sdl3` crate adds `-L /usr/local/lib`, which the Buildroot compiler wrapper refuses; the image packages and `os app-check` link through `os/package/neptron/link-filter.sh`, which drops only that path.
  5. KMS defaults: buffers up to 2560x1200, the connector rule for a CRT, GPU caps from the first connected output, safe start profile `generic_15`. The launcher does not blank a 15 kHz output before the first commit and does not open the device again after many restarts.
  6. The output connector has no menu: it changes in the settings file and takes effect at the next start, under the boot prompt.
  7. The recovery tone plays in the game list and in a game (`apps/neptron/README.md`).
  8. The control protocol does not serve `screenshot` or `clock_step`, and `video_state` gives no validator verdicts. `input_send`, `input_script`, `session_launch` and `settings_set` need the `control` build feature.
  9. Favourites and recent games are not kept.
  10. The menu is a 320x240 source at 60 Hz on every profile, 50 Hz profiles too.
  11. In the window backend, Back on the home screen does nothing; the run ends when the window closes.
  12. Logic that sits in the app and can move later: key repeat, the pages, preset loading, display detection.
- Where: `apps/neptron`, `apps/neptron-launch`, their READMEs, `xtask/src/os`
- How to change: see each README.

### OP-070: Licence of the 15 kHz kernel patch set. Must be closed before any image with the set is distributed.

- Status: closed by M12 (ADR 0018). The author set the patches to GPL-2.0-only (commits `c57e6508`, `7abc1ed3`, LICENSE `e106247a`; issue 22). NEPTRON pins `d85a343379f7f54155f69ef56dc544b666635894` (the nine `linux-6.18` files are byte-identical to the old pin). Every PC image applies the set, `LICENCES.txt` names the set, its commit and its licence, and each image carries a written offer for its sources. The Pi image never takes a kernel patch (`os/board/rpi/check-kernel-patches.sh`). `--no-kernel-15khz` stays for the bench comparison.
- Decision: the owner (2026-11-12): include the patches, ask the author, and if the licence is not GPL-2.0, write NEPTRON's own patch from the kernel source and documentation, without the text of the set, under GPL-2.0 in `os/patches`, and remove the set (a copy or an adapted copy does not clear the licence); the bench records which modes the unpatched driver refuses. The owner (2026-11-13): every PC release image includes the 15 kHz patches. The owner (2026-11-19) accepted the remaining risk: the earlier contributors Ansa89, MaKoTo, substring, Calamity and amxcs did not state GPL-2.0 themselves; if one objects, the own-patch plan applies.
- Where: `os/patches/linux/linux.hash`, `os/README.md`, `os/board/pc/LICENCES.txt`, `docs/adr/0018-*`

### OP-071: Owner decisions on flat panels.

- Status: closed by C08 (ADR 0010, `OutputKind`). Limits as built: OP-075.
- Decision: the owner (2026-11-12): flat panels are in the MVP and must be usable: native mode, the game's shape with a whole vertical factor, centred, black around it. Pixel exactness matters only on a CRT; a sharp filter is accepted with no alert; a blurry picture is never accepted; CRT shaders for panels come after the MVP. A VGA output behind a DAC is a CRT (an analog EDID or an adapter that reports VGA selects the CRT path); an adapter with its own digital EDID cannot be detected, which the owner accepts: the user's preset wins over detection, and the first boot on a new output is under the confirm prompt. CRT limits apply to an analog output only, and the type system tells digital from analog.

### OP-072: Build cache strategy, locally and in CI.

- Raised by: the owner (2026-11-12)
- Status: open, the local part exists; CI caches are in `docs/ci.md`
- Default: locally, Docker volumes hold the Buildroot downloads (`neptron-br-dl`), the compiler cache (`neptron-br-ccache`, `BR2_CCACHE=y`), the Buildroot output per target (`neptron-br-out-<arch>`) and the Cargo registry and target directory of the guest builds (`neptron-cross-<arch>`). All worktrees share them. A second image build reuses the output volume and builds only what changed. Each task worktree gets its `target/` as a clone of main's. The development container is tagged by the hash of its Dockerfile.
- Where: `xtask/src/os/build.rs`, `os/README.md` (the table of volumes), `docs/ci.md`
- How to change: never delete the volumes in a routine clean; give `cargo xtask os` a command that reports their sizes and a separate, explicit one that prunes. Build the frontend package from the Cargo cache volume so a Rust change compiles only changed crates in the release profile. Measure a cold, a warm and a frontend-only build per target.

### OP-073: Pixel clocks in whole kHz, and what follows.

- Raised by: T29 (found on vkms), C09 and its review
- Status: open, points 1 and 2 need the bench
- Default: the kernel mode record holds the pixel clock in kHz, and a backend refuses any other clock and never rounds (ADR 0009). The policy gives every CRT mode a whole-kHz clock: it changes the horizontal total by up to 8 pixels in the back porch (`HTOTAL_CHANGE_MAX`) and takes the first timing that keeps the game's field rate. Every exact result stays within 3 mHz of the game's rate. `FakeDisplay` refuses the same modes as the KMS backend. (1) A changed back porch shifts the picture by at most about 1.3 microseconds, 1.9 percent of a 15 kHz line for a 320-wide mode and up to 3.2 percent for a 160-wide 31 kHz mode, and changes the active width by the same share. Nobody has seen it on a tube. (2) The sync pulse of a changed timing stays between 90 and 110 percent of nominal (`HSYNC_MAX_PERCENT` is the implementer's bound; the validator allows 150). (3) `neptron_safety::validate` does not check the kHz step: it is a rule of the kernel interface, and a refused mode never reaches the wire. (4) `tools/kms-spike` takes raw Switchres modes and refuses a clock that is not whole kHz, so it shows refusals where the frontend sets a mode.
- Where: `crates/neptron-modeline/src/policy/clock.rs`, `docs/adr/0009-pixel-clock-in-whole-khz.md`
- How to change: the bounds are named constants in `clock.rs`.

### OP-074: The hardware self-test: choices and limits.

- Raised by: T29 and its review
- Status: open, for the bench
- Default: (1) `neptronctl selftest video` runs with the frontend and the launcher stopped and opens the DRM node itself. The dry run is the default; the real run needs `--i-have-checked-the-monitor <profile id>`. (2) Every run ends with the output off, because the backend gives no way to read the earlier state and validate it. (3) `tools/kms-spike` stays until the owner has run both tools on the bench and compared the reports (OP-060); then the spike and its rule in `xtask/src/rules.rs` can go. (4) The test pattern is at most 512 pixels wide (the texture limit of the smallest supported GPU, vc4) and is scaled by a whole factor on wider modes. A `KmsDisplay` accessor for the real texture size would lift this. (5) Missed vblanks are recorded and do not fail a mode. (6) For an unknown GPU the interlace timing unit is assumed and the report says so (`unit_assumed`). (7) The vkms gate `cargo xtask os selftest-check` passes: 2 profiles, 4 modes each. (8) The images hold `neptronctl` with the `kms` feature (`os/package/neptronctl`).
- Where: `apps/neptronctl/src/selftest`, `apps/neptronctl/README.md` (the report schema), `docs/hardware-checklist.md`
- How to change: the pass rules are named constants in the self-test.

### OP-075: Flat panels: what is built, and the limits.

- Raised by: C08 and its two reviews
- Status: open, for the bench and for after the MVP
- Default:
  1. Class rule: an EDID that says analog input, a `subconnector` of VGA or DVI-A, or an analog connector type is a CRT; every other digital EDID on a digital connector is a flat panel. A named or driven output uses only its own EDID. A preset in `display.monitor` wins; `flat` forces the panel path.
  2. An adapter that shows its own digital EDID with a CRT behind it looks like a panel (accepted by the owner, OP-071). A panel on a digital link is trusted by its class and boots with no prompt (ADR 0013).
  3. Limits by output kind (ADR 0010): analog keeps the old values; digital on Intel gen9: HDMI 300 MHz, DVI-D 165 MHz, DisplayPort 533.25 MHz, size 4096x2160. Other GPUs have no digital limits yet, so their panels run at 1080p or less. After the display opens on an analog output, the caps are cut to the analog values.
  4. When the native mode does not fit, a mode of 50 Hz or more comes before a larger slower one: a 4K panel on HDMI 1.4 runs at 1080p60 with a notice.
  5. Picture: the native mode, the game's shape, centred, black around it; the placement rules are in `docs/spec/video-policy-cases.md`. The status says `exact` or `sharp`, never an alert. A true downscale (a source taller than the panel mode) keeps sharp bilinear and can blur; no MVP source does that at 720p or more.
  6. Not parsed from the EDID: interlaced modes, 59.94 Hz variants of most CTA codes, other standard timings, DisplayID. EDID 1.0 to 1.2 or a bad extension checksum gives the safe start, so such a panel shows no picture.
  7. When the large buffers of a 4K panel cannot be allocated, the start fails; a retry at the smaller size needs an optional fallback size in `KmsConfig`.
  8. The pre-open connector choice does not know the subconnector.
  9. No CRT shaders on a panel (after the MVP).
- Where: `crates/neptron-platform` (class, panel profile, matrix), `apps/neptron` (display, shell), `crates/neptron-modeline/src/policy` (panel placement), `data/hardware/matrix.toml`, `docs/adr/0010-output-kind-picks-the-gpu-limits.md`
- How to change: digital limits per GPU go in `[[gpu.digital]]` of the matrix with their source.

### OP-076: The first boot shows a confirm prompt also when nothing would change.

- Status: closed by M01 (ADR 0013): a boot whose wanted monitor is the safe default, is trusted by its class, or is confirmed at this place opens no prompt and shows no notice.
- Where: `crates/neptron-video/src/guard/machine.rs` (`Guard::boot`)

### OP-077: The PC image: what exists, what is not proven, and what must happen before the bench.

- Raised by: T28 and its three reviews
- Status: open
- Default:
  1. The release image is built in CI only (decision below).
  2. The development image has SSH for one public key, the hot folder `/state/hot`, `cargo xtask os push`, `os logs` and `os ssh`, and the log under `/state/log`. It writes its address to `/run/neptron/network.toml` and nothing to the share.
  3. Mesa has the Intel `iris` driver; no firmware files are in the image, so AMD GPUs do not work and the Intel DMC firmware is not loaded (OP-086). The i915 and amdgpu firmware licences allow redistribution of the binaries only.
  4. About 104 MiB of clang libraries that nothing uses are in the root.
  5. Before the frontend's first commit a CRT gets the firmware and GRUB picture at 31 kHz or more; the launcher does not blank the output. A 15 kHz-only tube needs a sync-protecting adapter or a late power-on.
  6. The share partition is made at first boot. The format rule is strict (a marker, a clean read, no file system and no exFAT backup record). Not run: a real read error, a re-flash with a different layout, a 4096-byte sector disk. A power cut during mkfs leaves a share that needs a format on another computer.
  7. A quit in the UI restarts the frontend; nothing powers the PC off.
  8. Each variant has a fixed disk signature, so two sticks of one variant in one PC share a PARTUUID.
  9. Test tools and QEMU hooks are in the image; they refuse to act unless the DRM driver is vkms.
  10. The crates of the frontend are pinned by Cargo.lock checksums through `cargo vendor --locked`.
  11. The development image waits for DHCP at boot, so its first frame comes later.
  12. The boot check runs only in QEMU; the x86_64 times are emulated.
- Where: `os/`, `os/README.md`, `xtask/src/os`, `xtask/README.md`
- How to change: see `os/README.md`, sections "The PC image" and "Working on the bench".
- Decision: Owner, 2026-11-13: the release image is built in CI only and is uploaded as a workflow artifact; no local release build.

### OP-078: The game list does not see new files until the frontend starts again.

- Raised by: the owner, on the first hardware session (2026-11-13)
- Status: closed by C14; the hardware checks and the later steps stay in this entry
- Decision: Owner, 2026-11-13: do it now (task C14). Built by C14: a `Rescan games` row reads every folder on a thread; each time the game list opens after a game, only the folders whose time changed are read, and only when a full scan ran in this power cycle; a notice gives the games added and removed. Later steps, not built: a `library rescan` control command; a watch of the folders (inotify) with its limits on exFAT and SMB; a progress count on the row; the home focus kept by system after a scan; a visible report of entries that cannot be read; removal of the unused `ScanWorker` type.
- Where: `apps/neptron` (the library and its background scan), `crates/neptron-library`

### OP-079: No way to copy games over the network.

- Status: closed by C13 (`docs/spec/network-share.md`); the later steps are OP-092.
- Decision: Owner, 2026-11-13: an SMB share in every image, development and release, on by default, as Batocera does. Built: the in-kernel server `ksmbd`, one share of `/share`, guest read-write, plus the published account `neptron` with password `neptron` and the guest's rights (owner, 2026-11-13: Windows 11 24H2 refuses guest sessions); wired DHCP in the background in every image; mDNS name `neptron.local`; the off switch is the file `/state/network-share-off`. Any host on the local network can write files that the scan and the cores then read as root; the spec states the exposure.

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

- Status: closed by C11.
- Decision: each setting says on which display class it has an effect (`AppliesTo`); a row that a flat panel ignores is disabled with the reason "Not used on a flat panel". The scanline rows stay active, because scanlines work on a panel. Since M08e the picture mode acts on a flat panel, and the refresh rate row is disabled there.

### OP-083: The frontend starts before a monitor answers.

- Status: closed by C11; the hardware check is in `docs/hardware-checklist.md`.
- Decision: with output `auto` the frontend waits at most 15 s for a display that the connector rule accepts, then exits with code 69; the launcher restarts it after a fixed 1 s, counts no crash, and logs at most one line per minute. An analog connector with no EDID still needs a named output.

### OP-084: Small findings of the first hardware session.

- Status: closed by C11; the Intel display firmware continues in OP-086.
- Decision: (1) the Mesa shader cache is on the state partition, or off when that partition is in RAM or read-only. (2) A settings file with no value is not written, and is removed when its last value goes, unless it held a comment or a newer schema version. (3) The Realtek firmware (rtl8168h) is in the development image only.

### OP-085: Audio settings: volume and output device.

- Status: closed by C12 (ADR 0011, `docs/spec/audio-settings.md`); the later steps are OP-087 and OP-088.
- Decision: the owner asked for volume and output device in the MVP (2026-11-13). Built: `audio.volume` (percent, software, after the resampler; default 80, because the boot sets the hardware mixer to 0 dB; 100 is bit-exact; global, system and game levels) and `audio.output` (`auto` or a stored ALSA name such as `hw:CARD=PCH,DEV=3`; a new device takes effect at the next start; `auto` takes the one HDMI/DP output with a monitor, else the first analog output, else the default device; an absent named device falls back to the default with a notice).

### OP-086: The Intel display firmware (DMC) is not in the image.

- Raised by: C11, from the kernel log of the first hardware session (2026-11-13)
- Status: decided
- Default: not added. The display works without it; only the low-power states of the display are lost. The i915 driver is built into the kernel and asks for the file before the root file system mounts, so a file in `/lib/firmware` does not help.
- Where: `os/board/pc` (kernel config), `os/board/pc/LICENCES.txt`
- How to change: (a) put the file inside the kernel image with `CONFIG_EXTRA_FIRMWARE`, which places an Intel binary inside the GPL-2.0 kernel image and needs a licence check; (b) build i915 as a module, which changes a recorded design choice.
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
- Default: not built: a volume hotkey in a game; a test tone when the output changes; a notice when the device is lost during a game (SDL's device-removed event); a `neptronctl` command that lists the playback devices (the start log line lists them); Bluetooth audio; a sound server; a volume per system or per game beyond the levels; a follow of the HDMI device when the monitor changes. The mixer step sets a fixed list of control names, so a card with other names keeps its reset state. The `sdl3` crate 0.22.1 can panic on a device name that is not UTF-8; only the window build on a desktop uses that path.
- Where: `docs/spec/audio-settings.md`, `os/board/pc/rootfs-overlay/etc/init.d/S08neptron-mixer`
- How to change: one task per step.

### OP-089: The shared cross-build cache gives stale results between worktrees.

- Status: closed by M11 (2026-11-23).
- Answer: each checkout has its own target folder in the cross volume `neptron-cross-<arch>`; `os build` adds the rebuild targets of the NEPTRON packages itself, and the packages copy their source fresh. A warm image build from a task worktree proved it. M11 also pushes the arcade index and `input/` with `os push --data` (OP-113); that push has not run against the bench PC.
- Where: `xtask/src/os`

### OP-090: Settings under one home row, a submenu arrow and a breadcrumb.

- Status: closed by C15 (`docs/spec/ui-navigation.md`).
- Decision: the owner (2026-11-13): in the MVP. Built: one `Settings` row on home that opens a page with Display, Audio and the other panels; an arrow at the right of every row that opens a page; a breadcrumb in the title bar; the same rules in the quick menu of a game.

### OP-091: `os build` fails to copy the images on a Linux host whose user is not uid 1000.

- Raised by: the CI work (2026-11-13)
- Status: open
- Default: the last step of `cargo xtask os build` copies the images as the container user (uid 1000) into the images directory of the host. On a Linux host with another uid the copy fails with "Permission denied" after a complete build. macOS is not affected. The CI workflow gives the directory to uid 1000 before the build.
- Where: `xtask/src/os` (the image copy), `.github/workflows/images.yml`
- How to change: run the container with the uid of the caller, or copy the images out with `docker cp`.

### OP-092: Later steps of the network share.

- Raised by: C13 (2026-11-13)
- Status: open, after the MVP unless the owner asks sooner
- Default: Settings > Network shows the address, the host name and the state of the share (`docs/spec/network-panel.md`), with the switch as text. Not built: a settings row for the switch (the hook is the off switch file and `S45neptron-share restart`); WS-Discovery, so the machine shows in the Windows network view (needs a new package); Wi-Fi; shares for saves, states and config, which `docs/definition/10-os-layer.md` describes; DHCP for a USB network adapter plugged in after boot; a clean power-off on the release image. On Windows 11 a plain open of the share in Explorer probably shows an error and no prompt; the ways that work are in the spec. The check for `inet dhcp` in `os/board/common/post-build.sh` can never fail after the fixed write above it and can go.
- Where: `os/`, `docs/spec/network-share.md`, `crates/neptron-ui`
- How to change: one task per step.
- Decision: Owner, 2026-11-13: a second share for saves and states is wanted (task C19, after version 0.1); config stays local.

### OP-093: The control protocol does not give the kind of a UI row.

- Raised by: C15 (2026-11-13)
- Status: open
- Default: `neptronctl ui tree` shows no row kind (page, value, action, info, disabled). The crawler reads the kind in the process. The protocol is at version 3 (`PROTOCOL_VERSION`, `crates/neptron-ipc/src/envelope.rs`): version 2 adds `fields_missed` and version 3 adds `late_frames` and `late_flips` to the stats reply; `Reply::for_version` removes a newer field from a reply to an older request, and `Client::call` falls back to the version of an older server. No `kind` field exists.
- Where: `crates/neptron-ipc`, `apps/neptronctl`
- How to change: a small contract branch with an ADR: the `kind` field in a new protocol version, removed by `Reply::for_version` for older requests. The `library rescan` command of OP-078 can go in the same change.

### OP-094: Reboot and shut down from the UI.

- Status: closed by C17 (`docs/spec/ui-navigation.md`).
- Decision: the owner (2026-11-13): build it. Built: a `System` panel with `Reboot` and `Shut down`, each behind a confirm; the frontend ends cleanly, and the launcher runs the stop step and then the reboot or the power-off. Later steps, not built: a handler for the power button of a PC, the version and the network address in the panel, a sleep timer, a tier 3 check of a clean file system after a power-off.

### OP-095: The first boot did not attach the new share partition on real hardware.

- Status: closed by C16 and C18; OP-101 continues the limits.
- Decision: the owner (2026-11-13): the share partition must always work. Built: the partition step is judged by its result, the partition attaches in the same boot, an existing share is adopted as it is, no silent exit, messages go to `/state/log/storage.log`, a tier 3 case has an old share on the disk, and C18 adds the user-confirmed format for the states the script does not fix. The root cause on the bench stick (likely a stick that rejects the cache flush) is not proven.
- Where: `os/board/common/rootfs-overlay/etc/init.d/S06neptron-storage`, `xtask/src/os/boot_check.rs`

### OP-096: The Raspberry Pi image: composite first, GPIO DACs later.

- Status: closed by P01 to P07 (`docs/spec/raspberry-pi.md`, ADR 0012, the TV policy, the `TV mode` property in `neptron-kms`, the `rpi-arm64` image, the app wiring, the import of `neptron.toml`). P09 (the bench session) is left. OP-097 to OP-103 hold the points of the first tests.
- Decision: the owner (2026-11-13): composite is the default output, NTSC at 60 Hz, and the user can change the standard (PAL at 50 Hz, PAL at 60 Hz and the others the hardware gives); the picture must be as sharp as composite allows and work on a 15 kHz tube. The GPIO DACs (VGA666, RGB-Pi, Pi2SCART) are not in the first image; they need a line in the boot configuration on the card (`docs/definition/05-crt-safety-model.md`). Owner, 2026-11-13: the test board is a Pi 3 B+ (the tests of 2026-11-14 ran on a Pi 3 B); HDMI on the Pi is in the first image as a flat panel; the default standard stays NTSC; the default width rule is `aspect`; the names NTSC and PAL are used only for composite, other outputs show lines and rates.

### OP-097: Picture size and position on a TV-class CRT (overscan).

- Raised by: the owner (2026-11-13)
- Status: open, a design is wanted before the MVP is called done
- Default: a CRT TV hides part of the picture at its edges, on composite and on RGB alike (a PC monitor does not). The only allowance is the 5 % safe area of the UI theme, on every output. On composite the menu is 640 of the 720 samples wide by design (the UI rectangle of `policy/tv.rs`), which keeps its text 10 % from the side edges; the top and the bottom have only the theme's margin. A game is placed as its console placed it. No setting moves or sizes the picture. The first test on a tube (Samsung, composite, Pi 3 B, 2026-11-14): the game picture reached the edges of the visible area with correct geometry; the menu shows a black band at each side (OP-102 point 1).
- Where: `crates/neptron-ui` (the theme's safe area), `crates/neptron-modeline` (on RGB the porches of the modeline set the size and the position at no cost in sharpness; on composite only the placement inside the fixed 720 samples can change), the settings schema
- How to change: one setting for the menu's safe area on a 15 kHz profile, and picture position and size settings: through the timings on RGB, through the placement on composite. Check on more tubes what is cut.

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

### OP-100: Keep the video rules few, and check them against a real library.

- Raised by: the owner (2026-11-13)
- Status: open; the owner accepts the rules as they are
- Default: three independent rule sets, one for each kind of display (CRT, flat panel, TV), and no rule for a single game. Each rule works from the geometry and the rate that the core reports. The last proposal of every plan always fits, so a game always gets a mode. The points that carry risk:
  1. The width rule for composite has the most special cases: a general rule, a console case for 4:3 pictures of 224 to 240 lines, a snap to whole and half factors, and a 3 % tolerance. One TV and a few games (2026-11-14) are not a proof for every source.
  2. The session takes a source of 289 lines or more as interlaced (except an `always_progressive` system). This is a guess from the line count.
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

### OP-102: First composite picture on a Pi 3 B, and what it showed.

- Raised by: the owner's first test on a tube (Samsung, composite, Pi 3 B, 2026-11-14)
- Status: open for point 1 (after 0.1, with OP-097); point 2 is closed by commit `0c7e3d3a` (M10), with its check on a Pi 3 open in the hardware checklist; point 3 is closed by commit `fd713d9`
- Default: the owner reports (2026-11-15, image of commit `7f2e31e`): the picture locks in NTSC 240p, the menu is sharp, the 240p test suite grid fills the tube with correct geometry, SNES and Mega Drive games are very sharp, and the audio is fine. The points:
  1. The menu is 640 samples wide in the 720 samples of the line, so the tube shows a black band at each side of the menu. This is the UI rectangle of `policy/tv.rs` by design, not the TV. A wider menu needs the overscan decision of OP-097.
  2. The audio list labels the PCM of the `vc4-hdmi` card "Analog", because the kind of a PCM comes from its name only, and its name (`MAI PCM i2s-hifi-0`) has no HDMI word (`crates/neptron-audio/src/alsa.rs`). With no jack card, `auto` then took the HDMI card as an analog output, and the open failed with error 524 while HDMI was off.
  3. Closed: `os/board/rpi/config.txt` puts `dtparam=audio=on` before the overlay line (after it, no jack card), and a test pins the order.
- Where: `crates/neptron-modeline/src/policy/tv.rs` (UI rectangle), `crates/neptron-audio` (the output list and its labels), `os/board/rpi/config.txt`
- How to change: for 1, a setting for the menu width on a TV, or the result of OP-097. For 2, take the kind of a PCM from its card driver (`vc4-hdmi` is HDMI) as well as from its name, and do not offer an HDMI output while its connector is off.

### OP-103: Frame budget on the Raspberry Pi 3, and what still costs fields.

- Raised by: P10 and P11 (measured on a Pi 3 B, composite, 2026-11-14)
- Status: open
- Default: NES, SNES and Mega Drive games hold the field rate with no missed field. The CPU governor of the Pi image is `performance`. The points that stay open:
  1. P12 fixed the dialog over a running game: the canvas is drawn only when its scene changes, the GPU gets only the changed canvas rows, and the game is drawn only outside the UI. A dialog frame ends 8.4 ms after the flip event (a game frame alone: 9.9 ms); 0 missed fields in 36 NTSC dialogs over the SNES 240p Suite. Still open: an unchanged canvas costs a comparison of its rows in the renderer (about 0.35 ms a frame), because the `Display` contract does not say that the UI did not change.
  2. A change of the TV standard takes about 75 ms from the last flip of the old mode to the first flip of the new one. The muted audio queue covers about 32 ms, so it runs dry in silence and the underrun count goes up by one.
  3. Super Mario World leaves 1.66 ms in its worst frame with the development build (`os push`, no LTO). The release image is not measured yet.
  4. The source texture grows at the first frame of a larger size, because the `Display` contract does not carry the maximum geometry of the core. A game that changes its size in play makes one allocation at that moment.
  5. The DRM vblank sequence of vc4 in 480i is not checked: if it counts frames and not fields, `fields_missed` hides real misses in interlace.
  6. `neptronctl screenshot` answers `unavailable` on the image, because the build has no PNG encoder.
- Where: `crates/neptron-session` (frame loop), `crates/neptron-ui` and `apps/neptron` (the dialog path), `crates/neptron-audio` (mute cover), `crates/neptron-gfx/src/gles`, `os/board/rpi/system.config`
- How to change: for 1, add a UI generation number to `Scene` so the renderer skips the row comparison (a contract change, so an ADR). For 2, a longer muted cover, which adds delay after a short modeset. For 3, measure with the release image. For 4, pass the maximum geometry through the contract (an ADR). For 5, the hardware check in the checklist.

### OP-104: What the CI does not cover.

- Raised by: the CI task (2026-11-14)
- Status: open
- Default: the runners for arm64 have no KVM, so the aarch64 guest runs under emulation, and the checks that measure time cannot pass there. `kms-check`, `selftest-check` and `core-trace` run in CI on x86_64 under KVM only. For aarch64 they run on a developer machine with a hypervisor. The Raspberry Pi image is built and its layout is checked, but no guest boots it. Two paths of the workflow are not proven: the resume of a build from a partial base, and the downloads cache, which only a run on `main` saves. The check `ci-ok` is not yet a required check on `main`; the owner sets it. The storage cost of the base artifacts (about 5 to 9 GB each) on a public repository is not confirmed.
- Where: `.github/image-targets.json`, `.github/workflows/images.yml`, `docs/ci.md`
- How to change: add a self-hosted arm64 runner with KVM and add `kms`, `selftest` and `core` to the aarch64 entry of `.github/image-targets.json`. Force a partial base once and read the run. Read the billing page after some days.
- Decision: the owner: the three timing checks run on x86_64 only (2026-11-14).

### OP-105: The keep or revert dialog of the display behaves wrongly.

- Status: closed by M01 (2026-11-16): `docs/spec/display-guard.md` and ADR 0013; the hardware checks are in `docs/hardware-checklist.md`.
- Decision: the owner accepted trust by display class (an EDID flat panel and a composite TV get no boot prompt), a 15 s window from the first settled picture, a failed boot only for a run that ends abnormally before its first picture, one record per place, and two rows to leave the safe mode. The causes found: the boot prompt asked to confirm the safe default itself, a return from a game was a new boot, and the record held one place only.

### OP-106: Open questions of the arcade spec.

- Raised by: M06 (2026-11-16)
- Status: open; each question has a default in `docs/spec/arcade.md`, section 19
- Default: the questions A1 to A6 of the arcade spec. A1 (`roxmltree`, xtask only) and A2 (the install step of `os/package/neptron` runs the index generator) are built. A3: block only when no Neo Geo BIOS file matches. A4: one-time notices are stored in `state/config/notices.toml`. A5: PAL composite runs arcade boards 9 to 16 % slow, accepted with the speed alert. A6, the shape of the TATE setting, is decided in OP-107. The owner decided on 2026-11-16: FinalBurn Neo on every board, no donations, 480i as the default for a vertical game on a horizontal 15 kHz CRT, the speed change on composite accepted. The Pi 3 measurement is in the arcade spec, section 2.
- Where: `docs/spec/arcade.md`, ADR 0014, 0015 and 0016
- How to change: an owner decision per question.

### OP-107: TATE mode: a monitor on its side, with the UI and the games turned.

- Raised by: the owner (2026-11-16); important to him, planned after the MVP
- Status: open; not on the taskboard of the MVP, which holds only the tasks of version 0.1
- Default: version 0.1 shows a vertical game upright on a horizontal display (rotation in the renderer, ADR 0015). It has no mode for a monitor that stands on its side: the menu is not turned, and a vertical game is not shown at its native lines on such a monitor.
- Where: `docs/spec/arcade.md` (the TATE section has the design notes), ADR 0015, `docs/definition/06-flat-panels-shaders-and-vertical-games.md` (`screen_orientation`)
- How to change: a task of the first taskboard after the MVP. It covers a setting for the screen orientation with a direction (left or right), the UI drawn turned, a vertical game at its native lines with no rotation (240p at the game's own rate on 15 kHz), a horizontal game turned or refused on such a screen, the input directions turned in the menu, and the boot picture.
- Decision: Owner, 2026-11-19: TATE is the first task after version 0.1. One system setting gives the screen orientation: 0 (YOKO), 90, 180 or 270 degrees, and the UI follows it. In Auto a vertical game runs at its own lines, full screen; a horizontal game shows upright and small, with a notice. Each game has an orientation override (Auto, 0, 90, 180, 270) in its display settings.

### OP-108: Arcade leftovers after the library tasks (M07a, M07b, M07c).

- Raised by: M07b, M07c and their reviews (2026-11-16, 2026-11-17)
- Status: open
- Default: no diagnostics page exists, so the set check writes the list of wrong files to the log, and no line names a `mame` folder, an unknown set or the state of the index. `data/cores/fbneo.toml` does not set `fbneo-diagnostic-input`, because the free test set does not declare that option; the service menu stays on the core default, hold Start. A launch through the control protocol skips the set check. A local build after a new pin of the core keeps the old index until the `neptron` package is built again (CI always does); the image check proves only that the index exists. On a boot with the state folder in RAM, the warning about it comes again in each game run. No x86_64 trace of FinalBurn Neo and no trace of a set with a missing ROM exist. The test of the Back key in the versions list asserts only the screen name. The texts of messages 4, 4b and 5 of the set check are the owner's (2026-11-17) and are in `docs/spec/arcade.md` section 10.
- Where: `apps/neptron`, `crates/neptron-ui`, `crates/neptron-library`, `data/cores/fbneo.toml`, `os/package/neptron`, `tests/traces`
- How to change: a diagnostics page as its own task; a trace of a Neo Geo set that declares the diagnostic input before the key goes in the data file; a version compare in `check-arcade-index.sh`.

### OP-109: Video policy: questions for the owner and leftovers of the policy task.

- Raised by: the policy task of M08 and its reviews (2026-11-16, 2026-11-17)
- Status: open
- Default: the decisions below are built (M08d). V8: a game wider than the face with too few lines (Virtual Boy on `ntsc`, a 3:2 source of 224 lines on a 15 kHz CRT) goes to interlaced 1:1 at about 50 % of the height. The overscan crop is bound by 8 source lines and by 8 of 240 lines of the screen per edge. The fill rule of a flat panel applies to every source. The app sets `PolicySettings::arcade` for the systems that run arcade boards (`apps/neptron/src/shell/boot.rs`) and shows the notice reasons (`apps/neptron/src/shell/picture.rs`). Shaders for flat panels would replace the hard scanlines; they are planned after the MVP. Low findings of the last review: the continuity exception of the sweep covers every plan that has a window rung; `onto_one_field_rate` does not try the GPU floor for a wider multiple before the validator; the diagnostic of `interlaced_rung` gives the source lines, not the shaped lines.
- Where: `crates/neptron-modeline`, `crates/neptron-video`, `apps/neptron`, `docs/spec/video-policy-cases.md`, `docs/spec/arcade.md` sections 11 and 12
- How to change: an owner decision per question, then a golden change with its reason.
- Decision: Owner, 2026-11-16: with scanlines on, a flat panel uses the largest whole multiple and never the fill; every picture that is not exact shows a notice. Owner, 2026-11-17: V6 takes the exact lines (the slower exact rung wins over the fit, for example DOS 640x400 on a TV); V7 takes 2x with the squeeze of 5 % (WonderSwan); V8 stays interlaced 1:1; the crop bound stays scaled to the screen; the fill of a flat panel stays for every source. M08d built V6 and V7.

### OP-110: Input for four players: owner decisions and leftovers.

- Raised by: M04 (2026-11-17)
- Status: open
- Default: a pad gets its port by plug order, and a replug in the menus changes the order. Every pad and every keyboard controls the menus. Pads 5 to 8 drive only the menus and the hotkeys, and a ninth pad is refused until it is plugged again. Every port is a plain joypad: no multitap and no Four Score, so a game that needs one sees two pads. Every keyboard is player 1. Two pads of one model that both come back at new paths in a game can swap ports. The service menu of FinalBurn Neo stays on the core default (OP-108). The notices of the input backend go to the log and to the player. The image ships the community gamepad database (ADR 0017), and the guided mapping stores layouts in `config/input/pads.toml`.
  - Left: a second pad that nobody touches gets the silent pad notice once per power cycle; a pad notice that is pushed out of a full notice list does not come back in that run; `usbhid.jspoll=1` is not on the kernel command line.
  - Left by the mapping: no remapping during a game (the page is read only there); an input used twice is refused, not swapped; two pads of one model with an empty path share a key; the crawler does not reach the review dialog; no per-game or per-system layout (the file leaves room); nothing of the input work of 2026-11-17 ran on hardware.
- Where: `docs/spec/input-players.md`, `docs/spec/input-remapping.md`, `crates/neptron-input`, `apps/neptron`
- How to change: a task of its own for four-player adapters, with the device type per system in `data/systems`.
- Decision: Owner, 2026-11-17: a way in Settings > Input to pick the pad of each player is wanted, with a good UI (task M05b). Every pad controls the menus. More than four pads as built, for now. Four-player adapters stay open. The service menu key keeps the core default. Owner, 2026-11-19: M05b is dropped; the plug order stays (OP-114).

### OP-111: Findings on the PC of 2026-11-17.

- Raised by: the bench session on the i3-6100T (2026-11-17)
- Status: open
- Default:
  - The PC image booted with the `powersave` governor and FinalBurn Neo missed fields; `os/board/pc/system.config` sets `performance`, and the result on the PC is a line of the hardware checklist.
  - A Mad Catz FightStick Tournament Edition for PS3 (`0738:8838`) on a direct port of the PC sends only neutral reports (it works on a Mac behind a USB hub). The PS3 start request (feature report, reply `21 26 01 07`), the two Sony requests and an LED output report do not change that. The cause is the default enumeration scheme of Linux: its first read of 64 bytes at address 0 leaves the firmware of the stick with no button data until the stick loses power. With `usbcore.old_scheme_first=1` the stick gives input in each trial that starts with a physical replug (owner trials, 2026-11-18); the images boot with that parameter (OP-112). A test that enumerates a device again without a cut of its power does not show this.
  - The cold build fault of the PC image with the 15 kHz set ("No hash found for 01_linux_15khz.patch" in the kernel headers step) is fixed: `linux-headers.hash` is a link to `linux.hash` (M12), proven by CI (OP-114). The base key of the CI image cache hashes the kernel and BusyBox configuration fragments of each target.
  - The image builds SDL3 without HIDAPI and without libudev.
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
- Default: `cargo xtask os push --data` pushes the arcade index and `input/` since M11 (`DATA_DIRS`, `xtask/src/os/bench.rs`); a push without `--data` keeps the data of the image. The notice of a game that did not start is written to the log two times, by the game run and by the menu run. A set with a disk image passes the set check when the file is present; NEPTRON does not read it, and no game with a disk image is a tested board family: in the guest, FinalBurn Neo ran Killer Instinct for 5 minutes on a file of zero bytes with no stop. In a game, a blind recovery hold opens the quick menu at 0.5 s and the recovery fires at 5 s under it; the owner was told. The hold time of the quick menu is a constant.
- Where: `xtask/src/os` (`DATA_DIRS`), `apps/neptron/src/start_failure.rs`, `crates/neptron-library`, `crates/neptron-input/src/hotkeys.rs`
- How to change: a SHA-1 check of a disk image as a diagnostics job; a setting for the hold time if the owner wants one.
- Decision: Owner, 2026-11-18: the texts of message 8 and of the notices S1, S2 and S3 are approved; the quick menu opens after a hold of 0.5 seconds.

### OP-114: Decisions and findings of 2026-11-19.

- Raised by: the owner and the bench session on the i3-6100T (2026-11-19)
- Status: open
- Default:
  1. M05b, the picker of the pad of each player, is dropped: the plug order stays.
  2. The M12 gate is met in CI: the first run with the 15 kHz set (commit `62029d3e`) built the PC image from an empty output and passed the patch check, the boot with BIOS and UEFI, kms-check, the GLES comparison, the video self-test, the core trace and the share check. The local release image of 2026-11-13 is older than the test tooling, so the local tier 3 commands need a new one.
  3. Killer Instinct (FinalBurn Neo, a MIPS interpreter with no recompiler at the pinned commit) needs about 1.0 to 1.5 field periods for one frame on the i3-6100T at 3.2 GHz. The result after M16 is in OP-115.
  4. `data/hardware/matrix.toml` has no entry for the HDMI output of the Raspberry Pi; the flat panel path checks the panel's own mode list, so the effect is small.
  5. A setting file can still name `display.vertical` and `display.panel_scale`; the menus do not show them (expert keys).
- Where: `docs/taskboard.toml`, `.github/workflows/images.yml`, `crates/neptron-session`, `data/hardware/matrix.toml`
- How to change: a new release image for the local tier 3 commands; an entry for the Pi HDMI output when its limits are measured on the bench.
- Decision: Owner, 2026-11-19: the notice texts of M16 are approved ("This game runs at {p}% speed. This machine is too slow for it." and "Sound off: the game runs at less than half speed."); when Auto finds no picture for a game, the Fill picture is the default (built in M08e).

### OP-115: Decisions and findings of 2026-11-23.

- Raised by: the measurement of M16 on the bench PC and the closing review of M08e (2026-11-23)
- Status: open
- Default:
  1. M16 is merged at the build that the owner tested. On the i3-6100T at 3.2 GHz, Killer Instinct in its attract mode gives 58.6 frames per second over 120 s with 165 missed fields and 27 audio underruns. A dropped later round (a bound of 4 underruns in `a_fast_core_with_a_spike_every_10_frames_is_measured`, where the merged test allows 15, and a changed speed measure at the start of a heavy scene) gave 48.3 frames per second, 1402 missed fields and 76 underruns on the same run, and the simulation did not show the loss. The few audio underruns of a heavy scene stay.
  2. The picture answers of a menu page are made at 2 decisions per frame. One decision that falls back to Fill on a 31 kHz CRT walks three plans; the estimate for a Pi 3 from the dev profile is 11 ms for that decision, which is more than a frame can give. The work runs only while a menu shows, never during play.
  3. The screensaver of M15 is a black screen with the NEPTRON wordmark that moves and takes the next colour of a fixed palette at each wall (`docs/spec/screensaver.md`).
- Where: `crates/neptron-session/src/pacing.rs`, `crates/neptron-session/src/speed.rs`, `crates/neptron-video/src/answers.rs`, `docs/hardware-checklist.md`
- How to change: for (1), first give the fake audio sink the period and the blocking of the real device so the loss shows in a test, then take up the speed measure again; for (2), measure `policy_cost` on a Pi 3, and if needed give the step a budget in plans and make the fallback of `decide_shown` resumable.
- Decision: Owner, 2026-11-23: merge the tested build of M16 and drop the last round; the screensaver starts after 5 minutes in the menu, a paused game and the quick menu, never in a running game, with the settings On or Off and 1, 5, 10 or 30 minutes. M09 and C19 move after 0.1. The screensaver word has twice the font size and moves one pixel on each axis in each frame.

### OP-116: Release review of 2026-11-23, and what waits until after 0.1.

- Raised by: eight read-only reviews of `main` before release 0.1 (monitor safety, mode policy, user data, core host and `unsafe`, frame loop and audio, input, OS layer, release and licences)
- Status: open for the points of 2; the fixes of 1 are on `main` (2026-11-23)
- Default:
  1. Fixed before 0.1, each in its own task: a named 15 kHz profile falls back to a mode from a false analog EDID, and the blind recovery keeps the safe mode of the monitor of the boot; the PC kernel has IPv6 and the share has no address limit; the share and a damaged state partition get no check or repair, a RAM folder has no size limit, the save backup covers one write and not a session, and two games of one system with the same file name share a save; a guided mapping is lost when a held A or B ends the last skip, a damaged `pads.toml` map is applied with no check, and a joystick with 2 buttons gets no Accept; a pick of the refresh rate removes `display.vertical` and `display.panel_scale`; the licence fields, notices and sources archive of the images are not complete.
  2. After 0.1: the confirm or revert of a pending mode when the frontend hangs (the mode is a validated mode of the named profile); the sync polarity of a CRT mode is not checked against the profile; no notice when the sound device goes away; a display that stops for 5 s ends the game; the sound plays in bursts while the display does not flip; a modeset at each change between the 256 and 512 widths of the SNES on a GPU with a pixel clock floor, and no limit on the rate of modesets from a source that changes; the power button of a PC does nothing; two sticks of one build variant share a root PARTUUID; a PC with an AMD GPU has no firmware in the image; the menu on a TV has a black band at each side (OP-097, OP-102 point 1). Added by the fix tasks: ADR 0013 makes `generic_15` the last resort for a named 25 or 31 kHz profile with no usable EDID, and no document says that 15 kHz is harmless for a fixed 25 or 31 kHz arcade chassis; a hot plug with no recovery keeps the old mode on the wire; the `.bak` of a bad save is replaced at the next session that changes the save; no notice when `/state` goes read-only in a game; a first format of the share that a power cut interrupted needs a format on a computer; the exit codes of `fsck.exfat` are not verified against its source; `drm-fourcc` and `gbm-sys` ship no licence text; the written source offer has no contact that needs no account.
  3. The core host and the frame loop have no finding above minor. In release, `panic = "abort"` ends the process, so the comment in `neptron-libretro/src/ffi.rs` about `catch_unwind` is wrong for the shipped binary.
- Where: the findings are in the task reports of the fix tasks; `apps/neptron/src/display.rs`, `os/board`, `crates/neptron-session`, `crates/neptron-input`, `apps/neptron/src/config.rs`, `os/package`
- How to change: each point of 2 is a task of its own after 0.1.
- Decision: Owner, 2026-11-23: the menu width on a TV waits until after 0.1.

### OP-117: Decisions of 2026-11-24.

- Raised by: the owner (2026-11-24)
- Status: decided
- Default: release 0.1.0 is an internal milestone: a tagged state of `main` with images that the release workflow built and checked. It need not be perfect. Versions 0.x are published and anyone can use them; the release for the public is 1.0. TATE mode is the first task after 0.1 (OP-107), and the points that wait until after 0.1 (OP-116, and the list in `docs/definition/13-roadmap.md`) are the work list toward 1.0. The bench monitor on VGA is a Samsung SyncMaster 750s, a 31 kHz PC CRT that never gets 15 kHz; the 15 kHz display is a consumer TV on RGB SCART or composite.
- Where: `.github/workflows/release.yml`, `docs/hardware-checklist.md`
- How to change: a release 0.x is a GitHub pre-release until 1.0.
- Decision: Owner, 2026-11-24: 0.1.0 is an internal milestone and the public release is 1.0; the notice texts of the review fixes of 2026-11-23 are approved. Task C19 changes: the saves move to the share partition, in a `saves` folder beside `roms` and `bios`, so one share holds all three and a stick in a computer shows the saves; there is no second network share. The owner chose this over a mount of the state partition's saves inside the share, and knows the cost: exFAT has no journal, so a power cut can damage a save, and a share that mounts read-only needs a fallback for the saves. It is one of the first tasks after 0.1, with TATE mode. On a turned screen the menu has its own portrait layout (240 wide, 320 high); it is never squeezed and never interlaced to fit. This is a hard requirement of the owner, and it settles questions T3 and T4 of `docs/spec/tate.md`. The UI becomes responsive: the layout follows the canvas size, because later versions add box art, themes and a high-definition UI. The owner also decided on 2026-11-24: orientation 90 and 270 get the portrait menu, and 180 keeps the landscape menu, turned; a flat panel mounted in portrait gets the portrait menu, scaled up sharp; the quick menu over a horizontal game on a turned screen is the portrait menu, with the small game behind it; a text that does not fit a line of the portrait menu wraps, and gets a shorter wording only where a wrap looks bad. Run rule 20 of `CLAUDE.md` (product questions first) comes from the owner's request of the same day. Picture mode (task N08): the option "Fill" is renamed "Fit". Fit shows the whole picture, as large as the screen allows, at its right shape, never stretched and never cut: a vertical game on a horizontal screen fills the height, a horizontal game on a turned screen fills the width, and a 256-line game on NTSC shows all its lines in 240. An uneven line scale exists only in Fit and is always smoothed and progressive; a whole-step option never gets one. Auto stays the default and keeps 240p with the 8-line crop on NTSC. Texts: help "The whole picture, as large as the screen allows. The picture can be soft."; notice "Fit picture: Auto has no picture for this game here."
