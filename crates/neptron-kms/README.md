# neptron-kms

`Display` on DRM/KMS: connector choice, atomic modeset, GBM buffers, flip events. It accepts a `ValidatedMode` only. `docs/spec/kms.md` defines the behaviour; this file is the short form.

## Public API

On every host (pure, no I/O):

- `choose(&[ConnectorFacts], named, crt) -> Result<usize, ChoiceError>`: the output rule of the Definition. `ConnectorClass::of_type` maps a DRM connector type. `ConnectorClass::of_connector` adds the subconnector and the EDID: DVI-I carries both signals, so it is digital only when the subconnector is not `DVI-A` and the EDID passes the structural checks of `neptron-platform` before the descriptors (length, header, every checksum, version 1.3 or 1.4, manufacturer id, a detailed timing first) and has the digital input bit (byte 0x14, bit 7). Else it is analog: analog wins when in doubt. This is the rule of `display_class` in `neptron-platform`, which this crate does not depend on; the tests of both crates hold the same table of rows. Two cases can still differ, both toward analog here: an EDID that fails only a check of the display descriptors (the platform rejects it, so the app also takes it as analog, the same class), and an EDID blob that the device cannot read: the connector is then analog for the output choice, while the app reads the EDID from sysfs and can find a panel. The output choice then prefers that connector for a CRT profile, and needs no name with an EDID; the app's own class decides the caps and the profile.
- `ModeRecord::from_validated(&ValidatedMode)`: the kernel mode record, the only way to make one for the kernel. `ModeRecord::to_modeline` reads a kernel mode back.
- `PlaneRect::for_mode`: the part of the scanout buffers a mode shows.
- `VblankCounter`: the kernel's 32-bit vblank count as 64 bits.
- `FlipState`: the order rules of the `Display` calls, the same as the fake display of `neptron-testkit`.
- `WireMode`: the mode on the wire and what `set_mode` must do for a request; after a blank, every request commits.
- Inside the crate: `Request`, an atomic request in fixed arrays, and `output_off`, the request that turns the output off.

On Linux with the `kms` feature:

- `KmsDisplay::from_env(KmsConfig) -> Result<KmsDisplay, KmsError>`: the display on the descriptor in `NEPTRON_DRM_FD` (`DRM_FD_ENV`). This is the path of the frontend (T27). A process takes the descriptor once.
- `KmsDisplay::open(OwnedFd, KmsConfig)`: the same on a descriptor the caller opened (tests, tools).
- `impl Display for KmsDisplay`: `caps`, `set_mode`, `present`, `wait_flip`, `blank`.
- `KmsDisplay::capture() -> Result<Capture, DisplayError>`: the composed picture through a writeback connector, when `KmsConfig::writeback` is on. For tests and the self-test, not for the frame loop.
- `connector_name()`, `crtc_id()`, `driver_name()`, `linear_by_modifier()`: for logs and reports.
- `lower_caps(&GpuCaps)`: lowers the highest pixel clock and the largest mode that `caps()` reports, never up (`lowered_caps`, pure, on every host). The buffers stay. The app uses it when the output is analog after the display opened with digital limits.
- `subconnector()`: the enum name of the connector's `subconnector` property, when it has one. On DisplayPort it is the downstream port type that the kernel reads from the DPCD (`VGA` for a DP-to-VGA DAC, `HDMI`, `DVI-D`, `DP`, `Native`, `Unknown`); i915 attaches it to every DP connector but eDP. On DVI-I it is `DVI-D` or `DVI-A`. The backend only reads it; the caller decides the class of the display with it. A failed read gives `None`.
- `KmsConfig { caps, output, crt, buffer_width, buffer_height, writeback }`: everything the caller decides. The buffers have the size of the largest mode the caller will set; the backend makes them once.
- Limits: `FLIP_WAIT_LIMIT_MS` (500), `WRITEBACK_WAIT_LIMIT_MS` (1000).

```rust
let config = KmsConfig {
    caps,                         // from neptron-platform
    output: settings.output,      // None for the rule of the Definition
    crt: profile.is_crt(),
    buffer_width: 3840,
    buffer_height: 576,
    writeback: false,
};
let mut display = KmsDisplay::from_env(config)?;
let readback = display.set_mode(&validated)?;   // the first and only modeset path
display.present(&scene)?;
let flip = display.wait_flip()?;
```

## How it works

- Atomic API only. `set_mode` sends a test-only commit, then the real one, with `ALLOW_MODESET`, and only when the mode differs from the mode on the wire. `present` sends a nonblocking commit with a page-flip event that changes only the plane's framebuffer. `blank` sends one commit, test-only first, with `ALLOW_MODESET`, that detaches the connectors and the plane and sets `MODE_ID` 0 and `ACTIVE` 0; it is busy while a flip is pending. `MODE_ID` is only ever a blob of `ModeRecord::from_validated`, or 0.
- Buffers: one GBM surface, XRGB8888, linear (the modifier list `[LINEAR]`, or the linear usage flag when the driver takes no modifiers), at the configured size, made at open. `neptron-gfx` draws into it through EGL on GBM (`GlesRenderer::on_gbm`, `draw_to_window`). A mode shows the last rows of the buffer, where the window surface stores a picture drawn at the GL origin.
- Flips: `wait_flip` polls with a bound and reads only readable data. A flip that does not come within `FLIP_WAIT_LIMIT_MS` is an error, not a hang, and stays pending.
- The present path allocates nothing: the atomic request is fixed arrays (`drm-ffi`), the framebuffers of the GBM buffers are kept in a table of four, and the event read uses a stack buffer.
- Drop: after a blank, nothing. With a mode set, the plane goes off and the validated mode stays on the wire; when the driver refuses that, the output goes off. A panic in the dev profile unwinds into this drop. In the release profile a panic aborts; then every mode that ever reached the kernel is validated, the images have no console that sets a mode, and the launcher's descriptor keeps the mode on the wire.

## The T3 check

`cargo xtask kms-check` is timing check 7 on `vkms` in the aarch64 QEMU guest. It cross-builds `examples/kms_check.rs` in the development container (warnings denied, every target of the crate checked first) and runs it with the device on descriptor 3 and `NEPTRON_DRM_FD=3`. The spec lists the checks and what `vkms` cannot prove.

## Why the mode record is not shared with `kms-spike`

`kms-spike` has the same conversion. The dependency rules allow that tool only `drm`, `neptron-types` and `neptron-safety`, so it cannot use this crate, and a change to the rule needs a decision. Both refuse a clock that is not a whole number of kHz.

## Dependencies

- `neptron-backend`, `neptron-safety`, `neptron-types`: the contract.
- `neptron-gfx` with `gles` (through the `kms` feature): the renderer.
- `drm`, `drm-ffi`, `gbm-sys`, `rustix` (`event`): optional, Linux only, with the `kms` feature. `gbm-sys` links `libgbm`.

## Must never depend on

- `neptron-modeline` (the policy stays out of the backend)
- `neptron-video`, `neptron-ui`, `neptron-session`

## Rules

- `unsafe` is allowed in this crate, only in `src/kms/ffi.rs` (taking a descriptor by number, libgbm, the GBM pointers for EGL). Every other module carries `#[forbid(unsafe_code)]`. Every `unsafe` block has a `// SAFETY:` comment.
- Features: `kms`. No feature is on by default. The headless build compiles no DRM, GBM or EGL code.

## Tests

- Tier 1, every host: the connector rule (named, one connected, analog without EDID, class by profile, fallbacks, errors, type numbers), the mode record (each field and flag, the kHz refusal, the readback refusals), the plane rectangles, the vblank counter across a wrap, the order rules of the calls (blank included), the mode on the wire after a blank, the fixed atomic request (grouping, overflow, the output-off request).
- Tier 3: `cargo xtask kms-check`.

## Status

The backend runs on `vkms` in the QEMU guest. It is not checked on a real GPU or monitor; `docs/hardware-checklist.md` has the checks.
