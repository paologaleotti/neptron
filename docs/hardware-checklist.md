# Hardware check list

Emulation cannot verify these checks. Each one needs real hardware, and each one is labelled `needs-hardware`. No result is claimed until a person runs the check and writes the result here. The orchestrator adds the checks of each task when the task merges.

The list is in the order of the work on the bench: first the safety checks, then the self-test, then the checks of each area, and at the end the checks that the bench cannot do yet.

## The bench

- Computer: Intel i3-6100T with Intel HD 530 (i915, capability table entry `intel_gen9`).
- Monitor: Samsung SyncMaster 740s, a 31 kHz PC CRT, through a DisplayPort to VGA bridge. Its preset is `pc_31_120`: 31.4 to 31.6 kHz, 100 to 130 Hz for 200 to 256 lines, 50 to 65 Hz for 400 to 512 lines, no interlace.
- Also needed for some checks: a USB gamepad, a USB keyboard, a flat panel, a second console (SSH or another virtual terminal).

## How to record a result

- Each check has three parts. **Do** is what you do. **Expect** is what a pass looks like. **Record** is what you write down.
- Tick the box only when the check passes. When it fails, leave the box empty, write what you saw under the check, and open a point for the orchestrator.
- Keep these files with each result: the JSON report of the tool, and the output of `dmesg | tail -50` after the run. A photo of the screen helps when the picture is wrong.
- Run every tool as root on a text console, with no X or Wayland. Stop the frontend and the launcher before a tool that opens the DRM device (`neptronctl selftest video`, `kms-spike`, `gles_compare`).

## Safety rules for the 740s

The frontend falls back to its safe default when it has no settings, after two failed boots, and on blind recovery. The safe default is the EDID profile when the monitor gives EDID, and else `generic_15` (15 kHz). A 15 kHz signal is out of range for the 740s. It is deliberate on an unknown display, but do not send it to this monitor.

- Do the check "Find the VGA connector" first. Its result decides the rest:
  - **EDID**: the `edid` file of the connector is not empty. The safe default is the EDID profile, and every check below can run.
  - **No EDID**: the file is empty. Before the frontend starts with the 740s connected, write the preset into `/state/config/global.toml` (`[display]` with `monitor = "pc_31_120"`), and confirm the boot prompt within 10 s; with no confirmation the guard reverts to the safe default. Skip each check marked "Needs EDID", and write "skipped: no EDID through the bridge" under it.
- Keep a hand on the power switch of the monitor. Switch the monitor off at once if it whines, clicks, or shows a rolling or folded picture, and write down what the tool or the frontend showed at that moment.
- A check marked "Needs EDID" can make the frontend fall back to the safe default.

## 1. First boot and safety checks

Do these before the CRT gets a mode that nobody checked on it.

- [ ] `needs-hardware` **The image boots on any monitor.**
  - Do: boot the USB image of the x86_64 target with a flat panel connected, not the CRT.
  - Expect: the game list shows.
  - Record: the boot time to the game list, and the connector name in the log of the frontend.
- [ ] `needs-hardware` **Find the VGA connector.**
  - Do: with the CRT connected through the bridge, run `ls /sys/class/drm/` and `cat /sys/class/drm/card0-*/status`, or `kms-spike --list-connectors`.
  - Expect: one connector of the bridge is connected. It is probably `DP-1`, not `VGA-1`.
  - Record: the connector name, its status, and the size of its `edid` file (0 means that the bridge passes no EDID).
- [ ] `needs-hardware` **Check the ranges before the first mode.**
  - Do: `neptronctl selftest video --profile pc_31_120 --connector <name> --report /tmp/dry-pc.json`. If the `edid` file is not empty, also `neptronctl selftest video --profile edid --connector <name> --report /tmp/dry-edid.json`.
  - Expect: the dry run sets no mode. It prints the ranges of the profile, the GPU values (table entry `intel_gen9`, pixel clock 25 to 165 MHz, no interlace) and the modes: 240p at 120 Hz (`240p_double_refresh`), 480p, 768x224 at 120 Hz (`super_width_dynamic`) and 1920x224 at 120 Hz (`super_width_fixed`).
  - Record: compare each range with the manual of the 740s. Write down the modes and the EDID profile id (`edid_...`) and its ranges.
- [ ] `needs-hardware` **The first start on the 740s.**
  - Do: with **EDID**, boot the image with only the CRT connected and no settings file. With **no EDID**, do not boot with no settings: write `pc_31_120` into the settings first (see "Safety rules for the 740s"), boot, and confirm the boot prompt.
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

## 2. The self-test

`neptronctl selftest video` shows one test mode per mode family of the profile, each for 10 seconds, with the kernel's read back and the flip timing. `apps/neptronctl/README.md` describes the report.

- [ ] `needs-hardware` **The self-test on `pc_31_120`.**
  - Do: `neptronctl selftest video --profile pc_31_120 --connector <name> --report /tmp/selftest-pc.json --i-have-checked-the-monitor pc_31_120`
  - Expect: each mode shows for 10 s. The white border is visible on all four edges, the cross is at the centre, and the text names the family, the size, the rates and the pixel clock. The monitor holds each mode with no roll, tear or flicker. If the monitor shows its frequencies, 240p and the super widths read about 31.4 kHz at 120 Hz, and 480p about 31.4 kHz at 60 Hz. The tool prints `PASS` for each mode, `Restore: the output is off`, `Result: PASS`, and exits with 0.
  - Record: for each mode, from the report: `readback_equal`, `timing.period_error_ppm`, `timing.implied_pclock_hz` against `request.pclock_hz`, `timing.missed_vblanks` and `timing.flips_per_second_mhz`. For each mode, what the picture showed: how much of the border is hidden, and whether the picture sits off centre.
- [ ] `needs-hardware` **The self-test on the EDID profile.**
  - Do: if the bridge passes EDID, the same with `--profile edid --i-have-checked-the-monitor <edid id>`.
  - Expect: as for `pc_31_120`. The modes follow from the ranges of the EDID.
  - Record: the modes and the same fields as above.
- [ ] `needs-hardware` **The clock the hardware makes.**
  - Do: during the self-test, read `/sys/kernel/debug/dri/0/i915_display_info` from a second console.
  - Expect: the clock of the CRTC is near the requested clock.
  - Record: the clock i915 reports, `implied_pclock_hz` and `request.pclock_hz` for each mode.
- [ ] `needs-hardware` **480p and the 2x picture style are stable.** (MVP exit)
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

## 3. Checks by area

### KMS and renderer

#### kms-spike (T21)

- [ ] `needs-hardware` **Build the spike.**
  - Do: build on Linux with `cargo build -p kms-spike --features kms` (dev profile).
  - Expect: it builds. The release profile aborts on a panic and skips the restore guard, so do not use it.
  - Record: the commit that was built.
- [ ] `needs-hardware` **Dry run of the spike.**
  - Do: `kms-spike pc_31_120 --connector <name>`.
  - Expect: the printed ranges match the manual of the 740s. The restore plan is "output off", because the console mode fails `pc_31_120`.
  - Record: the restore plan.
- [ ] `needs-hardware` **Confirmed run of the spike.**
  - Do: `kms-spike pc_31_120 --connector <name> --i-have-checked-the-monitor`.
  - Expect: for each mode, the border is visible on all four edges, the cross is centred, and the label matches the mode. The monitor's on-screen display shows about 31.4 kHz at 120 Hz for 240p, and 31.4 kHz at 60 Hz for 480p. In the report, `test_commit` and `commit` are ok, and `readback.equals_request` is true. For a progressive mode, `vblank.rate_mhz` is within 1 % of the field rate. At the end, the console comes back in its own mode after the "output off" restore, and the summary shows `restore:` with no `RESTORE FAILED`.
  - Record: for 480i, `rate_mhz` against the field rate and twice the field rate. `flip_cadence` together with `flips.skipped_vblanks`. `implied_pclock_hz` against `requested.pclock_hz` and against `/sys/kernel/debug/dri/0/i915_display_info`.
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
- [ ] `needs-hardware` **Keep the spike's files.**
  - Do: after every run of the spike, save `dmesg | tail -50` and the JSON report.
  - Expect: one pair of files per run.
  - Record: where the files are.

#### KMS backend (T23) and blank (C07)

Run these with the real frontend path (`KmsDisplay::from_env` from the launcher) on the i3-6100T, unless the check names the self-test.

- [ ] `needs-hardware` **Open picks the VGA output.**
  - Do: start the frontend with no `display.output` setting (with EDID, or with `pc_31_120` in the settings).
  - Expect: the connector rule picks the VGA output: an analog connector with EDID, or the DP-to-VGA bridge as `DP-n`.
  - Record: the name, the driver, and whether the buffers are linear by the modifier list or by the usage flag (`display.linear_by_modifier` in the self-test report says the same).
- [ ] `needs-hardware` **No modeset before the first commit.**
  - Do: open the display and do not set a mode.
  - Expect: the firmware's mode is still on the wire (i915 inherits it) until the first `set_mode`.
  - Record: what the monitor shows.
- [ ] `needs-hardware` **Each bench mode.**
  - Do: set 240p at 120 Hz and 480p at 60 Hz of `pc_31_120` (the self-test does it).
  - Expect: the read back equals the request, and the picture shows the full test scene with no offset.
  - Record: the self-test report.
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
- [ ] `needs-hardware` **Restore after a panic in the release build.**
  - Do: cause an abort in the release build, with the launcher holding the descriptor.
  - Expect: the mode and the last picture stay. The respawned frontend sets the last validated mode again.
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

#### GLES renderer (T19) and scanlines (C04)

- [ ] `needs-hardware` **The renderer on Intel HD 530.**
  - Do: run the `gles_compare` program (built by `cargo xtask gles-compare`) on the target.
  - Expect: all nearest and UI scenes are byte-equal, sharp bilinear is within 1 per channel, and the program reports `GL renderer: Mesa Intel(R) HD Graphics 530`. The surfaceless platform finds the render node (`/dev/dri/renderD128`) and does not fall back to software.
  - Record: the program output.
- [ ] `needs-hardware` **Draw time on a real GPU.**
  - Do: with the KMS backend, draw a 1920x240 super-width frame with sharp bilinear and the UI at 120 Hz.
  - Expect: each frame draws and presents within one frame period.
  - Record: the missed vertical blanks over 600 frames.
- [ ] `needs-hardware` **Scanline strength.**
  - Do: show 2x black scanlines at 100, 75 and 50 percent on the 740s.
  - Expect: the lines read as gaps at a brightness that looks right.
  - Record: the value that looks right.
- [ ] `needs-hardware` **No moiré from the bridge.**
  - Do: scanlines on, through the DisplayPort to VGA bridge.
  - Expect: no moiré.
  - Record: what the picture shows.

### Policy (C02)

- [ ] `needs-hardware` **Double refresh for `native` on 31 kHz.**
  - Do: play a 240-line game with the style `native` on the 740s (`pc_31_120`).
  - Expect: the mode runs at double refresh near 120 Hz, and the notice shows.
  - Record: the mode in the quick menu.

- [ ] `needs-hardware` **A narrow mode with a whole-kHz clock.** (C09)
  - Do: on a 31 kHz monitor, play a Game Gear game on `arcade_31`. Check the ranges of `arcade_31` against the monitor first with the self-test dry run.
  - Expect: the picture is centred and stable.
  - Record: the mode in the quick menu, and the position of the picture.

### Engine and guard (T26)

- [ ] `needs-hardware` **A change with no button press reverts.**
  - Do: on the 740s, change the monitor or the style and press nothing.
  - Expect: the change reverts after 10 s.
  - Record: the time to the revert.
- [ ] `needs-hardware` **A confirmed change stays.**
  - Do: confirm a change, then reboot.
  - Expect: the mode is the same after the reboot.
  - Record: the mode before and after.
- [ ] `needs-hardware` **Two failed boots give the safe default.** Needs EDID.
  - Do: kill the frontend twice before 10 s of picture.
  - Expect: the third start comes up in the safe default.
  - Record: the profile of the third start.
- [ ] `needs-hardware` **Blind recovery.** Needs EDID.
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
- [ ] `needs-hardware` **Double refresh motion.**
  - Do: play with double refresh on the 31 kHz CRT.
  - Expect: smooth motion at the right speed.
  - Record: what the motion looks like.
- [ ] `needs-hardware` **The quick menu over a frozen frame.**
  - Do: open the quick menu during a game on the CRT.
  - Expect: the menu shows over the frozen frame.
  - Record: what the picture shows.
- [ ] `needs-hardware` **Save RAM survives a power cut.**
  - Do: cut the power during play, after an in-game save.
  - Expect: the save file is there after the reboot.
  - Record: the file and its `.bak`.
- [ ] `needs-hardware` **Run rate in the window.**
  - Do: run a game in the window build on a real desktop.
  - Expect: the measured run rate is near the display rate.
  - Record: the measured run rate.
- [ ] `needs-hardware` **Audio latency on the i3-6100T.**
  - Do: play a click on a frame, and measure the time from the flip to the sound.
  - Expect: about 53 ms.
  - Record: the measured time.
- [ ] `needs-hardware` **SDL queue allocation.**
  - Do: a 10-minute game with an allocation tracer.
  - Expect: `SDL_PutAudioStreamData` stops allocating after warm-up.
  - Record: any stall in the frame times.
- [ ] `needs-hardware` **Device buffer.**
  - Do: read `Sdl3Sink::device_buffer_frames()` and the device rate on each output.
  - Expect: the 512-frame hint is honoured.
  - Record: the frames and the rate.
- [ ] `needs-hardware` **The tone is heard.**
  - Do: `cargo xtask test -p neptron-audio --features sdl3 --run-ignored only`.
  - Expect: a clean 440 Hz tone for one second, with no clicks.
  - Record: what you hear.

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
  - Do: hold the recovery combination on a gamepad, then on a keyboard, with no picture.
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

### App and launcher (T27)

- [ ] `needs-hardware` **The KMS frontend on the image.**
  - Do: boot the x86_64 image (after T28).
  - Expect: the KMS build of the frontend shows the game list.
  - Record: the boot time.
- [ ] `needs-hardware` **The descriptor survives a crash.**
  - Do: crash the frontend, then cause a relaunch, on i915.
  - Expect: the launcher's descriptor survives, and the mode stays on the wire.
  - Record: what the monitor shows.
- [ ] `needs-hardware` **SIGTERM to the launcher.**
  - Do: `kill -TERM` the launcher.
  - Expect: it forwards SIGTERM, and the frontend leaves a validated mode on the wire.
  - Record: what the monitor shows.
- [ ] `needs-hardware` **Blind recovery on the image.** Needs EDID.
  - Do: hold the combination on a gamepad and on a keyboard, also from power-on.
  - Expect: the safe default comes back.
  - Record: the device and the time.
- [ ] `needs-hardware` **No sound device.**
  - Do: boot a box with no sound device.
  - Expect: it boots and shows the "No sound" notice.
  - Record: the notice text.
- [ ] `needs-hardware` **A connector change.** Needs EDID.
  - Do: change the connector in `global.toml`.
  - Expect: the boot prompt shows.
  - Record: what the prompt says.
- [ ] `needs-hardware` **The window build.**
  - Do: run the window build on macOS.
  - Expect: the UI and a game show.
  - Record: the macOS version.

### Image (T28 and the MVP exit)

T28 adds the checks of the image. These come from the MVP exit gate.

- [ ] `needs-hardware` **Each core plays.**
  - Do: boot the USB image on any monitor. Play one game of each core (FCEUmm, Snes9x, Genesis Plus GX) with a gamepad.
  - Expect: each game plays with sound, and the quick menu quits to the game list.
  - Record: the game of each core.
- [ ] `needs-hardware` **Confirm-or-revert and blind recovery on the 740s.** (MVP exit) Needs EDID.
  - Do: as in "Engine and guard", on the EDID profile of the 740s.
  - Expect: confirm-or-revert reverts with no button press, and blind recovery restores the safe mode.
  - Record: the results.

## 4. Checks the bench cannot do yet

These need a Raspberry Pi 3 or a 15 kHz display (a SCART TV or an arcade monitor). They are not part of the MVP exit.

### 15 kHz display

- [ ] `needs-hardware` **The self-test on a 15 kHz profile.**
  - Do: on a 15 kHz display, `neptronctl selftest video --profile generic_15 --connector <name> --i-have-checked-the-monitor generic_15`.
  - Expect: 240p, 288p and the super-width modes pass. On i915 the interlaced families are not tested, because the capability table has no interlace.
  - Record: the same fields as on the bench.
- [ ] `needs-hardware` **The spike with 480i on 15 kHz.**
  - Do: `kms-spike ntsc --connector <name> --allow-low-pclock 6000000 --allow-interlace --modes 240p,480i --i-have-checked-the-monitor`.
  - Expect: the test-only commit of 480i with a linear dumb buffer passes or fails with a clear error.
  - Record: the result, and `flip_cadence` (per field or per frame).
- [ ] `needs-hardware` **A 15 kHz super-width mode on i915.** (T23)
  - Do: set 1920x240 at 60 Hz of `generic_15`, with the pixel clock floor of the capability table.
  - Expect: the test-only commit and the commit pass, the read back equals the request field by field, and the monitor syncs.
  - Record: the self-test report.
- [ ] `needs-hardware` **480i with linear buffers and no kernel patch.** (T23)
  - Do: set 480i on a 15 kHz display.
  - Expect: the commit passes and the picture is stable.
  - Record: the result.
- [ ] `needs-hardware` **Interlace cadence.** (T23, OP-030)
  - Do: in 480i, count the flip events per second.
  - Expect: i915 signals one flip per field (about 59.94 per second) or per frame (about 29.97).
  - Record: the count, and set `flip_per_field` in `data/hardware/matrix.toml` from it.
- [ ] `needs-hardware` **A Mega Drive game with a whole-kHz clock.** (C09)
  - Do: on a 15 kHz CRT, play a Mega Drive game at 320x224 on `generic_15` (horizontal total 411, was 416).
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
- [ ] `needs-hardware` **A SCART TV.** (MVP exit, not required for the MVP)
  - Do: with a 15 kHz profile, show 288p at 50 Hz, 240p at 60 Hz and 576i.
  - Expect: each mode is stable.
  - Record: the TV model and the result.

### Raspberry Pi 3

- [ ] `needs-hardware` **The renderer on the Pi 3.** (T19)
  - Do: run `gles_compare` on a Raspberry Pi 3B (vc4, Mesa).
  - Expect: highp floats are available in the fragment shader, and `GL_MAX_TEXTURE_SIZE` (2048 on vc4) is enough: a 320x240 XRGB8888 frame needs a 1280-texel-wide texture, and a 1920x240 output needs 1920.
  - Record: the program output.
- [ ] `needs-hardware` **Precision on the Pi 3.** (T19)
  - Do: the same run.
  - Expect: `headless()` passes the highp check (at least 23 bits). Nearest texel addressing at `(x + 0.5) / width` picks the exact byte texel on textures up to 2048. The every-value scenes and the 1920x240 scenes are byte-equal.
  - Record: the program output.
- [ ] `needs-hardware` **Blank on the Pi 3.** (C07)
  - Do: the three blank checks of the KMS section (blank, set a mode after a blank, blank before the first mode) on vc4.
  - Expect: as on i915.
  - Record: the results.
- [ ] `needs-hardware` **Audio latency on the Pi 3.** (T17)
  - Do: play a click on a frame, and measure the time from the flip to the sound.
  - Expect: about 53 ms.
  - Record: the measured time.
- [ ] `needs-hardware` **Resampler cost on the Pi 3.** (T17)
  - Do: a release build, and the CPU share of `AudioPipeline::end_frame` with Sinc.
  - Expect: 3 to 4 % of one core.
  - Record: the share. Use Polynomial if it is over budget.

## C08 flat panels

Each check is `needs-hardware`.

- [ ] The bench (DP-to-VGA bridge, SyncMaster 740s). Do: start with `display.monitor = "auto"` and read the display panel. Expect: the CRT path, never the panel path; the limits are 165 MHz and 1920x1200. Record: the `subconnector` value and the profile id.
- [ ] An HDMI 1080p panel on the HD 530. Do: first boot with no settings. Expect: 1920x1080 at 60 Hz under the confirm prompt, the menu at 4x and centred; after a confirmation, the next boot has no prompt. A SNES game is 1194x896, centred, sharp, with black around it. Record: the mode line of the Video block.
- [ ] The same panel, no confirmation. Do: let the prompt time out, then press a button once. Expect: no picture after 10 s, the prompt again after the press, and no further offer in that run.
- [ ] A 4K panel on DisplayPort. Expect: 3840x2160 at 60 Hz. Record: whether the start succeeds and the frame times.
- [ ] A 4K panel on HDMI. Expect: 1920x1080 at 60 Hz with the notice.
- [ ] A 720p TV. Expect: 1280x720, and 50 Hz for a PAL game.
- [ ] An HDMI-to-VGA adapter with its own digital EDID, with a PC monitor behind it, never a 15 kHz tube. Expect: at most two windows of the panel mode, then the safe start. Record: what the monitor shows.

## T28 the PC image on the i3-6100T

Each check is `needs-hardware`. Use the development image first.

- [ ] Do: boot the stick with UEFI, then with legacy BIOS (CSM). Expect: GRUB shows no menu, no console appears, the frontend starts. Record: the time from power-on to the first frame.
- [ ] Do: read the address from `neptron-ip.txt` on the share partition or from the router, then `cargo xtask os ssh --host <address> -- uptime`. Expect: a login with the key; a password login is refused.
- [ ] Do: `cargo xtask os logs --host <address>`. Expect: the binary paths, the guard events and each `mode set` line with its read back. Record: the driver line; it must name the Intel HD Graphics 530 (iris), not llvmpipe.
- [ ] Do: `ls /dev/dri` and read `/sys/class/drm` over SSH. Expect: card0 is i915; no simpledrm and no vkms device.
- [ ] Do: change one line of the code, then `cargo xtask os push --host <address>`. Expect: the log shows the frontend from `/state/hot/bin`. Record: the push time.
- [ ] Do: first boot on a stick larger than the image. Expect: the share partition is made and can be read and written from macOS and Windows. A second boot keeps a file put there.
- [ ] Do: write the image again over the stick with ROMs on it. Expect: the ROMs are still there.
- [ ] Do: cut the power during play, then boot. Expect: the state partition mounts, the guard record and the save are kept.
- [ ] Do: `poweroff` over SSH during a game. Expect: the log ends with the clean stop line, and the save is on the disk.
- [ ] Do: `kill -ABRT` of the frontend over SSH. Expect: the mode stays on the VGA output and the launcher starts the frontend again.
- [ ] Do: check sound on the HDA output, a USB pad and a keyboard. Expect: all work; no key reaches a console.
- [ ] Do: run the self-test with the launcher held (see `os/README.md`). Record: the JSON report.
- [ ] Do: the same self-test on a kernel built with `--no-kernel-15khz`, when that image exists. Record: which modes the unpatched driver refuses (OP-070).

## C11 findings of the first hardware session

- [ ] `needs-hardware` Do: boot with the Dell U2724D on DP-1 in standby, `display.output` at `auto`. Expect: the log shows no "cannot open the display" line; it shows "a display answered after N ms" or nothing, and the first picture comes in one start of the frontend. Record: N, and the time from power-on to the first frame.
- [ ] `needs-hardware` Do: boot with no monitor connected, wait 2 minutes, then connect the DisplayPort monitor. Expect: each start logs "no connected display after 15000 ms"; the launcher line "the frontend found no display; it starts again every 1000 ms", with the hint about an analog connector with no EDID, comes at the first start and then at most once a minute; the wait between starts stays 1 s; the picture comes about 1.5 s after the connection (the next look of the wait, then the start); `/state/video/guard.toml` counts no failed boot. Record: the time from the connection to the first picture.
- [ ] `needs-hardware` Do: on the flat panel, open the quick menu in a game and the display panel. Expect: "Picture style" and "Refresh policy" are grey with "Not used on a flat panel", and accept does nothing on them; the scanline rows still change the picture. Record: a photo of the quick menu.
- [ ] `needs-hardware` Do: boot twice, then `ls /state/cache/mesa` and read the log of the frontend. Expect: the folder holds the Mesa cache files, and no line says that Mesa cannot create its shader cache. Record: `du -sh /state/cache/mesa`, and whether the second start is faster than the first.
- [ ] `needs-hardware` Do: in the development image, bring eth0 up on the Realtek port and read `dmesg | grep -i rtl`. Expect: no "unable to load firmware patch rtl_nic/rtl8168h-2.fw" line. Record: the dmesg lines of r8169.
- [ ] `needs-hardware` Do: read `dmesg | grep -i dmc`. Expect: the line about the missing i915 DMC firmware is still there: the file is not in the image until the owner decides the licence question of a firmware file inside the kernel image. Record: the line.

## Flow defects (task C10)

Each check is `needs-hardware`.

- [ ] Do: on a PC with a DVI-I output, boot once with a flat panel on DVI-D pins and once with a CRT through a DVI-I to VGA adapter, with `display.output` = `auto`. Expect: the log line of the open names the DVI-I connector, a panel is chosen as a digital output and the CRT as an analog one, and a CRT profile prefers the DVI-I connector only for the CRT. Record: the `subconnector` value and the first 32 bytes of the EDID (`/sys/class/drm/card0-DVI-I-1/edid`) for both.
- [ ] Do: in a game, hold Select and Start for 5 s. Expect: a short 880 Hz tone, the game goes on in the safe picture with no quick menu, and the game did not pause itself or reset during the hold. Opening the quick menu shows "Safe mode: the picture was reset. ...". Record: whether the tone sounds whole and whether the game sound is late after it.
- [ ] Do: boot once with the state partition damaged (for example `dd if=/dev/zero` over its first MiB on a copy of the stick) and once on a stick with no free space after the state partition. Expect: `/run/neptron/storage.toml` names the RAM folder and its reason, the log has `neptron: storage: ...`, and a Warning notice shows until OK. Record: the file and the notice text.
- [ ] Do: in a game, tap Select and Start, then hold them for 2 s and let go. Expect: the quick menu opens at the release both times, and the game sees neither button while both are down; it can see Select for the one frame before Start goes down, when Select is pressed first. Record: how late the menu feels after the release.

## Library rescan (task C14)

Each check is `needs-hardware`.

- [ ] Do: with the game list on screen, copy a ROM into `roms/nes` of the exFAT share over the network (SMB or `scp`), then open `Settings > Library` and accept `Rescan games`. Expect: the row is disabled with `Scanning...`, then a notice `1 game added.`, and the game is in the list with no restart. Record: the time from the press to the notice for the library on the stick, and the game count.
- [ ] Do: start a game, copy a ROM into the share over the network while it runs, then quit the game. Expect: the list that comes back holds the new game, and the notice says `1 game added.`. Then `stat -c '%y' /share/roms/nes` before and after one more copy. Expect: the folder time changes with each new file on the exFAT driver of the image. Record: both times.
- [ ] Do: on a Windows PC and on a Mac, add a ROM to the share partition of the stick, then read the time of its folder on the device (`stat -c '%y'`) and compare it with the time before. Record: whether each system changed the folder time. A new boot reads every folder, so the game shows in both cases; the record says whether a check after a game would also find such a change.

## C13 network share

Each check is `needs-hardware`. `docs/spec/network-share.md` is the spec. Use the release image unless a check says otherwise, with the PC on the router by an Ethernet cable.

- [ ] Do: boot with no cable, then boot with a cable. Expect: the time from power-on to the first frame is the same within 1 s; with no cable, no line on the console waits for DHCP. Record: both times.
- [ ] Do: boot with no cable, wait for the game list, then plug in the cable. Expect: within 30 s the router lists `neptron` with an address, and `\\<address>\share` opens from another computer. Record: the time from the plug to the address.
- [ ] Do: from Windows 11 (24H2 or later, note the edition), open `\\NEPTRON\share`, then `\\neptron.local\share`, then `\\<address>\share`, with no change on Windows. Expect (from the spec): a refusal, because Windows 11 24H2 requires signing and refuses guest logons. Then turn on "Enable insecure guest logons" and turn off "Digitally sign communications (always)" (or `Set-SmbClientConfiguration -EnableInsecureGuestLogons $true -RequireSecuritySignature $false`) and try again. Expect: the share opens with `roms` and `bios`, with no password prompt. Record: the edition and build, the error text of the first try, which names resolve, and whether NEPTRON shows in the "Network" view (expected: no, there is no WS-Discovery).
- [ ] Do: on Windows 11 24H2 with no change on Windows and no stored credential for NEPTRON (`cmdkey /list`), open `\\NEPTRON\share` in Explorer. Expect (from the spec): an error and no prompt, because Windows sends its own account name, which becomes a guest. Record: the exact text of what Explorer shows, and whether a prompt for a name and password appears.
- [ ] Do: from Windows 11 24H2 with no change on Windows, `net use \\NEPTRON\share /user:neptron neptron`, then copy a file in; then remove the connection (`net use \\NEPTRON\share /delete`) and try "Map network drive" with "Connect using different credentials", and then a Windows credential for `NEPTRON` in Credential Manager with a plain Explorer open. Expect: each way opens the share read-write, and the copy succeeds: the system user `neptron` cannot write `/share`, so a successful copy proves that the forced user `nobody` is in use. Record: which ways work. From this Mac: `smbutil view //neptron:neptron@<address>` lists `share` and `IPC$`, and `smbutil view -g //<address>` (guest) lists them too. Record: the edition and build, and the outputs.
- [ ] Do: on the development image, `grep neptron /etc/passwd /etc/shadow`, then `ssh neptron@<address>`. Expect: the shell is `/bin/false`, the shadow field is `*`, and SSH refuses the login. Record: the lines and the SSH error.
- [ ] Do: the same from Windows 10 (Home or Pro, note which). Expect: the share opens as guest with no change. Record: the edition, build and result.
- [ ] Do: on macOS, Finder, Go, Connect to Server, `smb://neptron.local/share`, Connect As Guest; also look under "Network" in the sidebar. Expect: the share opens read-write; NEPTRON shows under "Network". Record: the macOS version, and whether a `._<name>` file appears next to a copied ROM.
- [ ] Do: on Linux (GNOME Files or KDE Dolphin), open "Other Locations" or "Network", then `smb://neptron.local/share` as anonymous. Expect: NEPTRON shows in the list and the share opens read-write. Record: the desktop and version.
- [ ] Do: copy a 2 GB file and a folder of 500 small files into `roms/nes` from Windows, then from macOS. Expect: the copies finish; the SHA-256 of the large file on the PC (`sha256sum` over SSH on the development image, or on another computer after the stick is moved) equals the source; during the copy a game runs with no audio dropout. Record: the copy speed in MB/s and any dropout.
- [ ] Do: start a copy of a 2 GB file and pull the Ethernet cable in the middle. Expect: the client reports an error; the PC keeps running; after the cable is back the share opens again and the copy can be done again; the cut file is on the share with a smaller size. Record: what the client said, and the size of the cut file.
- [ ] Do: on the development image (the release image has no clean power-off yet), start a copy of a 2 GB file and run `poweroff` over SSH during the copy. Expect: the console shows `neptron-share` stop with no "still has sockets" line, and the share partition unmounts with no error; on the next boot `dmesg | grep -i exfat` shows no dirty-volume message. Record: the shutdown lines of the console.
- [ ] Do: start a copy of a 2 GB file and cut the power during the copy. Expect: the next boot mounts the share partition; the files copied before stay; the cut file may be short or missing. Record: `dmesg | grep -i exfat`, and whether `/run/neptron/storage.toml` says `share = "partition"`.
- [ ] Do: on the development image, `touch /state/network-share-off`, reboot, and try to open the share; then `netstat -ltnu`. Expect: no share and no `neptron.local`; nothing listens on 445 or 5353. Remove the file and run `/etc/init.d/S45neptron-share restart`. Expect: the share opens at once. Record: the `netstat` output of both states.
- [ ] Do: on the release image, from another computer, `nmap -sT -sU -p 1-65535 <address>` (or a TCP scan of all ports and a UDP scan of 53, 67, 68, 137, 138, 5353). Expect: TCP 445 and UDP 5353 open, UDP 68 open or filtered while the PC holds a lease, nothing else. Record: the scan output.
- [ ] Do: from Windows, try `\\NEPTRON\state`, `\\NEPTRON\root`, and in the share make a shortcut or a link that points to `/state` (from Linux: `smbclient //neptron.local/share -N -c 'symlink ../.. up'`). Expect: no other share; the link is refused or cannot be followed. Record: the error texts.
