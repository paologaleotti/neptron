# kms-spike

Standalone hardware spike (WP 1.1). It shows 240p, 480i and 480p test modes for a named monitor preset, each for 10 seconds, and records what the driver programmed. The owner runs it on real hardware before the KMS backend is written.

The tool is the first NEPTRON code that drives a real CRT. Safety comes first: read "Safety behaviour" before the first run.

## Commands

```text
kms-spike <PRESET> [options]                                 dry run: plan and validate, set no mode
kms-spike <PRESET> [options] --i-have-checked-the-monitor    show each valid mode
kms-spike --list-presets                                     print the embedded preset names
kms-spike --list-connectors [--device PATH]                  list the connectors, set no mode
kms-spike --help
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--modes LIST` | `240p,480i,480p` | The test modes, in the order they show. Each once. |
| `--width PX` | `640` | Active pixels per line, 64 to 3840. |
| `--seconds N` | `10` | Display time per mode, 1 to 10. |
| `--allow-low-pclock HZ` | not set | Allow pixel clocks down to `HZ` (1 000 000 to 24 999 999). |
| `--allow-interlace` | not set | Allow interlaced modes. |
| `--device PATH` | `/dev/dri/card0` | The DRM device node. |
| `--connector NAME` | not set | The connector, for example `VGA-1` or `DP-1`. |
| `--report PATH` | `kms-spike-report.json` | The JSON report file. |

`--worker` is internal: the supervisor process adds it when it starts the worker (see below). The command line refuses `--worker` without `--i-have-checked-the-monitor`, and the worker does not open the device when its output is not the supervisor's pipe or when no mode passed `validate`.

### Build

The default build has no KMS code, so it builds on every host and runs the dry run. The KMS part needs Linux and the `kms` feature:

```sh
cargo build -p kms-spike --features kms
```

Use the dev profile, as above. The release profile of the workspace aborts on a panic, and then the restore guard cannot run (see "Exit paths").

### The run on the i3-6100T

Before the first run:

- Build the dev profile on Linux: `cargo build -p kms-spike --features kms`. The release profile of the workspace aborts on a panic, so the restore guard cannot run there.
- Run the tool as root on a text console. Stop X, Wayland and any running NEPTRON first: another DRM master makes every commit fail with "Permission denied".
- Run `--list-connectors` first and use the name it prints. On an i3-6100T board the VGA port is probably behind a DP-to-VGA bridge, so it shows up as a DisplayPort connector such as `DP-1`, not as `VGA-1`.
- After each run, keep the output of `dmesg | tail -50` with the report.

```sh
kms-spike --list-connectors
kms-spike pc_31_120 --connector DP-1                   # dry run: read the ranges and the modes
kms-spike pc_31_120 --connector DP-1 --i-have-checked-the-monitor
```

Use the connector name that `--list-connectors` printed in place of `DP-1`.

What to expect at the end: the console of a desktop kernel runs at a 31 kHz-and-higher mode, for example 1024x768, which fails validation for `pc_31_120`. So the tool switches the output off at the end, and it says so before the first modeset. With fbdev emulation the kernel console then takes the display back in its own mode when the tool closes the device. This is expected, and it is not a failure of the restore.

The SyncMaster 740s is a 31 kHz and higher PC monitor. On it, 240p shows at 120 Hz and 480p at 60 Hz. 480i cannot be tested on a 31 kHz monitor: it needs a preset with an interlaced range, and every shipped 31 kHz range and the 31 kHz hard rails refuse interlace. To measure Intel interlace and a native low pixel clock, use a 15 kHz preset on a 15 kHz display, for example:

```sh
kms-spike ntsc --connector DP-1 --width 640 --allow-low-pclock 6000000 --allow-interlace
```

### The run on `vkms`

`vkms` is a virtual display driver. It runs in the QEMU guest of the OS task. In the guest:

```sh
modprobe vkms
kms-spike --list-connectors --device /dev/dri/card0     # find the card with Virtual-1
kms-spike pc_31_120 --device /dev/dri/card0 --seconds 2 --i-have-checked-the-monitor
kms-spike ntsc --device /dev/dri/card0 --seconds 2 --allow-low-pclock 6000000 \
    --allow-interlace --i-have-checked-the-monitor --report vkms-ntsc.json
```

A pass on `vkms`:

- The exit code is 0, and the summary ends with `restore:` and no `RESTORE FAILED`.
- For every mode that passed `validate`, `test_commit.ok` and `commit.ok` are true in the report. An interlaced mode is the one exception: a refused 480i answers the open question "does `vkms` accept interlaced user modes", and it is not a failure of the tool.
- `readback.equals_request` is true, and `readback.validation.passed` is true.
- For a progressive mode, `vblank.rate_mhz` is within 1 % of the field rate of the mode. For an interlaced mode on `vkms` the 1 % check does not apply: `vkms` delivers vblanks at twice the field rate, so `rate_mhz` is near 2 times the field rate and `implied_pclock_hz` near 2 times the requested clock (open point OP-030).
- `flips_completed` is at least `flips_queued` less 1.
- `--list-connectors` after the run shows the connector in its state from before the run.

What the `vkms` run showed: the commits succeed, the readback equals the request, the restore succeeds, and Ctrl-C (SIGINT) or SIGTERM to the supervisor restores within 100 ms. After SIGKILL of the worker the CRTC stays active with its mode and no plane framebuffer.

What the `vkms` run cannot show:

- `vkms` has one fixed connector, so the connector choice between several displays does not run.
- It has no fbdev console. The "restore the original mode" path and the console takeover after an "output off" restore never run there.
- Its vblank timing comes from a kernel timer, not from a scanout clock. The measured rate shows that the tool counts vblanks correctly, not what a real GPU makes.
- It accepts interlace and any low pixel clock, with no hardware limit. A test-only commit that passes on `vkms` says nothing about i915.
- In an interlaced mode it delivers vblanks at twice the field rate (open point OP-030). For 480i in `generic_15` (field rate 59.999 Hz) it gave `rate_mhz` 120227 and `implied_pclock_hz` 26 352 255 against a requested 13 151 000. The interlaced vblank timing of real hardware stays unmeasured.
- Under QEMU the flip loop completed about one flip per 2 vblanks, so `flip_cadence` came out `other` for progressive modes.

On a PC, `generic_15` and the other 15 kHz presets give no valid mode with the default GPU values: at the default width of 640 pixels every 15 kHz mode is below the conservative minimum pixel clock of 25 MHz. Add `--allow-low-pclock HZ` to show them, or choose a much larger `--width`.

A note for scripted runs: a shell starts a background job (`kms-spike ... &`) with SIGINT ignored, so `kill -INT` does not stop it. Use SIGTERM, or run the tool in the foreground.

## Safety behaviour

1. **Type gate.** A mode reaches the kernel only as a `KernelMode`, and `KernelMode::from_validated` takes a `ValidatedMode`. Only `neptron_safety::validate` makes one, against the chosen preset and the GPU values. No flag skips it. A mode that fails is printed with its rejection and is skipped. The kernel takes the pixel clock in kHz, so a validated clock that is not a whole number of kHz is refused, never rounded.
2. **Readback check.** After each commit the tool reads the mode back and validates it again. A read-back mode that fails, or that cannot be read, stops the run and restores at once.
3. **Preset.** There is no default preset. The presets of `data/monitors` are embedded at build time (see "Presets"). The tool prints the ranges of the preset before anything else.
4. **Confirmation.** Without `--i-have-checked-the-monitor` the run is a dry run. It prints the modes it would set and their validation results, writes the report, and opens no device.
5. **GPU values.** `validate` gets the values of an unknown GPU: 25 MHz to 165 MHz pixel clock, no interlace, at most 1920 by 1200. Only `--allow-low-pclock HZ` and `--allow-interlace` widen them, and only those two values. The ranges of the preset and the hard rails still apply. A mode outside the conservative values is marked with a line that says why and which flag allows it.
6. **Test-only commit.** Each mode goes to the kernel first with `DRM_MODE_ATOMIC_TEST_ONLY`. When the test fails, the tool prints the kernel error and skips the mode.
7. **Connector.** A named connector is used in any state, because a VGA monitor often reports nothing. With no name, the one connected connector is used. With none or several, the tool stops and asks for a name; it never guesses. A CRTC that also drives another connector is refused, because that display would get the test modes too.
8. **Time limits.** Each mode shows for at most its time (10 seconds, or less with `--seconds`). The worker stops showing modes after the number of modes times the time per mode, plus 15 seconds. The supervisor kills the worker 10 seconds after that. With 3 modes of 10 seconds, the hard limit is 55 seconds. No wait in the worker is unbounded: the vblank wait has the kernel's own timeout, and the device is opened non-blocking, so the event read never waits.

### Exit paths

At the start the worker saves the state of the connector, the CRTC and the primary plane: the connector's `CRTC_ID`, the CRTC's `ACTIVE` and mode, and the plane's `FB_ID`, `CRTC_ID` and source and destination rectangles. The restore puts them back with an atomic commit, after a test-only commit.

The original mode also goes through `validate`. When it passes, the restore puts back the original mode and the original framebuffer. When it does not convert to a modeline or does not pass `validate` for the chosen preset, the restore switches the output off instead (no mode, no plane): no signal is safe for every display. The tool prints which of the two it will do before the first modeset. After an "output off" restore on a kernel with fbdev emulation, the kernel console takes the display back in its own mode when the tool closes the device. The owner then sees the console again, as before the run; this is expected. Without fbdev emulation the screen stays dark until the next program sets a mode. When the restore commit fails, the tool switches the output off; when that fails too, it prints a large `RESTORE FAILED` block.

| Exit path | What happens |
| --- | --- |
| Normal end | The worker restores, writes the report and prints the summary. |
| Error in a mode | The mode is recorded and skipped. The run goes on, then restores. |
| Error before the first modeset | Nothing to restore. The error goes to the report. |
| Panic in the worker | A guard restores while the stack unwinds (dev profile). |
| Ctrl-C, SIGTERM, SIGHUP to the supervisor | The supervisor dies. The worker's next write to the pipe fails within 100 ms, and the worker restores and exits. |
| Worker past the hard limit | The supervisor kills it. See SIGKILL. |
| SIGKILL of the worker, or a signal sent to the worker's own process id | No code runs. See below. |

**Why two processes.** A signal handler needs `unsafe` code or a crate such as `signal-hook`, and the tool may use neither (see "Dependencies"). The process the user starts is a supervisor. It starts the worker (the same program with `--worker`) in a new process group, so Ctrl-C at the terminal reaches only the supervisor. The worker writes a heartbeat line every 100 ms to a pipe that the supervisor reads. When the supervisor dies, the pipe has no reader, the write fails (Rust ignores SIGPIPE, so it is an error, not a signal), and the worker restores. The worker writes its messages after that to standard error, which still goes to the terminal. If the terminal has `stty tostop` set, the kernel stops the background worker when it writes; the supervisor's hard limit then kills it.

**SIGKILL.** No code of the tool runs. The kernel closes the worker's DRM file descriptor and removes the framebuffers that the worker made. What the display does then depends on the driver. On `vkms` the CRTC stays active with its mode and no plane framebuffer. On some drivers (i915 is a probable case) the same happens, so the validated test mode can stay on the wire with a black picture until the fbdev console takes the display back at the last close. Other drivers switch the CRTC off. The supervisor prints this note whenever a signal ends the worker. It is not verified on hardware; the hardware checklist has a check for it. The test mode on the wire passed `validate`, so it is inside the ranges of the preset. To recover: switch to another virtual terminal and back, run the tool again with a valid preset, or reboot.

## The modes

For each test mode the tool derives one modeline from the preset. The derivation only proposes; `validate` judges.

1. It takes the first range of the preset whose line limits accept the mode: 240 progressive lines, 480 interlaced lines, or 480 progressive lines.
2. It tries the field rates 60 Hz and 120 Hz when the range allows them, then the allowed rate nearest to 60 Hz. So 240p is 120 Hz on a 31 kHz range, and the fixed `ntsc` and `pal` ranges give 59.94 Hz and 50 Hz.
3. It takes the smallest vertical total whose line rate is in the range and whose blanking holds the vertical porches and sync of the range. The extra lines go to the porches.
4. It takes the smallest horizontal total that holds the width and the horizontal porches and sync of the range. Each time is rounded up to whole pixels. It then tries up to 256 larger totals until the pixel clock, in whole kHz, puts both rates inside the range.

When no range of the preset accepts the mode, the timings come from a built-in reference: NTSC 15 kHz for 240p and 480i, VGA 31 kHz for 480p. `validate` then rejects the mode and says why. The width is the same for every mode (`--width`). A narrow width at 15 kHz gives a low pixel clock, which the conservative GPU values refuse.

## The picture

The CPU draws the pattern into two dumb buffers, made once at the largest size of the valid modes. Each mode shows the top-left part of them through the plane's source rectangle. The pattern:

- a white border on the outermost pixels of the active area, to check the geometry and the overscan;
- a centre cross;
- eight colour bars at 75 %;
- the mode label, for example `240P 640X240 120.00HZ`, at the top left, scaled for the size of the mode.

The tool flips between the two buffers once per vblank to measure the page-flip events. Both buffers hold the same picture, so the flips are not visible.

## The report

The report is one JSON file, written through a temporary file and a rename. `report_version` is 2. Version 2 adds `event_rate_mhz` to the timing statistics; in version 1, `rate_mhz` counted the samples and came out too low when the loop missed vblanks. The format is append-only: a new field gets a new version, and no field is renamed or removed. The summary on the terminal shows the same data in short form.

| Field | Meaning |
| --- | --- |
| `preset`, `dry_run`, `width`, `seconds_per_mode` | The run. |
| `gpu_caps`, `allowed` | The values `validate` got, and the flags that widened them. |
| `system` | Device node, driver name, version and description, kernel release. |
| `output` | Connector name, id and state; encoder, CRTC (and its index), primary plane. |
| `original` | The CRTC state before the run, any problem with its mode, and the restore plan. |
| `framebuffer` | Format (`XR24`), modifier, count, size and pitch of the buffers. A dumb buffer is linear; the tool gives no modifier. |
| `modes[].requested` | The modeline, with `line_rate_mhz` and `field_rate_mhz`. |
| `modes[].validation` | `passed`, and the rejection text when it failed. |
| `modes[].beyond_conservative` | Why the mode is outside the values of an unknown GPU. |
| `modes[].kms.test_commit`, `.commit` | `ok`, and the kernel error text when it failed. |
| `modes[].kms.readback` | The mode the kernel stores for the CRTC, `equals_request`, the fields that differ, and the second validation. |
| `modes[].kms.vblank` | Vblank timing: `count` of samples; `rate_mhz`, the rate of the vblank events that the driver delivers (the vblanks the kernel counted over the time span, so a sample that the loop misses does not lower it); `mean_interval_ns`, the mean time per vblank; the shortest and longest time between two samples; `jitter_ns`, the largest distance from the mean of one interval divided by the vblanks it spans; `skipped_vblanks`, vblanks with no sample; `event_rate_mhz`, the rate of the samples themselves. On hardware `rate_mhz` is the field rate; for an interlaced mode that is not verified on hardware, and `vkms` delivers vblanks at twice the field rate there. |
| `modes[].kms.flips` | The same for the page-flip events. Here `event_rate_mhz` is the flip rate. |
| `modes[].kms.flips_queued`, `.flips_completed` | Flips asked for and flip events received. The difference is the missed flips. |
| `modes[].kms.flip_cadence` | `per_field`, `per_frame` or `other`: the flip rate (`flips.event_rate_mhz`) against the field rate, within 2 %. In an interlaced mode this says whether flips come once per field or once per frame. Read it together with `flips.skipped_vblanks`: when the flip loop misses vblanks (on `vkms` under QEMU it completed about one flip per 2 vblanks, so progressive modes came out `other`), a 480i result of `per_frame` is not proof of one flip per frame. |
| `modes[].kms.implied_pclock_hz` | The pixel clock that the measured vblank rate implies. The kernel's readback gives the requested clock, not the clock the hardware makes, so compare this value with `requested.pclock_hz` to see the rounding of the driver. |
| `modes[].kms.kernel_errors` | Kernel errors during the display time, each once with a count. |
| `restore` | `ok`, and what was restored or what failed. |
| `stopped`, `errors` | Why the run stopped early, and errors outside one mode. |

The kernel log can hold more detail than the error code. After a run on hardware, keep the output of `dmesg | tail -50` with the report.

## Presets

The build script reads every file in `data/monitors` with the serde types of `neptron-types` and writes it as Rust constants. The binary then needs no TOML parser, which the dependency rules do not allow, and a preset that does not parse fails the build. A custom monitor file is not supported. The preset files are licensed GPL-2.0-or-later (see `data/monitors/README.md`), so a binary of this tool contains GPL-2.0-or-later data. The `license` field of the crate says so: `(MIT OR Apache-2.0) AND GPL-2.0-or-later`. The tool is a development aid and is not shipped.

## Dependencies

`xtask rules` allows `drm`, `neptron-types` and `neptron-safety` as normal dependencies, and nothing else.

- `drm` (optional, Linux only, `kms` feature): the safe binding of DRM/KMS.
- `neptron-types`, `neptron-safety`: the modeline, the profile and the validator.
- Build dependencies `neptron-types` and `toml`: the build script that embeds the presets.

So the tool parses its arguments by hand, writes JSON with a small writer of its own, implements `Display` for its errors by hand (no `thiserror`), and handles Ctrl-C with two processes instead of a signal handler. It contains no `unsafe` code.

## Must never depend on

- Any crate except `drm`, `neptron-types` and `neptron-safety` (normal dependencies).

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `kms`. No feature is on by default.

## Tests

The pure parts are tested on every host: argument parsing, the embedded presets, the mode derivation (every preset, every mode, three widths; each mode from a preset range passes `validate`), the GPU values, the plan, the kernel mode record and its conversions, the connector choice, the pattern, the timing statistics, the JSON writer and the report, the refusal of `--worker` without confirmation, without the supervisor's pipe or without a valid mode, and the supervisor with a shell script as the worker. The DRM code (`src/kms`) builds only on Linux with `--features kms`, and the workspace gate does not build it.
