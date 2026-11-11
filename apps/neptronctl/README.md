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

An error prints one JSON line on stderr: `{"error":{"code":"...","message":"..."}}`.

## Argument parsing

The parser is written by hand. The grammar is a few fixed words, and a parser crate costs more compile time than it saves.

## Must never depend on

- `neptron-ui`
- Any async runtime

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features. The client speaks only the Unix socket.

## Status

The control API client is done. `selftest video`, `modeline` and a mode test are not written yet.
