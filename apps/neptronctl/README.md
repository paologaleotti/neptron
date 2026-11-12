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
                 [--connector NAME]
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

The families, in the order they show: `240p`, `288p`, `240p_double_refresh`, `480i`, `576i`, `480p`, `2x`, `super_width_dynamic`, `super_width_fixed`. Each family is a game source and the policy settings that ask for it (`src/selftest/family.rs`). The policy decides the mode (`decide`, then `choose` with `neptron_safety::validate`), and the family takes the winner only when it belongs to the family: for example a 240-line progressive mode with one field per game frame for `240p`. A family with no such winner is "not tested", with the policy's mode and reason. A family whose mode equals the mode of an earlier family is "the same mode as" that family, so no mode shows twice.

On `pc_31_120` with the values of an Intel gen9 GPU the plan is 240p at 120 Hz (`240p_double_refresh`), 480p (`2x` gives the same mode), 768x224 at 120 Hz (`super_width_dynamic`) and 1920x224 at 120 Hz (`super_width_fixed`).

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
