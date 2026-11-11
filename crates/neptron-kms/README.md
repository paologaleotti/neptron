# neptron-kms

`Display` on DRM/KMS: connector choice, atomic modeset, GBM buffers, flip events. It accepts a `ValidatedMode` only.

## Public API sketch

- `struct KmsDisplay: Display`
- `fn open(fd: OwnedFd) -> Result<KmsDisplay, DisplayError>`

## Must never depend on

- `neptron-modeline` (the policy stays out of the backend)
- `neptron-video`, `neptron-ui`, `neptron-session`

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it. Every `unsafe` block has a `// SAFETY:` comment.
- Features: `kms`. No feature is on by default.

## Status

Stub.
