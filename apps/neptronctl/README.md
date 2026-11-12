# neptronctl

Command-line client: print a modeline, test a mode, self-test, diagnostics. It talks to the frontend on the control socket.

Each command sends one request of the control API (see `crates/neptron-ipc/README.md`) and prints the `data` of the reply as one JSON line on stdout. The client asks at its own protocol version, and prints the fields of that version. A server answers only with names of the request version, so nothing that a correct server sends is lost.

## Usage

```text
neptronctl [--socket PATH] [--timeout-ms N] COMMAND

  hello
  screenshot ui|output|scanout --out FILE
  ui tree
  input send PORT CONTROL VALUE
  input script FILE
  clock step FRAMES
  wait screen ID|state STATE|frame N [--within-ms N]
  session launch CORE ROM
  session state
  video state
  stats
  settings get KEY [--level LEVEL]
  settings set KEY LEVEL VALUE
  log tail [LINES]
  selftest video --profile ID|edid [--monitors DIR] [--sysfs DIR]
                 [--connector NAME] [--report FILE] [--device PATH]
                 [--seconds-per-mode N]
                 [--i-have-checked-the-monitor ID]
  help
```

- The socket is `--socket`, else `$NEPTRON_CONTROL_SOCKET`, else `/run/neptron/control.sock`.
- The client waits 10 seconds for a reply, or `--timeout-ms`, which must be above 0. A `wait` waits 10 seconds for its condition, or `--within-ms`, and the client then waits 5 seconds more for the server's `timeout` answer.
- Names (stage, control, state, level) are the wire names: `ui`, `left_x`, `quick_menu`, `global`.
- `screenshot` writes the PNG to `FILE` and prints `{"stage","width","height","path","bytes"}`.
- `input script` reads a JSON array of events, each `{"frame","port","control","value"}`.
- `settings set` reads `VALUE` as JSON. A text that is not JSON is a string, so `native` and `"native"` are the same value.

## Exit codes

Scripts and tests depend on these codes. A new code can be added; a code never changes its meaning.

| Code | Meaning | `error.code` on stderr |
| --- | --- | --- |
| 0 | The command succeeded | none |
| 1 | The server answered with an error | the server code, for example `unavailable` or `timeout` |
| 2 | The command line does not follow the usage | `usage` |
| 3 | The client cannot connect to the socket | `connect` |
| 4 | No reply in time, the connection broke, or the reply is not valid | `no_response`, `transport` or `protocol` |
| 5 | A local file cannot be read or written | `file` or `stdout` |
| 6 | The self-test cannot start: the preset cannot be read, there is no EDID profile, the run is refused, the build has no KMS backend, or the DRM device cannot be used | `selftest_setup` or `selftest_device` |
| 7 | The self-test ran and a check failed; the report says which | `selftest_failed` |

An error prints one JSON line on stderr: `{"error":{"code":"...","message":"..."}}`.

## The video self-test

`selftest video` plans one test mode for each mode family that the monitor profile supports, prints the plan and writes a JSON report. A dry run is the default: it opens no device and sets no mode. With `--i-have-checked-the-monitor ID` it shows each mode on the DRM device for 10 seconds with its read back, and restores the display at the end.

- `--profile ID`: a preset of the monitor folder, read at run time (`--monitors`, default `/usr/share/neptron/monitors`). `--profile edid`: the profile from the EDID of the connector. There is no default profile.
- `--sysfs DIR` (default `/sys`): the GPU, its capability table entry and the connector come from sysfs, as in the frontend. With nothing detected, `validate` gets the values of an unknown GPU. The EDID profile also lowers the highest pixel clock to the EDID limit.
- `--connector NAME`: the connector, for example `DP-1`. Without it, the first connected connector.
- `--report FILE` (default `neptron-selftest-video.json` in the current folder): the JSON report. The dry run writes it too, with `dry_run` true.
- `--device PATH` (default `/dev/dri/card0`): the DRM device of the run on a display.
- `--seconds-per-mode N` (1 to 10, default 10): the display time of each mode. A test option: the check on `vkms` uses 2 s. A person needs the 10 s to read the pattern and to see that the monitor holds the mode.
- `--i-have-checked-the-monitor ID`: the run on a display. `ID` must be the id of the profile that the dry run prints (for the EDID profile, `edid_<manufacturer>_<product>`), so the person names the monitor whose ranges they checked.

### Safety of the tool

- Every mode is a `ValidatedMode` from the policy and `validate`. The tool has no flag that widens the GPU values or skips the validator. The display gets nothing else, except `blank()`, which turns the output off.
- The dry run is the default. The run on a display needs the confirmation flag with the profile id; another id is refused.
- The run is refused when the frontend holds the display: the control socket (`--socket`, `$NEPTRON_CONTROL_SOCKET` or `/run/neptron/control.sock`) accepts a connection, a process named `neptron` or `neptron-launch` runs (`/proc/*/comm`), or `NEPTRON_DRM_FD` is set. The tool runs with the frontend and the launcher stopped and opens the device itself, read and write; it does not run under the launcher, because the launcher would start it again after a failure. The device must be a DRM character device (major 226).
- The report is written before the device opens, so a report that cannot be written stops the run before any modeset.
- The connector: `--connector`, else the backend's connector rule. The GPU values and the EDID profile come from the card and the connector that sysfs detection chose. The run is refused before any modeset when `--device` (with links followed) is the node of another card, or when the backend chooses another connector.
- SIGINT, SIGTERM and SIGHUP set a stop flag (`signal-hook`, no `unsafe`). The run reads it before each flip and stops within one flip wait (at most 500 ms on KMS), then turns the output off. SIGKILL runs no code: the kernel closes the descriptor, and the validated test mode can stay on the wire with no picture, or the driver turns the CRTC off.
- A panic in the dev profile unwinds into the drop of `KmsDisplay`, which turns the plane off with the validated mode kept, or the output off. A release build aborts on a panic; then the last mode on the wire is a validated mode.
- Limits, each a named constant with its unit: `SECONDS_PER_MODE_MAX_S` (10), `SECONDS_PER_MODE_MIN_S` (1), `FLIP_WAIT_RETRIES` (2), `FLIPS_PER_MODE_MAX` (2000), `FLIPS_MIN` (2), `VBLANK_PERIOD_TOLERANCE_PPM` (10 000), `PATTERN_WIDTH_MAX_PX` (512, from `TEXTURE_SIDE_MAX_TEXELS` 2048 and `TEXELS_PER_PIXEL` 4).

### On the bench

Build `neptronctl` for Linux with the `kms` feature (the image task installs it; on a development machine, `cargo xtask os selftest-check` builds it for the guest). On the target, as root, with the frontend and the launcher stopped:

```sh
neptronctl selftest video --profile pc_31_120 --connector DP-1 --report /tmp/dry.json
neptronctl selftest video --profile pc_31_120 --connector DP-1 --report /tmp/selftest.json \
    --i-have-checked-the-monitor pc_31_120
```

The first command is the dry run: it prints the ranges, the GPU values and the modes. The second shows each mode for 10 seconds. The report is the file of `--report`. `docs/hardware-checklist.md` says what to check and what to write down.

The families, in the order they show: `240p`, `288p`, `240p_double_refresh`, `480i`, `576i`, `480p`, `2x`, `super_width_dynamic`, `super_width_fixed`. Each family is a game source and the policy settings that ask for it (`src/selftest/family.rs`). The policy decides the mode (`decide`, then `choose` with `neptron_safety::validate`), and the family takes the winner only when it belongs to the family: for example a 240-line progressive mode with one field per game frame for `240p`. A family with no such winner is "not tested", with the policy's mode and reason. A family whose mode equals the mode of an earlier family is "the same mode as" that family, so no mode shows twice.

On `pc_31_120` with the values of an Intel gen9 GPU the plan is 240p at 120 Hz (`240p_double_refresh`), 480p (`2x` gives the same mode), 768x224 at 120 Hz (`super_width_dynamic`) and 1920x224 at 120 Hz (`super_width_fixed`).

### One mode on the display

`src/selftest/show.rs` runs on any `Display` and `Clock`: the KMS backend in the real run, the fake display of `neptron-testkit` in the tests.

1. `set_mode` with the mode's `ValidatedMode`. On KMS this is a test-only commit, then the real one. `ModeRefused` is a refused test-only commit, and a device error of the operation `modeset commit` (`KMS_COMMIT_OPERATION`) is a refused real commit: a commit is all or nothing, so the mode before stays, and the run goes on with the next mode. Any other error of `set_mode` leaves the display state unknown, and the run stops and turns the output off.
2. The read back is compared field by field with the request and validated again. A read back that fails `validate` stops the run at once and turns the output off. A read back that differs but passes fails the mode, and the run goes on; the pattern does not show, because it is drawn for the request.
3. The pattern shows for `seconds_per_mode`: `present`, then `wait_flip`, until the clock passes the time. The pattern is a white border on the outermost pixels of all four edges, a centre cross, eight colour bars at 75 %, and four lines of text: the family, the size and scan, the line and field rates, the pixel clock. The text is a 5x7 font of the tool (`neptron-ui` and its font are only for the app). The frame is at most 512 pixels wide (`PATTERN_WIDTH_MAX_PX`): the GLES renderer uploads each byte of a row as one texel, so a pixel takes 4 texels (`TEXELS_PER_PIXEL`), and 2048 texels (`TEXTURE_SIDE_MAX_TEXELS`) is the `GL_MAX_TEXTURE_SIZE` of vc4, the smallest of the supported GPUs. A wider mode gets a frame narrower by a whole factor (`width_factor`: the smallest that divides the width, when one fits), which the renderer scales back with nearest sampling. The border stays on the outermost pixels of the mode, and no present fails on the texture size. A failed wait is tried `FLIP_WAIT_RETRIES` (2) more times; then the flip stays queued and the run stops.
4. The timing comes from the kernel's vblank counter and time stamps of the flip events: the mean vertical blank period over the whole time, the expected period of the mode (a field; for an interlaced mode a field when the capability table gives `flip_per_field`, and a frame when it does not; for a GPU that the table does not know, the nearer of the two, with `unit_assumed` true; `vblank_unit` says which), the error in ppm, the pixel clock that the period implies, the flips per second and the vertical blanks with no flip.
5. The stop flag (SIGINT, SIGTERM, SIGHUP in the real run) is read before each flip and before each mode.

At the end, on a stop and on an error with the display state unknown, the run calls `blank()`: the output goes off. The state before the run cannot be read back and validated through `KmsDisplay`, so no signal is the restore, as the safety model allows. With a flip still queued, the backend refuses the blank as busy and the report says so; the backend's drop then turns the plane off with the validated mode kept, or the output off.

A mode passes when (`src/selftest/verdict.rs`): both commits passed; a mode was read back, equal to the request in every field and valid; the pattern showed for the whole time; at least 2 flips completed (`FLIPS_MIN`); the vertical blank period is within 1 % (`VBLANK_PERIOD_TOLERANCE_PPM`, 10 000 ppm) of the mode's period; and no display error came. Missed vertical blanks do not fail a mode: they show how fast the tool draws (software GL in the QEMU guest misses most), and the period does not depend on them. The run passes when it finished, it had at least one mode, every mode passed, the restore succeeded, and no other error came.

### The report

One JSON file, written as a temporary file in the same folder, fsync, rename, and fsync of the folder, so a reader sees a whole report or the one before. `report_version` is 2; version 2 adds `timing.unit_assumed`, and a version 1 report reads it as false. The format is append-only: a new field gets a new version, and no field is renamed or removed. Every value is an integer or a text: rates in millihertz, times in nanoseconds, clocks in hertz. A value that the run did not measure is `null`.

| Field | Meaning |
| --- | --- |
| `report_version` | 2 |
| `tool` | `name` and `version` of the program |
| `dry_run` | True when no mode was set |
| `state` | `planned` (dry run), `running` (written before the first mode and after each mode), `finished`, `stopped` |
| `pass` | True when every mode passed and the restore succeeded; `null` in a dry run and while running |
| `seconds_per_mode` | The display time of each mode |
| `profile` | `id`, `source` (`preset` or `edid`), `edid_pclock_max_hz`, and the whole `profile` with its ranges |
| `caps` | The GPU values that `validate` got |
| `gpu` | From sysfs: `card`, `driver`, `pci_id`, the capability table `table_entry` and `table_name` (`null` for an unknown GPU), the `connector` and its `connector_status` |
| `display` | From the KMS backend: `device`, `driver`, `connector`, `linear_by_modifier` |
| `families[]` | Each family: `family`, `result` (`planned`, `same_as`, `unsupported`), `same_as`, `reason` |
| `modes[].family`, `.title`, `.policy` | The family, the text on the pattern, and the policy facts: `source`, `style`, `refresh_policy`, `horizontal`, `rung`, `refresh_kind`, `repeat`, `width_rule` |
| `modes[].request`, `.request_rates` | The mode that `validate` approved, with `line_rate_mhz` and `field_rate_mhz` |
| `modes[].test_only_commit`, `.commit` | `result` (`passed`, `refused`, `failed`, `not_sent`, `unknown`) and `detail` |
| `modes[].readback` | The mode the kernel reports after the commit |
| `modes[].readback_equal`, `.readback_differences[]` | Equal in every field, and each field that differs: `field`, `requested`, `read_back` (a flag is 0 or 1) |
| `modes[].readback_valid`, `.readback_rejection` | The read back passes `validate`, or why not |
| `modes[].shown_ns` | How long the pattern showed |
| `modes[].timing` | `flips`, `vblanks`, `missed_vblanks`, `flips_per_second_mhz`, `vblank_period_ns`, `expected_period_ns`, `vblank_unit` (`field` or `frame`), `period_error_ppm`, `implied_pclock_hz`, `unit_assumed` (version 2) |
| `modes[].errors`, `.pass`, `.failures` | Display errors in the mode, the verdict, and each check that failed |
| `restore` | `method` (`blank`), `ok`, `error`; `null` when no mode was set |
| `stopped`, `errors` | Why the run stopped early, and errors outside one mode |

## Argument parsing

The parser is written by hand. The grammar is a few fixed words, and a parser crate costs more compile time than it saves.

## Dependencies

`neptron-ipc` for the control API. For the self-test: `neptron-modeline` (the policy), `neptron-safety`, `neptron-types`, `neptron-backend` (the `Display` and `Clock` traits), `neptron-platform` (detection, capability table, EDID profile) and `toml` (the preset files). With the `kms` feature: `neptron-kms` and `signal-hook`. For the tests: `neptron-testkit`. `neptron-modeline` links Switchres, so the binary is a combined work under GPL-3.0-or-later, as the frontend is (ADR 0002). The presets stay files on the image and are not embedded (OP-044).

## Must never depend on

- `neptron-ui`
- Any async runtime

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `kms`, off by default. With it, on Linux, `selftest video` shows its modes through `neptron-kms` (`KmsDisplay`), with `signal-hook` for the stop signals. Without it, the run on a display ends with exit code 6 and the dry run works.

## Status

The control API client is done. `selftest video` is done, and it runs in the QEMU guest on `vkms` (`cargo xtask os selftest-check`). The policy gives every CRT mode a pixel clock in whole kHz (ADR 0009), which the KMS backend takes as it is. The tool is not run on real hardware yet. `modeline` and a mode test are not written yet.

## The copy of the mode conversion in `tools/kms-spike` (OP-060)

The self-test uses `KmsDisplay` of `neptron-kms`, so it has no mode conversion of its own. `tools/kms-spike` keeps its own copy, because its dependency rule allows only `drm`, `neptron-types` and `neptron-safety`. The proposal: keep the spike as it is until the owner has run the hardware checklist with both tools, and compare the two reports for the same modes. Then delete `tools/kms-spike` and its dependency rule in `xtask/src/rules.rs` in one change, because `selftest video` covers what it did (the test modes, the read back, the timing, the restore) through the backend that the product uses. Until then, a change to `ModeRecord` must be made in both places.
