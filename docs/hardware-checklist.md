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

## T17 audio

- [ ] Latency to the speaker: play a click on a frame and measure the time from the flip to the sound, on the i3-6100T and on the Pi 3. Expected about 53 ms.
- [ ] SDL queue allocation on the target: a 10-minute game with an allocation tracer; `SDL_PutAudioStreamData` stops allocating after warm-up. Note any stall in the frame times.
- [ ] Pi 3 CPU cost: release build, CPU share of `AudioPipeline::end_frame` with Sinc. Estimate 3 to 4 % of one core; use Polynomial if it is over budget.
- [ ] Device buffer: record `Sdl3Sink::device_buffer_frames()` and the device rate on each output. The 512-frame hint is honoured.
- [ ] The tone is heard: `cargo xtask test -p neptron-audio --features sdl3 --run-ignored only`. A person confirms a clean 440 Hz tone for one second, with no clicks.
