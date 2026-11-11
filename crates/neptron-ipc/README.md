# neptron-ipc

The control protocol: JSON lines on a Unix socket for screenshot, UI tree, input, clock, session, video state, stats, settings and log. A TCP listener exists only with the `control` feature.

The crate carries the protocol only. The app implements the handlers.

## Public API sketch

- Messages: `enum Command` with one argument struct per command, `enum Reply` with one data struct per reply, `ErrorBody { code: ErrorCode, message }`.
- Codec, with no I/O: `encode_request`, `decode_request`, `encode_response`, `decode_response`, `read_line` with a byte limit.
- Server: `trait Handler` (one method per command), `dispatch`, `handle_line`, `serve_lines` over any `BufRead` and `Write`, `serve_stream` over a socket.
- Transport: `ControlListener::bind(path)`, `Client::connect(path)`, `Client::call(command)`. With `control`: `TcpControlListener`, `Client::connect_tcp`.
- `DEFAULT_SOCKET_PATH` is `/run/neptron/control.sock`. The environment variable `NEPTRON_CONTROL_SOCKET` names a different path.

## Wire format

One JSON object per line, in UTF-8, ended by `\n`. A `\r` before the `\n` is removed. Each request line gets exactly one response line, in order.

```text
-> {"v":1,"id":7,"cmd":"clock_step","args":{"frames":60}}
<- {"v":1,"id":7,"reply":"clock_step","data":{"frame":60}}
-> {"v":1,"id":8,"cmd":"ui_tree"}
<- {"v":1,"id":8,"error":{"code":"unavailable","message":"this server does not serve \"ui_tree\""}}
```

- `v` is the protocol version, `id` is a number the client chooses, and the response echoes both. A command without arguments has no `args`.
- A success has `reply` (the command name) and `data`. A failure has `error` with `code` and `message`. A client acts on `code` only.
- Every name is snake_case. The tests pin every request and every reply against its literal JSON.

### Commands

| `cmd` | `args` | `data` of the reply |
| --- | --- | --- |
| `hello` | none | `protocol`, `server` |
| `screenshot` | `stage`: `ui`, `output` or `scanout` | `stage`, `width`, `height`, `png_base64` |
| `ui_tree` | none | `screen`, `focused`, `rows` (`id`, `label`, `value`, `setting`, `source`, `clipped`, `bounds`) |
| `input_send` | `port`, `control`, `value` | `frame` at which the change applies |
| `input_script` | `events` (`frame`, `port`, `control`, `value`) | `start_frame`, `events` |
| `clock_step` | `frames` | `frame` |
| `wait` | `until` (`{"screen":id}`, `{"session_state":s}` or `{"frame":n}`), `timeout_ms` | `frame` |
| `session_launch` | `core`, `rom` | `state`, `core`, `rom`, `frame` |
| `session_state` | none | `state`, `core`, `rom`, `frame` |
| `video_state` | none | `mode`, `line_rate_mhz`, `field_rate_mhz`, `profile`, `verdicts`, `switch_count` |
| `stats` | none | `frames_presented`, `frames_dropped`, `frames_repeated`, `audio_underruns`, `audio_queued_frames`, `input_to_frame_ns` |
| `settings_get` | `key`, optional `level` | `key`, `value`, `source` |
| `settings_set` | `key`, `level`, `value` | `key`, `level` |
| `log_tail` | `lines` | `lines` |

`control` uses the names of `neptron_types::Control` (`a`, `start`, `left_x`, ...). `level` is `game`, `system`, `global` or `default`. A session `state` is `idle`, `loading`, `running`, `paused`, `quick_menu` or `exiting`. Values are integers, with the unit in the name.

### Errors

The server checks a line in this order, and the first fault gives the code:

| Fault | `code` | `id` in the response |
| --- | --- | --- |
| Not JSON, not UTF-8, or an empty line | `malformed` | `null` |
| A key twice in one object, at any depth | `invalid_request` | `null` |
| Longer than 1 MiB | `line_too_long`, then the server closes the connection | `null` |
| Not an object, or `id` missing or not a `u64` | `invalid_request` | `null` |
| `v` missing or not a `u32`, `cmd` missing or not a string, or a key other than `v`, `id`, `cmd`, `args` | `invalid_request` | the id |
| `v` is outside the versions the server speaks | `unsupported_version`, with `min_version` and `max_version` | the id |
| `cmd` names no known command | `unknown_command` | the id |
| `args` missing, of the wrong type, or with an unknown key | `invalid_arguments` | the id |

A handler answers `unavailable` (not served in this build or state), `timeout` (a `wait` passed its timeout) or `failed`. No line makes the server crash or hang: a line that is not JSON gets `malformed`, and the reader never holds more than the limit of one line.

The request side refuses unknown keys, because a key the server does not know can change the meaning of a request. The response side ignores unknown keys, so that a newer server can add fields.

The JSON parser keeps only the last of two equal keys, so the server reads the line a second time to find them. A key twice is refused, not resolved.

### Versioning

The protocol is append-only. A new version adds commands, reply fields, enum values or error codes, and never renames or removes one. These rules keep each side readable for the other:

1. A server serves every request version from `MIN_PROTOCOL_VERSION` to `PROTOCOL_VERSION`. It answers a version outside that range with `unsupported_version`, and the error carries `min_version` and `max_version` as fields.
2. A response echoes the `v` of its request. It holds only names (commands, fields, enum values, error codes) that exist in that version. A server that adds a name in version N writes it only to a request of version N or above.
3. A field that a later version adds to `args` or to a reply is optional: an `Option` or a field with `#[serde(default)]`. An older request and an older reply stay valid.
4. A client ignores reply keys it does not know. An error code it does not know reads as `ErrorCode::Unknown`, so a failure stays a failure.
5. A reply enum value that a client does not know is a codec error. Rule 2 makes this impossible for a correct server, so the client does not try to recover from it.

Version discovery: a client sends `hello` at its own version. A server that speaks it answers with `protocol`, its newest version. A server that does not answers `unsupported_version` with `min_version` and `max_version`, and the client then picks a version in that range.

### Binary payloads

A screenshot travels as standard base64 with padding in the `png_base64` field of the reply. A server-side file path would be simpler for the server, but it works only when the client runs on the same machine, and the TCP listener exists for a client that does not, such as the host of a QEMU guest. Base64 also needs no shared directory, no file cleanup and no trust in a path from the network. The cost is a third more bytes, and the client reads a response line of up to 64 MiB, which holds a 4096x2160 picture with no compression.

## What the application must do

The crate carries the protocol only. The application that writes the handlers must also:

- Set a read timeout and a write timeout on each accepted stream. Without the write timeout, a client that never reads its responses holds the I/O thread forever.
- Set the permissions of the socket file: the owner only, or one group. `ControlListener::bind` does not change them.
- Refuse `settings_set` at the level `default`, with `invalid_arguments`. The default comes from the schema and is not written.
- Send every display change (`session_launch`, `settings_set` of a video setting) through `validate` and confirm-or-revert, as a menu choice does. The protocol has no command that sets a modeline.
- Serve the socket on an I/O thread, and pass each command to the frame thread over a bounded channel. The frame thread never waits on the socket.
- Never serve `input_send`, `input_script`, `clock_step`, `session_launch` or `settings_set` unless the application is built with its `control` feature. Without it, these handlers keep the default answer `unavailable`.

## Must never depend on

- `neptron-kms`, `neptron-ui`
- Any async runtime

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `control` adds the TCP listener and the TCP client. No feature is on by default, and without it no TCP code compiles.
- Dependencies: `neptron-types`, `serde`, `serde_json`, `thiserror`. The base64 codec is in the crate, so it needs no dependency.

## Status

The protocol, the codec, the server loop, the Unix transport and the TCP transport behind `control` are done. The handlers live in the app.
