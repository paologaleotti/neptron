# KMS backend

This spec settles how `neptron-kms` implements the `Display` trait of `neptron-backend` on Linux DRM/KMS. The code is in `crates/neptron-kms`, behind the Cargo feature `kms`, on Linux only. The headless build compiles no DRM, GBM or EGL code.

The backend executes and decides nothing. The engine (`neptron-video`) decides which mode to set and when. The safety crate decides if a mode is safe. The platform crate gives the GPU capabilities. The backend programs what it gets, reads it back and reports.

## Sources

- Linux 6.18, `Documentation/gpu/drm-kms.rst` and `drm-uapi.rst`: the atomic API, `DRM_MODE_ATOMIC_TEST_ONLY`, `DRM_MODE_ATOMIC_NONBLOCK`, `DRM_MODE_ATOMIC_ALLOW_MODESET`, `DRM_MODE_PAGE_FLIP_EVENT`, the writeback connector properties `WRITEBACK_FB_ID` and `WRITEBACK_OUT_FENCE_PTR`.
- Linux 6.18, `drivers/gpu/drm/vkms`: the virtual driver of the QEMU images. Its vblank comes from a timer, its writeback connector checks that the buffer has the size of the mode, its CRCs go to debugfs.
- Mesa 26.1, `src/gbm` and `src/egl/drivers/dri2/platform_drm.c`: EGL on GBM (`EGL_PLATFORM_GBM_KHR`). A window surface is drawn with the GL origin at the bottom left, so the last GL row is the first row of the buffer in memory.
- `tools/kms-spike` (T21) and its runs on `vkms`, `os/README.md` (T14) and its `vkms` probe, the open points OP-018, OP-030, OP-032, OP-034, OP-043 and OP-045.

## Device and connector

**The device.** In the product, the launcher (`neptron-launch`) opens the DRM device and passes the open descriptor to the frontend. The frontend finds its number in the environment variable `NEPTRON_DRM_FD`. `KmsDisplay::from_env` takes it once per process: a second call is an error, because one descriptor has one owner (`OwnedFd`). Every check runs before the process owns the descriptor: the numbers 0, 1 and 2 (the standard streams) are refused, and the descriptor must be open and a character device of the DRM major number (226), read through `/proc/self/fd`. A refused descriptor stays open and untouched, and a refused call does not count as the one take. The descriptor and the launcher's own descriptor share one open file description, so they share DRM master, framebuffers and buffer handles.

`KmsDisplay::open` takes an `OwnedFd` directly, for tests and tools.

The backend turns on the client capabilities `UNIVERSAL_PLANES` and `ATOMIC`. When the configuration asks for writeback (see "What `vkms` proves"), it turns on `WRITEBACK_CONNECTORS` too. A driver without atomic modesetting is refused: the backend uses the atomic API only.

**The connector.** The rule of the Definition (CRT safety model, "Which output is used"), as a pure function with no I/O:

1. A connector named in the settings is used in any state, because a VGA monitor often reports nothing. A name that does not exist is an error that lists the names that exist.
2. With no name, the candidates are the connected connectors, without the analog connectors that report no EDID. The kernel cannot tell that a monitor is on such a connector, so it is used only when it is named.
3. One candidate: it is used.
4. Several candidates: with a CRT profile, the first analog candidate wins; with a flat-panel profile, the first digital candidate wins. When the class the rule asks for has no candidate, the first candidate wins (open point, see below).
5. No candidate: an error. The blind probe (driving every analog connector without EDID) is the safety layer's work, not the backend's.

Analog connector types: VGA, DVI-A, DVI-I, Composite, S-Video, Component, 9-pin DIN, TV and DPI (the Raspberry Pi GPIO DAC). Every other type is digital. A writeback connector is never a candidate. The order is the order of the kernel's resource list.

**The CRTC and the plane.** The CRTC that drives the connector now, or else a CRTC that no connector uses and that an encoder of the connector can drive. A CRTC that also drives another connector is refused at open: that display would get the modes of this one. The primary plane that feeds the CRTC now, or else a free primary plane that can feed it.

`open` sets no mode and makes no commit. It only reads the state. Until the first `set_mode`, the display shows what it showed before: on x86 the firmware's mode that i915 inherited, on a Pi and on `vkms` nothing.

## Buffers

The backend makes one GBM device on a duplicate of the descriptor (same open file description) and one GBM surface of format XRGB8888. The surface has the size of the configuration (`KmsConfig::buffer_width`, `buffer_height`), the largest mode the caller will set, for example 3840 by 576. It is made once, at open, and never again: a mode change changes only timings and the visible part of the buffers, never the GL or buffer setup. A larger size than `GpuCaps::max_width` or `max_height` is refused at open.

The buffers are linear. The backend asks for a surface with the modifier list `[DRM_FORMAT_MOD_LINEAR]`. A driver that does not take modifiers gets a surface with the usage flag `GBM_BO_USE_LINEAR`, which also asks for a linear layout. The framebuffer of each buffer is added with no modifier (the implicit layout of a linear buffer). i915 takes only linear or X-tiled buffers for an interlaced mode; linear buffers make 480i possible with no kernel patch (hardware support matrix).

The GBM surface holds a few buffers (Mesa: up to 4). The backend adds one DRM framebuffer per buffer the first time it gets that buffer, and keeps it in a table of `FB_SLOTS` (4) entries. After the first frames the table is full, and a present allocates nothing.

**The renderer.** `neptron-gfx` makes an EGL display on the GBM device (`EGL_PLATFORM_GBM_KHR`), a window surface on the GBM surface, and an OpenGL ES 2 context. The backend gives it the two pointers through the unsafe trait `GbmTarget`. GBM and EGL types do not leave their crates: `neptron-gfx` sees two raw pointers, and `neptron-kms` sees a renderer.

**The visible part.** A mode of `w` by `h` shows a `w` by `h` part of the buffer. The renderer draws the scene with a viewport of `w` by `h` at the GL origin, the bottom left. The window surface stores the last GL row first, so the picture is in the last `h` rows of the buffer. The plane reads it with the source rectangle `(0, H - h, w, h)`, where `H` is the buffer height, and shows it at `(0, 0, w, h)` on the CRTC, with no scaling. The pixels are the pixels of the reference renderer: the T3 check proves this through the writeback connector.

## The commit sequence

Every commit uses the atomic API. A commit is all or nothing: when the kernel refuses it, nothing changes.

**`set_mode(mode)`**

1. A flip is pending: `DisplayError::Device`, nothing changes (OP-032, as the fake display). The engine calls `wait_flip` first.
2. The mode equals the mode set now: no commit. The call returns the mode read back before. When that readback failed (step 8), the call reads the mode back again, still with no commit.
3. The kernel mode record is built from the `ValidatedMode` and from nothing else (see "The kernel mode record"). A pixel clock that is not a whole number of kHz, and a mode larger than the buffers, give `DisplayError::ModeRefused`, with no commit.
4. The renderer draws a black frame of the new size, and the backend takes its buffer.
5. One request holds: the connector on the CRTC, the writeback connector on the CRTC when writeback is on, the mode blob and `ACTIVE` 1 on the CRTC, and the plane (framebuffer, CRTC, source and destination rectangles).
6. A test-only commit with `ALLOW_MODESET`. A refusal is `DisplayError::ModeRefused` with the kernel's error. The old mode stays.
7. The real commit with `ALLOW_MODESET`, blocking. When it returns, the mode is on the wire. A refusal here is `DisplayError::Device`; the old mode stays, because the commit is all or nothing.
8. The backend records the new mode as the mode on the wire, then reads the mode of the CRTC back, converts it to a `Modeline` and returns it. A mode that cannot be read or converted is `DisplayError::Device`, and the new mode stays recorded as on the wire: a later request for the old mode is a real commit, and the restore at drop runs (`WireMode`).

`ALLOW_MODESET` is set only here, and only for a mode that differs from the mode set now. No other commit of the backend can change a mode.

**`present(scene)`**

1. No mode: `DisplayError::NoMode`. A flip is pending: `DisplayError::Device` (OP-032). The GL draw of a later frame cannot start while the kernel still reads the buffer of the frame before.
2. The renderer draws the scene with the size of the mode and swaps. A scene the renderer refuses is `DisplayError::SceneRefused`, and nothing is queued.
3. The backend locks the new front buffer of the GBM surface and finds its framebuffer.
4. A nonblocking commit with `PAGE_FLIP_EVENT` sets the plane's framebuffer, and nothing else. It is not tested first: it changes no mode, no routing and no rectangle, only a buffer of the format and size that the mode's test-only commit accepted. A refusal is `DisplayError::Device`, and the buffer goes back to the surface.

The present path allocates nothing: the request is a fixed array of object, property and value ids, and the event read uses a buffer on the stack. No lock is held across a commit; the backend has no lock.

## Page flips and timing

`wait_flip` waits for the page-flip event of the CRTC and returns it as a `FlipEvent`:

- `time_ns`: the kernel's timestamp of the event, `CLOCK_MONOTONIC`, in nanoseconds (microsecond resolution). On hardware it is the time of the vblank in which the new buffer took effect.
- `sequence`: the kernel's vblank counter of the CRTC, extended from 32 to 64 bits across a wrap. A gap of more than one between two events is a missed vblank. The counter starts where the kernel's counter is; only the differences have a meaning.

When the event comes, the buffer that was on the screen goes back to the GBM surface, and the new one becomes the buffer on the screen.

**Bounded waits.** `wait_flip` polls the descriptor with a limit of `FLIP_WAIT_LIMIT_MS` (500 ms) in total, and reads only when the poll says that data is there, so a read never blocks. The slowest field of any shipped profile is about 20 ms, and an interlaced frame about 40 ms; 500 ms also covers `vkms` under a loaded QEMU. A flip that does not complete in that time is `DisplayError::Device` with `operation` "wait for the page flip". The flip stays pending: a later `wait_flip` waits again, and `present` and `set_mode` stay refused until the event comes. The frame thread never hangs on a flip.

This is the bounded wait that the `Display` contract asks for: on a timeout `wait_flip` returns `DisplayError::Device`, the flip can still complete, a later `wait_flip` returns its event, and until then `set_mode` and `present` return `DisplayError::Device` as busy.

`wait_flip` with no flip queued is `DisplayError::Device` and does not block, and before the first `set_mode` it is `DisplayError::NoMode`, as on the fake display.

**Interlace.** `GpuCaps::flip_per_field` comes from the capability table (`neptron-platform`), not from the backend. The backend passes the kernel's flip events on as they come: one event per vblank that the driver signals. On hardware with `flip_per_field`, the driver signals each field, and a flip can complete at each field. Without it, the driver signals each frame. The backend does not change the cadence and does not drop events. The engine, which reads the same capability, paces the core.

On `vkms` the vblank timer of an interlaced mode runs at twice the field rate (OP-030): for 480i at 59.94 Hz it gives about 120 events per second. This is an artefact of how `vkms` computes the frame time, not the behaviour of a real driver. So on `vkms` an interlaced mode is tested for the commit, the readback, the CRC and the writeback only, and the flip timing of interlaced modes is tested with the simulated clock (fake display) and on hardware.

## The kernel mode record

`ModeRecord::from_validated` takes a `ValidatedMode` and is the only way to make a record. It copies the timings, sets the sync polarity flags, `INTERLACE` and `DBLSCAN`, the type `USERDEF`, `vrefresh` rounded from the field rate, and a name such as `640x480i`. The kernel takes the clock in kHz. A validated clock that is not a whole number of kHz is refused, never rounded: the kernel gets exactly the modeline that passed. `hskew` and `vscan` are 0.

The conversion back (the readback) refuses a kernel mode with a flag or a field that a `Modeline` cannot hold, so it is never read as a different mode.

`kms-spike` has the same conversion. The two do not share code: the dependency rules allow `kms-spike` only `drm`, `neptron-types` and `neptron-safety`, and a change to that rule needs a decision (run rule 9). The tests of both use the same rule: a non-kHz clock is refused.

## Errors

| `DisplayError` | When |
| --- | --- |
| `ModeRefused` | The mode cannot be expressed for the kernel (clock not a whole kHz), is larger than the buffers, or the test-only commit refused it. Nothing changed |
| `SceneRefused` | The renderer refuses the scene, with the reasons of the reference renderer, or a limit of the GLES renderer |
| `NoMode` | `present` or `wait_flip` before the first `set_mode` |
| `Device` | A flip is pending (`present`, `set_mode`), no flip is queued (`wait_flip`), a flip did not complete in time, the real commit, the readback, the GL context, the GBM surface or the event read failed |

Errors of `open` and `from_env` are `KmsError`: the descriptor, the device, the connector choice, the CRTC, the plane, a property that is missing, the buffers and the renderer.

## Restore

**At drop.** When the display drops after a mode was set, the backend makes one commit, test-only first: the plane off (no framebuffer, no CRTC) and the CRTC with its mode. The last validated mode stays on the wire with a black picture, which is the state the supervisor rule wants (CRT safety model, layer 7: the mode stays across a restart of the frontend). When that commit fails, for example because a driver needs a primary plane on an active CRTC, the backend switches the output off (no mode, no plane): no signal is safe for every display. Then it removes its framebuffers and returns the buffers. A display that never set a mode makes no commit at drop.

**A panic.** In the dev profile a panic unwinds, the display drops, and the restore above runs. In the release profile a panic aborts, and no code runs. Then:

- Every mode that the backend ever committed passed `validate`, so the mode on the wire is a validated mode.
- The kernel sets no mode of its own: the images have no fbdev emulation and no framebuffer console (layer 8).
- When the launcher holds the descriptor (the product), its open file description stays open. The framebuffers stay, and the mode and the last picture stay on the wire until the next frontend commits.
- When no other process holds the descriptor (a tool or a test), the kernel removes the framebuffers at the last close. What the display does then depends on the driver: on `vkms` the CRTC stays active with its mode and no plane (`kms-spike`), other drivers switch the CRTC off. Both states are safe: a validated mode, or no signal.

On a desktop kernel with a framebuffer console, the console takes the display back at the last close, in its own mode. That mode is not NEPTRON's, and NEPTRON images do not have a console.

**After a crash.** The supervisor restarts the frontend, and its first action is to set the last validated mode again (layer 7). The backend's `open` sets nothing on its own.

## Writeback

`KmsConfig::writeback` is off in the product. With it on, `open` finds a writeback connector (an error when there is none), and `set_mode` routes it to the CRTC as part of the modeset. `KmsDisplay::capture` then returns the picture that the CRTC composes:

1. No mode: `NoMode`. A flip is pending: `Device`.
2. A dumb buffer of the mode size, XRGB8888, and its framebuffer.
3. A commit, test-only first, that sets `WRITEBACK_FB_ID` and `WRITEBACK_OUT_FENCE_PTR` on the writeback connector. No `ALLOW_MODESET`: the routing did not change.
4. A wait for the out fence of at most `WRITEBACK_WAIT_LIMIT_MS` (1000 ms).
5. A copy of the buffer into a `Capture` (rows of XRGB8888 with no padding), then the framebuffer and the buffer are removed.

A capture is a test and diagnostic tool (T3, T28, the self-test of T29). It allocates, and it is not for the frame loop.

## What `vkms` proves

Timing check 7 runs in the aarch64 QEMU guest on `vkms`, with Mesa softpipe for GL: `cargo xtask kms-check`. The program `examples/kms_check.rs` gets the device from the shell as the launcher gives it (`NEPTRON_DRM_FD=3`, the device open on descriptor 3) and checks:

1. No modeset before the first commit: after `from_env` the CRTC has no mode and is not active, as before. Two short runs before the main one: with a pipe on descriptor 3, `from_env` gives `NotDrm`, the pipe stays open and readable, and a second call gives `NotDrm` again (not `LauncherFdTaken`); with `NEPTRON_DRM_FD=1`, `from_env` gives `StandardFd` and standard output still works.
2. For each test mode (240p at 6.7 MHz, 640x480 at 31 kHz, 1920x240 super width, 480i): `set_mode` succeeds, the mode it returns equals the request field by field, and a second read of the CRTC through another descriptor gives the same mode.
3. The frame CRC from debugfs after each switch: stable over several frames of a still picture, different for a different picture, and equal when the same mode and picture come back.
4. The writeback picture equals the reference renderer's output for the same scene, byte for byte in the colour bytes, for two nearest scenes with the UI in each test mode. In each mode with a whole vertical multiple of 2 (480p, 480i), a scene with black scanlines at 50 % gives the same writeback picture as the reference renderer and as `GlesRenderer::render` (the read-back path): the window path and the read-back path share one draw function, and this proves it on the output.
5. Flips: 30 presents with `wait_flip` in each progressive mode. The sequence grows by at least one per flip, the time grows, and the mean time per vblank is within 1 % of the period of the mode. Missed vblanks are counted and printed, not failed: the guest is not real time.
6. The errors of the contract: `present` and `set_mode` while a flip is pending, `wait_flip` with no flip queued, `present` and `wait_flip` before the first mode, a mode larger than the buffers, all as the fake display.
7. Drop: after the display drops, the CRTC keeps the last mode with no plane, or is off.

What the run showed (aarch64 guest, Mesa softpipe, kernel 6.18): every check passes. Mesa's GBM on `vkms` takes no modifiers, so the buffers are linear through the usage flag. The flip period equals the period of each progressive mode to 0 ppm; 480i gives one vblank per 8.33 ms, twice the field rate (OP-030). The dev build draws about four frames per second with software GL, so most vblanks have no flip; the check counts them and does not fail on them. The kernel logs "Overflow of CRC buffer" while the CRC file is open and not read; the check reads the newest entries after the picture settles.

`vkms` cannot prove:

- Anything about a real GPU or a real monitor: the pixel clock the hardware makes, the minimum pixel clock, the timing of a real vblank, the flip cadence of an interlaced mode (OP-030), the linear-buffer path for 480i on i915.
- The connector choice between several displays: `vkms` on kernel 6.18 has one connector (`Virtual-1`). The rule is tested as a pure function in tier 1.
- What a real driver does at the last close (the CRTC off or on), and the console takeover: `vkms` has no console.
- The modifier path: Mesa's `kms_swrast` on `vkms` may not take modifiers, so the fallback with the linear usage flag can be the one that runs. The program prints which path ran.
- Real-time behaviour: QEMU on a loaded host misses vblanks.

The hardware checks go to `docs/hardware-checklist.md`.

## Open points

- With several connected connectors and no connector of the class the rule asks for (a CRT profile and only digital candidates, or the reverse), the first candidate wins. Where: `connector.rs`. How to change: a decision in the Definition.
- A CRTC that drives two connectors (a firmware clone) is refused at open, not split. Where: `device.rs`. How to change: detach the other connector in the first modeset, after a decision that it is safe.
- After a crash with the launcher holding the descriptor, the framebuffers of the dead frontend stay until the launcher closes the descriptor; each restart adds a few. Where: the launcher (T27). How to change: the launcher re-opens the device after a number of restarts, or the frontend removes framebuffers it does not know at open.
