# neptron-ipc

The control and test protocol: JSON lines on a Unix socket for screenshot, UI tree, input and clock. A TCP listener exists only with the `control` feature.

## Public API sketch

- `enum Request`, `enum Response`
- `fn serve(socket, handler)`, `struct Client`

## Must never depend on

- `neptron-kms`, `neptron-ui`
- Any async runtime

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `control`. No feature is on by default.

## Status

Stub.
