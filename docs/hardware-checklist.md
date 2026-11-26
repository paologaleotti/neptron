# Hardware check list

Emulation cannot verify these checks. Each one needs real hardware, and each one is labelled `needs-hardware` until a person runs it. A result is written here only when a person ran the check, with its date and its source (a document or a commit). A check that is done keeps one line with its result. The orchestrator adds the checks of each task when the task merges.

Sections 1 to 7 group the checks by machine and setup: the PC with the VGA CRT, the PC on a flat panel, the PC with any monitor, the PC with a 15 kHz display, the Raspberry Pi 3 on HDMI, the Raspberry Pi 3 on composite, and any machine. Each group starts with the results recorded so far. The sections after section 7 hold the checks of one feature on several machines; each check names its machine.

Two limits of the images decide where a check can run:

- Every PC GPU in `data/hardware/matrix.toml` has `interlace = false` (the bench PC is `intel_gen9`). The validator refuses an interlaced mode on the PC, so a check of an interlaced mode (480i, 576i, 448i, 768i) runs on the Pi 3 composite output, never on the PC. Only `kms-spike` with `--allow-interlace` commits an interlaced mode on the PC.
- The Pi image ships only the `vc4-kms-v3d` overlay (and `disable-bt` in the development variant, `os/board/rpi/post-image.sh`), with no DPI overlay. A check on "a Pi 3 on DPI" is marked "Not possible in this image".

## How to record a result

- Each check has three parts. **Do** is what you do. **Expect** is what a pass looks like. **Record** is what you write down.
- Tick the box only when the check passes. When it fails, leave the box empty, write what you saw under the check, and open a point for the orchestrator.
- Keep these files with each result: the JSON report of the tool, and the output of `dmesg | tail -50` after the run. A photo of the screen helps when the picture is wrong.
- Run every tool as root on a text console, with no X or Wayland. Stop the frontend and the launcher before a tool that opens the DRM device (`neptronctl selftest video`, `kms-spike`, `gles_compare`).

## 1. PC i3-6100T with the VGA CRT (SyncMaster 750s)

- Computer: Intel i3-6100T with Intel HD 530 (i915, capability table entry `intel_gen9`).
- Monitor: Samsung SyncMaster 750s, a 31 kHz PC CRT, through a DisplayPort to VGA bridge. Its preset is `pc_31_120`: 31.4 to 31.6 kHz, 100 to 130 Hz for 200 to 256 lines, 50 to 65 Hz for 400 to 512 lines, no interlace.
- Also needed for some checks: a USB gamepad, a USB keyboard, a second console (SSH or another virtual terminal).

Results recorded: none on the CRT.

### Safety rules for the 750s

The frontend falls back to its safe default when it has no settings, after two failed boots, and on blind recovery. The safe default is the EDID profile when the monitor gives EDID, and else `generic_15` (15 kHz). A 15 kHz signal is out of range for the 750s. It is deliberate on an unknown display, but do not send it to this monitor.

- Do the check "Find the VGA connector" first. Its result decides the rest:
  - **EDID**: the `edid` file of the connector is not empty. The safe default is the EDID profile, and every check below can run.
  - **No EDID**: the file is empty. Before the frontend starts with the 750s connected, write the preset into `/state/config/global.toml` (`[display]` with `monitor = "pc_31_120"`), and confirm the boot prompt within 15 s (`CONFIRM_WINDOW_NS`); with no confirmation the guard reverts to the safe default. Skip each check marked "Needs EDID", and write "skipped: no EDID through the bridge" under it.
- Keep a hand on the power switch of the monitor. Switch the monitor off at once if it whines, clicks, or shows a rolling or folded picture, and write down what the tool or the frontend showed at that moment.
- A check marked "Needs EDID" can make the frontend fall back to the safe default.

### First boot and safety checks

Do these before the CRT gets a mode that nobody checked on it. First boot the image on a flat panel (section 3, the first check of "The PC image (T28)").

- [ ] `needs-hardware` **Find the VGA connector.**
  - Do: with the CRT connected through the bridge, run `ls /sys/class/drm/` and `cat /sys/class/drm/card0-*/status`, or `kms-spike --list-connectors`.
  - Expect: one connector of the bridge is connected. It is probably `DP-1`, not `VGA-1`.
  - Record: the connector name, its status, and the size of its `edid` file (0 means that the bridge passes no EDID).
- [ ] `needs-hardware` **Check the ranges before the first mode.**
  - Do: `neptronctl selftest video --profile pc_31_120 --connector <name> --report /tmp/dry-pc.json`. If the `edid` file is not empty, also `neptronctl selftest video --profile edid --connector <name> --report /tmp/dry-edid.json`.
  - Expect: the dry run sets no mode. It prints the ranges of the profile, the GPU values (table entry `intel_gen9`, pixel clock 25 to 165 MHz, no interlace) and the modes: 240p at 120 Hz (`240p_double_refresh`), 480p, 768x224 at 120 Hz (`super_width_dynamic`) and 1920x224 at 120 Hz (`super_width_fixed`).
  - Record: compare each range with the manual of the 750s. Write down the modes and the EDID profile id (`edid_...`) and its ranges.
- [ ] `needs-hardware` **The first start on the 750s.**
  - Do: with **EDID**, boot the image with only the CRT connected and no settings file. With **no EDID**, do not boot with no settings: write `pc_31_120` into the settings first (see "Safety rules for the 750s"), boot, and confirm the boot prompt.
  - Expect: with EDID, the frontend uses the EDID profile and shows the game list. With no EDID, it uses `pc_31_120` and shows the game list. No noise comes from the tube.
  - Record: which case it was, what the monitor shows, and which profile the frontend log names.
- [ ] `needs-hardware` **The output can be turned off.** (C07)
  - Do: in the self-test, or with the frontend, cause a `blank` (the end of `neptronctl selftest video` is a blank).
  - Expect: the CRTC goes inactive, the monitor shows no sync and goes to standby, with no damage.
  - Record: what the monitor shows, and the time to standby.
- [ ] `needs-hardware` **A stop signal restores the display.**
  - Do: start `neptronctl selftest video ... --i-have-checked-the-monitor pc_31_120` and press Ctrl-C during a mode. Do it again with `kill -TERM <pid>` from a second console, and once with `kill -HUP <pid>`.
  - Expect: the run stops within about 1 s, prints `Stopped: a stop signal came` and `Restore: the output is off`, and exits with 7. The report has `state` `stopped`.
  - Record: the time to the restore, and the `restore` field of the report.
- [ ] `needs-hardware` **A kill shows what the driver does with no restore.**
  - Do: `kill -KILL <pid>` of `neptronctl` during a mode.
  - Expect: no code of the tool runs. The tube shows the validated test mode with a black picture, or no signal. When the tool closes, the console comes back in its own mode, or the output stays off.
  - Record: what the tube shows, and whether the console came back or the output stayed off. Recover with a switch to another virtual terminal, or run the self-test again.
- [ ] `needs-hardware` **The self-test refuses while the frontend runs.**
  - Do: with the frontend running (with EDID, or with `pc_31_120` in the settings), run the self-test with the confirmation flag.
  - Expect: exit code 6 and an error that says the frontend answers on the control socket, or that `neptron` or `neptron-launch` runs. No mode changes.
  - Record: the error text.

### The self-test

`neptronctl selftest video` shows one test mode per mode family of the profile, each for 10 seconds, with the kernel's read back and the flip timing. `apps/neptronctl/README.md` describes the report.

- [ ] `needs-hardware` **The self-test on `pc_31_120`.**
  - Do: `neptronctl selftest video --profile pc_31_120 --connector <name> --report /tmp/selftest-pc.json --i-have-checked-the-monitor pc_31_120`
  - Expect: each mode shows for 10 s. The white border is visible on all four edges, the cross is at the centre, and the text names the family, the size, the rates and the pixel clock. The read back equals the request, and the monitor holds each mode with no roll, tear, flicker or offset. If the monitor shows its frequencies, 240p and the super widths read about 31.4 kHz at 120 Hz, and 480p about 31.4 kHz at 60 Hz. The tool prints `PASS` for each mode, `Restore: the output is off`, `Result: PASS`, and exits with 0.
  - Record: for each mode, from the report: `readback_equal`, `timing.period_error_ppm`, `timing.implied_pclock_hz` against `request.pclock_hz`, `timing.missed_vblanks` and `timing.flips_per_second_mhz`. For each mode, what the picture showed: how much of the border is hidden, and whether the picture sits off centre.
- [ ] `needs-hardware` **The self-test on the EDID profile.**
  - Do: if the bridge passes EDID, the same with `--profile edid --i-have-checked-the-monitor <edid id>`.
  - Expect: as for `pc_31_120`. The modes follow from the ranges of the EDID.
  - Record: the modes and the same fields as above.
- [ ] `needs-hardware` **The clock the hardware makes.**
  - Do: during the self-test, read `/sys/kernel/debug/dri/0/i915_display_info` from a second console.
  - Expect: the clock of the CRTC is near the requested clock.
  - Record: the clock i915 reports, `implied_pclock_hz` and `request.pclock_hz` for each mode.
- [ ] `needs-hardware` **480p and the 2x picture mode are stable.** (MVP exit)
  - Do: watch the `480p` mode of the self-test. The `2x` family gives the same mode on this bench.
  - Expect: a stable picture for the full 10 s.
  - Record: stable or not.
- [ ] `needs-hardware` **240p at 120 Hz is stable.** (MVP exit)
  - Do: watch the `240p_double_refresh` mode.
  - Expect: a stable picture for the full 10 s.
  - Record: stable or not.
- [ ] `needs-hardware` **The super-width modes through the bridge.**
  - Do: watch `super_width_dynamic` (768x224) and `super_width_fixed` (1920x224, about 77 MHz).
  - Expect: the bridge passes both clocks, and the picture has the same width in both.
  - Record: stable or not, and any noise on vertical edges.

### kms-spike (T21)

Keep the JSON report and `dmesg | tail -50` of every run of the spike.

- [ ] `needs-hardware` **Build the spike.**
  - Do: build on Linux with `cargo build -p kms-spike --features kms` (dev profile).
  - Expect: it builds. The release profile aborts on a panic and skips the restore guard, so do not use it.
  - Record: the commit that was built.
- [ ] `needs-hardware` **Dry run of the spike.**
  - Do: `kms-spike pc_31_120 --connector <name>`.
  - Expect: the printed ranges match the manual of the 750s. The restore plan is "output off", because the console mode fails `pc_31_120`.
  - Record: the restore plan.
- [ ] `needs-hardware` **Confirmed run of the spike.**
  - Do: `kms-spike pc_31_120 --connector <name> --i-have-checked-the-monitor`.
  - Expect: for each mode, the border is visible on all four edges, the cross is centred, and the label matches the mode. The monitor's on-screen display shows about 31.4 kHz at 120 Hz for 240p, and 31.4 kHz at 60 Hz for 480p. 480i is refused: `pc_31_120` has no interlaced range. In the report, `test_commit` and `commit` are ok, and `readback.equals_request` is true. For a progressive mode, `vblank.rate_mhz` is within 1 % of the field rate. At the end, the console comes back in its own mode after the "output off" restore, and the summary shows `restore:` with no `RESTORE FAILED`.
  - Record: `flip_cadence` together with `flips.skipped_vblanks`. `implied_pclock_hz` against `requested.pclock_hz` and against `/sys/kernel/debug/dri/0/i915_display_info`.
- [ ] `needs-hardware` **A low clock through the bridge.**
  - Do: `kms-spike pc_31_120 --connector <name> --width 320 --allow-low-pclock 10000000 --i-have-checked-the-monitor` (about 12.8 MHz).
  - Expect: the test-only commit passes or fails with a clear error.
  - Record: whether the picture is stable, and what `implied_pclock_hz` reports.
- [ ] `needs-hardware` **Stop signals of the spike.**
  - Do: Ctrl-C during a mode. Then, from a second console, `kill -TERM <supervisor pid>` during a mode. Then `kill -KILL <worker pid>` during a mode.
  - Expect: Ctrl-C and SIGTERM restore the display within about 1 s.
  - Record: for SIGKILL, what the tube shows (the test mode with a black picture, no signal, or the console). Recover with a VT switch.
- [ ] `needs-hardware` **The vertical porches of 480p.**
  - Do: show 480p of `pc_31_120` with the spike.
  - Expect: the vertical porches are about 14, 3 and 27 lines (VGA is 10, 2 and 33).
  - Record: whether the picture sits off centre.

### KMS backend (T23) and blank (C07)

Run these with the real frontend path (`KmsDisplay::from_env` from the launcher) on the i3-6100T, unless the check names the self-test.

- [ ] `needs-hardware` **Open picks the VGA output.**
  - Do: start the frontend with no `display.output` setting (with EDID, or with `pc_31_120` in the settings).
  - Expect: the connector rule picks the VGA output: an analog connector with EDID, or the DP-to-VGA bridge as `DP-n`.
  - Record: the name, the driver, and whether the buffers are linear by the modifier list or by the usage flag (`display.linear_by_modifier` in the self-test report says the same).
- [ ] `needs-hardware` **No modeset before the first commit.**
  - Do: open the display and do not set a mode.
  - Expect: the firmware's mode is still on the wire (i915 inherits it) until the first `set_mode`.
  - Record: what the monitor shows.
- [ ] `needs-hardware` **Positive sync on the wire.**
  - Do: set a mode with positive horizontal and vertical sync.
  - Expect: the read back has both polarities positive, and a scope or the monitor's on-screen display shows positive sync.
  - Record: what the scope or the monitor shows.
- [ ] `needs-hardware` **Connector choice with several outputs.**
  - Do: connect VGA and a digital monitor, with no setting.
  - Expect: a CRT profile picks the analog output and a flat-panel profile the first digital one. A VGA output with no EDID is used only when named.
  - Record: the connector of each case.
- [ ] `needs-hardware` **The lowest pixel clock.**
  - Do: lower the pixel clock of a 240p mode step by step, with `--allow-low-pclock` in `kms-spike`, then through the backend.
  - Expect: below some clock the test-only commit fails or the monitor loses sync.
  - Record: the lowest clock the test-only commit takes and the lowest the monitor syncs to, against the 25 MHz of the capability table.
- [ ] `needs-hardware` **Flip timing.**
  - Do: 600 presents with `wait_flip` per progressive mode.
  - Expect: the mean time per vertical blank is within 0.1 % of the mode period.
  - Record: the jitter, the missed vertical blanks, and the pixel clock that the period implies against the requested clock.
- [ ] `needs-hardware` **A flip that never completes.**
  - Do: this cannot be caused by hand. Run the frontend for 30 minutes.
  - Expect: no `wait_flip` error in the log.
  - Record: the number of `wait_flip` errors.
- [ ] `needs-hardware` **A late flip.**
  - Do: watch for a flip whose wait times out at 500 ms on the real driver.
  - Expect: the event of the late flip comes on the next `wait_flip`, and `present` and `set_mode` stay busy until then.
  - Record: whether it happened, and the log lines.
- [ ] `needs-hardware` **Restore at a normal exit.**
  - Do: end the frontend normally.
  - Expect: the plane goes off and the last validated mode stays on the wire, or the output goes off if i915 refuses an active CRTC with no primary plane.
  - Record: which of the two.
- [ ] `needs-hardware` **Restore after a crash of the frontend.**
  - Do: in the release build, cause an abort, with the launcher holding the descriptor (over SSH: `kill -ABRT` of the frontend).
  - Expect: the launcher's descriptor survives, the mode and the last picture stay on the VGA output, and the launcher starts the frontend again. The respawned frontend sets the last validated mode again.
  - Record: what the monitor shows during the respawn.
- [ ] `needs-hardware` **Restore after a kill with no launcher.**
  - Do: kill a frontend that opened the device itself.
  - Expect: i915 keeps the CRTC on with no plane, or switches it off.
  - Record: which of the two.
- [ ] `needs-hardware` **Set a mode after a blank.** (C07)
  - Do: `blank`, then `set_mode`.
  - Expect: the picture comes back, and the read back equals the request.
  - Record: the read back.
- [ ] `needs-hardware` **Blank before the first mode.** (C07)
  - Do: `blank` before any `set_mode`.
  - Expect: the mode that i915 inherited from the firmware goes off.
  - Record: what the monitor shows.

### GLES renderer (T19) and scanlines (C04)

- [ ] `needs-hardware` **The renderer on Intel HD 530.**
  - Do: run the `gles_compare` program (built by `cargo xtask gles-compare`) on the target.
  - Expect: all nearest and UI scenes are byte-equal, sharp bilinear is within 1 per channel, and the program reports `GL renderer: Mesa Intel(R) HD Graphics 530`. The surfaceless platform finds the render node (`/dev/dri/renderD128`) and does not fall back to software.
  - Record: the program output.
- [ ] `needs-hardware` **Draw time on a real GPU.**
  - Do: with the KMS backend, draw a 1920x240 super-width frame with sharp bilinear and the UI at 120 Hz.
  - Expect: each frame draws and presents within one frame period.
  - Record: the missed vertical blanks over 600 frames.
- [ ] `needs-hardware` **Scanline strength.**
  - Do: show 2x black scanlines at 100, 75 and 50 percent on the 750s.
  - Expect: the lines read as gaps at a brightness that looks right.
  - Record: the value that looks right.
- [ ] `needs-hardware` **No moiré from the bridge.**
  - Do: scanlines on, through the DisplayPort to VGA bridge.
  - Expect: no moiré.
  - Record: what the picture shows.

### Policy (C02, C09)

- [ ] `needs-hardware` **Double refresh for `native` on 31 kHz.**
  - Do: play a 240-line game with the style `native` on the 750s (`pc_31_120`).
  - Expect: the mode runs at double refresh near 120 Hz, and the notice shows. The motion is smooth, at the right speed.
  - Record: the mode in the quick menu, and what the motion looks like.
- [ ] `needs-hardware` **A narrow mode with a whole-kHz clock.** (C09)
  - Do: on a 31 kHz monitor, play a Game Gear game on `arcade_31`. Check the ranges of `arcade_31` against the monitor first with the self-test dry run.
  - Expect: the picture is centred and stable.
  - Record: the mode in the quick menu, and the position of the picture.

### Engine and guard (T26)

- [ ] `needs-hardware` **A change with no button press reverts.** (MVP exit)
  - Do: on the 750s, change the monitor profile and press nothing. For the MVP exit, do it on the EDID profile of the 750s (Needs EDID).
  - Expect: the change reverts 15 s after its picture appears.
  - Record: the time to the revert.
- [ ] `needs-hardware` **A confirmed change stays.**
  - Do: confirm a change, then reboot.
  - Expect: the mode is the same after the reboot.
  - Record: the mode before and after.
- [ ] `needs-hardware` **Two failed boots give the safe default.** Needs EDID.
  - Do: kill the frontend with SIGKILL twice before 3 s of picture.
  - Expect: the third start comes up in the safe default.
  - Record: the profile of the third start.
- [ ] `needs-hardware` **Blind recovery.** (MVP exit) Needs EDID.
  - Do: hold Select and Start for 5 s with no picture (output off), and again during an open confirm dialog.
  - Expect: the safe default comes back with the audio cue.
  - Record: the time to the picture.
- [ ] `needs-hardware` **A monitor with no valid mode.**
  - Do: make a pending change to a monitor with no valid mode.
  - Expect: the output goes off, and the old mode comes back after about 1 s.
  - Record: the time to the old mode.

### Session and audio (T25, T17)

- [ ] `needs-hardware` **One modeset at launch.**
  - Do: launch a Genesis Plus GX game on KMS.
  - Expect: one modeset at launch, and no audio gap at launch or at a mode switch.
  - Record: the modesets in the log.
- [ ] `needs-hardware` **The quick menu over a frozen frame.**
  - Do: open the quick menu during a game on the CRT.
  - Expect: the menu shows over the frozen frame.
  - Record: what the picture shows.
- [ ] `needs-hardware` **Audio latency on the i3-6100T.**
  - Do: play a click on a frame, and measure the time from the flip to the sound.
  - Expect: about 64 ms.
  - Record: the measured time.
- [ ] `needs-hardware` **SDL queue allocation.**
  - Do: a 10-minute game with an allocation tracer.
  - Expect: `SDL_PutAudioStreamData` stops allocating after warm-up.
  - Record: any stall in the frame times.
- [ ] `needs-hardware` **Device buffer.**
  - Do: read `Sdl3Sink::device_buffer_frames()` and the device rate on each output.
  - Expect: the 1024-frame hint is honoured.
  - Record: the frames and the rate.

### Display guard (M01)

Each check is `needs-hardware`. `docs/spec/display-guard.md` describes the rules.

- [ ] With EDID through the bridge, no settings file. Do: boot. Expect: the EDID profile with no prompt and no notice. Record: the mode.
- [ ] No EDID, `pc_31_120` written in `global.toml`. Do not run this check on the 750s: with no answer the guard reverts to the 15 kHz safe start, which this monitor does not take (see the safety rules above). The revert of an unanswered prompt is checked on the 15 kHz TV and in the tier 2 tests. Write "skipped: 31 kHz monitor".
- [ ] No EDID, `pc_31_120` written. Do: boot, press Right, then Accept within the window. Expect: Keep is taken at once (no wait after the press), no "Display change undone" notice, and the next boot has no prompt. Record: the notices.
- [ ] Blind recovery on this monitor, with EDID only (with no EDID the safe default is 15 kHz: skip it and write "skipped: no EDID"). Do: hold Select and Start for 5 s. Expect: the safe default with the tone; in Settings, Display, the row "Keep this picture" ends safe mode. Record: what the monitor shows.
- [ ] A named 15 kHz preset with an analog EDID of a PC monitor: `arcade_15` written in `global.toml`, and a VGA EDID emulator (or a KVM) that reports 30 to 70 kHz in front of a PC CRT, never in front of a 15 kHz tube. Do: boot and press nothing; then boot again; then hold Select and Start for 5 s. Expect: the boot prompt, then the 15 kHz safe start (out of range on the PC CRT); `global.toml` names `generic_15`, not `auto`; the next boot and the recovery send 15 kHz too. Record: the line rate of each mode in the log of the frontend.

### The bridge is a CRT output (C08)

- [ ] The bench (DP-to-VGA bridge, SyncMaster 750s). Do: start with `display.monitor = "auto"` and read the display panel. Expect: the CRT path, never the panel path; the limits are 165 MHz and 1920x1200. Record: the `subconnector` value and the profile id.

## 2. PC i3-6100T on a flat panel

The first hardware session (2026-11-13) used the development image on a Dell U2724D (2560x1440, DisplayPort). OP-078 to OP-084 record its findings.

Results recorded:

- 2026-11-13: the development image boots on the Dell U2724D, and a Mario game plays (the owner's report).
- 2026-11-13: the menu runs at 2560x1440 and 120 Hz, and a 59.923 Hz game switches to the 59.951 Hz mode (OP-081).
- 2026-11-13: the frontend started three times with "no connected display found" before the DisplayPort monitor answered (OP-083, fixed by C11; the check is below).
- 2026-11-13: sound plays on the PC, and the network share takes files (the owner's report; the output and the client are not recorded).
- 2026-11-13: with the `powersave` governor (`intel_pstate`, `energy_performance_preference` `balance_performance`), FinalBurn Neo and Metal Slug X (59.18 Hz) on the 59.951 Hz mode presented about 42 frames per second and missed about 18 fields per second; with `performance` written by hand, 60.2 frames per second and none missed. The image now sets `performance` (`os/board/pc/system.config`).
- 2026-11-19, commit `c02ae52e` (the orchestrator, Killer Instinct in its attract mode, 40 s averages): late pacing presents 48.1 frames per second at 3.2 GHz with 0.45 underruns per second, and 49.8 at 2.7 GHz with 0.75, against 38.2 and 29.8 before late pacing; no dropped frame; light scenes 58 to 60 per second, heavy scenes 39.6 to 42 with 18 to 21 missed fields; about 3 pacing changes in 30 s. `audio_queued_frames` read 2000 to 5200 frames, and the underruns came in bursts at the entry into a heavy scene; the queue correction of later commits addresses both.

### C08 flat panels

Each check is `needs-hardware`.

- [ ] An HDMI 1080p panel on the HD 530. Do: first boot with no settings. Expect: 1920x1080 at 60 Hz with no prompt (ADR 0013), the menu at 4x and centred. A SNES game is 1194x896, centred, sharp, with black around it. Record: the mode line of the Video block.
- [ ] A 4K panel on DisplayPort. Expect: 3840x2160 at 60 Hz. Record: whether the start succeeds and the frame times.
- [ ] A 4K panel on HDMI. Expect: 1920x1080 at 60 Hz with the notice.
- [ ] A 720p TV. Expect: 1280x720, and 50 Hz for a PAL game.
- [ ] An HDMI-to-VGA adapter with its own digital EDID, with a PC monitor behind it, never a 15 kHz tube. Expect: the panel mode of the adapter's EDID with no prompt (ADR 0013 accepts this). With `display.monitor` set to the monitor's preset: the boot prompt, and with no answer the 15 kHz safe start. Record: what the monitor shows in each case.

### Display guard on a panel (M01)

Each check is `needs-hardware`.

- [ ] An HDMI panel on the first boot of a new image. Expect: its native mode with no prompt and no notice. Record: the mode.
- [ ] The same panel switched on 30 s after power-on. Expect: when the kernel read the EDID at boot, the native mode when the panel wakes, with no revert. With no EDID at boot the display is not trusted, and the 15 kHz safe start applies. Record: which case, and what the panel shows.
- [ ] Blind recovery on the panel. Expect: the panel mode again, with the tone. Record: the mode.
- [ ] A hotplug within one run: boot on the trusted panel, then unplug it and plug a CRT (or an adapter with a CRT) into the same connector while the frontend runs. Expect: the panel mode stays on the wire, because the run handles no hot plug; then hold Select and Start for 5 s: the blind recovery reads the display again and sends the safe default of the CRT (the 15 kHz safe start with no EDID), not the panel mode. The next boot reads the new display. Record: the mode on the wire before and after the recovery, the `display:` line of the log, and what the CRT shows, with a hand on its power switch.

### C11 findings of the first hardware session

- [ ] `needs-hardware` Do: boot with the Dell U2724D on DP-1 in standby, `display.output` at `auto`. Expect: the log shows no "cannot open the display" line; it shows "a display answered after N ms" or nothing, and the first picture comes in one start of the frontend. Record: N, and the time from power-on to the first frame.
- [ ] `needs-hardware` Do: boot with no monitor connected, wait 2 minutes, then connect the DisplayPort monitor. Expect: each start logs "no connected display after 15000 ms"; the launcher line "the frontend found no display; it starts again every 1000 ms", with the hint about an analog connector with no EDID, comes at the first start and then at most once a minute; the wait between starts stays 1 s; the picture comes about 1.5 s after the connection (the next look of the wait, then the start); `/state/video/guard.toml` counts no failed boot. Record: the time from the connection to the first picture.
- [ ] `needs-hardware` Do: on the flat panel, open the quick menu in a game and the display panel. Expect: "Refresh rate" is grey with "Not used on a flat panel", and accept does nothing on it; "Picture mode" opens its options; the scanline rows still change the picture. Record: a photo of the quick menu.
- [ ] `needs-hardware` Do: boot twice, then `ls /state/cache/mesa` and read the log of the frontend. Expect: the folder holds the Mesa cache files, and no line says that Mesa cannot create its shader cache. Record: `du -sh /state/cache/mesa`, and whether the second start is faster than the first.
- [ ] `needs-hardware` Do: in the development image, bring eth0 up on the Realtek port and read `dmesg | grep -i rtl`. Expect: no "unable to load firmware patch rtl_nic/rtl8168h-2.fw" line. Record: the dmesg lines of r8169.
- [ ] `needs-hardware` Do: read `dmesg | grep -i dmc`. Expect: the line about the missing i915 DMC firmware is still there: the file is not in the image until the owner decides the licence question of a firmware file inside the kernel image. Record: the line.

### CPU governor

The result of 2026-11-13 is at the top of this section.

- [ ] `needs-hardware` Do: over SSH, `cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor /sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference /sys/devices/system/cpu/cpu0/cpufreq/scaling_driver`. Expect: `performance` for each CPU, in both files; the driver is `intel_pstate`. Record: the output.
- [ ] `needs-hardware` Do: play Metal Slug X (FinalBurn Neo) for 60 s on the flat panel, and read `neptronctl stats` before and after. Expect: `fields_missed` does not change, and `audio_underruns` does not change. Record: both values before and after, and the frames presented.

### Late pacing of a slow core

The session leaves the flip of a late core frame queued and runs the next core frame at once (`docs/spec/pacing-audio.md`, "A core slower than the field"). With no late pacing, Killer Instinct (FinalBurn Neo) on this PC presented 29.8 frames and missed 30.1 fields per second at 2.7 GHz, with about 13 audio underruns per second, and 38.2 frames, 21.7 missed fields and 9.2 underruns per second at 3.2 GHz. The result of 2026-11-19 is at the top of this section.

Read `neptronctl stats` twice, 60 s apart, for each check, and compute the rates per second. Run the pushed client: `cargo xtask os ssh --host A.B.C.D -- neptronctl stats`, or `/state/hot/bin/neptronctl stats` over a plain `ssh`. A plain `ssh root@A.B.C.D neptronctl stats` runs no login shell, gets the image's own `neptronctl`, and can speak an older protocol with no `late_frames` and `late_flips`.

- [ ] `needs-hardware` **Killer Instinct at the `performance` governor.**
  - Do: play Killer Instinct for 60 s on the flat panel (59.951 Hz mode), with the CPU at its full clock (3.2 GHz).
  - Expect: in light scenes `frames_presented` grows by 58 to 60 per second; in heavy scenes by `59.95 / x` (about 40 at 1.5 intervals), and never by about 30. `frames_presented` plus `fields_missed` per second is the field rate, 59.95. `frames_dropped` does not change. `audio_underruns` grows by at most 1 at each change into a heavy scene, never in bursts, and the sound has no crackle. `audio_queued_frames`, read each second, stays under about 3000 frames (62 ms) and is back near its usual 1100 to 2700 within 3 s of each change of scene. When the speed is below 98% for 2 s, the session reports it (`Session::slow_speed_percent`).
  - Record: the four counters and `late_frames` and `late_flips` before and after, the CPU clock (`grep MHz /proc/cpuinfo`), the `late pacing` and `wait pacing` lines of the log, and what the sound is like.
- [ ] `needs-hardware` **Killer Instinct at 2.7 GHz.**
  - Do: limit the CPU to 2.7 GHz (`echo 2700000 > /sys/devices/system/cpu/cpu*/cpufreq/scaling_max_freq`), play 60 s, then write the old value back.
  - Expect: `frames_presented` grows by about `59.95 / x` per second, where `x` is the CPU time of a frame over a field (about 1.04 measured before: about 57.6 per second), not 29.8. The sound plays slower and a little lower with the game, with no crackle; `audio_underruns` grows by at most 1.
  - Record: the counters before and after, and what the sound is like.
- [ ] `needs-hardware` **A light game keeps its latency and smoothness.**
  - Do: play Metal Slug X for 60 s, and read `neptronctl stats` before and after; tap a button and read `input_to_frame_ns`.
  - Expect: 59.95 frames per second, `fields_missed`, `audio_underruns` and `late_frames` do not change, and `input_to_frame_ns` is one field, about 16.7 ms. Scrolling is smooth.
  - Record: the counters and three values of `input_to_frame_ns`.
- [ ] `needs-hardware` **A GPU that makes the flips late.**
  - Do: play Killer Instinct at the clock where its CPU time is just under one field (raise `scaling_max_freq` step by step from 2.7 GHz), for 60 s each, and read `neptronctl stats` before and after.
  - Expect: at no clock does `frames_presented` grow by about 30 per second. Where the CPU time fits but the flips are late, `late_flips` grows by about 3 when the game starts and then stays, `late_frames` grows with the frames, and the log has one `late pacing` line and no `wait pacing` line after it.
  - Record: for each clock, the counters and the log lines of the pacing.

## 3. PC i3-6100T with any monitor

### The PC image (T28)

Each check is `needs-hardware`. Use the development image first.

- [ ] Do: boot the stick with UEFI, then with legacy BIOS (CSM), first on a flat panel. Expect: GRUB shows no menu, no console appears, the KMS build of the frontend starts and shows the game list. Record: the time from power-on to the first frame, and the connector name in the log of the frontend.
- [ ] Do: read the address from the router or from the console line `neptron: address eth0 A.B.C.D`, then `cargo xtask os ssh --host <address> -- uptime`. Expect: a login with the key; a password login is refused.
- [ ] Do: (M02) cold boot the PC with the cable plugged in, then `cat /run/neptron/network.toml` over SSH. Expect: the file holds the address of `eth0`, the one the router shows. Record: the file and the uptime of the lease (`grep -e udhcpc -e neptron-address /var/log/messages`).
- [ ] Do: (M02) boot the PC with no cable and wait 2 minutes; then plug the cable in, wait for the lease, and `cat /run/neptron/network.toml` over SSH. Expect: syslog shows `neptron-address: addresses: none` first, then the address, with no reboot. Record: the syslog lines of `neptron-address`.
- [ ] Do: (M02) on the router, give the PC a new fixed address (or end its lease), then pull and plug in the cable. Expect: `/run/neptron/network.toml` holds the new address; a renew with the same address leaves the inode of the file alone (`ls -i`). Record: the file before and after.
- [ ] Do: `cargo xtask os logs --host <address>`. Expect: the binary paths, the guard events and each `mode set` line with its read back. Record: the driver line; it must name the Intel HD Graphics 530 (iris), not llvmpipe.
- [ ] Do: `ls /dev/dri` and read `/sys/class/drm` over SSH. Expect: card0 is i915; no simpledrm and no vkms device.
- [ ] Do: change one line of the code, then `cargo xtask os push --host <address>`. Expect: the log shows the frontend from `/state/hot/bin`. Record: the push time.
- [ ] Do: cut the power during play, after an in-game save, then boot. Expect: the state partition mounts, the guard record and the save are kept. Record: the save file and its `.bak`.
- [ ] Do: `poweroff` over SSH during a game. Expect: the log ends with the clean stop line, and the save is on the disk.
- [ ] Do: run the self-test with the launcher held (see `os/README.md`). Record: the JSON report.

- [ ] `needs-hardware` **Each core plays.**
  - Do: boot the USB image on any monitor. Play one game of each core (FCEUmm, Snes9x, Genesis Plus GX) with a gamepad.
  - Expect: each game plays with sound, and the quick menu quits to the game list.
  - Record: the game of each core.

### The 15 kHz patch set at its GPL-2.0-only pin (M12)

The pin is commit `d85a343379f7f54155f69ef56dc544b666635894` of `D0023R/linux_kernel_15khz` (ADR 0018, OP-070). The build and QEMU checks of a pin change are in `os/README.md`, section "The 15 kHz kernel patch set". These checks need the PC and an image built with this pin.

- [ ] `needs-hardware` **The stick boots on the PC with UEFI and with BIOS.**
  - Do: write the image to a stick and boot the PC with UEFI, then with legacy BIOS (CSM).
  - Expect: the frontend starts each time. `cat /proc/version` shows 6.18.63, and `LICENCES.txt` on the boot partition names commit `d85a3433...`.
  - Record: the boot time and the SHA-256 of the image.
- [ ] `needs-hardware` **A 15 kHz mode on the real CRT with the patched kernel.** The PC with a 15 kHz display (section 4).
  - Do: `neptronctl selftest video --profile generic_15 --connector <name> --i-have-checked-the-monitor generic_15` on the image with the set, then the same on an image built with `--no-kernel-15khz`.
  - Expect: with the set, the driver accepts 240p and 288p with their low pixel clocks, the read back equals the request, and the tube syncs.
  - Record: both self-test reports, and which modes only the patched kernel accepts.

### Audio settings (C12)

- [ ] `needs-hardware` **The mixer is set at boot.**
  - Do: boot the PC image, then read `/proc/asound/card0/codec#0` over SSH (`grep -A3 -e "Node 0x02" -e "Node 0x14" -e "Node 0x21"`) and the log line of `S08neptron-mixer` on the serial console.
  - Expect: the DAC amplifiers are near the top of their range (0 dB), the output pins no longer have the mute bit `0x80`, and the console says the mixer of each card is at 0 dB and unmuted.
  - Record: the `Amp-Out vals` of nodes `0x02`, `0x03`, `0x14` and `0x21`, and `amixer -c 0 scontents`.
- [ ] `needs-hardware` **Sound on the analog output at volume 100.**
  - Do: plug headphones or speakers into the green jack, start a game with `audio.volume` at 100 and `audio.output` at `auto`.
  - Expect: clear sound at a normal level, with no distortion on loud passages.
  - Record: what you hear, and the log line `audio: driver alsa; devices ...; opened default`.
- [ ] `needs-hardware` **The volume steps and the ramp.**
  - Do: in a game, open the quick menu and move Volume from 100 to 0 in steps of 5, then back.
  - Expect: each step is a little quieter, there is no click at a step, 0 is silent, and the game keeps its pitch and speed.
  - Record: the lowest step you still hear, and any click.
- [ ] `needs-hardware` **The device list on the bench.**
  - Do: open Audio at home, then Audio output; read the log line `audio: driver alsa; devices ...; auto: ...; opened ...` with `cargo xtask os logs`.
  - Expect: `auto`, `Analog (HDA Intel PCH)`, then `HDMI/DP 1 (HDA Intel PCH)`, `HDMI/DP 2 (...)` and `HDMI/DP 3 (...)` with no detail; the log lists `hw:CARD=PCH,DEV=0`, `DEV=3`, `DEV=7`, `DEV=8`, and says that `auto` took the first analog output, because `/proc` does not show which HDMI/DP output the monitor uses.
  - Record: the log line and the options.
- [ ] `needs-hardware` **DisplayPort audio on the bench monitor.**
  - Do: with the DELL U2724D on DisplayPort, choose `HDMI/DP 1 (HDA Intel PCH)` in Audio output, then start a game. If there is no sound, try `HDMI/DP 2` and `3`.
  - Expect: the game's sound comes from the monitor's speakers or its headphone jack, at the level of the volume setting. After a boot with the monitor connected, `HDMI/DP 1` (`DEV=3`) is the one.
  - Record: which option plays on the monitor, and `cat /proc/asound/card0/pcm3p/sub0/hw_params` while the game runs.
- [ ] `needs-hardware` **`auto` with and without a monitor that takes audio.**
  - Do: set Audio output to `auto`. Start a game with the DisplayPort monitor, then with the CRT behind the DP-to-VGA bridge (no ELD), then with a monitor that has no speakers.
  - Expect: the CRT and the monitor with no audio give the analog output. With the DELL the bench has three HDMI PCMs and one monitor, so `auto` also gives the first analog output, and the log says why. A card with one HDMI PCM (an AMD or NVIDIA GPU) and a monitor with audio gives that output.
  - Record: the `auto:` part of the log line for each case, and `cat /proc/asound/card0/eld#*`.
- [ ] `needs-hardware` **A monitor connected after the boot.**
  - Do: boot with no monitor on DisplayPort, then connect the DELL, then quit to the game list and start a game.
  - Expect: the list of the new run shows the HDMI/DP outputs again (the list is read at each start of the frontend), the chosen output plays on the monitor, and nothing stops while the monitor is connected.
  - Record: the ELD files and the log line before and after the connect.
- [ ] `needs-hardware` **A USB sound card by name, and its fallback.**
  - Do: plug a USB sound card, restart the frontend, choose it in Audio output, then start a game. Quit, unplug the card, and start the game again.
  - Expect: with the card, the sound comes from it. Without it, the notice "The audio output ... is not connected. The default output plays.", sound on the analog output, and `global.toml` still holds the name of the USB card. Plugged in again, the next run uses it with no change in the menu.
  - Record: the notices, the log lines and `audio.output` in `global.toml`.
- [ ] `needs-hardware` **A USB card unplugged during a game.**
  - Do: play a game with the USB card chosen, and unplug it.
  - Expect: the game goes on with no sound, no freeze and no crash. The next run falls back to the default output with the notice.
  - Record: what happened, and `dmesg | tail -20`.
- [ ] `needs-hardware` **The mixer of a USB card.**
  - Do: with a USB card plugged in at boot, read `amixer -c 1 scontents`.
  - Expect: its playback control (often `PCM` or `Speaker`) is at 0 dB and unmuted. A control with another name keeps its own state: write its name down for `S08neptron-mixer`.
  - Record: the control names and their state.

### Input (T18)

- [ ] `needs-hardware` **A USB gamepad.**
  - Do: on the x86_64 image, connect a gamepad before boot, then another one after boot, then disconnect one.
  - Expect: both are found, the buttons follow the gamepad map, and a disconnect releases its buttons.
  - Record: the gamepad model.
- [ ] `needs-hardware` **Hotplug with no libudev.**
  - Do: plug a gamepad in after start.
  - Expect: SDL3 finds it (inotify on `/dev/input`).
  - Record: the time to the first input.
- [ ] `needs-hardware` **A USB keyboard.**
  - Do: plug a keyboard in, use it, unplug it.
  - Expect: it is found, grabbed (no key reaches a console), mapped, and released when unplugged.
  - Record: the keyboard model.
- [ ] `needs-hardware` **Blind recovery from each device.** Needs EDID.
  - Do: hold the recovery combination on a gamepad, then on a keyboard, with no picture. Do it also from power-on.
  - Expect: the safe default comes back.
  - Record: the device and the time.
- [ ] `needs-hardware` **Recovery held at boot on a keyboard encoder.** Needs EDID.
  - Do: hold the combination on a keyboard encoder from power-on.
  - Expect: recovery triggers.
  - Record: the encoder model.
- [ ] `needs-hardware` **Start hold on a panel with no Select.**
  - Do: hold Start on an arcade panel encoder with no Select button.
  - Expect: the quick menu opens.
  - Record: the encoder model.

### Input (M04)

Use USB pads that SDL3 knows (an Xbox pad, a PlayStation pad, an 8BitDo pad). Use a game for two players and a game for four players on a core that reads four ports, for example a four-player NES game with the Four Score option of the core on. Open `Settings > Input` to read the ports.

- [ ] `needs-hardware` **Two pads in a game.**
  - Do: plug two pads before boot. Read `Settings > Input`. Start a two-player game and play a moment with each pad.
  - Expect: the panel shows the two pads as players 1 and 2, in the order the game uses. Each pad drives its own player.
  - Record: the pad models and the game.
- [ ] `needs-hardware` **Four pads in a game.**
  - Do: plug four pads. Start a four-player game.
  - Expect: each pad drives its player, in the order of the panel.
  - Record: the pad models, the core, and the core option for four players.
- [ ] `needs-hardware` **An unplug and a replug in a game.**
  - Do: in a two-player game, unplug player 1. Plug a third pad of another model. Plug player 1 again.
  - Expect: the third pad is not player 1 (the panel in the quick menu shows player 1 as away, then the third pad as player 3). Player 1 drives player 1 again after the replug, and no button stays down at the unplug.
  - Record: the pad models.
- [ ] `needs-hardware` **Two pads of one model across a launch.**
  - Do: plug two pads of the same model. Mark which is player 1 in the panel. Start a game, check which pad drives player 1, quit, and read the panel again.
  - Expect: the same pad is player 1 in the menus, in the game and after the quit.
  - Record: the model, and whether the event node of each pad stayed the same (`ls -l /dev/input/by-id` over SSH on a development image).
- [ ] `needs-hardware` **A fifth pad.**
  - Do: plug five pads, start a four-player game, press buttons on the fifth pad, then open the quick menu with it.
  - Expect: the fifth pad does not drive a player. Select and Start on it open the quick menu, and it drives the menu.
  - Record: the pad models.
- [ ] `needs-hardware` **The order after an unplug in the menus.**
  - Do: with two pads in the game list, unplug player 1 and plug it in again.
  - Expect: the panel shows the other pad as player 1 and the replugged pad as player 2 at once.
  - Record: the time until the panel changes.

### Input: every USB gamepad (M04b)

Use a USB pad that SDL3 has no mapping for: a cheap "USB Joystick" or "Generic USB Gamepad" with a PS3-type button order, and, when there is one, an arcade stick that reports its lever as axes. `docs/spec/input-generic-pads.md` has the layout.

- [ ] `needs-hardware` **An unmapped pad on the PC.**
  - Do: on the x86_64 image, plug the pad in the game list. Use the D-pad and every button in the menus. Start a game and press each button.
  - Expect: the pad gets a port. The D-pad (or the lever) moves in the menus, Cross is back (B) and Circle is accept (A). In the game every button of the PS3 order drives its RetroPad control, and Select with Start opens the quick menu.
  - Record: the pad name, its USB id (`lsusb`), its counts of buttons, hats and axes (the `input:` line of the log), and each button that is in another place.
- [ ] `needs-hardware` **An unmapped pad on the Pi.**
  - Do: the same on the Raspberry Pi image.
  - Expect: the same result as on the PC.
  - Record: the same as on the PC.
- [ ] `needs-hardware` **The notice of the generic layout and its button.**
  - Do: plug the unmapped pad in the game list. Choose `Set up this controller` in the notice. Then start a game, quit, and plug the pad again into another USB port.
  - Expect: one notice with the name of the pad. The button opens `Settings > Input` with the focus on the row of the pad, which reads `<name> (generic layout)`. No notice at the launch and the quit. One notice again after the plug into another port.
  - Record: the notice text as shown.
- [ ] `needs-hardware` **A pad that sends no input.**
  - Do: plug a pad that enumerates but sends only neutral reports (the PS3 arcade stick of the bench straight into the PC). Use the keyboard or another pad in the game list for more than 20 seconds.
  - Expect: one notice: `<name> is connected but sends no input. Try another USB port or a USB hub.` No second notice for that plug, and none in a game.
  - Record: the pad, the port, and whether the pad works behind a hub.
- [ ] `needs-hardware` **Two pads of one unknown model across a launch.**
  - Do: plug two pads of one model that SDL3 has no mapping for. Mark which is player 1 on `Settings > Input`. Start a game, check which pad drives player 1, quit, and read the panel again.
  - Expect: each pad keeps its port in the menus, in the game and after the quit. Each pad gives its own notice of the generic layout once.
  - Record: the model, and the paths of the two pads (the `input:` lines of the log, or `ls -l /dev/input/by-path` over SSH on a development image).
- [ ] `needs-hardware` **A generic pad whose triggers rest at full travel.**
  - Do: plug a pad with no mapping whose axes 2 and 3 are analog triggers (many PS3-type pads report a released trigger at one end of the axis). Leave it untouched in the game list for a minute, then play a game.
  - Expect: the right stick of the generic layout reads the triggers. Record what a game sees with the triggers released: a right stick held to one side is a known limit of the generic layout, which a map of the player (M05) corrects. The menus must not scroll by themselves.
  - Record: the pad, and the value of axes 2 and 3 at rest and pressed.
- [ ] `needs-hardware` **The mapping files on the image.**
  - Do: read the log of a boot (`cargo xtask os logs`).
  - Expect: no `input:` line about `gamecontrollerdb.txt`: the NEPTRON file is in `/usr/share/neptron/input`, the community database in `/usr/share/sdl-gamecontrollerdb`, and neither has a damaged line.
  - Record: the lines about the mapping files, if any.
- [ ] `needs-hardware` **A pad that only the community database knows.**
  - Do: plug a pad that has a `platform:Linux` line in `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt` and no line in the built-in database of SDL 3.4 (compare the GUID of the `input:` line of the log with both). Use it in the game list and in a game.
  - Expect: no notice of the generic layout. Its row on `Settings > Input` has no ` (generic layout)` suffix. Its face buttons, D-pad, Start and Select are where the standard layout puts them.
  - Record: the pad, its GUID, and any button in the wrong place.
- [ ] `needs-hardware` **Pads that work today keep their layout with the community database.**
  - Do: on an image with the community database, plug an Xbox 360 pad, then an 8BitDo pad. With each, use every button and the D-pad in the game list and in a game, and compare with an image with no community database (or with notes of such an image).
  - Expect: each pad opens with a known layout and no notice of the generic layout. Every button and the D-pad work in the menus and in the game. The face buttons are in the same places as with no database: the same button accepts and the same button goes back in the menus, and each face button gives the same RetroPad button in the game.
  - Record: each pad, its GUID (the `input:` line of the log), and any button that moved.
- [ ] `needs-hardware` **The load time of the mapping files on the Pi 3.**
  - Do: build two development images of the Pi, one with `BR2_PACKAGE_SDL_GAMECONTROLLERDB` off. On each, read the time from the start of the frontend to its first frame in the log of a boot.
  - Expect: the image with the community file takes less than 50 ms more.
  - Record: both times.

### USB enumeration scheme

Every image has `usbcore.old_scheme_first=1` on the kernel command line (OP-112). With the default scheme, the Mad Catz PS3 FightStick TE (0738:8838) enumerates but sends only its neutral report until it loses power. Over SSH, `cat /sys/module/usbcore/parameters/old_scheme_first` shows `Y`.

Results recorded:

- 2026-11-18: on the PC, with the parameter set at run time, the stick gives input after a replug, and XInput pads work (the owner's report).

- [ ] `needs-hardware` **The stick at power-on on the PC.**
  - Do: plug the stick 0738:8838 into the PC, then power on. Use it in the game list and in a game.
  - Expect: the stick gives input after the boot, with no replug.
  - Record: the USB port, and the `input:` line of the log.
- [ ] `needs-hardware` **The stick replugged on the PC.**
  - Do: after the boot, unplug the stick and plug it in again, first into a port of the PC, then through a USB hub.
  - Expect: the stick gives input after each plug.
  - Record: the port, the hub model, and `dmesg | grep -i usb` for each plug.
- [ ] `needs-hardware` **The other USB devices of the bench on the PC.**
  - Do: plug in each USB device of the bench (keyboard, pads, the USB stick of the image, a hub), then read `dmesg`.
  - Expect: each device enumerates and works. No "device not accepting address" line in the kernel log.
  - Record: the devices, and any USB error line of `dmesg`.
- [ ] `needs-hardware` **The stick on the Raspberry Pi 3.**
  - Do: on the Pi image, plug the stick in before power-on, then replug it after the boot.
  - Expect: the stick gives input in both cases.
  - Record: the Pi model, and `cat /sys/module/usbcore/parameters/old_scheme_first`.
- [ ] `needs-hardware` **Every other controller of the owner.**
  - Do: on the PC and on the Pi, plug in each other controller of the owner, before power-on and after the boot.
  - Expect: each controller enumerates and gives input.
  - Record: each controller, its USB id (`lsusb`), and any controller that fails.

### Input remapping (M05)

`docs/spec/input-remapping.md` has the flow. Use a 240-line CRT picture (the 15 kHz safe start) for every check, so the texts of the mapping screen show at their real size. Use one pad that SDL3 has no mapping for and one known pad (an Xbox or 8BitDo pad). Keep a USB keyboard plugged.

- [ ] `needs-hardware` **An unknown pad mapped on the PC.**
  - Do: on the x86_64 image, plug the unknown pad in the game list. Choose `Set up this controller` in its notice. Answer each prompt on the pad; hold a button for 2 s on a control the pad lacks. Choose `Keep` with the D-pad and A of the new map.
  - Expect: the mapping screen names the pad and each control, and every text is whole and sharp at 240 lines. A press moves to the next step at its release; a hold skips. After `Keep` the pad drives the menus with its new map at once, and `Settings > Input` shows `<name> (your layout)`. In a game every mapped button drives its control.
  - Record: the pad, its counts (the `input:` lines of the log), and the content of `/state/config/input/pads.toml`.
- [ ] `needs-hardware` **An unknown pad mapped on the Pi.**
  - Do: the same on the Raspberry Pi image, on its composite output.
  - Expect: the same result as on the PC.
  - Record: the same as on the PC.
- [ ] `needs-hardware` **A known pad remapped.**
  - Do: open `Settings > Input`, the row of the known pad, `Map buttons`. Swap A and B in the answers, and keep.
  - Expect: the page shows `Yours`, and A and B are swapped in the menus and in a game. The gamepad is read by the raw indices of its joystick: every other button stays where its answer put it. `Reset to default` brings the standard layout back at once.
  - Record: the pad, and the binding of each control on its page before and after.
- [ ] `needs-hardware` **A trigger that rests at full travel.**
  - Do: map a pad whose triggers report their rest at one end of an axis. Press each trigger at its prompt.
  - Expect: L2 and R2 read `Axis N + (from its end)` on the page, a released trigger is never down, and no prompt moves on by itself.
  - Record: the pad and the axes of its triggers.
- [ ] `needs-hardware` **Cancel with no working button.**
  - Do: start `Map buttons`, then press Esc on the keyboard (back). Start it again and touch nothing for 15 s. Start it again, answer every prompt, and do not answer the review.
  - Expect: each time the old layout comes back and nothing is written. The countdown of the prompt and of the review counts down each second.
  - Record: what the screen showed at each end.
- [ ] `needs-hardware` **The layout after a reboot.**
  - Do: keep a layout, reboot the machine, and plug the pad into another USB port.
  - Expect: the pad has its layout at once, with no notice of the generic layout. Two pads of the model both have it.
  - Record: the pad and the ports.

### App and launcher (T27)

The boot of the image, the crash of the frontend and the blind recovery from each device are in "The PC image (T28)", "KMS backend (T23) and blank (C07)" and "Input (T18)".

- [ ] `needs-hardware` **SIGTERM to the launcher.**
  - Do: `kill -TERM` the launcher.
  - Expect: it forwards SIGTERM, and the frontend leaves a validated mode on the wire.
  - Record: what the monitor shows.
- [ ] `needs-hardware` **A connector change.** Needs EDID.
  - Do: change the connector in `global.toml`, and boot.
  - Expect: a new connector is a new place of the guard. With `display.monitor = "auto"` the EDID profile is the safe default, so no boot prompt shows. With a named preset that is not the last known good of the new place, the boot prompt shows (`docs/spec/display-guard.md`, rule 3).
  - Record: the setting, and what the prompt says.

### Flow defects (C10)

Each check is `needs-hardware`.

- [ ] Do: on a PC with a DVI-I output, boot once with a flat panel on DVI-D pins and once with a CRT through a DVI-I to VGA adapter, with `display.output` = `auto`. Expect: the log line of the open names the DVI-I connector, a panel is chosen as a digital output and the CRT as an analog one, and a CRT profile prefers the DVI-I connector only for the CRT. Record: the `subconnector` value and the first 32 bytes of the EDID (`/sys/class/drm/card0-DVI-I-1/edid`) for both.
- [ ] Do: in a game, hold Select and Start for 5 s. Expect: the quick menu opens after 0.5 s, then at 5 s a short 880 Hz tone and the safe picture, with the game paused under the menu and the notice "Safe mode: the picture was reset. ..." over it. The game did not pause itself or reset during the hold. Record: whether the tone sounds whole and whether the game sound is late after the resume.
- [ ] Do: boot once with the state partition damaged (for example `dd if=/dev/zero` over its first MiB on a copy of the stick) and once on a stick with no free space after the state partition. Expect: `/run/neptron/storage.toml` names the RAM folder and its reason, the log has `neptron: storage: ...`, and a Warning notice shows until OK. Record: the file and the notice text.
- [ ] Do: in a game, tap Select and Start together, then hold them for 2 s and let go. Expect: the tap opens nothing; the hold opens the quick menu 0.5 s after both are down, before the release. The tap opens no menu, and the game gets both presses, late by at most the tap. With the hold, the game gets no press from the button that completed the combination, and no press of either when the menu closes. Record: how late the menu feels after 0.5 s.

### Library rescan (C14)

Each check is `needs-hardware`.

- [ ] Do: with the game list on screen, copy a ROM into `roms/nes` of the exFAT share over the network (SMB or `scp`), then open `Settings > Library` and accept `Rescan games`. Expect: the row is disabled with `Scanning...`, then a notice `1 game added.`, and the game is in the list with no restart. Record: the time from the press to the notice for the library on the stick, and the game count.
- [ ] Do: start a game, copy a ROM into the share over the network while it runs, then quit the game. Expect: the list that comes back holds the new game, and the notice says `1 game added.`. Then `stat -c '%y' /share/roms/nes` before and after one more copy. Expect: the folder time changes with each new file on the exFAT driver of the image. Record: both times.
- [ ] Do: on a Windows PC and on a Mac, add a ROM to the share partition of the stick, then read the time of its folder on the device (`stat -c '%y'`) and compare it with the time before. Record: whether each system changed the folder time. A new boot reads every folder, so the game shows in both cases; the record says whether a check after a game would also find such a change.

### Network share (C13)

Each check is `needs-hardware`. `docs/spec/network-share.md` is the spec. Use the release image unless a check says otherwise, with the PC on the router by an Ethernet cable.

- [ ] Do: boot with no cable, then boot with a cable. Expect: the time from power-on to the first frame is the same within 1 s; with no cable, no line on the console waits for DHCP. Record: both times.
- [ ] Do: boot with no cable, wait for the game list, then plug in the cable. Expect: within 30 s the router lists `neptron` with an address, and `\\<address>\share` opens from another computer. Record: the time from the plug to the address.
- [ ] Do: from Windows 11 (24H2 or later, note the edition), open `\\NEPTRON\share`, then `\\neptron.local\share`, then `\\<address>\share`, with no change on Windows. Expect (from the spec): a refusal, because Windows 11 24H2 requires signing and refuses guest logons. Then turn on "Enable insecure guest logons" and turn off "Digitally sign communications (always)" (or `Set-SmbClientConfiguration -EnableInsecureGuestLogons $true -RequireSecuritySignature $false`) and try again. Expect: the share opens with `roms` and `bios`, with no password prompt. Record: the edition and build, the error text of the first try, which names resolve, and whether NEPTRON shows in the "Network" view (expected: no, there is no WS-Discovery).
- [ ] Do: on Windows 11 24H2 with no change on Windows and no stored credential for NEPTRON (`cmdkey /list`), open `\\NEPTRON\share` in Explorer. Expect (from the spec): an error and no prompt, because Windows sends its own account name, which becomes a guest. Record: the exact text of what Explorer shows, and whether a prompt for a name and password appears.
- [ ] Do: from Windows 11 24H2 with no change on Windows, `net use \\NEPTRON\share /user:neptron neptron`, then copy a file in; then remove the connection (`net use \\NEPTRON\share /delete`) and try "Map network drive" with "Connect using different credentials", and then a Windows credential for `NEPTRON` in Credential Manager with a plain Explorer open. Expect: each way opens the share read-write, and the copy succeeds: the system user `neptron` cannot write `/share`, so a successful copy proves that the forced user `nobody` is in use. Record: which ways work. From a Mac: `smbutil view //neptron:neptron@<address>` lists `share` and `IPC$`, and `smbutil view -g //<address>` (guest) lists them too. Record: the edition and build, and the outputs.
- [ ] Do: on the development image, `grep neptron /etc/passwd /etc/shadow`, then `ssh neptron@<address>`. Expect: the shell is `/bin/false`, the shadow field is `*`, and SSH refuses the login. Record: the lines and the SSH error.
- [ ] Do: the same from Windows 10 (Home or Pro, note which). Expect: the share opens as guest with no change. Record: the edition, build and result.
- [ ] Do: on macOS, Finder, Go, Connect to Server, `smb://neptron.local/share`, Connect As Guest; also look under "Network" in the sidebar. Expect: the share opens read-write; NEPTRON shows under "Network". Record: the macOS version, and whether a `._<name>` file appears next to a copied ROM.
- [ ] Do: on Linux (GNOME Files or KDE Dolphin), open "Other Locations" or "Network", then `smb://neptron.local/share` as anonymous. Expect: NEPTRON shows in the list and the share opens read-write. Record: the desktop and version.
- [ ] Do: copy a 2 GB file and a folder of 500 small files into `roms/nes` from Windows, then from macOS. Expect: the copies finish; the SHA-256 of the large file on the PC (`sha256sum` over SSH on the development image, or on another computer after the stick is moved) equals the source; during the copy a game runs with no audio dropout. Record: the copy speed in MB/s and any dropout.
- [ ] Do: start a copy of a 2 GB file and pull the Ethernet cable in the middle. Expect: the client reports an error; the PC keeps running; after the cable is back the share opens again and the copy can be done again; the cut file is on the share with a smaller size. Record: what the client said, and the size of the cut file.
- [ ] Do: on the development image, start a copy of a 2 GB file and run `poweroff` over SSH during the copy (`Settings > System > Shut down` runs the same steps). Expect: the console shows `neptron-share` stop with no "still has sockets" line, and the share partition unmounts with no error; on the next boot `dmesg | grep -i exfat` shows no dirty-volume message. Record: the shutdown lines of the console.
- [ ] Do: start a copy of a 2 GB file and cut the power during the copy. Expect: the next boot mounts the share partition; the files copied before stay; the cut file may be short or missing. Record: `dmesg | grep -i exfat`, and whether `/run/neptron/storage.toml` says `share = "partition"`.
- [ ] Do: on the development image, `touch /state/network-share-off`, reboot, and try to open the share; then `netstat -ltnu`. Expect: no share and no `neptron.local`; nothing listens on 445 or 5353. Remove the file and run `/etc/init.d/S45neptron-share restart`. Expect: the share opens at once. Record: the `netstat` output of both states.
- [ ] Do: on the release image, from another computer, `nmap -sT -sU -p 1-65535 <address>` (or a TCP scan of all ports and a UDP scan of 53, 67, 68, 137, 138, 5353). Expect: TCP 445 and UDP 5353 open, UDP 68 open or filtered while the PC holds a lease, nothing else. Record: the scan output.
- [ ] Do: on the release image, on a network where the router gives IPv6 addresses, from a second computer: `ping6` the PC (its link-local address `fe80::...%<interface>` and any global address the router lists for it), and open the share over IPv6 (`smbclient -L //[<IPv6 address>] -N`, or `\\<address>.ipv6-literal.net\share` from Windows). Then from the same computer, over IPv4 on the LAN, open `\\<IPv4 address>\share`. Expect: `ping6` and the SMB connection over IPv6 fail; over IPv4 on the LAN it works. On the PC (development image) `ls /proc/net/if_inet6` gives "No such file" and `iptables-legacy -S neptron-lan` ends with `-A neptron-lan -j DROP`. Record: the outputs, on the PC and on a Raspberry Pi 3.
- [ ] Do: from Windows, try `\\NEPTRON\state`, `\\NEPTRON\root`, and in the share make a shortcut or a link that points to `/state` (from Linux: `smbclient //neptron.local/share -N -c 'symlink ../.. up'`). Expect: no other share; the link is refused or cannot be followed. Record: the error texts.

### Network panel (M03)

Each check is `needs-hardware`. `docs/spec/network-panel.md` is the spec. Use the release image, with the machine on the router by an Ethernet cable unless a check says otherwise.

- [ ] Do: on the PC, boot with the cable in, wait for the game list, open `Settings > Network`. Expect: `Address eth0` with the address that the router lists for `neptron`, `Host name` `neptron`, `Network name` `neptron.local`, `Share` `On`, and the two share paths with that address; `\\<address>\share` from Windows and `smb://<address>/share` from a Mac open the share as the panel says. Record: the address, the router's address for `neptron`, and whether each path opened.
- [ ] Do: the same on the Raspberry Pi 3 (its USB Ethernet port appears after the boot). Expect: the port name of the Pi (`eth0`, or an `enx...` name) with the address the router lists; every row fits with no `...`. Record: the port name and the address.
- [ ] Do: boot with no cable, open `Settings > Network` and stay there. Expect: `Address` `None (no cable or no network)`. Plug in the cable. Expect: within about 2 s after the router gives the lease (the console line `neptron: address eth0 ...` on the development image), the row changes to `Address eth0` with the address, with no key press. Record: the time from the plug to the change on screen.
- [ ] Do: with the panel open and an address on screen, pull the cable, wait 30 s, then plug it in again. Expect: the address row goes back to `None (no cable or no network)` once `udhcpc` drops the lease (or stays, when the lease is kept: record which), and the address shows again after the replug. Nothing else on screen changes; the menu stays responsive. Record: what the row shows at each step and how long each change took.
- [ ] Do: on the PC, plug in a USB Ethernet adapter with a second cable after the boot, with the panel open. Expect: a second address row for the adapter's port, in the order of the port names; the share paths keep the first address. Record: both port names and addresses.
- [ ] Do: on the development image, `touch /state/network-share-off`, reboot, open the panel. Expect: `Share` `Off`, and no `Network name` row, no share path and no account rows. Remove the file and reboot. Expect: the rows are back. Record: what the panel shows in each state.
- [ ] Do: on a 15 kHz CRT (the PC with a 15 kHz display, or the Pi on a composite TV), open the panel. Expect: every row reads clearly, the dotted address and the `\\` and `/` of the paths included. Record: a photo of the screen.

### Reboot and shut down (C17)

Each check is `needs-hardware`. `docs/spec/ui-navigation.md`, "The System panel", is the spec, and `docs/flows/01-power-on.md`, "Shutdown", the order of the steps. Use the release image unless a check says otherwise, with the serial console on `ttyS0` when one is at hand.

- [ ] Do: in the game list, `Settings > System > Shut down`, accept, then right and accept. Expect: the box `Shutting down...` (no buttons) replaces the question at once and stays, in the menu mode, until the launcher stops, then the screen goes dark and the PC turns off (fans and power light off) within 15 s. The CRT gets no mode outside its limits at any time: the last mode is the validated menu mode, then no signal. Record: the time from the accept to the power-off, and the console lines `neptron-launch: the frontend asks for a power-off; /sbin/poweroff`, `neptron-stop: ...`, `neptron-share` and the unmount.
- [ ] Do: on the development image, change the volume in `Settings > Audio`, then `Settings > System > Shut down`. Boot again. Expect: the volume is kept; `dmesg` shows no ext4 journal recovery for the state partition (`e2fsck -p` reports it clean) and no exFAT dirty-volume message for the share partition. Record: the `neptron-storage:` and `e2fsck` lines of the next boot, and `dmesg | grep -i -e exfat -e ext4`.
- [ ] Do: `Settings > System > Reboot`, confirmed. Expect: the PC starts again and reaches the game list with the settings kept; the firmware POST mode is the only mode the monitor sees between the menu mode and the first NEPTRON picture. Record: the time from the accept to the first picture.
- [ ] Do: in a game, open the quick menu, `Settings > System`. Expect: both rows are dim with `Not during a game`, and accept does nothing. Quit the game, then shut down from the game list. Expect: the game's save RAM is on disk at the next boot. Record: the save file time.
- [ ] Do: open the confirm question and press accept at once. Expect: `Cancel` has the focus and the question closes with no shutdown. Record: nothing if it holds.
- [ ] Do: on a PC whose firmware has no ACPI power-off (if one is at hand), shut down. Expect: the file systems are unmounted and the kernel halts; the screen stays dark. Record: the last console line.

### The share partition on the first boot (C16)

Each check is `needs-hardware`. Before each one, read `/run/neptron/storage.toml`, `grep neptron-storage /var/log/messages` and `/state/log/storage.log` over SSH (or `cargo xtask os logs --host <address>`; on the release image, read `log/storage.log` of the state partition on a Linux computer), and keep them in the record. In QEMU, `cargo xtask os share-check` runs the case of a stick written again; a real stick on a real USB port can behave in another way.

- [ ] Do: write the new image with `dd` over a stick that has an older NEPTRON image with a share partition at the same start and files on it, then boot. Expect: `share = "partition"` on this first boot, with no notice; the syslog lines say that an old exFAT share starts at the planned sector, that the table holds partition 4 as planned, and that `/dev/sda4` is mounted at `/share`; no line says "formatting"; the files are there, and `\\NEPTRON\share` shows them. Record: the syslog lines, the exit status of sfdisk and of `blockdev --flushbufs` in them, every `sfdisk --dump try` and `addpart try` line, and `sfdisk --dump /dev/sda`.
- [ ] Do: the first boot of the new image on a new stick (larger than the image, never used). Expect: `share = "partition"`; the syslog lines say that partition 4 is made and formatted exFAT; a file put there from another computer is there after a second boot. Record: the syslog lines.
- [ ] Do: write the image to a stick that has other partitions after the end of the image (for example a stick that held another system, with a partition at the planned start of the share). Expect: no format of any space that holds a filesystem; when a partition 4 or a filesystem is in the way, `share = "ram"` with a reason that names it, and the notice shows the same reason. Record: `sfdisk --dump /dev/sda` before the boot, the syslog lines, and the notice text.
- [ ] Do: write the image to a new stick of 32 GB or more, boot it, and pull the power cord while the first boot formats the share (the console or the serial log shows `formatting it exFAT`). Boot again. Expect: no second format; either the share mounts (the format had ended), or `share = "ram"` with `share_first_format = "interrupted"` and the notice "The first format of the share partition did not finish...". Record: the storage lines of both boots, `/state/share-pending` or `/state/share-format-interrupted` after the first boot, and the time of `mkfs.exfat`.
- [ ] Do: on the stick of the first check, run `dmesg | grep -i -e sda -e 'sync cache' -e 'I/O error'` after the first boot. Record: the lines. They show whether the stick fails a cache flush, which makes sfdisk exit with an error after it wrote the table.

### The checks of the state and the share partition after a power cut

Each check is `needs-hardware`. Read `/run/neptron/storage.toml` and `/state/log/storage.log` after each boot, as for C16. The script tests run each branch against fakes; these checks prove the real tools on a real stick.

- [ ] Do: copy a large ROM set to `\\NEPTRON\share` from a PC and pull the power cord in the middle of the copy, then boot. Expect: the storage log has `fsck.exfat /dev/sda4: clean` or `fsck.exfat repaired /dev/sda4 (exit 1)`; `share = "partition"` with no `share_read_only`; the ROMs that were there before the copy are complete (compare a checksum). Record: the `fsck.exfat` lines and the time of the check (the `neptron-storage` lines before and after it in `/var/log/messages`).
- [ ] Do: on a stick of 32 GB or more with a full share (many small files), time the `fsck.exfat -n` of a boot. Expect: well under the 120 s limit (`SHARE_CHECK_SECONDS`). Record: the time and the file count.
- [ ] Do: damage the share on another computer so that `fsck.exfat -p` leaves errors (for example, overwrite part of the FAT with `dd`), then boot. Expect: `share_read_only = true` with the reason in `storage.toml`, the notice "The share partition is read-only...", the games still start, a copy of a new file over SMB fails, and nothing is formatted. Record: the notice, the `fsck.exfat` lines and its exit code.
- [ ] Do: on the development image, find the exit codes of the `fsck.exfat` of the image: `fsck.exfat -V`, then on a spare exFAT stick that is clean `fsck.exfat -n /dev/sdX1; echo $?` (expect 0), and after damage on a PC (for example a cleared bit of the allocation bitmap) `fsck.exfat -n` (expect 4) and `fsck.exfat -p` (expect 1). The storage script relies on these codes; they were not checked against the exfatprogs source. Record: the version and the three exit codes.
- [ ] Do: on the development image, `mount | grep /state`. Expect: `errors=remount-ro` in the options. Record: the line.
- [ ] Do: damage the state partition on a Linux computer so that `e2fsck -p` exits with 4 (for example, `debugfs -w -R "clri <inode of a file in /saves>"`), then boot. Expect: the storage log has `e2fsck -p ... left errors (exit 4); e2fsck -fy repairs it once`, the `e2fsck -fy:` lines, and the repair line; `state = "partition"`; the notice "The state partition had errors, and they were repaired..."; the other saves are there. Record: the lines and the notice.

### The format of the share partition that the user confirms (C18)

Each check is `needs-hardware`. Read `/run/neptron/storage.toml`, `grep neptron-storage /var/log/messages` and `/state/log/storage.log` before and after each boot, as for C16. In QEMU, `cargo xtask os share-check --case offer --case retype` runs these steps on a virtual disk; a real stick can behave in another way.

- [ ] Do: on a stick with the image, zero the head of the share partition on another computer (`dd if=/dev/zero of=/dev/sdX4 bs=1M count=16`; partition 4 stays in the table), then boot. Expect: `share = "ram"` and a notice that ends with `Settings > System can format it.`; `Settings > System` shows `Format share`; its question names the size of partition 4 in GB. Record: the notice text, the size in the question and `sfdisk --dump /dev/sdX` before the boot.
- [ ] Do: on a stick with the image and a share, run `wipefs -a /dev/sdX4` on another computer, then boot. `wipefs` clears the main exFAT boot sector only. Expect: `share = "ram"`, no `Format share` row, and the line `it holds an exFAT backup boot sector (sector 12), so it is a damaged share`. Record: the line.
- [ ] Do: confirm the format. Expect: the working box `Formatting at the next start...`, a reboot, then `share = "partition"`, the Info notice `The share partition is formatted.`, and no `share-format-request` on the state partition. Record: the storage lines of both boots, and the time from the confirm to the game list.
- [ ] Do: as above, but first change the type of partition 4 to 83 on another computer (`sfdisk --part-type /dev/sdX 4 83`). Expect: the offer, then after the confirm the line `sfdisk --part-type /dev/sda 4 7 exit 0` (or another exit with the table read back as type 7), the format and the mount. Record: the lines, and `sfdisk --part-type /dev/sdX 4` on another computer after the boot.
- [ ] Do: on a stick with a share that holds files, plant a request file by hand over SSH (`printf 'device=/dev/sda4\nstart=<start>\nsize=<size>\ntype=7\n' > /state/share-format-request; sync`) and reboot. Expect: the files stay; the Warning notice `The share partition was not formatted (...it holds a filesystem or a partition table).`; no request file after the boot. Record: the notice and the lines.
- [ ] Do: on a stick with an empty partition 4 of 32 GB or more, time the boot that offers the format. Record: the time of the storage script (the first and last `neptron-storage` lines of the boot in `/var/log/messages`), which includes the scan for an old share inside the partition.
- [ ] Do: on another computer, make an ext4 file system on partition 4 of a stick, then wipe only its first 1 MiB (`dd if=/dev/zero of=/dev/sdX4 bs=1M count=1`), and boot. Expect: `share = "ram"`, no `Format share` row, and the line `cannot be offered a format: it holds an ext2, ext3 or ext4 superblock at byte <n>`. Record: the line, and the block size of that file system (`dumpe2fs -h` before the wipe).
- [ ] Do: on a stick of 32 GB or more with an empty partition 4 at the planned start, time the boot that offers the format and the boot of the request. Record: the first and the last `neptron-storage` line of each boot in `/var/log/messages`, and the count of 1 MiB steps (4096 at most for a partition 4 at the planned start).
- [ ] Do: on a stick whose partition 4 starts 2 to 4 GiB after the planned start (made on another computer, empty), time the boot that offers the format. Expect: the offer, after up to 8192 steps. With a gap of more than 4 GiB, expect `it cannot be checked` and no offer. Record: the times and the lines.
- [ ] Do: write the image to an SD card or stick that held Raspberry Pi OS (or RetroPie, or Armbian) before, with no wipe, and boot. Expect: on the first boot the share partition is made, formatted and mounted (`share = "partition"`), with no offer and no notice. Record: the storage lines of the boot, and the old layout of the card (`sfdisk --dump` before the write).

## 4. PC with a 15 kHz display

These need a 15 kHz display (a SCART TV or an arcade monitor) on a PC output. They are not part of the MVP exit. The PC has no interlace (`intel_gen9`, `interlace = false`), so the interlaced checks are in section 6.

- [ ] `needs-hardware` **The self-test on a 15 kHz profile.**
  - Do: on a 15 kHz display, `neptronctl selftest video --profile generic_15 --connector <name> --i-have-checked-the-monitor generic_15`.
  - Expect: 240p, 288p and the super-width modes pass. On i915 the interlaced families are not tested, because the capability table has no interlace.
  - Record: the same fields as on the bench.
- [ ] `needs-hardware` **The spike with 480i on 15 kHz.** (T23, OP-030)
  - Do: `kms-spike ntsc --connector <name> --allow-low-pclock 6000000 --allow-interlace --modes 240p,480i --i-have-checked-the-monitor`. The spike has its own GPU values; `--allow-interlace` lets it commit 480i, which the product refuses on the PC.
  - Expect: the test-only commit of 480i with a linear dumb buffer passes or fails with a clear error.
  - Record: the result, and `flip_cadence`: i915 signals one flip per field (about 59.94 per second) or per frame (about 29.97).
- [ ] `needs-hardware` **A 15 kHz super-width mode on i915.** (T23)
  - Do: set 1920x240 at 60 Hz of `generic_15`, with the pixel clock floor of the capability table.
  - Expect: the test-only commit and the commit pass, the read back equals the request field by field, and the monitor syncs.
  - Record: the self-test report.
- [ ] `needs-hardware` **A Mega Drive game with a whole-kHz clock.** (C09)
  - Do: on a 15 kHz CRT, play a Mega Drive game at 320x224 on `generic_15` (horizontal total 411).
  - Expect: the picture is centred and has the right width.
  - Record: the position against a console or a test pattern.
- [ ] `needs-hardware` **Space Invaders on `arcade_15`.** (C02)
  - Do: play it on an arcade monitor.
  - Expect: the mode runs at 16.135 kHz, the picture locks, and the height is correct.
  - Record: the mode in the quick menu.
- [ ] `needs-hardware` **Donkey Kong unrotated on `arcade_15`.** (C02)
  - Do: play it on an arcade monitor.
  - Expect: the mode runs at 16.181 kHz and the picture locks.
  - Record: what the crop hides.
- [ ] `needs-hardware` **The quick menu over a 224-line game is soft and whole.** (L4, `docs/spec/ui-layout.md` section 6.4.1)
  - Do: on `generic_15`, play a SNES or Mega Drive game at 224 lines and open the quick menu, then a dialog of the quick menu.
  - Expect: every line of each glyph shows, a little soft (`SharpBilinear` on 240 canvas lines in 224 lines). No line of a glyph is missing, and the text is readable.
  - Record: a photo of the quick menu, and how soft the text looks at a normal distance.
- [ ] `needs-hardware` **A SCART TV.** (MVP exit, not required for the MVP)
  - Do: with a 15 kHz profile, show 288p at 50 Hz and 240p at 60 Hz. 576i needs interlace: it is checked on the Pi composite output (section 6, "TV standard on Composite-1 (P05)").
  - Expect: each mode is stable.
  - Record: the TV model and the result.

## 5. Raspberry Pi 3 on HDMI (P04)

Each check is `needs-hardware`. The image is `images/rpi-arm64-dev/neptron-rpi-arm64.img` (development variant, built with `--dev-key`) unless the check says release. The setup: a Pi 3 B+ or 3 B, a 2.5 A power supply, an HDMI flat panel, a USB gamepad, Ethernet with DHCP, and a 3.3 V USB serial adapter on GPIO 14 (TX), 15 (RX) and ground at 115200 baud. QEMU does not boot this image.

With an HDMI display that answers within 3 s (`COMPOSITE_FALLBACK_WAIT_NS`), the frontend drives HDMI through the panel path. With none, it opens `Composite-1` (section 6).

Results recorded: none on HDMI.

- [ ] Do: write the image to an SD card (`os/README.md`, "Write the image to an SD card"), connect the serial adapter and the HDMI panel, and power on. Expect: no rainbow splash; the serial console shows the whole kernel log from the start (the development command line has no `quiet`), with `Machine model: Raspberry Pi 3 Model B Plus`; the root is `/dev/mmcblk0p2` (EROFS) and init starts. Record: the serial log up to the first line of the frontend.
- [ ] Do: watch the HDMI panel from power on. Expect: no text and no logo at any time; the first picture is the NEPTRON menu through the panel path in the native mode, with no boot prompt on the first boot (a trusted flat panel, `docs/spec/display-guard.md`, "Trust"). Record: the connector and the mode in the log (`cargo xtask os logs --host <address>`), and a photo.
- [ ] Do: the first boot on a new card. Expect: `share = "partition"` in `/run/neptron/storage.toml`; the storage log says that partition 4 is made, formatted exFAT and mounted as `/dev/mmcblk0p4`; after a second boot the share is mounted and not formatted. Record: `/state/log/storage.log`, `sfdisk --dump /dev/mmcblk0`, and the time of `mkfs.exfat` on the card.
- [ ] Do: (M02) 10 cold boots with the Ethernet cable plugged in: power off by unplugging the supply, wait 10 s, power on. For at least 2 of them, power on the Pi and the router at the same time, so the lease comes late. After each boot, `cat /run/neptron/network.toml` over SSH. Expect: the file holds the address of `eth0` at each boot, also when its USB chip appears after `S40neptron-network` ran (`mdev` and `net-hotplug` start its client) and when the lease came after 60 s; `ps` shows one `udhcpc` for `eth0`, never two; on the development image the serial console shows `neptron: address eth0 A.B.C.D`. Record: for each boot, the uptime of the lease (`grep -e udhcpc -e neptron-address /var/log/messages`), the line of `S40neptron-network`, and the file.
- [ ] Do: (M02) pull the Ethernet cable of the Pi for 30 s and plug it in again; then end the lease on the router, or give the Pi a new address, and pull and plug the cable again. Read `cat /run/neptron/network.toml` over SSH after each step. Expect: the file holds the old address while the lease runs, then the new address after the new lease. Record: the file and the syslog lines of `neptron-address` and `udhcpc`.
- [ ] Do: on a cold boot of the Pi, note when `eth0` gets its address (`grep -e udhcpc -e neptron-network /var/log/messages`), then, from another computer, open `\\NEPTRON\share` and `ping neptron.local`. Expect: both answer, also when `eth0` came up after `S40neptron-network` ran: the SMB server opens a listener on a port that comes up later, and `mdnsd` follows the new address. Record: the uptime of the address, and the time until the share and the name answer.
- [ ] Do: plug a USB Ethernet adapter with a cable into the Pi, wait for its address, unplug the adapter, wait 10 s, and plug it in again. Expect: syslog shows `the DHCP client of ethN stops` at the removal and a new `DHCP on ethN in the background` at the add; `ps` shows one `udhcpc` for the port; the port gets an address. Record: the syslog lines.
- [ ] Do: from another computer, open `\\NEPTRON\share` (or `smb://neptron.local/share`) and copy an NES ROM to `roms/nes`, then rescan the library. Expect: the game shows and runs with the gamepad. Record: the copy speed and the frame time in the log.
- [ ] Do: `cargo xtask os ssh --host <address> -- uptime`, then `cargo xtask os push --arch rpi-arm64 --host <address>`. Expect: SSH with the key only; the push builds with the toolchain of the `rpi-arm64-dev` output, and the log shows the binaries of `/state/hot/bin` after the restart. Record: the time of the push.
- [ ] Do: read the log of the frontend after the first start. Expect: one card, the `vc4` driver, the connectors `HDMI-A-1` and `Composite-1`; Mesa reports the vc4 driver for GLES 2. Record: the GL renderer string in the log.
- [ ] Do: play a game on the HDMI panel and listen on the 3.5 mm jack with headphones, then on the speakers of the HDMI panel if it has them. Expect: sound on the jack (the `snd_bcm2835` card "Headphones") at the volume of the frontend; the mixer script sets it to 0 dB. Record: the card names in `/proc/asound/cards` and the output of `amixer -c 0 scontrols`.
- [ ] Do: during a game on the HDMI panel, run `grep -i cma /proc/meminfo` over SSH, then once more after 10 minutes of play and after a return to the menu. Expect: `CmaTotal` is the default CMA area of the `vc4-kms-v3d` overlay, and `CmaFree` stays well above zero and does not shrink from one reading to the next; no `cma: cma_alloc` failure in `dmesg`. Record: the three readings of `CmaTotal` and `CmaFree`, and the HDMI mode.
- [ ] Do: Settings > System > Shut down. Expect: the picture goes off, the share and the state partition unmount clean (no `e2fsck` repair at the next boot), and the board halts: the green ACT LED stops, the red power LED stays on, and the board still draws power. Only a power cycle (unplug and plug in) starts it again. Record: the serial log of the shutdown and the time to the halt.
- [ ] Do: Settings > System > Reboot. Expect: the board restarts through the watchdog and boots again. Record: the serial log.
- [ ] Do: the release image (`images/rpi-arm64/neptron-rpi-arm64.img`) on the same card. Expect: no SSH (port 22 closed), no serial console (no `enable_uart`), the network share on, the menu on HDMI. Record: a port scan of the Pi.
- [ ] Do: after 30 minutes of play, run `dmesg | grep -i voltage` over SSH (the image has no `vcgencmd`). Expect: no line. Record: the power supply used.

### Display guard on HDMI (M01)

Each check is `needs-hardware`.

- [ ] Do: keep a monitor preset on an HDMI TV, then power on with the TV off. Wait until the boot falls back to composite, then reboot with the TV on. Expect: the HDMI boot starts on the kept preset with no prompt: the composite boot did not delete the HDMI record. Record: `/state/video/guard.toml` after each boot.

## 6. Raspberry Pi 3 on composite

The setup: a Pi 3 with the development image, the frontend from `cargo xtask os push --arch rpi-arm64` where a check says so, a 15 kHz TV on composite through a TRRS cable with video on the sleeve, and SSH. `docs/spec/raspberry-pi.md`, section 6, holds the full checklist of the first bench session (task P09); the checks below are the ones that tasks added. The composite output is the only interlaced output of the images (`pi_vc4_composite`, `interlace = true`).

Results recorded (a Pi 3 B on a Samsung TV, 2026-11-14):

- The first composite picture: the picture locks in NTSC 240p, the menu is sharp, and the grid of the 240p test suite fills the tube with correct geometry. The game picture reaches the edges of the visible area. The menu is 640 of 720 samples wide by design, so the tube shows a black band at each side of it (OP-097, OP-102).
- `dtparam=audio=on` after the overlay line gave no jack card; the order is fixed (commit `fd713d9`, OP-102).
- With device periods of 512 frames the analog output ran short 14 times a second (838 ALSA underruns in 60 s); with 1024 frames none in 60 s (commit `dff9f44`).
- NES, SNES and Mega Drive games hold 60.054 frames per second with no missed field, with the `performance` governor; an SNES game held it for 155 s with no audio underrun (commit `d6f51ea`, OP-103).

### TV standard on Composite-1 (P05)

Each check is `needs-hardware`. The setup: a Pi 3 B+ with the development image, a 15 kHz TV on composite through a TRRS cable with video on the sleeve, and SSH. Stop the frontend and the launcher first (`touch /run/neptron/hold && kill $(pidof neptron-launch)`). Run `kms-check` and `modetest` as root over SSH. The four table modes are 720x240p and 720x480i (525 lines) and 720x288p and 720x576i (625 lines), all at 13 500 kHz.

- [ ] Do: `modetest -M vc4 -c`. Expect: the connector `Composite-1` has the property `TV mode` with its 8 values (`NTSC`, `NTSC-443`, `NTSC-J`, `PAL`, `PAL-M`, `PAL-N`, `SECAM`, `Mono`), the legacy `mode` property, and the four margins `left margin`, `right margin`, `top margin` and `bottom margin`. Record: the property lines of `Composite-1`.
- [ ] Do: commit each table mode with each standard of its line system: 240p and 480i with `ntsc`, `ntsc_j`, `ntsc_443`, `pal_60`, `pal_m` and `mono_60`; 288p and 576i with `pal`, `pal_n`, `secam` and `mono_50`. That is 20 commits. Expect: each commit passes, the mode and the `TV mode` value read back as asked, and the TV shows a locked, stable picture (in colour where the TV decodes the standard). Record: for each commit, the read back and what the TV shows.
- [ ] Do: with 240p on `ntsc`, change only the standard to `ntsc_j`. Expect: the commit is a modeset (the picture goes off for a moment), and `TV mode` reads back `NTSC-J`. Record: the log of the commit.
- [ ] Do: after any modeset on `Composite-1`, read the four margin properties. Expect: all four read 0. Record: the values.
- [ ] Do: try to commit 288p with the value `NTSC`. Expect: the commit is refused, nothing changes on the wire, and the earlier mode and standard stay on the TV and read back unchanged. Record: the error and the read back.
- [ ] Do: with an HDMI display connected, start the frontend with `display.output = "Composite-1"`. Expect: only composite is driven: `modetest -M vc4 -c` shows `HDMI-A-1` with no CRTC, and the HDMI display gets no signal. Record: the CRTC of each connector.
- [ ] Do: start the frontend on HDMI with the margins forced to 0 by the backend. Expect: the HDMI picture is correct, not cut and not shifted, and its margins read 0. Record: the mode and a photo.

### Display guard on composite (M01)

Each check is `needs-hardware`.

- [ ] Do: first boot of a new image on the composite TV, then wait 30 s. Expect: NTSC 240p with no prompt and no "Display change undone" notice. Record: the log lines that start with `guard:`.
- [ ] Do: start a game, quit it, and do it again 3 times. Expect: no prompt and no notice at any return to the game list. Record: the `guard:` log lines.
- [ ] Do: 5 power cuts, each 10 s or more after the menu shows. Expect: no safe mode at any boot; `failed_boots = 0` and `run_open = false` in `/state/video/guard.toml`. Record: the file after the last boot.
- [ ] Do: 2 power cuts, each within 2 s of the first picture, then power on. Expect: the third boot starts in safe mode with one Warning notice; a relaunch (a game, then the list) shows the notice no more. Then Settings, Display, "Keep this picture". Expect: safe mode ends, and the next boot has none. Record: the notices and the file.
- [ ] Do: change the TV standard to PAL, press Right, then Accept at once. Expect: PAL is kept, with no "undone" notice. Record: the notices.
- [ ] Do: the blind recovery. Expect: NTSC with the tone. Record: the time to the picture.

### The renderer, the frame budget and the audio (T17, T19, C07, P11)

Each check is `needs-hardware`. The setup: the Pi 3 with the development image, the frontend from `cargo xtask os push --arch rpi-arm64`, a 15 kHz TV on composite, SSH. The frame budget comes from ftrace (`/sys/kernel/debug/tracing`): `drm_vblank_event_delivered` (the flip event), `vc4_submit_cl_ioctl` (the GPU submit) and `vc4_rcl_end_irq` (the end of the render). Disable every event after the run.

- [x] **The renderer against the reference on vc4, and its precision.** (T19, P11) 2026-11-14, Pi 3 B, VC4 V3D 2.1, Mesa 26.1.8, commit `e03f6f7`: `gles_compare` passes every scene through the framebuffer object and through the window path, except `nearest_at_the_axis_product_limit` and `sharp_bilinear_at_the_axis_product_limit` (their 4096-pixel output is above the 2048 limit of vc4); highp passes (at least 23 bits).
- [ ] `needs-hardware` **The picture on the TV.**
  - Do: the 240p test suite on the SNES (sharp bilinear, 256 to 640 columns) and Sonic on the Mega Drive (nearest), on the TV.
  - Expect: the top line of the game is at the top of the picture, not one line lower; each source column edge shows at most one blended column; the colours of the colour bars match the suite.
  - Record: a photo of the grid and of the colour bars.
- [ ] `needs-hardware` **The frame budget of each system.**
  - Do: 3 minutes of play with scrolling scenes in the SNES 240p suite and in Super Mario World, 2 minutes of NES (nova) and of Sonic, with the trace on.
  - Expect: 60.054 frames per second over each 60 s, `fields_missed` 0 in steady play, `audio_underruns` steady, no `underrun occurred` line of ALSA in the log, and at least 2 ms left in the 16.65 ms field at the worst frame.
  - Record: the frames per second, `fields_missed`, the CPU time from the flip event to the submit and the GPU time (mean and worst), and the margin at the worst frame.
  - Result in part (2026-11-14, Pi 3 B, the development build of `os push`, no LTO): NES, SNES and Mega Drive games hold 60.054 frames per second with no missed field. Super Mario World leaves 1.66 ms at its worst frame, under the 2 ms of the expectation. The release image is not measured (OP-103).
- [x] **The CPU clock in play.** 2026-11-14, Pi 3 B, commit `d6f51ea`: with `ondemand` the clock fell to 600 MHz in a light scene, and an SNES game lost 24 to 30 fields in a burst; with `performance`, the governor of the image, the same game held 60.054 frames per second for 155 s with no missed field and no audio underrun.
- [ ] `needs-hardware` **PAL.**
  - Do: set `display.tv` to `pal` through the app (`neptronctl settings set display.tv global pal`, then Keep), run the SNES 240p suite for 60 s with the trace, then set the old value back and Keep.
  - Expect: 288p at 50.08 Hz, 50.08 frames per second, no missed field in steady play, then 240p at 60.054 Hz again.
  - Record: the video state of both modes and the frame budget.
- [x] **A dialog over a running game.** (P12) 2026-11-14, Pi 3 B, the development build of `os push`: no missed field while a dialog shows, in 36 NTSC dialogs over the SNES suite and in each Sonic run. A modeset adds 1 to 2 audio underruns and at most 1 missed field (OP-103 point 2). The quick menu adds no missed field, and the game list holds 60.06 frames per second. A dialog frame: GPU submit 6.2 ms and render end 8.35 ms after the flip event, GPU time 2.2 ms. A game frame alone: 6.45 ms, 9.87 ms and 3.41 ms (OP-103 point 1).
- [ ] `needs-hardware` **The vblank count and the flip cadence of an interlaced mode.** (OP-030)
  - Do: run a game in 480i on composite with the trace of `drm_vblank_event_delivered`, and compare the `seq` of the flip events with their times. Count the flip events per second.
  - Expect: to know whether the DRM vblank sequence of vc4 counts fields or frames in 480i, and whether vc4 signals one flip per field (about 59.94 per second) or per frame (about 29.97). The session counts a missed field when two flips are more than 2 sequence numbers apart in an interlaced mode on a driver that flips once per frame (`flip_per_field` false). If vc4 counts frames, two flips one frame apart differ by 1 and a late frame by 2, so the subtraction of 2 hides each late frame.
  - Record: the sequence step and the time step of steady flips, the flips per second, and set `flip_per_field` of `pi_vc4_composite` in `data/hardware/matrix.toml` from it.
- [ ] `needs-hardware` **The field order of a 480i mode in late pacing.** The fake display cannot show a swapped field, so only this check proves the field order in late pacing.
  - Do: on the composite TV, show a 480i mode with a game that has fine horizontal lines (a test pattern or a menu with thin text). Make the core slow on purpose: limit the CPU clock with `scaling_max_freq` until `late_frames` grows in `neptronctl stats`. Watch for 60 s, then read the stats again.
  - Expect: no swapped fields: no line pairs out of order, no picture that jumps up and down by one line, no comb on thin lines that was not there at full clock. `frames_presented` falls in proportion to the overrun and not to half, and the sound plays with no crackle.
  - Record: the counters, the mode, the clock, and a close photo of the thin lines at full clock and in late pacing.
- [ ] `needs-hardware` **Listening.**
  - Do: play each game above with headphones on the analog output.
  - Expect: no crackle and no gap in steady play.
  - Record: what is heard.
- [ ] `needs-hardware` **Blank on the Pi 3.** (C07)
  - Do: the three blank checks of the KMS section (blank, set a mode after a blank, blank before the first mode) on vc4.
  - Expect: as on i915.
  - Record: the results.
- [ ] `needs-hardware` **Audio latency on the Pi 3.** (T17)
  - Do: play a click on a frame, and measure the time from the flip to the sound.
  - Expect: about 64 ms.
  - Record: the measured time.
- [ ] `needs-hardware` **Resampler cost on the Pi 3.** (T17)
  - Do: a release build, and the CPU share of `AudioPipeline::end_frame` with Sinc.
  - Expect: 3 to 4 % of one core.
  - Record: the share. Use Polynomial if it is over budget.
- [ ] `needs-hardware` **A heavy core on the Pi 3 in late pacing.**
  - Do: play a game whose core cannot keep 60 frames per second on the Pi 3 (for example a CPS2 or Neo Geo game on FinalBurn Neo) for 60 s, and read `neptronctl stats` before and after.
  - Expect: `frames_presented` per second is the field rate over the overrun, not half of it; `frames_presented` plus `fields_missed` per second is the field rate; `frames_dropped` does not change; `audio_underruns` grows by at most 1; the sound slows with the game and does not crackle. A game below half speed plays no sound. The loop uses no code for vc4.
  - Record: the counters, the game, and what the sound is like.

### `neptron.toml` on the boot partition (P07)

Each check is `needs-hardware`. The setup: a Pi 3 with the image, a 15 kHz TV on composite, a PC that reads FAT32 cards, and SSH for the log.

- [ ] Do: boot a new image once with the file unchanged. Expect: `/state/config/global.toml` gets no `[display]` value from the file; the log says `neptron.toml has changed: no setting to copy`. Record: the log line.
- [ ] Do: write the image to the SD card. On the PC, open `neptron.toml` at the root of the `NEPTRONBOOT` partition, uncomment the `tv` line and set it to `"pal"`, save, and eject the card. Boot the Pi. Expect: the first picture is PAL 288p, with no prompt and no notice; `/state/config/global.toml` holds `tv = "pal"`; `mount` shows `/dev/mmcblk0p1 on /boot type vfat (ro,...)`. Record: the log lines that start with `neptron: neptron.toml` and the `mount` line.
- [ ] Do: in Settings, Display, set the TV standard to NTSC and keep it, then reboot. Expect: NTSC stays; the log says `neptron.toml has not changed since its import`. Record: the log line.
- [ ] Do: on the PC, add `wifi = "home"` under `[display]`, uncomment the `pi_dac` line and set it to `"vga666"`, boot. Expect: two notices, one that names `display.wifi` with its line and one with "GPIO DACs are not supported by this image; the value is ignored"; no GPIO pin is driven. Record: the notices.
- [ ] Do: on the PC, remove the `]` of `[display]`, boot twice. Expect: at each boot a notice "neptron.toml does not parse (...). No setting changes.", and the settings stay as they were. Record: the notice.
- [ ] Do: with an HDMI display connected, uncomment the `output` line in the file on the PC, set it to `"Composite-1"`, and boot. Expect: the first boot after the edit already drives composite, and the HDMI display gets no signal. Record: `modetest -M vc4 -c`.
- [ ] Do: on the PC image, uncomment the `monitor` line in the file on a stick, set it to `"generic_15"`, and boot a PC with a 15 kHz CRT on VGA. Expect: the boot uses `generic_15`. With no EDID or a PC-range EDID it opens no boot prompt, because `generic_15` is then its own safe default (`docs/spec/display-guard.md`, "A named preset bounds the safe default"). Record: the `guard:` log lines.

### The Raspberry Pi leftovers (M10)

- [ ] `needs-hardware` **The kind of each sound card on the Pi 3.** A Pi 3 B with the development image, on composite, then on HDMI.
  - Do: `cat /proc/asound/cards /proc/asound/pcm`, then open Audio, then Audio output, and read the log line `audio: driver alsa; devices ...`. Do it with `dtparam=audio=on`, then again with that line removed from `config.txt` (no jack card).
  - Expect: the cards are `Headphones` (driver `bcm2835_headpho`) and `vc4hdmi` (driver `vc4-hdmi`). The list shows `Analog (bcm2835 Headphones)` and `HDMI/DP 1 (vc4-hdmi)`, and `auto` takes the jack. With no jack card, `auto` opens the ALSA default device, which is then the `vc4hdmi` card. On composite with no jack card, expect that open to fail and the output to be "no sound": this is correct, because no audio output exists. On HDMI with no jack card, expect sound from the TV through the default device (alsa-lib's `cards/vc4-hdmi.conf` makes it convert to IEC958).
  - Record: the two files, the list, the log line of each run, and whether `/usr/share/alsa/cards/vc4-hdmi.conf` is in the image (`ls /usr/share/alsa/cards`).
- [ ] `needs-hardware` **The audio cover of a TV standard change on the Pi 3.** A Pi 3 B with the development image, on composite, headphones on the jack.
  - Do: run the SNES 240p suite on NTSC with a tone or music. Read `neptronctl stats`. Change `display.tv` to `pal` with Keep, then back to `auto` with Keep, 8 times (`neptronctl settings set display.tv global pal`, then `auto`). Read the stats again. Listen at each change.
  - Expect: `audio_underruns` does not grow at a change (it grew by 1 to 2 on 2026-11-14, before the 100 ms mute cover). No click, no gap of old sound, and no echo of the sound before the change: the sound fades out, stays silent through the change, and fades in. The delay of the sound after a change is the same as before it.
  - Record: `audio_underruns` and `fields_missed` before and after, and what is heard.

### The share partition on an SD card (C18)

- [ ] Do: on a Raspberry Pi 3 with an SD card, an empty partition 4 with no marker, time the first boot that offers the format and the second boot. Expect: the second boot scans nothing (no new `neptron-storage` lines of the scan; `/state/cache/share-scan` holds the key and `result=none`). Record: the time of the storage script at each boot (the first and the last `neptron-storage` line in `/var/log/messages`).

## 7. Any machine

- [ ] `needs-hardware` **Run rate in the window.**
  - Do: run a game in the window build on a real desktop.
  - Expect: the measured run rate is near the display rate.
  - Record: the measured run rate.
- [ ] `needs-hardware` **The tone is heard.**
  - Do: `cargo xtask test -p neptron-audio --features sdl3 --run-ignored only`.
  - Expect: a clean 440 Hz tone for one second, with no clicks.
  - Record: what you hear.
- [ ] `needs-hardware` **No sound device.**
  - Do: boot a box with no sound device.
  - Expect: it boots and shows the "No sound" notice.
  - Record: the notice text.
- [ ] `needs-hardware` **The window build.**
  - Do: run the window build on macOS.
  - Expect: the UI and a game show.
  - Record: the macOS version.

## Release images (M14)

The images of a GitHub Release, not the development variant. `os/README.md`, "Check a release", reads the files with no hardware.

- [ ] `needs-hardware` **The PC release image on Intel graphics.**
  - Do: write `neptron-pc-x86_64-<version>.img.xz` of the release to a USB stick and boot the i3-6100T with a 31 kHz monitor or a flat panel.
  - Expect: the NEPTRON menu shows; `LICENCES.txt` on the boot partition names the release, the commit of the tag and `Variant: release`; no SSH port answers.
  - Record: the release, the first lines of `LICENCES.txt`, a photo of the menu.
- [ ] `needs-hardware` **The Pi release image.**
  - Do: write `neptron-rpi-arm64-<version>.img.xz` to an SD card and boot a Pi 3 on composite.
  - Expect: the menu shows on the TV; `LICENCES.txt` names the release and `Variant: release`.
  - Record: the release and a photo of the menu.
- [ ] `needs-hardware` **The time of the firmware picture on a PC.**
  - Do: with a 31 kHz monitor, time from power-on until the NEPTRON menu replaces the picture of the firmware and of GRUB, three times.
  - Expect: a time of some seconds; the README warns that a 15 kHz-only monitor gets a 31 kHz signal for that time.
  - Record: the three times, and whether fast boot of the firmware was on.
- [ ] `needs-hardware` **A PC with AMD graphics.**
  - Do: boot the PC release image on a PC with an AMD graphics card.
  - Expect: release 0.1 does not promise a picture (the image has no AMD firmware); write down what happens.
  - Record: the card, the picture or its absence, and `dmesg | grep -i amdgpu` if a console answers.

## Video policy of the arcade rule (M08b)

These checks need a CRT. The policy tests prove each mode in emulation; only a tube shows the picture. An interlaced result needs the Pi composite output: on a PC tube, record the mode that the policy gives instead.

- [ ] `needs-hardware` **A turned game keeps its shape on a CRT.** Needs the rotation of M08a.
  - Do: start a vertical game (DoDonPachi, 240x320 upright) on the 31 kHz PC CRT, and on a multi-sync tube if one is at hand.
  - Expect: the picture stands upright, 3:4, with black bars left and right (pillarboxed): about 56 % of the screen width at full height on a multi-sync tube (2x, 427x640p), and the same shape in the bordered 1:1 mode on 31 kHz. A round object of the game looks round, not wide.
  - Record: a photo, the mode in the status, and the width of the picture against the height of the screen.
- [ ] `needs-hardware` **A mode moved onto a fixed line rate.** Needs a 25 kHz monitor (`arcade_25` or a 15, 25 and 31 kHz tube with its preset) on the PC.
  - Do: start the NES (60.099 Hz) and a 480x640 source on the `arcade_25` preset.
  - Expect: the NES shows 1:1 in 384 lines at 60.000 Hz. The 480x640 source shows interlaced 1:1 in 768i at about 24.93 kHz only with a GPU that has interlace, which no PC GPU of the matrix has: record the mode it gets. The monitor locks with no roll and no bend; the picture is centred.
  - Record: the line rate and field rate in the status, and whether the tube held sync.
- [ ] `needs-hardware` **Double refresh below every range.**
  - Do: start a Mega Drive PAL game (49.701 Hz) on a PC CRT whose EDID starts at 50 Hz; then Little Robin (30 Hz) on a 15 kHz tube.
  - Expect: the Mega Drive game shows at 99.40 Hz, each frame twice, at its own speed (the music has its pitch); Little Robin shows at 60 Hz, each frame twice, with the double-refresh notice.
  - Record: the field rate in the status, the notice, and whether the motion is smooth.
- [ ] `needs-hardware` **A tall console game is fitted into 480i on a composite TV.** The Pi 3 composite output.
  - Do: on the `ntsc` TV standard, start a PAL source of 576 lines (a PAL 576i game, or a PAL Amiga or PlayStation game in interlaced mode).
  - Expect: the game shows in 720x480i at 59.940 Hz, scaled to 480 lines with sharp bilinear, at the speed of NTSC (the speed notice shows). The lines are steady, with no line pairs missing and no roll.
  - Record: the mode and the notice in the status, and a photo of a fine horizontal detail.
- [ ] `needs-hardware` **A handheld keeps its borders on 15 kHz.**
  - Do: start a Game Boy game (160x144, 59.727 Hz) on a 15 kHz tube with the `generic_15` preset.
  - Expect: the game shows 1:1 in 299x224p at 59.728 Hz, centred, with black borders on all four sides, at its own shape (10:9). The tube locks with no roll.
  - Record: the mode in the status, a photo, and the width and height of the picture.
- [ ] `needs-hardware` **Scanlines on a flat panel draw at a whole multiple.** Needs a 1080p panel.
  - Do: start a SNES game on a 1080p panel with `display.panel_scale = auto`. Turn the scanlines on (strength 50 %), then off.
  - Expect: with scanlines on, the game shows at 4x (1194x896, centred), and every dark line has the same thickness across the height. With scanlines off, the game fills the height (1440x1080).
  - Record: a close photo of the scanlines at the top, the middle and the bottom of the picture.

## The quick menu hold on an arcade stick

The hotkey tests prove the 0.5 s hold and what the game gets in emulation, with a simulated clock. Only a real stick and a real arcade game show how a player presses the coin and Start.

- [ ] `needs-hardware` **Quick coin then Start presses do not open the quick menu; a hold of both for 0.5 s opens it.** An arcade stick with Select as the coin, FinalBurn Neo.
  - Do: start an arcade game. Press the coin (Select), then Start, in a quick sequence, ten times, also with the two overlapping for a moment. Then hold both for half a second.
  - Expect: the quick sequences never open the quick menu, and each gives the game one credit and one start. The hold of both for half a second opens the quick menu, before the release. When the menu closes, the game gets no extra credit and no extra start.
  - Record: the stick, the game, the credits counted, and whether any quick sequence opened the menu.

## Arcade set check (M07c)

Run each check twice: once on the Raspberry Pi 3 (composite or HDMI) and once on the PC. Use your own sets. Copy them to the share over the network or on the stick; never into the repository. Record which message showed for each case, and tick the box only when both machines pass.

- [ ] `needs-hardware` **The real title and a missing BIOS.** Pi 3 and PC.
  - Do: put `mslugx.zip` in `roms/neogeo`, with no `neogeo.zip` anywhere on the share. Rescan, open Neo Geo, start the game.
  - Expect: the list shows the title of the index, not `mslugx`. The launch stops with "This game needs the BIOS file neogeo.zip...". The screen mode does not change.
  - Record: the title in the list and the message.
- [ ] `needs-hardware` **A complete Neo Geo set starts.** Pi 3 and PC.
  - Do: add `neogeo.zip` to `bios/` (then try `bios/fbneo/` and `roms/neogeo/`). Rescan and start the game.
  - Expect: `neogeo` is not in the list. The game starts and runs.
  - Record: each place of `neogeo.zip` that worked.
- [ ] `needs-hardware` **A vertical game starts.** Pi 3 and PC.
  - Do: put `ddonpach.zip` in `roms/arcade` and start it.
  - Expect: the list shows its title with `V`; the check passes and the game starts.
  - Record: the mark and the result.
- [ ] `needs-hardware` **Damaged and wrong sets stop with their messages.** Pi 3 and PC.
  - Do: copy a set and cut the copy to half its size; make a copy with one ROM removed from the zip; rename a set to a name that is no set; put a set for another emulator (for example a MAME set of a different version) in `roms/arcade`. Start each one.
  - Expect: the cut copy gives "The file ... cannot be read"; the copy with one ROM removed gives "1 file of this game is missing or differs: <name>..."; the wrong name gives "... is not a set name of FinalBurn Neo"; the set for another emulator gives "Most files of this game do not match..." when more than half of its files differ. Each dialog fits the screen with no cut line.
  - Record: each message, and whether the text fits on composite.
- [ ] `needs-hardware` **The alias folder and the missing index.** Pi 3 and PC.
  - Do: put a set in `roms/fbneo`. Then, on a development image only, move `/usr/share/neptron/arcade/fbneo.idx` away (the root is read-only: use the hot folder of the development variant, or a test image), restart the frontend, and start an arcade game.
  - Expect: the set in `roms/fbneo` shows under Arcade. With no index the list shows set names, and the launch asks "Cancel" or "Start"; "Start" runs the game unchecked.
  - Record: the list and the dialog.

## Rotation of a vertical game (M08a)

The renderers turn the game picture in emulation, and `gles-compare` checks the GLES renderer against the reference on softpipe. Only the real core, the real GPU and a real screen show that a vertical board stands upright, the right way up, at its speed.

- [ ] `needs-hardware` **DoDonPachi stands upright on a flat panel.** PC with a 1080p panel.
  - Do: start `ddonpach` (FinalBurn Neo asks for `R90`).
  - Expect: the picture is upright, 3:4, 720x960 in the middle of the panel; the text of the attract mode reads from left to right, not from top to bottom; nothing is stretched. The status shows `1920x1080p ... panel`.
  - Record: a photo, and the core's rotation in the log (`rotation r90` in a core trace, or the source in the switch trace).
- [ ] `needs-hardware` **A flipped vertical board and a flipped horizontal board.** PC or Pi 3, with sets that FinalBurn Neo marks `BDF_ORIENTATION_FLIPPED` (for example Gunlock, `R270`).
  - Do: start each one.
  - Expect: each picture is upright and the right way up; text reads from left to right.
  - Record: the set, the turn in the log, and a photo.
- [ ] `needs-hardware` **A vertical game on a 15 kHz CRT and on composite.** A 15 kHz tube with `generic_15` on the PC, and the Pi 3 composite output on NTSC.
  - Do: start `ddonpach` with the picture mode Auto; then set Picture mode to Fill in the Video page of the quick menu.
  - Expect: on composite, Auto shows 320 lines 1:1, interlaced (480i), upright and pillarboxed, and the one-time notice of a vertical game shows on the first start only. The interlaced picture has the flicker of 480i and no line pairs out of order. On the PC tube the policy has no interlace (448i is not possible): record the mode Auto gives. Fill shows a progressive picture that fills the height, soft, and right after the change the notice says "Soft picture: Picture mode is set to Fill.".
  - Record: the mode in the status, photos of both, and whether the one-time notice came back after a restart.
- [ ] `needs-hardware` **Scanlines on a turned game.** A 31 kHz or multi-sync PC CRT.
  - Do: start `ddonpach` with `display.scanlines = on` on a mode with a whole vertical multiple of 2 or more (2x 640p on a multi-sync tube).
  - Expect: the dark lines are horizontal on the screen, one in each pair of lines of the upright picture, and not vertical.
  - Record: a close photo.
- [ ] `needs-hardware` **The GPU cost of a turn on the Pi 3.** Pi 3, with the bench build of `cargo xtask os push`.
  - Do: run `gles_compare` on the Pi as the GLES section of `crates/neptron-gfx/README.md` says, then a vertical game and a horizontal game of the same size on HDMI, and read the render time of each frame from the frame statistics.
  - Expect: every turned scene of `gles_compare` passes on `vc4` (the two axis product cases above 2048 pixels excepted, as for the scenes with no turn); the render time of the turned game is the time of the horizontal one within the noise.
  - Record: the `gles_compare` summary and both render times (ADR 0015 asks for this number).
- [ ] `needs-hardware` **A game that does not start returns to the list with a notice.** The PC, FinalBurn Neo, with an image whose arcade index has the disk lines.
  - Do: put `kinst.zip` with no disk image in `roms/arcade` and start it. Then put the disk image at `roms/arcade/kinst/kinst.chd` and start it again. Then, on an image or a `cargo xtask os push` with the current frontend but an arcade index with no disk lines, remove the disk image, start it again and hold the launch button.
  - Expect: with the index with disk lines and no disk image, message 8 ("This game needs the hard disk image kinst.chd. Put it in the folder kinst, next to kinst.zip.") shows in the game list and no game run starts. With the disk image, the game starts and plays; this is the first run of a board with a disk image, and it is not a tested board family. With the old index, the core error screen shows for a moment, then the game list with the notice "kinst did not start: the core stopped it at the start. The core says: "HDD: kinst.chd not found". Check that the game has all its files.", and the log has that line and the error lines of the core. Wait more than 10 s on the error screen before a press: the notice is then "kinst stopped: the core ended the game. The core says: ..." with the last message of the core. No case returns to the list with no notice. In both cases the frontend never stops and the list mode is the mode before the launch.
  - Record: photos of both notices, whether the game runs with the disk image, and the log lines.

## Picture mode (M08e)

The policy tests, the goldens of `crates/neptron-modeline/tests/golden/policy_modes.toml` and the exhaustive run of `offer` prove the decision of each picture mode and refresh choice in emulation, and tier 2 tests change them in a running game. Only a real screen shows the picture, the sync and the speed. Change the options in the Video page of the quick menu while the game runs, and read the mode in the status of the Video page after each change. A greyed option cannot be chosen: check that its help line gives the reason.

A 15 kHz tube with `generic_15` is on the PC in these images, and the PC has no interlace. Where an expectation below names an interlaced mode on such a tube, record the mode the policy gives instead.

- [ ] `needs-hardware` **Each picture mode on a 15 kHz CRT.** A 15 kHz tube with `generic_15`.
  - Do: start a SNES game, DoDonPachi (`ddonpach`, vertical) and a DOS game in 640x400 at 70 Hz. For each, choose every option of Picture mode that is not greyed.
  - Expect: 2x, 3x and 4x are greyed for all three ("Not possible with this screen and video output"); Native is greyed for DoDonPachi and the DOS game; Game speed is greyed for the DOS game ("This screen cannot hold the game's speed"). The SNES game shows 1:1 in 224 lines at 60.10 Hz in Auto, Sharp lines, Game speed, Fill and Native. DoDonPachi shows interlaced 1:1 (448i) in Auto, Sharp lines and Game speed, and a progressive soft picture of 252 lines in Fill, with the notice "Soft picture: Picture mode is set to Fill.". The DOS game shows interlaced 1:1 at 64.29 Hz in Auto and Sharp lines, and a progressive soft picture of 221 lines at 65 Hz in Fill. The tube holds sync at each change, with no roll and no bend, and no confirm dialog opens. In Settings > Display, 2x, 3x, 4x and Double rate are greyed for every game, and every other option is plain.
  - Record: the mode in the status after each change, a photo of each different picture, and whether the tube lost sync at any change.
- [ ] `needs-hardware` **Each picture mode on a 31 kHz CRT, with scanlines.** A 31 kHz tube with `arcade_31`.
  - Do: start a SNES game and a Game Boy game with Scanlines on Auto. For each, choose every option of Picture mode that is not greyed. Then set Scanlines to On and choose Native.
  - Expect: the SNES game shows 2x (448 lines) at 60.10 Hz with scanlines in Auto, Sharp lines, Game speed and Fill, and 1:1 in a bordered mode of about 478 lines with no scanlines in Native; 3x and 4x are greyed. The Game Boy game shows 3x (432 lines) in Auto, Sharp lines, Game speed and Fill, 2x in 2x, and 1:1 with borders in Native. Every picture is sharp, with no flicker. With Scanlines On, Native shows the notice "No scanlines: they need each game line on two or more screen lines.".
  - Record: the mode in the status after each change, and a close photo of the 2x picture with its scanlines.
- [ ] `needs-hardware` **The refresh rate on a CRT that takes 50 Hz and 120 Hz.** A PC CRT with `pc_31_120`.
  - Do: start a SNES game. Choose each option of Refresh rate that is not greyed: Auto, Double rate, 50 Hz, 60 Hz.
  - Expect: Auto shows 2x at 60.10 Hz; Double rate shows the game's own 224 lines at 120.19 Hz, each frame twice, at the game's speed; 50 Hz runs the game slower, with the speed notice; 60 Hz runs it at 60 Hz. An option the monitor does not take is greyed with "This monitor does not take N Hz" (in the row "Not available" when the reason is long, and the full reason in the help line). In Settings > Display, Double rate is selectable with the tag "Not all games", and the notice after its pick says "Some games need 99.840 Hz, which this monitor does not take.".
  - Record: the mode and the result in the status after each change.
- [ ] `needs-hardware` **Each picture mode on a flat panel.** A 1080p panel that lists a 50 Hz mode.
  - Do: start a SNES game, a Mega Drive PAL game and DoDonPachi. For each, choose every option of Picture mode that is not greyed. Then set Scanlines to On with the SNES game in Auto.
  - Expect: the SNES game fills the height (1440x1080) in Auto, Game speed and Fill, shows 4x (1194x896) in 4x and Sharp lines, 3x (896x672) in 3x, 2x (597x448) in 2x and 1:1 (299x224) in Native, centred, with no stretch. The Mega Drive PAL game runs at 50 Hz in every mode, at its own speed. DoDonPachi shows 3x (720x960) in Auto, 3x and Sharp lines, the fill (810x1080) with the notice "Soft picture: Picture mode is set to Fill." in Fill, 2x (480x640) in 2x and 1:1 (240x320) in Native; 4x is greyed ("Too large for this screen"). In Settings > Display, Game speed, 3x and 4x show the tag "Not all games", and the notice after a pick says why. Refresh rate is disabled ("Not used on a flat panel"). With Scanlines On, the SNES game shows 4x (1194x896) with scanlines and the notice "Whole steps, with borders: scanlines are on, and they need whole steps on a flat panel.".
  - Record: the mode and the size in the status after each change, and a close photo of the Sharp lines and the Fill pictures.
- [ ] `needs-hardware` **A picture mode that also sets the refresh rate.** A 15 kHz tube with `generic_15`.
  - Do: start a SNES game, set Refresh rate to 50 Hz in the Video page of the quick menu, then open Picture mode.
  - Expect: Game speed is selectable with the tag "Rate to Auto too" in its row, and its help line keeps its own text. A pick shows the game at 60.10 Hz at its own speed, the notice "Refresh rate is set to Auto too." shows, and the Refresh rate row reads Auto. Then Resume and keep the change for the System: the keep moves the pair (Game speed, Auto) to the system with no notice, the system file holds both keys and the game file neither, and another game of the system starts in Game speed at its own rate. The tube holds sync.
  - Record: the mode in the status before and after the pick, and the mode of the second game.
- [ ] `needs-hardware` **The notice after a slow mode change.** A 15 kHz tube with `generic_15`, a game whose Fill picture needs a modeset (DoDonPachi).
  - Do: set Picture mode to Fill in the quick menu and watch the screen until the new picture shows.
  - Expect: the notice "Soft picture: Picture mode is set to Fill." shows once the new picture is on screen, also when the tube takes long to lock: the app waits for the picture, not for a count of frames.
  - Record: the time from the pick to the picture, and when the notice shows.
- [ ] Not possible in this image (no DPI overlay). **Settings > Display while its answers are made.** A Raspberry Pi 3 on DPI to a 15 kHz tube.
  - Do: open Settings > Display, then Picture mode, at once and again after a scanline change.
  - Expect: the options show "..." for a moment, then their tags and reasons; the menu and the sound never stutter while they are made.
  - Record: how long the "..." shows.
- [ ] Not possible in this image (no DPI overlay). **The cost of the picture answers on a Pi.** A Raspberry Pi 3 with `pc_31_120` on DPI, and with `generic_15`.
  - Do: run `policy_cost` of `neptron-modeline` on the Pi (release build of the image), and open Settings > Display and the keep prompt of the quick menu while a game runs.
  - Expect: two decisions of the common case take less than 8 ms; the frame of the menus and the sound never stutter. The worst case (a stored mode and Auto with no picture, the Fill fallback) can pass 8 ms: record it.
  - Record: the numbers of `policy_cost`, and the time until the buttons of the keep prompt take a choice.
- [ ] `needs-hardware` **The speed alert.** A 15 kHz tube with `generic_15`, and a TV.
  - Do: start a NES game (60.099 Hz) on `ntsc`-rate modes (a TV, or the `ntsc` preset) and a WonderSwan game (75.47 Hz) on the 15 kHz tube.
  - Expect: the NES game at 59.94 Hz (0.26 % off) shows no `[!]` and no speed sentence in the notice; on a CRT the status still shows `speed (60.099 to 59.940 Hz)`. The WonderSwan game at 65 Hz (-13.9 %) shows `[!] speed (75.472 to 65.000 Hz)` and the speed sentence.
  - Record: the status line of each, and whether the sound of the NES game is right.
- [ ] `needs-hardware` **Picture mode, refresh rate and scanlines on the Pi 3 with a flat panel.** Pi 3 B or B+ on HDMI to a 1080p panel.
  - Do: start a SNES game and DoDonPachi. Choose every option of Picture mode that is not greyed; set Scanlines to On, then Off.
  - Expect: the same pictures as on the PC with a panel (the check above). Refresh rate is disabled ("Not used on a flat panel"). No option that is not greyed leaves the screen black; each change shows the picture at once.
  - Record: the mode and the size in the status after each change, and the render time of the frame statistics for the Fill picture.
- [ ] `needs-hardware` **Picture mode, refresh rate and scanlines on the Pi 3 with a TV.** Pi 3 composite output on NTSC, then on PAL.
  - Do: open the options of Picture mode in Settings > Display. Start a SNES game and DoDonPachi, and choose every option of Picture mode that is not greyed. Set Scanlines to On.
  - Expect: Game speed, 2x, 3x and 4x are greyed; the help line of Game speed says "Not available. A TV has one rate.". Refresh rate is disabled ("Not used on a TV"). DoDonPachi shows interlaced 1:1 in Auto and Sharp lines, and a progressive soft picture in 240p (288p on PAL) in Fill, with no flicker. With Scanlines On, the notice says why no scanline shows. The SNES game on NTSC (60.054 Hz, 0.07 % off) shows no `[!]`.
  - Record: a photo of the options and of each picture, and the status lines.

## Screensaver (M15)

- [ ] `needs-hardware` **No missed field while the screensaver runs on the Pi 3.** Raspberry Pi 3 on composite (NTSC). The second setup of this check, DPI to a 15 kHz tube, is not possible in this image (no DPI overlay).
  - Do: set Screensaver time to 1 minute in Settings > Display. Leave the game list for 2 minutes, then pause a game with the quick menu and leave it for 2 minutes. Read the frame statistics of the control protocol before and after.
  - Expect: the screensaver starts after 1 minute in both places. The count of missed flips does not grow while it runs. The word moves smoothly, one pixel at a time, with no stutter.
  - Record: the missed flips before and after each wait, and the render time of the frame statistics.
- [ ] `needs-hardware` **The screensaver on a CRT.** A 15 kHz tube with `generic_15`, and the SyncMaster 750s with `pc_31_120`.
  - Do: let the screensaver start in the game list. Watch it for 1 minute, then press a button.
  - Expect: a black screen with the word NEPTRON, sharp, that bounces at each edge of the screen and takes a new colour at each wall. No colour looks white or bright. No part of the word leaves the visible area. The mode does not change (no resync). The button brings back the menu as it was, and does not open the focused row.
  - Record: a photo of the screensaver, and whether the word reaches the edges inside the visible area.
- [ ] `needs-hardware` **Even movement on a flat HD panel.** A 1080p panel at 60 Hz, and the same panel with Refresh rate at Double rate (a 120 Hz menu mode) when it offers one.
  - Do: let the screensaver start in the game list and watch it for 1 minute.
  - Expect: the word moves on a straight 45 degree line, one canvas pixel on each axis in each frame, with no wobble or staircase. At 120 Hz it moves at the same speed as at 60 Hz.
  - Record: the mode, and whether the movement is even.
- [ ] `needs-hardware` **Picture mode Fit on a TV and a CRT.** A TV on composite (NTSC and PAL) with R-Type, the SNES and DoDonPachi, and the SyncMaster 750s with `pc_31_120` and DoDonPachi.
  - Do: set Picture mode to Fit in the quick menu of each game.
  - Expect: the whole picture shows, no line cut, as large as the screen allows, with round circles (the shape kept). The lines are even and soft, with no line that looks thicker or missing. DoDonPachi fills the height of each screen. R-Type on NTSC shows the lines that Auto cuts at the top and the bottom.
  - Record: a photo of each picture, and any line that looks uneven.

## TATE: a screen turned on its side (N01)

The checks of `docs/spec/tate.md`, section 11.5. Each one needs a monitor that a person turns on its side. Set "Screen orientation" in Settings > Display (or `orientation` in `neptron.toml` on the boot partition) to the side the monitor is turned to.

- [ ] `needs-hardware` **The portrait menu and the games on a turned 15 kHz CRT.** A 15 kHz tube with `generic_15`, turned 90 degrees, with Screen orientation at Tate left (90) or Tate right (270).
  - Do: read the game list and Settings > Display. Start DoDonPachi, open the quick menu, then close it. Start a horizontal game (a Neo Geo or a CPS-2 game).
  - Expect: the menu is upright for the player, at 240p, with no flicker, and each line of up to 34 characters can be read. DoDonPachi fills the screen at 240p at its own rate (57.55 Hz), every pixel exact. The quick menu over DoDonPachi is 1:1 at 240p, with no mode change when it opens. The horizontal game is upright across the screen, small; the quick menu shows "This game is horizontal. It shows small on a turned screen."
  - Record: a photo of the menu and of each game, the status line of each game, and any line that cannot be read.
- [ ] `needs-hardware` **The portrait menu and a vertical game on a turned 31 kHz CRT.** The SyncMaster 750s with `pc_31_120`, or a 31 kHz arcade monitor with `arcade_31`, turned 90 degrees.
  - Do: read the game list. Start DoDonPachi with Scanlines at Auto.
  - Expect: the menu is upright at 2x, sharp. DoDonPachi shows at 2x with scanlines, upright.
  - Record: a photo of the menu and of the game, and the status line.
- [ ] `needs-hardware` **A flat panel turned on a pivot.** A 1080p panel on a pivot stand, at 90 and at 270.
  - Do: at each orientation, read the game list and start DoDonPachi.
  - Expect: the menu is upright at 4x, nearest, sharp. DoDonPachi fills the width of the turned panel, upright.
  - Record: a photo of the menu and of the game at each orientation.
- [ ] `needs-hardware` **A composite TV turned on its side.** Pi 3 composite output on NTSC, if the TV can be turned safely.
  - Do: read the game list and Settings > Display.
  - Expect: the menu is upright at 240p, with no flicker.
  - Record: a photo of the menu. If the TV cannot be turned, write that here.
- [ ] `needs-hardware` **The render time of a turned canvas on the Raspberry Pi 3.** Pi 3 B or B+ on HDMI, the existing bench build.
  - Do: set Screen orientation to Tate left (90). Read the frame statistics of the control protocol in the game list and in the quick menu over a game, then the same at Yoko (0).
  - Expect: the render time with the turned canvas stays inside the frame budget, near the time at 0.
  - Record: p50, p99 and the maximum of the render time at 90 and at 0, in the game list and in the quick menu.

## A core that draws with GL (S02)

The checks of `docs/spec/hw-render.md`, sections 7.3 and 8, that only hardware can prove. `gles-compare` and `kms-check` prove the path on softpipe and `vkms` in QEMU; the fake display proves the session. The GL test core of `neptron-testcore` (script version 5, `hw_render`) is the core of the first checks; the real cores come with S10 (PlayStation) and S11 (Dreamcast and Naomi). Each check names its machine.

### What the drivers give (W1)

- [ ] `needs-hardware` **The context types of `iris` on the PC.** PC i3-6100T, any monitor, the PC image.
  - Do: run `gles_compare` on the target. Read the line `core offer:` and the checks `core gles2`, `core gles3`, `core glcore`, `core gl`.
  - Expect: the offer holds OpenGL ES 3.2, the core profile 4.6 and the compatibility profile 4.6 (`docs/spec/hw-render.md`, section 1.1). Every core check passes: the picture, both origins, the crop, `hw_dirty`, `hw_fbo0`, the resize, the read back, the release after `core_end` for each type, and the refusals inside a core frame.
  - Record: the offer line, the `gles_compare` summary, and the EGL and GL strings. Write here whether the image has a desktop GL library with EGL and no GLX (section 11).
- [ ] `needs-hardware` **The context types of `vc4` on the Pi 3.** Pi 3 B or B+, HDMI, the Pi image.
  - Do: run `gles_compare` on the target and read the `core offer:` line and the core checks.
  - Expect: the offer holds OpenGL ES 2.0 only, and no desktop GL. The `gles2` checks pass. If the offer is empty, the log names the missing extension (`EGL_KHR_surfaceless_context`, `EGL_KHR_image_base`, `EGL_KHR_gl_texture_2D_image` or `GL_OES_EGL_image`).
  - Record: the offer line, the summary, and each missing extension.
- [ ] `needs-hardware` **The context types of `v3d` on the Pi 4 and the Pi 5, through `kmsro`.** Pi 4 and Pi 5, HDMI, the new Pi 4 and 5 images (not in S02).
  - Do: run `gles_compare` on each board.
  - Expect: the offer holds OpenGL ES 3.1; the EGL display on the card of `vc4` renders with `v3d`, and no check falls back to software.
  - Record: the offer line, the GL renderer string and the summary of each board.

### The picture of a GL core

- [ ] `needs-hardware` **The GL test core on a 15 kHz CRT at 1x.** PC or Pi 3 with a 15 kHz display (section 4 or section 6), `generic_15`.
  - Do: put a script `neptron-testcore 5`, `hw_render gles2 2 0`, `geometry 320 240 0` in a system folder of the test core and start it.
  - Expect: 320x240p at the rate of the core (the status line), every pixel exact: a white border, a blue top-left corner, a green staircase to the bottom-right corner, a yellow arrow that points up. No tear, no black frame.
  - Record: a photo and the status line.
- [ ] `needs-hardware` **The GL test core on a 1080p panel at 4x.** PC i3-6100T on a flat panel (section 2), the script above.
  - Do: start the game, then turn Scanlines On in the quick menu, and start the game again.
  - Expect: with scanlines Off the card fills the height, upright and sharp; the session draws it at 4x (1280x960), which the tier 2 test `a_pc_panel_takes_the_largest_whole_internal_resolution` proves on the fake display. With scanlines On the card shows at 4x of its 240 lines with a scanline on each line.
  - Record: a photo of each.

### Cost and real cores (W6)

Measure each with the bench build (`cargo xtask os push`) and the frame statistics of the control protocol.

- [ ] `needs-hardware` **The two context switches per frame.** PC i3-6100T and Pi 4.
  - Do: run the GL test core 60 s, then a software core of the same size 60 s, and read the frame time.
  - Expect: the GL core costs less than 0.5 ms more per frame.
  - Record: p50 and p99 of the frame time of each.
- [ ] `needs-hardware` **The fence wait against `glFinish`.** PC i3-6100T and Pi 4.
  - Do: read the log line that says whether EGL has `EGL_KHR_fence_sync` and `EGL_KHR_wait_sync`, and the GPU time per frame of the GL test core.
  - Expect: both extensions on `iris` and `v3d`; the GPU time of the composition of a GPU picture is the time of an uploaded frame of the same size, within the noise.
  - Record: the extensions and both GPU times.
- [ ] `needs-hardware` **Beetle PSX HW on the PC at 1x and 4x.** PC i3-6100T, a 15 kHz CRT and a 1080p panel (S10 packages the core).
  - Do: play a 3D game for 5 minutes on each display.
  - Expect: full speed (no slow-speed notice) at 1x on the CRT and at 4x on the panel; the picture is upright, the right way up, with no black border from the origin.
  - Record: the slow-speed counter, a photo of each, and the time to the first picture.
- [ ] `needs-hardware` **Flycast on the Pi 4 and the PC at 1x.** Pi 4 on HDMI and PC i3-6100T (S11 packages the core).
  - Do: play a Dreamcast game and a Naomi game for 5 minutes each.
  - Expect: full speed on the PC; the speed on the Pi 4 is a number for the owner. The first picture comes in less than 10 s, so no notice H3.
  - Record: the slow-speed counter and the time to the first picture of each.
- [ ] `needs-hardware` **A lost context on Intel.** PC i3-6100T, the GL test core.
  - Do: during the game, cause a GPU reset (`echo 1 > /sys/kernel/debug/dri/0/i915_wedged` as root, then wait for the recovery), once, then a second time.
  - Expect: after the first reset the game goes on with a new context; after the second the game ends with the notice "The game stopped: the graphics driver lost the picture of the game two times." The machine never freezes.
  - Record: the log lines of each reset and a photo of the notice.

### The watchdog of a hung game

- [ ] `needs-hardware` **A hung core ends after 15 s.** Any machine with the launcher, the development variant.
  - Do: start a game, then stop the frontend in the middle of a frame (`kill -STOP` of the frontend process from SSH).
  - Expect: after 15 s the launcher logs `the game sent no heartbeat for 15000 ms; SIGKILL`, the game list comes back with the notice "The game stopped: it did not answer for 15 seconds.", and the save file of the game is the last one written before the stop.
  - Record: the log lines, a photo of the notice, and the size and time of the save file before and after.
- [ ] `needs-hardware` **A slow first frame is not killed.** Pi 4, Flycast (S11).
  - Do: start a Dreamcast game with an empty shader cache.
  - Expect: the game starts; no kill and no notice.
  - Record: the time to the first picture.
