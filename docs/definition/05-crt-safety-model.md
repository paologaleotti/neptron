## CRT safety model

The rule is structural: the only function that can program a display mode accepts a `ValidatedMode`, and only the safety crate can construct one. Bypassing validation is a compile error, not a code-review finding.

| Layer | What it does |
| --- | --- |
| 1. Type gate | `neptron-safety::validate(mode, profile, gpu_caps) -> Result<ValidatedMode, Rejection>`. `neptron-kms` takes `ValidatedMode` only. |
| 2. Range checks | Horizontal and vertical frequency, total lines, sync pulse width, minimum porches, pixel clock, interlace allowed. Checked again on the values after pixel-clock rounding, because the hardware rarely hits the requested clock exactly. |
| 3. Hard rails | Compiled-in absolute limits per profile class (15, 25, 31 kHz) that no user setting or custom profile can widen without an explicit expert switch. A build step generates them: a class is a line-rate band (15 kHz class under 19 kHz, 25 kHz class from 19 to 28 kHz, 31 kHz class from 28 to 40 kHz), and its limits are the widest values among the shipped Switchres presets in that band. Above 40 kHz the limits come from the monitor's EDID. |
| 4. Confirm or revert | Any user-initiated change to output, profile or geometry applies for 10 seconds. No button press means automatic revert. |
| 5. Last known good | The last confirmed configuration is stored separately from user settings. Two boots that never reach a confirmed picture trigger safe mode. |
| 6. Blind recovery | Holding a fixed button combination for 5 seconds, at boot or at any time, restores the safe default for the detected output. An audio cue confirms it. |
| 7. Supervisor | The init system restarts the frontend on a crash, and the frontend's first action is to re-apply the last validated mode. A small launcher process holds the DRM file descriptor and passes it to the frontend. The kernel removes the framebuffers when the last holder closes that descriptor, and some drivers (amdgpu) then switch the CRTC off; with the launcher holding it, the mode stays on the wire across a restart or a re-exec. |
| 8. Silent kernel | The kernel sets no mode of its own: framebuffer console and fbdev emulation are off. On Pi GPIO outputs the first signal is then NEPTRON's validated mode. On x86 the firmware's 31 kHz mode stays on the wire until NEPTRON's first commit, because i915 inherits it. |

**Unknown monitor on first boot.** When no display is named in neptron.toml and none reports EDID, NEPTRON runs the blind probe described below, which starts at NTSC-timed 15.7 kHz, 240p. A 31 kHz monitor shows "out of range" harmlessly, while the opposite mistake can stress an old fixed-frequency chassis. The profile can also be preset in a text file on the boot partition before the first boot. A PC CRT that reports EDID over its VGA cable gets a profile built from its stated frequency ranges. For TVs the wizard offers profiles in the order the Batocera CRT guide recommends: arcade\_15, generic\_15, ntsc, then pal.

**What software cannot fix.** On x86 the BIOS/UEFI POST screen is emitted by firmware at 31 kHz before any NEPTRON code runs. The documentation and first-boot wizard must say this plainly for arcade-monitor owners. Raspberry Pi does not have this problem on GPIO outputs. Sync-protecting adapters such as the Ultimarc J-PAC block 31 kHz sync and belong in the hardware documentation.

**Shortening the 31 kHz window on x86.** NEPTRON cannot stop the firmware's boot screen, but it controls everything after it. When the active profile is a 15 kHz one, the launcher blanks the analog output (no sync at all) as its first action, about a second after the kernel starts, and the frontend then commits the 15 kHz mode. An unknown display is treated the same way, so the probe's first 15 kHz step replaces the firmware's signal directly. With a zero-timeout bootloader, the tube sees 31 kHz only for the firmware's own seconds. This is on by default. The documentation also recommends enabling the firmware's fast-boot option and, for arcade monitors, a sync-protecting adapter.

**First boot when the display is unknown.** A VGA-to-SCART cable or an arcade monitor reports nothing, so NEPTRON cannot know what is attached. Two paths cover it:

1. **Tell it beforehand.** `neptron.toml` on the FAT32 boot partition is a short, commented text file. Plug the USB stick or SD card into any PC, edit a few lines, and the first boot comes up in the right mode. The same file is the recovery path if a setting ever leaves the screen black.
2. **Blind probe.** With no file entry and no EDID display, NEPTRON shows a large "press any button if you can read this" screen at 15 kHz 240p for 15 seconds with a sound cue. If nobody presses, it shows the same screen at 31 kHz 480p for 10 seconds. A button press in either step makes that mode the profile, and the wizard continues there. A 31 kHz monitor shows "out of range" harmlessly during the 15 kHz step. The 31 kHz step adds no new exposure for a 15 kHz set, because the firmware has already sent the same on that connector.

The probe is the last resort. NEPTRON uses the first source that gives an answer: the `neptron.toml` file, then settings saved from an earlier boot, then EDID from any connector, then HDMI detection on a Pi. Only when all of those are silent does the probe run.

Three rules keep the probe safe:

- **31 kHz is never adopted without a button press.** Silence cannot tell an absent user from a 15 kHz set that cannot show the prompt, and parking 31 kHz on a 15 kHz tube for hours is the one outcome to avoid.
- **The resting state is no signal.** After three unanswered rounds the probe blanks the output and repeats only the sound cue. Any button press starts the rounds again. No signal is the one state that is safe for every display, including old monitors with no out-of-range protection. A 31 kHz monitor shows "out of range" harmlessly during the 15 kHz steps; the reverse is the riskier direction, which is why the 15 kHz step comes first and lasts longer.
- **Nothing unconfirmed is saved.** The next boot probes again. The probe restarts when a controller or keyboard is plugged in, and ends by itself when a display with EDID appears.

Owners of fixed-frequency arcade monitors should set `monitor` in the file and not rely on the probe, because even its short 31 kHz steps are unwelcome there.

On Raspberry Pi the GPIO DAC cannot be detected, and loading its overlay needs a line in the boot configuration, so the file is the way to select it. With no file entry, a Pi uses HDMI when a display is detected there and composite otherwise.

**Composite on a Pi at first boot** needs no file and carries no signal risk, because composite timings are fixed broadcast standards.

- Pi 3: composite is available alongside HDMI, so with no HDMI display detected NEPTRON simply uses it.
- Pi 4, Pi 5 and Zero 2 W: composite needs `enable_tvout=1`, which the firmware reads before NEPTRON starts and which switches HDMI off. With no HDMI display detected and no file entry, NEPTRON writes that line and reboots once, then comes up on composite. Setting `output = "Composite-1"` in the file skips the extra reboot.
- The standard: with `tv = "auto"` NEPTRON starts in NTSC timing at 60 Hz with a sound cue, and the wizard's first question is whether the picture is stable and in colour, with a one-button switch to PAL under confirm-or-revert. A wrong standard gives a black-and-white or rolling picture, never damage.
- Composite is never chosen when a GPIO DAC is named in the file. A DAC is never tried automatically: driving GPIO pins with an unknown board attached is the one probe NEPTRON does not make.

```toml
# NEPTRON first-boot settings. This partition is FAT32: edit it on any PC.
# Delete a line, or leave "auto", to let NEPTRON decide.

[display]
output  = "auto"       # auto | VGA-1 | HDMI-A-1 | DP-1 | DPI-1 | Composite-1
monitor = "auto"       # auto | arcade_15 | generic_15 | ntsc | pal | pc_31_120 | vesa_480 | flat
style   = "auto"       # auto | native | 2x | 3x | 4x
pi_dac  = "none"       # none | vga666 | rgb-pi | pi2scart   (Raspberry Pi only)
tv      = "auto"       # auto | pal | ntsc              (composite only)

[network]
wifi_ssid     = ""
wifi_password = ""
```

- Every value passes through the same validator as a menu choice. An unknown or unsafe value is ignored, the blind probe runs instead, and the reason appears on the diagnostics page.
- The file uses the same keys as the main settings file. It is read at every boot; when it has changed since the last boot, its values are copied into the saved settings once. Otherwise the settings made in the menus stand. The whole picture is under Configuration in the OS layer section.

* A file that cannot be parsed is ignored as a whole, with the line number of the error on the diagnostics page.
* Wi-Fi credentials are moved to the state partition on first use and blanked in the file, so a password does not stay readable on a FAT partition.

**Which output is used.** The output named in settings or in `neptron.toml` wins. With none named and one connector connected, that one is used. With several, an analog connector wins when the profile is a CRT, and the first digital one otherwise. A VGA connector that reports no EDID is used only when it is named, because the kernel cannot tell that a monitor is there. The blind probe is the one exception: with no other display found, it drives every analog connector without EDID, and any DisplayPort output that reports a VGA adapter but no monitor. When the display is unplugged the mode stays set; nothing is re-probed until the user opens the display panel.

**Burn-in protection.** A static menu left on a tube leaves a mark. After five minutes without input in any menu, in the quick menu or while paused, the picture dims to about a quarter brightness; any input restores it. The delay is a setting, and so is the style: dim (default), black screen, or off. It never triggers during play. The video mode does not change, so the monitor stays in sync.

**Tests that guard the rule**

- Property tests: for random source geometry and every shipped profile, the solver output always passes the validator.
- Golden tests: a fixed list of well-known systems must keep producing the same modes when Switchres or the policy code changes.
- Fuzzing of profile parsing and of the settings that feed geometry offsets.
