# neptron-kms

`Display` on DRM/KMS: connector choice, atomic modeset, GBM buffers, flip events. It accepts a `ValidatedMode` only. [`docs/spec/kms.md`](../../docs/spec/kms.md) defines the behaviour; this file is the short form.

## Must never depend on

- `neptron-modeline` (the policy stays out of the backend)
- `neptron-video`, `neptron-ui`, `neptron-session`

Dependencies: `neptron-backend`, `neptron-safety`, `neptron-types` (the contract); `neptron-gfx` with `gles` through the `kms` feature (the renderer); `drm`, `drm-ffi`, `gbm-sys` (links `libgbm`) and `rustix` (`event`), optional, Linux only, with the `kms` feature.

## Public API

On every host (pure, no I/O):

- `choose(&[ConnectorFacts], named, crt) -> Result<usize, ChoiceError>`: the output rule of the Definition ([spec, "Device and connector"](../../docs/spec/kms.md#device-and-connector)).
- `ConnectorClass::of_type` maps a DRM connector type. `ConnectorClass::of_connector` adds the subconnector and the EDID: DVI-I is digital only when the subconnector is not `DVI-A` and the EDID passes the structural checks before the descriptors (length, header, every checksum, version 1.3 or 1.4, manufacturer id, a detailed timing first) and has the digital input bit (byte 0x14, bit 7). Else it is analog: analog wins when in doubt. It follows `display_class` in `neptron-platform`, which this crate does not depend on; the tests of both crates hold the same table of rows. Two cases can still differ, both toward analog here: an EDID that fails only a check of the display descriptors (the app also takes it as analog), and an EDID blob that the device cannot read (the app reads the EDID from sysfs and can find a panel). The app's own class decides the caps and the profile.
- `ModeRecord::from_validated(&ValidatedMode)`: the only way to make a kernel mode record. `ModeRecord::to_modeline` reads a kernel mode back.
- `PlaneRect::for_mode`: the part of the scanout buffers a mode shows.
- `VblankCounter`: the kernel's 32-bit vblank count as 64 bits.
- `FlipState`: the order rules of the `Display` calls, the same as the fake display of `neptron-testkit`.
- `WireMode`: the mode on the wire with its TV standard, and whether a request must commit. A change of the standard alone commits. After a wrong TV standard read back (`recommit`), the same request commits again. After a blank, every request commits.
- `lowered_caps`: the pure rule of `lower_caps`.
- Inside the crate: `Request` (an atomic request in fixed arrays), `modeset`, `output_off`, and the module `tv`, which chooses the `TV mode` value and decodes the standard read back.

On Linux with the `kms` feature:

- `KmsDisplay::from_env(KmsConfig) -> Result<KmsDisplay, KmsError>`: the display on the descriptor in `NEPTRON_DRM_FD` (`DRM_FD_ENV`), the path of the frontend. A process takes the descriptor once. `KmsDisplay::open(OwnedFd, KmsConfig)` takes a descriptor the caller opened (tests, tools).
- `impl Display for KmsDisplay`: `caps`, `set_mode`, `present`, `wait_flip`, `blank`.
- `core_render()` gives `Some(&mut dyn CoreRender)`: the display hosts the GL context of a core on the EGL display of its renderer (`docs/spec/hw-render.md`). `offer`, `create`, `resize`, `begin`, `end`, `read_back` and `destroy` go to the `core_*` calls of `GlesRenderer`, with the types of `neptron-backend` mapped to the ones of `neptron-gfx`. `present` draws a `GamePicture::Gpu` picture of that image; an image the display does not hold, or a picture larger than its image, is `SceneRefused`.
- `capture() -> Result<Capture, DisplayError>`: the composed picture through a writeback connector, when `KmsConfig::writeback` is on. For tests and the self-test, not for the frame loop.
- `connector_name()`, `crtc_id()`, `driver_name()`, `linear_by_modifier()`: for logs and reports.
- `lower_caps(&GpuCaps)`: lowers the highest pixel clock and the largest mode that `caps()` reports, never raises them. The buffers stay. The app uses it when the output is analog after the display opened with digital limits.
- `connector_class()`: the class of the connector in use. A composite connector is `Analog`.
- `tv_standards() -> Option<Vec<TvStandard>>`: `None` when the connector has no `TV mode` property (HDMI, VGA, `vkms`), else the standards whose `kms_name()` is a value of it. `PAL` gives `Pal` and `Pal60`, `Mono` gives `Mono50` and `Mono60`; the driver's atomic check refuses a pair it does not have. The backend reads the property once, at open.
- `subconnector()`: the enum name of the connector's `subconnector` property, or `None` (also on a failed read). On DisplayPort it is the downstream port type that the kernel reads from the DPCD (`VGA` for a DP-to-VGA DAC, `HDMI`, `DVI-D`, `DP`, `Native`, `Unknown`); i915 attaches it to every DP connector but eDP. On DVI-I it is `DVI-D` or `DVI-A`. The caller decides the class with it.
- `KmsConfig { caps, output, crt, buffer_width, buffer_height, writeback }`: everything the caller decides. The buffers have the size of the largest mode the caller will set and are made once.
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

## Rules for a caller

- Atomic API only. `set_mode` commits (test-only, then real, with `ALLOW_MODESET`) only when the mode or the TV standard differs from the wire. `MODE_ID` is only ever a blob of `ModeRecord::from_validated`, or 0. The steps are in [spec, "The commit sequence"](../../docs/spec/kms.md#the-commit-sequence).
- TV standard (ADR 0012): a mode with a standard needs a connector whose `TV mode` property has that value, and a mode with no standard is refused on a connector with a `TV mode` property. Both refusals are `ModeRefused`, before anything is drawn. Every modeset sets each margin property to 0, because a margin makes the scaler resample the lines. After the commit the value name and the horizontal total read back (858 for 525 lines, 864 for 625) must give the requested standard, else `Device` and `WireMode::recommit`.
- The CRTC mode read back is the blob of the request with its flags: `drm_mode_getcrtc` converts the CRTC state's mode, and the `vc4` encoder check only reads the adjusted mode. The `720x240` and `720x288` modes that `vc4` lists have no sync flag, and `to_modeline` refuses them (`Polarity`); the backend never reads a listed mode.
- `present`, `set_mode` and `blank` are `Device` (busy) while a flip is pending. `wait_flip` waits at most `FLIP_WAIT_LIMIT_MS`; a late flip is an error, not a hang, and stays pending.
- The present path allocates nothing.
- Drop and panic: see [spec, "Restore"](../../docs/spec/kms.md#restore).
- `kms-spike` has its own copy of the mode record conversion: the dependency rules allow that tool only `drm`, `neptron-types` and `neptron-safety`. Both refuse a clock that is not a whole number of kHz.
- `unsafe` only in `src/kms/ffi.rs` (a descriptor by number, libgbm, the GBM pointers for EGL). Every other module carries `#[forbid(unsafe_code)]`. Every `unsafe` block has a `// SAFETY:` comment.
- Feature: `kms`, off by default. The headless build compiles no DRM, GBM or EGL code.

## Tests

- Tier 1, every host (`cargo xtask test -p neptron-kms`): the connector rule, the mode record (each field and flag, the kHz refusal, the readback refusals, the four TV table modes, the listed TV modes with no sync flag), the plane rectangles, the vblank counter across a wrap, the call order rules, the wire mode after a blank and after a change of the standard alone, the fixed atomic request, the `TV mode` value of each standard and each refusal, the read back of each of the ten standards and of a mismatch.
- Tier 3: `cargo xtask kms-check`, timing check 7 on `vkms` in the aarch64 QEMU guest. It cross-builds `examples/kms_check.rs` (warnings denied, every target of the crate checked first) and runs it with the device on descriptor 3 and `NEPTRON_DRM_FD=3`. [Spec, "What `vkms` proves"](../../docs/spec/kms.md#what-vkms-proves) lists the checks and the limits. `vkms` has no `TV mode` property, and no virtual driver of the pinned kernel has one (only `vc4` and `sun4i` call `drm_mode_create_tv_properties`), so tier 3 checks the four table modes as plain modes and the refusal of each standard. Setting a standard needs a Raspberry Pi (`docs/hardware-checklist.md`). It also checks the GL core path on the 480p mode: a GLES 2 core context with depth and stencil on the display, the card of the GL test core drawn into it by the stand-in of `neptron-gfx` with the GL state left dirty, and the GPU picture through the writeback connector equal to the reference renderer's picture of the card in memory, in both origins, at 2x with scanlines, turned and under the UI; the read back equals the card, and a destroyed image is refused.

## Status

The backend runs on `vkms` in the QEMU guest, on i915 on the i3-6100T, and on `vc4` composite on a Pi 3 B in NTSC 240p. `docs/hardware-checklist.md` lists the checks that are still open.
