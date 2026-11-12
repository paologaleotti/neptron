# Hardware check list

Emulation cannot verify these checks. Each one needs real hardware. No result is claimed until a person runs the check and records the result here. The orchestrator adds the checks of each task when the task merges.

Bench: Intel i3-6100T (HD 530) with a Samsung SyncMaster 740s through a DisplayPort to VGA bridge. Other targets are named in the check.

## T21 kms-spike (i3-6100T with SyncMaster 740s)

- [ ] Build on Linux with `cargo build -p kms-spike --features kms` (dev profile; the release profile aborts on panic and skips the restore guard).
- [ ] Run as root on a text console with no X, Wayland or NEPTRON running.
- [ ] Run `kms-spike --list-connectors` and record the name of the VGA port. It is probably `DP-n`, behind the DP-to-VGA bridge.
- [ ] Dry run `kms-spike pc_31_120 --connector <name>`: check the printed ranges against the 740s manual. Record the restore plan: output off is expected, because the console mode fails `pc_31_120`.
- [ ] Confirmed run with `--i-have-checked-the-monitor`, for each shown mode:
  - The border is visible on all four edges, the cross is centred, and the label matches the mode.
  - The monitor's on-screen display shows about 31.4 kHz at 120 Hz for 240p, and 31.4 kHz at 60 Hz for 480p.
  - In the report, `test_commit` and `commit` are ok, and `readback.equals_request` is true.
  - For a progressive mode, `vblank.rate_mhz` is within 1 % of the field rate. For 480i, record `rate_mhz` and compare it with the field rate and with twice the field rate.
  - Read `flip_cadence` together with `flips.skipped_vblanks`.
  - Compare `implied_pclock_hz` with `requested.pclock_hz` and with `/sys/kernel/debug/dri/0/i915_display_info`.
- [ ] At the end, the console comes back in its own mode after the "output off" restore, and the summary shows `restore:` with no `RESTORE FAILED`.
- [ ] Low clock through the bridge: `kms-spike pc_31_120 --connector <name> --width 320 --allow-low-pclock 10000000 --i-have-checked-the-monitor` (about 12.8 MHz). Record whether the picture is stable and what `implied_pclock_hz` reports.
- [ ] Ctrl-C during a mode: the display is restored within about 1 s.
- [ ] From a second console, `kill -TERM <supervisor pid>` during a mode: the same.
- [ ] `kill -KILL <worker pid>` during a mode: record what the tube shows (the test mode with a black picture, no signal, or the console). Recover with a VT switch.
- [ ] Keep `dmesg | tail -50` and the JSON report of every run.
- [ ] Later, on a 15 kHz display only: `kms-spike ntsc --connector <name> --allow-low-pclock 6000000 --allow-interlace --modes 240p,480i --i-have-checked-the-monitor`. Record whether 480i with a linear dumb buffer passes the test-only commit, and the `flip_cadence` (per field or per frame). This is not possible on the 740s.
- [ ] 480p on `pc_31_120`: the vertical porches are about 14, 3 and 27 lines (VGA is 10, 2 and 33). Record whether the picture sits off centre.

## T19 GLES renderer

- [ ] Intel HD 530 (i915, Mesa iris): run the `gles_compare` program (built by `cargo xtask gles-compare`) on the target. All nearest and UI scenes are byte-equal, sharp bilinear is within 1 per channel, and the program reports `GL renderer: Mesa Intel(R) HD Graphics 530`. The surfaceless platform finds the render node (`/dev/dri/renderD128`) and does not fall back to software.
- [ ] Raspberry Pi 3B (vc4, Mesa): the same comparison. highp floats are available in the fragment shader, and `GL_MAX_TEXTURE_SIZE` (2048 on vc4) is enough: a 320x240 XRGB8888 frame needs a 1280-texel-wide texture, and a 1920x240 output needs 1920.
- [ ] Timing on a real GPU: a 1920x240 super-width frame with sharp bilinear and the UI draws and presents within one frame period at 120 Hz. Measured with the KMS backend (T23).
- [ ] Raspberry Pi 3B (vc4): `headless()` passes the highp check (at least 23 bits). Nearest texel addressing at `(x + 0.5) / width` picks the exact byte texel on textures up to 2048. The every-value scenes and the 1920x240 scenes are byte-equal.

## T18 input

- [ ] A USB gamepad on the x86_64 image: found at boot and when plugged in later, buttons as in the gamepad map, a disconnect releases its buttons.
- [ ] SDL3 without libudev finds a gamepad plugged in after start (inotify on `/dev/input`).
- [ ] A USB keyboard on the image: found, grabbed (no key reaches a console), mapped, and released when unplugged.
- [ ] Blind recovery from a gamepad and from a keyboard with no picture.
- [ ] The combination held at boot on a keyboard encoder triggers recovery.
- [ ] The Start hold from an arcade panel encoder with no Select button opens the quick menu.

## T17 audio

- [ ] Latency to the speaker: play a click on a frame and measure the time from the flip to the sound, on the i3-6100T and on the Pi 3. Expected about 53 ms.
- [ ] SDL queue allocation on the target: a 10-minute game with an allocation tracer; `SDL_PutAudioStreamData` stops allocating after warm-up. Note any stall in the frame times.
- [ ] Pi 3 CPU cost: release build, CPU share of `AudioPipeline::end_frame` with Sinc. Estimate 3 to 4 % of one core; use Polynomial if it is over budget.
- [ ] Device buffer: record `Sdl3Sink::device_buffer_frames()` and the device rate on each output. The 512-frame hint is honoured.
- [ ] The tone is heard: `cargo xtask test -p neptron-audio --features sdl3 --run-ignored only`. A person confirms a clean 440 Hz tone for one second, with no clicks.

## C04 scanlines

- [ ] 2x black scanlines at 100, 75 and 50 percent on the SyncMaster 740s: brightness, and whether the lines read as gaps. Record the value that looks right.
- [ ] No moiré from the DisplayPort to VGA bridge with scanlines on.

## C02 policy

- [ ] Space Invaders on `arcade_15`: the mode runs at 16.135 kHz. The picture locks and the height is correct.
- [ ] Donkey Kong (unrotated) on `arcade_15`: the mode runs at 16.181 kHz. The picture locks. Record what the crop hides.
- [ ] `native` with a 240-line game on the SyncMaster 740s (`pc_31_120`): the mode runs at double refresh near 120 Hz and the notice shows.

## T23 KMS backend (Intel HD 530 with i915)

Run the steps with the real frontend path (`KmsDisplay::from_env` from the launcher) on the i3-6100T. Keep `dmesg | tail -50` with each result.

- [ ] Open: with no setting, the connector rule picks the VGA output (an analog connector with EDID, or the DP-to-VGA bridge as `DP-n`). Record the name, the driver, and whether the buffers got linear by the modifier list or by the usage flag.
- [ ] No modeset before the first commit: after open, the firmware's mode is still on the wire (i915 inherits it) until the first `set_mode`.
- [ ] Atomic modeset of a 15 kHz super-width mode (for example 1920x240 at 60 Hz, `generic_15`, with the pixel clock floor of the capability table): test-only commit and commit pass, the readback equals the request field by field, the monitor syncs.
- [ ] Atomic modeset of each bench mode: 240p at 120 Hz and 480p at 60 Hz on the SyncMaster 740s (`pc_31_120`); readback equal; the picture shows the full test scene with no offset.
- [ ] Positive sync on the wire: a mode with positive horizontal and vertical sync reads back with both polarities positive, and a scope or the monitor's on-screen display shows positive sync.
- [ ] Connector choice with several outputs: with VGA and a digital monitor both connected and no setting, a CRT profile picks the analog output and a flat-panel profile the first digital one; a VGA output with no EDID is used only when named.
- [ ] The real minimum pixel clock: lower the pixel clock of a 240p mode step by step (with `--allow-low-pclock` in `kms-spike`, then through the backend); record the lowest clock the test-only commit takes and the lowest the monitor syncs to, against the capability table.
- [ ] 480i on a 15 kHz display with linear buffers and no kernel patch: the commit passes and the picture is stable.
- [ ] Flip timing: 600 presents with `wait_flip` per progressive mode. The mean time per vblank is within 0.1 % of the mode period; record the jitter and the missed vblanks. Record the pixel clock the period implies against the requested clock.
- [ ] Interlace cadence: in 480i, count flip events per second. Record whether i915 signals one flip per field (about 59.94 per second) or per frame (about 29.97), and set `flip_per_field` in `data/hardware/matrix.toml` from it (OP-030).
- [ ] A flip that never completes: not testable by hand; check only that no `wait_flip` error appears in a 30-minute run.
- [ ] Restore at a normal exit: the plane goes off and the last validated mode stays on the wire (or the output goes off, if i915 refuses an active CRTC with no primary plane). Record which.
- [ ] Restore after a panic in the release build (abort), with the launcher holding the descriptor: the mode and the last picture stay; the respawned frontend sets the last validated mode again.
- [ ] Restore after a kill with no launcher (the frontend opened the device itself): record whether i915 keeps the CRTC on with no plane, or switches it off.

## C07 blank and the bounded flip wait

- [ ] `blank` on i915 with the real CRT: the CRTC goes inactive, the monitor shows no sync and goes to standby with no damage.
- [ ] `set_mode` after a blank brings the picture back, with the read back equal to the request.
- [ ] A blank before the first mode turns off the mode that i915 inherited from the firmware.
- [ ] The same three checks on the Raspberry Pi 3 (vc4).
- [ ] A flip whose wait times out at 500 ms on a real driver: the event of the late flip comes on the next `wait_flip`, and `present` and `set_mode` stay busy until then.

