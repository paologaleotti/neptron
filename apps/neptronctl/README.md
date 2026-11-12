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
                 [--connector NAME] [--report FILE]
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
| 6 | The self-test cannot start: the preset cannot be read, or there is no EDID profile | `selftest_setup` |

An error prints one JSON line on stderr: `{"error":{"code":"...","message":"..."}}`.

## The video self-test

`selftest video` plans one test mode for each mode family that the monitor profile supports, and prints the plan. It does not talk to the control socket.

- `--profile ID`: a preset of the monitor folder, read at run time (`--monitors`, default `/usr/share/neptron/monitors`). `--profile edid`: the profile from the EDID of the connector. There is no default profile.
- `--sysfs DIR` (default `/sys`): the GPU, its capability table entry and the connector come from sysfs, as in the frontend. With nothing detected, `validate` gets the values of an unknown GPU. The EDID profile also lowers the highest pixel clock to the EDID limit.
- `--connector NAME`: the connector, for example `DP-1`. Without it, the first connected connector.
- `--report FILE` (default `neptron-selftest-video.json` in the current folder): the JSON report. The dry run writes it too, with `dry_run` true.

The families, in the order they show: `240p`, `288p`, `240p_double_refresh`, `480i`, `576i`, `480p`, `2x`, `super_width_dynamic`, `super_width_fixed`. Each family is a game source and the policy settings that ask for it (`src/selftest/family.rs`). The policy decides the mode (`decide`, then `choose` with `neptron_safety::validate`), and the family takes the winner only when it belongs to the family: for example a 240-line progressive mode with one field per game frame for `240p`. A family with no such winner is "not tested", with the policy's mode and reason. A family whose mode equals the mode of an earlier family is "the same mode as" that family, so no mode shows twice.

On `pc_31_120` with the values of an Intel gen9 GPU the plan is 240p at 120 Hz (`240p_double_refresh`), 480p (`2x` gives the same mode), 768x224 at 120 Hz (`super_width_dynamic`) and 1920x224 at 120 Hz (`super_width_fixed`).

### One mode on the display

`src/selftest/show.rs` runs on any `Display` and `Clock`: the KMS backend in the real run, the fake display of `neptron-testkit` in the tests.

1. `set_mode` with the mode's `ValidatedMode`. On KMS this is a test-only commit, then the real one. `ModeRefused` is a refused test-only commit, and a device error of the operation `modeset commit` (`KMS_COMMIT_OPERATION`) is a refused real commit: a commit is all or nothing, so the mode before stays, and the run goes on with the next mode. Any other error of `set_mode` leaves the display state unknown, and the run stops and turns the output off.
2. The read back is compared field by field with the request and validated again. A read back that fails `validate` stops the run at once and turns the output off. A read back that differs but passes fails the mode, and the run goes on; the pattern does not show, because it is drawn for the request.
3. The pattern shows for `seconds_per_mode`: `present`, then `wait_flip`, until the clock passes the time. The pattern is a white border on the outermost pixels of all four edges, a centre cross, eight colour bars at 75 %, and four lines of text: the family, the size and scan, the line and field rates, the pixel clock. The text is a 5x7 font of the tool (`neptron-ui` and its font are only for the app). A mode wider than 2048 pixels (`PATTERN_WIDTH_MAX_PX`) gets a frame of half its width, scaled 2 times with nearest sampling. A failed wait is tried `FLIP_WAIT_RETRIES` (2) more times; then the flip stays queued and the run stops.
4. The timing comes from the kernel's vblank counter and time stamps of the flip events: the mean vertical blank period over the whole time, the expected period of the mode (a field; for an interlaced mode a field or a frame, whichever is nearer, as `vblank_unit` says), the error in ppm, the pixel clock that the period implies, the flips per second and the vertical blanks with no flip.
5. The stop flag (SIGINT, SIGTERM, SIGHUP in the real run) is read before each flip and before each mode.

At the end, on a stop and on an error with the display state unknown, the run calls `blank()`: the output goes off. The state before the run cannot be read back and validated through `KmsDisplay`, so no signal is the restore, as the safety model allows. With a flip still queued, the backend refuses the blank as busy and the report says so; the backend's drop then turns the plane off with the validated mode kept, or the output off.

A mode passes when (`src/selftest/verdict.rs`): both commits passed; a mode was read back, equal to the request in every field and valid; the pattern showed for the whole time; at least 2 flips completed (`FLIPS_MIN`); the vertical blank period is within 1 % (`VBLANK_PERIOD_TOLERANCE_PPM`, 10 000 ppm) of the mode's period; and no display error came. Missed vertical blanks do not fail a mode: they show how fast the tool draws (software GL in the QEMU guest misses most), and the period does not depend on them. The run passes when it finished, it had at least one mode, every mode passed, the restore succeeded, and no other error came.

### The report

One JSON file, written as a temporary file in the same folder, fsync, rename, and fsync of the folder, so a reader sees a whole report or the one before. `report_version` is 1. The format is append-only: a new field gets a new version, and no field is renamed or removed. Every value is an integer or a text: rates in millihertz, times in nanoseconds, clocks in hertz. A value that the run did not measure is `null`.

| Field | Meaning |
| --- | --- |
| `report_version` | 1 |
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
| `modes[].timing` | `flips`, `vblanks`, `missed_vblanks`, `flips_per_second_mhz`, `vblank_period_ns`, `expected_period_ns`, `vblank_unit` (`field` or `frame`), `period_error_ppm`, `implied_pclock_hz` |
| `modes[].errors`, `.pass`, `.failures` | Display errors in the mode, the verdict, and each check that failed |
| `restore` | `method` (`blank`), `ok`, `error`; `null` when no mode was set |
| `stopped`, `errors` | Why the run stopped early, and errors outside one mode |

## Argument parsing

The parser is written by hand. The grammar is a few fixed words, and a parser crate costs more compile time than it saves.

## Dependencies

`neptron-ipc` for the control API. For the self-test: `neptron-modeline` (the policy), `neptron-safety`, `neptron-types`, `neptron-platform` (detection, capability table, EDID profile) and `toml` (the preset files). `neptron-modeline` links Switchres, so the binary is a combined work under GPL-3.0-or-later, as the frontend is (ADR 0002). The presets stay files on the image and are not embedded (OP-044).

## Must never depend on

- `neptron-ui`
- Any async runtime

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features. The client speaks only the Unix socket; the self-test reads files only.

## Status

The control API client is done. `selftest video` plans and prints its modes. `modeline` and a mode test are not written yet.
