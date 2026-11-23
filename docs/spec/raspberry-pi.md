# Raspberry Pi image and composite video

This spec describes the Raspberry Pi image of NEPTRON and its composite video output: the boards, what the composite encoder of the kernel gives, how a fixed broadcast standard fits the ODREN video model, how sharp a composite picture can be, how the SD card image is built, and how it is verified (OP-096). Everything in it is built; the bench session (P09) is left (section 7).

## The owner's decisions

- Composite is the default output of the Pi image: NTSC at 60 Hz. The user can change the standard to PAL at 50 Hz, PAL at 60 Hz and every other combination that the hardware gives.
- The picture must be as sharp as composite allows and must work perfectly. A soft image is part of the charm of composite, but the result must be better than RetroPie, which "looked pretty OK".
- The test is a 15 kHz tube on composite.
- The first image targets the Raspberry Pi 3 B and 3 B+. The checklist names a 3 B+; the measurements of 2026-11-14 come from a Pi 3 B. The Zero 2 W is in the same image only if it costs nothing extra; it is not a goal.
- HDMI is in the first image, as a flat panel through the existing panel path.
- The GPIO DACs (VGA666, RGB-Pi, Pi2SCART) are not in the first image. They stay an open point, or the user edits a configuration file on the SD card before the boot.

## How to read the marks

Every technical statement carries one mark:

- **VERIFIED**: read in the named source, at the named version, on 2026-11-13. A statement about the code of the kernel is about the source text. It is not a hardware result.
- **UNVERIFIED**: from a secondary source, from reasoning, or not yet seen on a Pi. Each one is a line of the hardware checklist or an item of a build task.
- **COMPUTED**: arithmetic on verified values. The task that writes the code proves it with a test.

The kernel versions named below are these:

| Name | Version | Where it comes from |
| --- | --- | --- |
| Mainline 6.18 | `torvalds/linux` tag `v6.18` | kernel.org tree |
| Pi fork 6.12 | `raspberrypi/linux` commit `21b41014`, Linux 6.12.61 | The kernel that Buildroot 2026.08.1 pins for every Pi defconfig (VERIFIED: `configs/raspberrypi3_64_defconfig` in `/opt/buildroot` of the dev image, and the `Makefile` of that commit) |
| Pi fork 6.18 | `raspberrypi/linux` branch `rpi-6.18.y`, Linux 6.18.62 on 2026-11-13 | The default branch of the Pi fork (VERIFIED: GitHub repository metadata and the branch `Makefile`) |

The file `drivers/gpu/drm/vc4/vc4_vec.c` is the same in the Pi fork 6.12 and the Pi fork 6.18, except one line of the platform driver structure (VERIFIED: `diff` of the two files).

## Summary

1. One 64-bit image, `rpi-arm64`, for the Pi 3 B and 3 B+. It holds the Zero 2 W files too, untested. The Pi 4 is the next step and the Pi 5 the step after it.
2. The kernel is the Raspberry Pi fork, branch `rpi-6.18.y`, at the pinned commit `ea17ae02ca28`. The definition already decides this tree (`15-decisions-and-open-points.md`). No 15 kHz patch is in the Pi kernel.
3. Composite is a new kind of monitor profile: a TV standard. `MonitorKind::Tv { standard }` holds one of ten standards. Each standard has a fixed table of two modes: 240p and 480i for the 525-line standards, 288p and 576i for the 625-line standards. Switchres is not used for it.
4. The validator accepts a mode on a TV profile only when it is one of the two modes of the table, field for field, and when it also passes the 15 kHz hard rails. `ValidatedMode` carries the standard. The KMS backend sets the connector property `TV mode` in the same atomic commit as the mode.
5. A change of standard is a change of the monitor. The guard puts it under confirm-or-revert with the 15 s timed revert (`CONFIRM_WINDOW_NS`), keeps it as last known good, and the blind recovery goes back to NTSC.
6. The policy has a small planner for a TV profile: progressive 240p or 288p for every game up to 240 or 288 lines (centred with black lines, or with the existing crop of at most 8 lines per edge), interlaced 480i or 576i for taller games, and the filtered fit last. The game runs at the encoder's field rate, and the audio run rate is the field rate that the driver reads back. A difference up to 5000 ppm gives no alert; a larger one shows the speed alert, as on a flat panel.
7. Horizontally every mode has 720 samples at 13.5 MHz. The game gets the width of its display aspect (640 samples for a 256-wide or 320-wide console), snapped to a whole or half factor of its width when one is within 3 %, with nearest at a whole factor and the existing sharp bilinear filter otherwise. No line is ever scaled vertically.
8. The settings are `display.tv` (the standard, default NTSC) and `display.tv_width` (`aspect`, `integer`, `fill`). Monochrome (colour burst off) is one of the standards: it gives the sharpest luma.
9. The output rule: an HDMI display that answers wins; with none, the composite connector is used; a name in `display.output` wins over both (or `neptron.toml`). The first run of a boot chooses the output, and every relaunch of that boot keeps it (`run/output`). On a Pi 3 the two outputs share one pixel valve, so only one is driven at a time.
10. The SD card has the PC layout: FAT32 boot (firmware, kernel, `config.txt`, `neptron.toml`), EROFS root, ext4 state, and the exFAT share that the first boot makes as `mmcblk0p4` with the same storage script.
11. CI builds the image without a QEMU boot and uploads it. The aarch64 QEMU image keeps testing the frontend. The hardware checklist holds the checks of the board.

## 1. Boards

### Facts per board

| Board | SoC, CPU, GPU | 64-bit | KMS display driver | Composite connector | Composite turned on by | HDMI and composite at once |
| --- | --- | --- | --- | --- | --- | --- |
| Pi 3 B, 3 B+ | BCM2837, Cortex-A53, VideoCore IV | Yes, `bcm2711_defconfig` and `kernel8.img` (VERIFIED: [Pi kernel docs](https://www.raspberrypi.com/documentation/computers/linux_kernel.html)) | `vc4` (HVS, pixel valves, VEC) | 3.5 mm 4-pole AV jack (VERIFIED: [`config_txt/video.adoc`](https://github.com/raspberrypi/documentation/blob/master/documentation/asciidoc/computers/config_txt/video.adoc), "All other models") | `dtoverlay=vc4-kms-v3d,composite` (VERIFIED: same file, and the overlay README of the Pi fork 6.12). `enable_tvout` defaults to 1 on these boards (VERIFIED: same file) | No. Both connectors exist, but HDMI and the VEC share pixel valve 2 (VERIFIED: `bcm2835_pv2_data` in `vc4_crtc.c`, Pi fork 6.12). One of the two is driven at a time |
| Zero 2 W | BCM2710A1, Cortex-A53, VideoCore IV | Yes, the same kernel | `vc4` | Test pads on the underside (VERIFIED: `config_txt/video.adoc`) | `enable_tvout=1`, which turns HDMI off (VERIFIED: same file, "Zero models") | No |
| Pi 4 B | BCM2711, Cortex-A72, VideoCore VI | Yes, `kernel8.img` | `vc4` for display, `v3d` for 3D | 3.5 mm AV jack | `enable_tvout=1`, which turns HDMI off (VERIFIED: same file). The overlay parameter `composite` "Disables all other outputs on a Pi 4" (VERIFIED: overlay README, Pi fork 6.12) | No |
| Pi 5 | BCM2712 with RP1, Cortex-A76, VideoCore VII | Yes; the firmware loads `kernel_2712.img` and falls back to `kernel8.img` (VERIFIED: Pi kernel docs) | `vc4` for HDMI, `v3d` for 3D, `drm-rp1-vec` for composite | `J7` pad next to the HDMI socket (VERIFIED: `config_txt/video.adoc`) | The RP1 VEC overlay (UNVERIFIED: the exact overlay line) | Yes: the Pi docs say that composite turns HDMI off "for all models except Raspberry Pi 5" (VERIFIED: [`configuration/audio-display.adoc`](https://github.com/raspberrypi/documentation/blob/master/documentation/asciidoc/computers/configuration/audio-display.adoc)) |

**The jack.** The Pi 3 jack is a 4-pole (TRRS) jack. The common pinout is: tip left audio, first ring right audio, second ring ground, sleeve composite video (UNVERIFIED: two vendor pages, [pinouts.ru](https://pinouts.ru/HeadsetsHeadphones/Raspberry_pi_composite_vi.shtml) and the [Pi Hut cable](https://thepihut.com/products/adafruit-a-v-and-rca-composite-video-audio-cable-for-raspberry-pi); the Pi documentation does not give the pinout). Camcorder and other TRRS-to-RCA cables put ground or video on other rings, so the yellow plug of such a cable often carries no video. Use a cable sold for the Raspberry Pi, or check the sleeve with a meter.

**The kernel driver of the VEC.** The VEC is a part of `vc4` on the Pi 3, Zero 2 W and Pi 4 (VERIFIED: `vc4_vec.c` has a BCM2835 and a BCM2711 variant, mainline 6.18). The Pi 5 has a different encoder in RP1, with its own driver `drivers/gpu/drm/rp1/rp1-vec` that exists only in the Pi fork (VERIFIED: Pi fork 6.18). That driver accepts pixel clocks from 6.75 MHz to 21.6 MHz and any mode whose frame time fits the 525-line or 625-line system (VERIFIED: `rp1vec_mode_valid` in `rp1_vec.c`, Pi fork 6.18). So the Pi 5 composite path is close to the CRT path of the PC (Switchres with a narrow range), not to the fixed table below. That is a reason to treat the Pi 5 as its own later step.

### Recommendation

| Board | First image | Why |
| --- | --- | --- |
| Pi 3 B, 3 B+ | Yes, tested on a Pi 3 B | The owner's board. Composite needs no reboot and no firmware switch. HDMI and composite live on one card with no change to `config.txt`. vc4 GLES 2 and its limits are already in the hardware checklist |
| Zero 2 W | Files in the image, not tested, no checklist | Same SoC family and the same kernel and `vc4` driver. The cost is one device tree file and a `[pi02]` section in `config.txt` with `enable_tvout=1`. But it has no Ethernet (the network share needs a USB adapter or Wi-Fi), its composite is on test pads, it has 512 MB, and its audio needs HDMI, USB or a GPIO remap (see "Audio"). It is never a target until someone tests it |
| Pi 4 | Next image step | Same `vc4` VEC and the same fixed table. It needs the "write `enable_tvout=1` and reboot once" path of the definition, `start4.elf`, and `v3d` in Mesa |
| Pi 5 | After the Pi 4 | A different encoder and driver (RP1), a pad instead of a jack, and a variable pixel clock that needs its own policy work |

One 64-bit image serves all four boards in the end: the kernel `bcm2711_defconfig` runs on the Pi 3, 3 B+, Zero 2 W, 4 and 5 (VERIFIED: Pi kernel docs), and the firmware picks the device tree by model. The Cortex-A53 userland of the definition runs on all of them.

## 2. The composite encoder in the kernel

### Mainline against the Pi fork

| Feature | Mainline 6.18 | Pi fork 6.18 | Source |
| --- | --- | --- | --- |
| Connector type and name | `DRM_MODE_CONNECTOR_Composite`, `Composite-1` | The same | VERIFIED: `vc4_vec_connector_init`; the name `Composite-1` in [`audio-display.adoc`](https://github.com/raspberrypi/documentation/blob/master/documentation/asciidoc/computers/configuration/audio-display.adoc) |
| Hot plug detection | Always `unknown` | The same | VERIFIED: `vc4_vec_connector_detect` returns `connector_status_unknown` |
| Listed modes | 480i and 576i from `drm_connector_helper_tv_get_modes` | The same, plus `720x240` and `720x288` progressive | VERIFIED: `vc4_vec_connector_get_modes`, Pi fork only |
| Progressive scan | Yes, since v6.3 (commit `71c541ebe82b`, "drm/vc4: vec: Support progressive modes"); a mode without the interlace flag sets `VEC_CONFIG2_PROG_SCAN` | The same | VERIFIED: v6.2 has no `PROG_SCAN` use in the encoder, v6.3 has it |
| Generic `TV mode` property | Yes, since v6.3 (commit `91112a6fc12c`) | The same | VERIFIED: v6.3 source |
| PAL-60 | Yes, since v6.3 | The same | VERIFIED |
| `Mono` (burst and chroma off) | Yes, in v6.12 (commit `53c5da8573ba`); v6.10 has none | The same | VERIFIED: v6.10 and v6.12 sources |
| Margin properties | Attached in v6.12 (commit `fbe675b3696e`) | The same | VERIFIED |
| Default standard from the command line | `tv_mode=` of `video=` only | Also `vc4.tv_norm=`, and PAL when the `video=` mode looks like PAL | VERIFIED: `vc4_vec_get_default_mode`, Pi fork only |

**Which tree.** The project uses the Pi fork, branch `rpi-6.18.y`, pinned to a commit. Reasons:

- The definition decides it (`15-decisions-and-open-points.md`, "Kernel").
- Mainline has `vc4` and the VEC, but the Pi boards need more of the fork: the firmware overlays (`vc4-kms-v3d`) are written against the fork's device trees, the analog audio driver of the jack is in the fork, and the RP1 drivers of the Pi 5 are only in the fork.
- 6.18 is the line of the PC kernel (6.18.63). One line keeps the kernel headers, the DRM uAPI and the known behaviour the same on both images.
- Buildroot 2026.08.1 pins the Pi fork 6.12.61 in its own Pi defconfigs. NEPTRON does not use that pin. Nothing that this spec needs from the VEC differs between 6.12 and 6.18.

The progressive modes do not depend on the fork's listed `720x240` and `720x288`: NEPTRON commits its own mode blob, and the encoder's atomic check is the same in both trees.

### The TV standards

The encoder has one table of standards. Each row is a value of the generic `TV mode` property and a horizontal total. The horizontal total selects the line system: 858 is the 525-line system, 864 is the 625-line system (VERIFIED: `vc4_vec_tv_modes` and `vc4_vec_tv_mode_lookup`, mainline 6.18).

| NEPTRON name | `TV mode` value | Lines | Encoder standard and subcarrier | Notes |
| --- | --- | --- | --- | --- |
| `ntsc` | `NTSC` | 525 | NTSC, pedestal on (`VEC_CONFIG0_PDEN`) | 7.5 IRE setup, the US standard |
| `ntsc_j` | `NTSC-J` | 525 | NTSC, no pedestal | 0 IRE setup, the Japanese standard |
| `ntsc_443` | `NTSC-443` | 525 | NTSC with a custom 4.43 MHz subcarrier | For PAL sets and VCRs that decode NTSC 4.43 |
| `pal_60` | `PAL` | 525 | PAL-M timing with a custom 4.43 MHz subcarrier | The row is marked `/* PAL-60 */` in the source |
| `pal_m` | `PAL-M` | 525 | PAL-M (Brazil) | |
| `pal` | `PAL` | 625 | PAL B, D, G, H, I | |
| `pal_n` | `PAL-N` | 625 | PAL-N (Argentina, Uruguay, Paraguay) | |
| `secam` | `SECAM` | 625 | SECAM | |
| `mono_60` | `Mono` | 525 | PAL-M timing, burst and chroma off | Black and white |
| `mono_50` | `Mono` | 625 | PAL timing, burst and chroma off | Black and white |

All rows are VERIFIED in `vc4_vec.c`, mainline 6.18. No other combination exists: there is no 625-line NTSC and no 525-line SECAM row, and `vc4_vec_tv_mode_lookup` refuses any other pair (`-EINVAL`). PAL-60 is the generic value `PAL` with a 525-line mode: the property alone does not tell PAL from PAL-60. The kernel names of the property values are `NTSC`, `NTSC-443`, `NTSC-J`, `PAL`, `PAL-M`, `PAL-N`, `SECAM` and `Mono`, and the property is named `TV mode` (VERIFIED: `drm_tv_mode_enum_list` and `drm_mode_create_tv_properties` in `drm_connector.c`, mainline 6.18; the [DRM documentation of analog TV properties](https://docs.kernel.org/gpu/drm-kms.html#analog-tv-connector-properties)).

### What a mode must be

The encoder's atomic check (`vc4_vec_encoder_atomic_check`, VERIFIED, mainline 6.18):

| Rule | 525 lines (`htotal` 858) | 625 lines (`htotal` 864) |
| --- | --- | --- |
| Horizontal total | Exactly 858 | Exactly 864 |
| Active width | A multiple of 4 | A multiple of 4 |
| Horizontal sync | Not empty | Not empty |
| Vertical total per field (`crtc_vtotal`) | At most 262 | At most 312 |
| Active lines per field | 1 to 253 | 1 to 305 |
| Vertical sync | Exactly 3 lines, after at least 1 line of front porch | The same |
| Lines after the sync | At least 4 | At least 2 |

- **The pixel clock is fixed.** The check does not read the clock. The enable path sets the VEC clock to 108 MHz, 8 times 13.5 MHz (VERIFIED: `clk_set_rate(vec->clock, 108000000)` in `vc4_vec_encoder_enable`). Every listed mode is 13 500 kHz. So the line rate is 15 734.266 Hz on 525 lines and 15 625 Hz on 625 lines (COMPUTED). A mode with another clock passes the check, but the pixel valve then reads at a rate that the encoder does not use (UNVERIFIED: what the TV gets). NEPTRON sends 13 500 kHz only, a whole number of kHz (ADR 0009 holds).
- **Custom modes.** Any mode that passes the check is accepted, also one that is not listed (VERIFIED: the check is the only gate). So the active width and lines can vary inside the total. What the encoder makes of an active width other than 720 is not documented: `VEC_CONFIG3_HORIZ_LEN_STD` is set for every mode (VERIFIED), which suggests a standard horizontal timing (UNVERIFIED). NEPTRON uses only the four modes of its table, with 720 active samples.
- **Interlace.** An interlaced mode is accepted. For it, `crtc_vtotal` is the field total (262 for 525 lines), so 480i passes the same row as 240p (VERIFIED: the check uses the `crtc_` values, which DRM halves for interlace).
- **The highest progressive line count** is 262 lines per field at 525 and 312 at 625. So the progressive rates are fixed too: 60.054 Hz and 50.080 Hz (COMPUTED). A 263-line field, which would give 59.826 Hz, is refused.

### The four modes NEPTRON uses

| Mode | Timing (clock in kHz; h: active, sync start, sync end, total; v: the same) | Field rate | Source of the values |
| --- | --- | --- | --- |
| 525, 240p | 13 500; 720 734 798 858; 240 243 246 262 | 60.054 Hz (13 500 000 / 224 796) | VERIFIED: `drm_mode_240p` of the Pi fork 6.18 |
| 625, 288p | 13 500; 720 740 804 864; 288 290 293 312 | 50.080 Hz (13 500 000 / 269 568) | VERIFIED: `drm_mode_288p` of the Pi fork 6.18 |
| 525, 480i | 13 500; 720 736 ... 858; 480 ... 525, interlaced | 59.940 Hz | VERIFIED in part: `drm_test_modes_analog_tv_ntsc_480i` in `drivers/gpu/drm/tests/drm_modes_test.c` (mainline 6.18) checks 720, 736, 858, 480, 525. The rest comes from `drm_analog_tv_mode()` |
| 625, 576i | 13 500; 720 732 ... 864; 576 ... 625, interlaced | 50.000 Hz | VERIFIED in part: `drm_test_modes_analog_tv_pal_576i`, the same way |

The task that writes the table takes the full 480i and 576i values from `drm_analog_tv_mode()` of the pinned kernel, and checks them against the mode list that `modetest` prints on the Pi (UNVERIFIED until then). The rates are COMPUTED from the totals. Each of the four modes lies inside the 15 kHz hard rails of `neptron-safety` by these numbers: horizontal sync 4.74 us, horizontal back porch 4.44 us (the rail is 90 % of 4.7 us, 4.23 us), vertical sync 3 lines (191 us at 525, 192 us at 625), vertical back porch 16 or 19 lines (COMPUTED; the task's test proves it).

### How a standard change applies

- The standard is a connector property. A change of it alone sets `mode_changed` on the CRTC (VERIFIED: `drm_atomic_helper_connector_tv_check`, mainline 6.18), so it is a full modeset: the commit needs `DRM_MODE_ATOMIC_ALLOW_MODESET`, and the picture goes off for a moment.
- No reboot is needed on the Pi 3. A reboot is needed only to turn the VEC itself on or off on the boards where the firmware does that (`enable_tvout` on the Zero 2 W and the Pi 4).
- A change of a margin property sets `connectors_changed` (VERIFIED: the same helper).

### Margins and overscan

- `drm_connector_attach_tv_margin_properties` adds `left margin`, `right margin`, `top margin` and `bottom margin`, each 0 to 100 (VERIFIED: `drm_mode_create_tv_margin_properties`, mainline 6.18). In `vc4` the margins shrink the plane in the HVS scaler (UNVERIFIED: read in `vc4_plane.c` is still to do). A scaled plane resamples the lines, which breaks one game line per scan line. NEPTRON sets every margin to 0 in each modeset and does any overscan work in its own placement.
- The encoder has no overscan setting of its own. The 720 samples are wider than the visible line of a TV: about 704 samples hold the 4:3 picture in the ITU-R BT.601 practice (UNVERIFIED for a given TV).

### What the encoder does not offer

- No luma notch or chroma filter control. The register fields `YCDELAY`, `CYDELAY`, `PBPR_FIL` and `CBURST_GAIN` are defined in the driver, but no table row uses them (VERIFIED: mainline 6.18). A kernel patch could expose them; this spec does not ask for one.
- No brightness, contrast, saturation or hue: the driver attaches none of the generic TV properties for them (VERIFIED: only `TV mode`, the legacy `mode` and the margins are attached).
- The colour subcarrier phase is fixed at `0x28` (VERIFIED: `VEC_SCHPH`).

### Known defects and risks

- A composite connector never reports a display (`unknown`), so nothing can detect a TV. The output rule below handles it.
- On the Pi 3 the VEC and HDMI share pixel valve 2 (VERIFIED). A commit that drives both fails its test-only step; NEPTRON drives one connector only.
- The page flip events of an interlaced mode on `vc4` (one per field or one per frame) are not known (UNVERIFIED; OP-030). The matrix entry says `flip_per_field = false`, and the session reads it to count missed fields in an interlaced mode (`crates/neptron-session/src/session/frame.rs`). This touches 480i and 576i only, not 240p and 288p.
- The firmware drives composite with its own standard before Linux starts (`enable_tvout` defaults to 1 on the Pi 3). That is harmless: it is a broadcast standard (UNVERIFIED: what it shows with `disable_splash=1`).

## 3. A TV standard in the NEPTRON video model

### The problem

ODREN asks Switchres for a modeline from the ranges of a monitor profile, with a free pixel clock in whole kHz (ADR 0009). The composite encoder has a fixed clock, a fixed horizontal total, and two line totals per standard. Switchres cannot calculate for it: a CRT profile on `pi_vc4_composite` gets no mode (OP-047; `crates/neptron-modeline/tests/golden/policy_gpus.toml`).

### Design

ADR 0012 records the design. A TV is a **monitor kind**, not an output kind. `OutputKind` (ADR 0010) picks the GPU limits, and composite keeps `Analog`. The set of modes a display accepts is the business of the monitor profile, and the guard and the last known good already work on the monitor. So a standard change rides on everything that exists.

**`neptron-types`**

- `TvStandard`: `Ntsc`, `NtscJ`, `Ntsc443`, `Pal60`, `PalM`, `Pal`, `PalN`, `Secam`, `Mono60`, `Mono50`. Methods: `lines()` (525 or 625), `kms_name()` (the `TV mode` value of the table above), `key()` (the settings value), `modes()` (the progressive and the interlaced `Modeline` of its line system).
- Four constants: `TV_525_240P`, `TV_525_480I`, `TV_625_288P`, `TV_625_576I`.
- `MonitorKind::Tv { standard: TvStandard }`, a third variant. In TOML it is `kind.tv.standard = "ntsc"`. The format is append-only: a new variant, no field renamed or removed.

**`neptron-safety`**

- `validate` has one arm: for `MonitorKind::Tv { standard }`, the mode must equal one of `standard.modes()` field for field (new rejection `NotATvMode { mode, standard }`), and must then pass `check_rails` as a second opinion. The GPU check (`check_gpu`) runs first as for every kind. This is stricter than the CRT path, never wider.
- `ValidatedMode` has a private field `tv: Option<TvStandard>` and the reader `tv_standard()`. `validate` sets it from the profile. So the standard that reaches the wire is the one that was validated with the mode, and nothing else can set it.

**`neptron-backend`**

- The `Display::set_mode` doc has one rule: for a mode with a TV standard, the backend sets the connector's standard in the same atomic commit. A backend refuses with `ModeRefused` and changes nothing in three cases: a standard and no `TV mode` property; a standard and no value of that name; no standard on a connector that has the `TV mode` property. The third case keeps the standard on the wire equal to the standard that was validated with the mode: the encoder would keep the standard that is on the connector. So on such a connector every mode, recovery and safe mode included, comes from a TV profile. A request whose mode and standard both equal what is on the wire changes nothing; a change of either one is a modeset.

**`neptron-kms`**

- At open, the backend reads the `TV mode` property of the chosen connector and the values of its enum. It decides nothing: `TvStandard::kms_name()` names the value, and the backend finds its number.
- `set_mode` adds the `TV mode` property, and each margin property set to 0, to the test-only and the real commit, with `ALLOW_MODESET`. It reads the property back after the commit. A connector with no `TV mode` property, or with no value of that name, gives `ModeRefused` for a mode with a standard. A connector with the `TV mode` property gives `ModeRefused` for a mode with no standard.
- `WireMode` holds the standard beside the mode, so a standard change alone commits.

**`neptron-testkit`**

- `FakeDisplay` records the standard of each `set_mode` and follows the same rule as KMS.

**`neptron-modeline`**

- `decide` dispatches a `MonitorKind::Tv` profile to `policy/tv.rs`. It does not call Switchres. It returns the same `Plan`, `Considered`, `Proposal`, `Placement`, `Timing` and `Notice` types, so `choose`, `switch_for`, the engine and the status block need no change for it.
- The rungs, in order:
  1. **Native progressive.** The game's lines fit the progressive mode (240 at 525, 288 at 625): the game is centred with black lines. One game line is one scan line.
  2. **Native progressive with the overscan crop.** The game has up to 16 lines more: the existing crop rule hides at most `OVERSCAN_CROP_MAX_LINES_PER_EDGE` (8) lines per edge.
  3. **Interlaced 1:1.** An interlaced game, or a game with more lines than rung 2 holds, up to 480 or 576 lines: the interlaced mode, centred.
  4. **Fitted.** The last resort, as on every profile: the game scaled into the progressive mode, with the notice `fitted`.
- **Refresh.** The only target is the mode's own rate. `exact_x2`, `force_50` and `force_60` are skipped with the reason "the TV standard fixes the rate" (`Plan::skipped`), as for a target no range covers.
- **Picture mode.** `auto` and `native` both walk the rungs above. `2x`, `3x` and `4x` have no mode on a TV, and a TV has one rate for `speed`: the menus grey them with the reason ("Not possible with this screen and video output", "A TV has one rate"; `docs/spec/video-policy-cases.md`, section 8.7).
- **Width.** See "Sharpness". The field `PolicySettings::tv_width` (`aspect`, `integer`, `fill`) feeds it.
- **Speed notice.** On a TV profile the notice `game_mhz` and `shown_mhz` appears only when the game rate and the field rate differ by more than `TV_SPEED_SILENT_PPM` (5000 ppm), equal to the bound of the audio rate control. `TV_SPEED_SILENT_PPM` is `SPEED_ALERT_MIN_PPM`, the one alert threshold of every display class (`docs/spec/video-policy-cases.md`, section 9). Below it the game is called exact. This is the flat-panel rule (`PANEL_SPEED_SILENT_PPM`), for the same reason: the user cannot choose another rate. The bound is only the threshold of the alert: the game and its sound always run at the field rate (see "The audio rate").

**`neptron-video`**

- The engine needs no change for the rungs. `Monitor::Profile` holds the TV profile.
- On a TV the mode line of the video status names the standard first and leaves out the rung word: `NTSC 240p 15.73 kHz 60.054 Hz`, or `NTSC 4.43 480i 15.73 kHz 59.940 Hz` (`write_tv_mode` in `crates/neptron-video/src/status.rs`). The Display panel shows the short form `NTSC 240p 60.05 Hz`. The status uses `TV_SPEED_SILENT_PPM` for the `speed` result on a TV profile.
- The guard needs no change. A standard change is a new monitor: `propose` opens the prompt, `decline` or no press in `CONFIRM_WINDOW_NS` (15 s) reverts, `confirm` makes it the last known good.

**`neptron-platform` and `data/hardware/matrix.toml`**

- The entry `pi_vc4_composite` (13.5 MHz fixed, interlace, 720x576) has the quirk `tv_encoder`. The quirk drives only the composite fallback of the output rule and its shorter display wait (`tv_fallback` and `wait_max_ns` in `apps/neptron/src/display_wait.rs`). A composite connector of a GPU without the quirk (an old PC TV-out) is used only when named.
- The app detects a TV from the connector, not from the quirk: after the open, the KMS backend reads the `TV mode` property of the connector in use, and a connector with that property is a TV output (`is_tv` in `apps/neptron/src/display.rs`).

**`neptron-settings` and the app**

- `display.tv`: `auto` (the default, which is `ntsc`), `ntsc`, `ntsc_j`, `ntsc_443`, `pal_60`, `pal_m`, `pal`, `pal_n`, `secam`, `mono_60`, `mono_50`. Global level only, like `display.monitor`: the guard keeps the monitor of the boot for every game (`Guard::continue_boot`), so a per-game standard would revert at once.
- `display.tv_width`: `aspect` (default), `integer`, `fill`. System and game levels allowed: it changes only the placement.
- On a connector with the `TV mode` property, the app builds the monitor from `display.tv` with `TvStandard::profile()`: `MonitorProfile { id: "tv_<key>", kind: Tv { standard } }`, for example `tv_ntsc` (ADR 0012). The id carries the standard, because the guard record stores a monitor by its id. `display.monitor` has no effect there, and a value other than `auto` gives a notice that says so.
- The safe default of a TV output is `ntsc`, for the boot prompt and for the blind recovery (the definition's "safe default for the detected output").

### Choosing the mode for a game

| Game | Source | On NTSC (525) | On PAL (625) |
| --- | --- | --- | --- |
| NES | 256x240, 60.099 Hz | 240p, 240 of 240 lines, 60.054 Hz, silent (-738 ppm) | 288p, 240 of 288 lines centred, 50.080 Hz, speed alert (-16.7 %) |
| SNES | 256x224, 60.099 Hz | 240p, 224 lines centred (8 black lines above and below, as on the console), silent | 288p, speed alert |
| SNES, PAL game | 256x239, 50.007 Hz | 240p, 239 lines, 60.054 Hz, speed alert (+20.1 %) | 288p, 239 lines centred, silent (+1462 ppm) |
| Mega Drive | 320x224, 59.923 Hz | 240p, silent (+2199 ppm) | 288p, speed alert |
| Mega Drive, PAL game | 320x240, 49.701 Hz | 240p, speed alert | 288p, speed alert (+7628 ppm, 0.76 % fast). The alert shows; the sound is 13 cents high |
| PC Engine | 256x240, 59.826 Hz | 240p, silent (+3819 ppm) | 288p, speed alert |
| CPS1 | 384x224, 59.637 Hz | 240p, speed alert (+7000 ppm) | 288p, speed alert |
| Neo Geo | 320x224, 59.186 Hz | 240p, speed alert (+1.5 %) | 288p, speed alert |
| A 256-line board | 384x256, 55.018 Hz | 240p with 8 lines hidden per edge, speed alert | 288p, centred, speed alert |
| SNES interlaced high resolution | 512x448i, 60.099 Hz | 480i, 448 lines centred | 576i, speed alert |
| A 480i source | 640x480i, 59.940 Hz | 480i, exact | 576i, speed alert |

The ppm values are COMPUTED from the console rates in `crates/neptron-modeline/tests/golden/systems.toml` and the NES rates of the [nesdev cycle reference](https://www.nesdev.org/wiki/Cycle_reference_chart) (60.0988 Hz, 50.0070 Hz).

### Rates against real consoles

| | Rate | Comment |
| --- | --- | --- |
| NTSC broadcast, interlaced | 59.940 Hz | The encoder's 480i: 13.5 MHz / 858 / 262.5 (COMPUTED) |
| Encoder 240p | 60.054 Hz | 262 lines. Real 240p consoles also draw 262 lines, but with their own clocks: NES and SNES 60.0988 Hz, Mega Drive 59.92 Hz, PC Engine 59.83 Hz |
| Encoder 288p | 50.080 Hz | 312 lines. NES and SNES PAL 50.007 Hz, Mega Drive PAL 49.70 Hz |
| Encoder 576i | 50.000 Hz | |

**The audio rate.** The core runs one frame per flip, so the game runs at the encoder's field rate. The session sets the audio run rate to the field rate that the driver reads back (`RunRateSource::ReadBack`, `crates/neptron-session`), at every difference. So the sound plays faster or slower by the ratio of the field rate to the game's rate, and the pitch moves with it: by 1.3 cents for an NES on NTSC, by 8.6 cents at 5000 ppm (COMPUTED). The dynamic rate control of the audio pipeline (at most `MAX_RATE_DEVIATION_PPM`, 5000 ppm, `crates/neptron-audio/src/control.rs`) corrects only the drift between the read-back rate and the real rate of the display and the sound card. `TV_SPEED_SILENT_PPM` decides only whether the speed alert shows (`Timing::changes_speed`). Every 60 Hz console of the MVP is within 5000 ppm on NTSC, so none shows the alert; the Mega Drive PAL is the one 50 Hz console beyond it on PAL.

### The safety rails for a fixed standard

The definition says composite carries no signal risk, because the timings are broadcast standards (`05-crt-safety-model.md`). What stays, and why:

| Layer | On a TV output |
| --- | --- |
| Type gate | Unchanged: only `validate` makes a `ValidatedMode`, and only a table mode passes |
| Range checks and hard rails | The table check, then the 15 kHz rails as a second opinion. A table typo cannot reach the wire |
| Confirm or revert | Kept for every standard change. A wrong standard gives no colour, a rolling picture or no lock. The timed revert brings back the standard that worked, also when the user cannot read the prompt |
| Last known good | The confirmed standard is kept, with the connector, as for every monitor |
| Blind recovery | Holding the combination for 5 s goes back to NTSC |
| Silent kernel | Unchanged: no fbdev, no console, no `video=` on the command line |

### Contracts, and the ADR

ADR 0012, "A TV standard is a monitor kind with a fixed mode table", records:

1. `TvStandard` and `MonitorKind::Tv` in `neptron-types` (a profile format addition).
2. The TV arm of `validate`, the field of `ValidatedMode`, and `Rejection::NotATvMode` in `neptron-safety` (a contract change of the safety crate).
3. The `Display::set_mode` rule for the standard (a contract change of `neptron-backend`), with `KmsDisplay`, `FakeDisplay` and `WindowDisplay` (which ignores it).
4. The output rule for a composite connector of a TV encoder (below).
5. The speed notice bound of a TV profile (`TV_SPEED_SILENT_PPM`).

### What the TV path does not touch

The Switchres wrapper and every CRT and panel path of the policy, every golden of them, the guard state machine, the engine's directive machine, `Scene` and the renderer, the audio pipeline, the input stack and the launcher.

### The output rule

The definition: "a Pi uses HDMI when a display is detected there and composite otherwise" (`05-crt-safety-model.md`). `choose` in `neptron-kms` never takes an analog connector without EDID unless it is named, and composite reports nothing (`crates/neptron-kms/src/connector.rs`). So the app holds the Pi rule (`apps/neptron/src/display_wait.rs` and `boot_output.rs`, where the platform and the matrix are known), with `choose` unchanged:

1. `display.output` names a connector (in `global.toml`, or in `neptron.toml` through its import): that connector. `Composite-1` gives composite also with an HDMI display connected.
2. `auto`: `choose`. An HDMI display that reports its EDID is a candidate, so HDMI wins, through the flat-panel path.
3. `auto` and `choose` finds no candidate, and a `Composite` connector exists whose GPU entry has the `tv_encoder` quirk: the app opens that connector by name.
4. The display wait: with `auto`, the app waits for an HDMI display at most `COMPOSITE_FALLBACK_WAIT_NS` (3 s as a first value), not the full `DISPLAY_WAIT_MAX_NS` (15 s), before it takes composite. The value is UNVERIFIED: the time `vc4` needs to report a connected HDMI display after the driver loads is a hardware check.
5. The output is chosen once per boot. The first run of a boot removes `run/output`, runs the rule, and writes the connector it drives to `run/output` (a RAM folder). Each relaunch of that boot (a game launch, the return to the game list) opens that connector by name, with no wait. So a composite-only Pi waits once per boot, and an HDMI display that answers late cannot take a game away from the TV.
6. When a display is unplugged, the mode stays (the definition's rule). Nothing switches between HDMI and composite while the frontend runs.

This rule is safe: the composite fallback can only produce the four broadcast modes. It applies only to a GPU that the matrix marks as a TV encoder.

## 4. Sharpness

### Horizontal sampling

Every mode has 720 active samples at 13.5 MHz, 74.1 ns each (COMPUTED). A TV shows about 704 of them as the 4:3 width (UNVERIFIED for a given TV). Real consoles drew their picture with their own dot clock:

| Console | Width | Dot clock | Width in 13.5 MHz samples |
| --- | --- | --- | --- |
| NES, SNES, Mega Drive H32 (NTSC) | 256 | 5.369 MHz (21.477 MHz / 4) | 643.7 |
| Mega Drive H40 (NTSC) | 320 | 6.712 MHz (53.693 MHz / 8) | 643.7 |
| NES PAL | 256 | 5.320 MHz (26.602 MHz / 5) | 649.6 |
| SNES high resolution | 512 | 10.739 MHz | 643.7 |

The master clocks are VERIFIED for the NES ([nesdev](https://www.nesdev.org/wiki/Cycle_reference_chart)); the others are UNVERIFIED (community values); the sample counts are COMPUTED. So a console picture is about 640 to 650 samples wide on a TV, centred, with black at the sides. The Pi can match the shape of a real console within 1 %.

**The width rule** (`display.tv_width`):

- `aspect` (default), in `crates/neptron-modeline/src/policy/tv/width.rs`:
  1. The aspect width: `aspect x field_lines x 704 x 3 / (4 x picture_lines)`, rounded to the nearest sample. `aspect` is the game's display aspect (`SourceGeometry::aspect_milli`; the frame's own shape when the core gives 0), `field_lines` the lines of one field that the frame spans as placed, and `picture_lines` 240 on the 525-line system and 288 on the 625-line system. So a line of aspect 1 is 2.2 samples wide at 525 lines and 1.8333 at 625 lines (704 samples for 4:3 over 240 or 288 lines, COMPUTED).
  2. The console case: a 4:3 source (1333 or 1334 thousandths) of 224 to 240 lines per field takes the console width instead, 644 samples at 525 lines and 650 at 625 lines (`TV_525_CONSOLE_WIDTH_SAMPLES`, `TV_625_CONSOLE_WIDTH_SAMPLES`, from the dot clocks below). A taller 4:3 source takes the larger of the console width and the aspect width, so the width never falls as the lines rise.
  3. The snap: when a whole or half factor of the game width is within 3 % of that width (`TV_WHOLE_FACTOR_TOLERANCE_PERCENT`) and inside 704 samples, it wins.
  4. The result is at least 1 and at most 704 samples (`TV_SAFE_WIDTH_SAMPLES`).

  Results on 525 lines: 256 wide gives 640 (factor 2.5, sharp bilinear), 320 gives 640 (factor 2, nearest), 512 gives 644 (no factor near, sharp bilinear), 640 gives 640 (factor 1, nearest), 160x144 (Game Boy, 10:9) gives 352 (factor 2.2, sharp bilinear). On 625 lines 256 and 320 also give 640, and 512 gives 650.
- `integer`: the largest whole factor whose width is at most 704. 256 gives 512 (narrow: tall pixels), 320 gives 640, 512 gives 512, 640 gives 640. Every pixel has the same width.
- `fill`: 704 samples, the full 4:3 width. Wider than any console, and the edges may sit in the TV's overscan.

**Nearest against sharp bilinear.** The rule of every profile holds: nearest when the factor is whole, sharp bilinear otherwise (`Placement` and `ScaleClaim`). At 2.5 samples per pixel, nearest makes pixels of 2 and 3 samples in turn (148 and 222 ns), which shows as columns of uneven width on a sharp set. Sharp bilinear makes every pixel 2.5 samples wide with one blended sample at some edges. A composite channel with 4.2 MHz of luma bandwidth (NTSC) has a rise time of about 100 ns (UNVERIFIED for a given TV), longer than one sample, so the blended sample is not visible, and the even widths are. This is why 256-wide games get sharp bilinear and not nearest.

**Borders against fill.** Black side borders are what a console made. They are not lost picture: the TV's overscan eats the edges of 720 samples anyway.

### Vertically: 240p against 480i

- One game line is always one scan line. No mode scales lines.
- 240p and 288p have no interlace flicker and show the dark gaps between lines that a console showed. They are the default for every game up to 240 or 288 lines.
- 480i and 576i are used only when the game has more lines. A 240-line game is never doubled into 480i.

### Chroma artefacts

- **Dot crawl and rainbow** come from the TV, not from the encoder alone. Luma detail near the colour subcarrier (3.58 MHz NTSC, 4.43 MHz PAL) passes the TV's notch or comb filter and is decoded as colour (rainbow), and the subcarrier left in the luma shows as dots along colour edges (dot crawl).
- **Fine detail at 320 wide sits close to the subcarrier.** One-pixel stripes at 320 wide have a fundamental of 3.36 MHz (COMPUTED), near NTSC's 3.58 MHz. This is why Mega Drive dithering blends into colours and rainbows on a real console through composite. The Pi gives the same effect for the same picture. It is part of the look, not a fault.
- **The pattern in 240p.** At 525 lines the subcarrier makes 227.5 cycles per line, so a 262-line field holds a whole number of cycles (59 605) and the dot pattern stands still from field to field, as on 240p consoles; in 480i (262.5 lines per field) it crawls (COMPUTED from the standard; UNVERIFIED that the VEC keeps the subcarrier phase continuous across a progressive field).
- **What the encoder offers:** nothing to tune (no notch, no chroma filter, no burst gain; see section 2), except burst and chroma off.
- **Monochrome** (`mono_60`, `mono_50`): no colour burst, so a TV turns its colour decoder and, on most sets, its luma notch off. This gives the sharpest luma the cable can carry, in black and white. The firmware documentation says the same of its own option: monochrome, "but it might appear sharper" (VERIFIED: `sdtv_disable_colourburst` in [`legacy_config_txt/video.adoc`](https://github.com/raspberrypi/documentation/blob/master/documentation/asciidoc/computers/legacy_config_txt/video.adoc)).
- **A pre-filter** (later, optional): a small horizontal low-pass in the renderer before the encoder, for example a three-tap [1 2 1]/4 filter on the 720-sample line, lowers luma near 3.58 MHz by about 7 dB (COMPUTED) and so reduces rainbow on fine detail, at the cost of sharpness. It would be a built-in shader and a setting with `off` as the default. It is not in the first image.

### Overscan and the safe area

- A TV hides part of the 720 samples and a few lines at the top and the bottom. Console games kept their content inside that area. The default does nothing for overscan: the picture is placed as a console placed it. The menu is 640 of the 720 samples wide by design (`ui_dest` in `policy/tv.rs`: the largest whole factor inside 704 samples), so a TV shows a black band at each side of it (OP-097, OP-102).
- The kernel margin properties stay 0 (see section 2).
- Picture position (a shift in samples and in lines, in software, with no modeset) is the definition's "image position". It is a later setting (OP-097); the first image has none.

### Black level and setup

- NTSC has a 7.5 IRE pedestal (the `PDEN` bit); NTSC-J has none (VERIFIED: the table). A US set fed NTSC-J shows darker shadows; a Japanese set fed NTSC shows grey blacks. PAL has no pedestal.
- Whether the VEC maps RGB 0 to the blanking or the setup level as the standard asks is UNVERIFIED. The 240p test suite's PLUGE pattern checks it on the bench.

### What RetroPie-class systems did

- With the old firmware video driver, `sdtv_mode=0` and `2` gave interlaced NTSC and PAL, and `sdtv_mode=16` and `18` gave progressive NTSC and PAL (VERIFIED: [`legacy_config_txt/video.adoc`](https://github.com/raspberrypi/documentation/blob/master/documentation/asciidoc/computers/legacy_config_txt/video.adoc)). Users set one mode for everything in `config.txt`, often 240p through `sdtv_mode=16`, and RetroArch scaled each game into a fixed framebuffer (UNVERIFIED: RetroPie forum threads, for example [240p on composite](https://retropie.org.uk/forum/post/91393) and [custom configs for composite](https://retropie.org.uk/forum/post/165865)).
- With the KMS driver the `sdtv_mode` options have no effect, and users set a mode on the kernel command line, for example `video=Composite-1:720x240@60e` (UNVERIFIED: the same forum).

**Where NEPTRON does better:**

1. The mode is chosen per game: 240p or 288p for progressive games, 480i or 576i only for interlaced ones. No reboot and no file edit.
2. The width is computed per game to the console's own shape, with nearest at whole factors and sharp bilinear elsewhere. No firmware scaler with its bilinear smear, no fixed framebuffer.
3. No vertical resampling ever.
4. Every standard of the encoder, PAL-60 and monochrome included, is a menu choice under confirm-or-revert.
5. Frame pacing and the audio run rate follow the encoder's real field rate, read back from the driver: no tearing and no drift.

**What composite cannot do**, and the spec does not promise: RGB sharpness, clean colour edges, a picture without dot crawl on detailed colour content, or a 263-line 240p mode at a console's exact rate.

### Recommended defaults and settings

| Setting | Default | Values | Reason |
| --- | --- | --- | --- |
| `display.tv` | `auto` (NTSC) | The ten standards | The owner's decision. Changeable from the Display panel under confirm-or-revert |
| `display.tv_width` | `aspect` | `aspect`, `integer`, `fill` | The console's own shape, sharp at whole factors |
| Picture mode | `auto` | `auto`, `sharp`, `fit`, `native` (`speed`, `2x`, `3x`, `4x` greyed) | 240p or 288p for every progressive game |
| Pre-filter | none in the first image | later: `off`, `soft` | Composite blur is already there; the filter only helps rainbow |

## 5. The image

### Buildroot board

- **Base.** Buildroot 2026.08.1 (pinned in `os/README.md`). Its Pi defconfigs are a reference, not a base: they use an external Bootlin toolchain and an ext4 root (VERIFIED: `configs/raspberrypi3_64_defconfig`). The NEPTRON defconfig `os/configs/neptron_rpi_arm64_defconfig` follows the PC defconfig: the Buildroot glibc toolchain, BusyBox init, EROFS root, the NEPTRON packages, and a Cortex-A53 target (`BR2_cortex_a53`).
- **Board files** in `os/board/rpi`: `config.txt`, `cmdline.txt`, `genimage.cfg`, `post-build.sh`, `post-image.sh`, `LICENCES.txt`, kernel fragments (`video.config`, `sound.config`, `storage.config`, `input.config`, `network.config`, `console.config`), and a rootfs overlay for the Pi parts. The scripts and overlay of `os/board/common` are shared.
- **Firmware.** The Buildroot package `rpi-firmware` at its pinned commit `063bcab6` (2025-12-08), with `BR2_PACKAGE_RPI_FIRMWARE_BOOTCODE_BIN` and `BR2_PACKAGE_RPI_FIRMWARE_VARIANT_PI` (`start.elf`, `fixup.dat`). The variants are separate options, so the Pi 4 files (`VARIANT_PI4`) can be added later beside them (VERIFIED: `package/rpi-firmware/Config.in` and `.mk`). The Pi 5 needs no `start*.elf`: its firmware is in its EEPROM.
- **Device trees and overlays** come from the kernel build of the pinned commit (`bcm2710-rpi-3-b`, `bcm2710-rpi-3-b-plus`, `bcm2710-rpi-zero-2-w`, and the overlay `vc4-kms-v3d`), not from the firmware tarball. Buildroot's package copies the overlays of the firmware tarball (VERIFIED: `RPI_FIRMWARE_INSTALL_DTB_OVERLAYS`), which are built for another kernel. The board task makes the overlays and the base trees come from one kernel commit.

### `config.txt`

```ini
# Written by the NEPTRON image. Hand edits are lost at the next update.
arm_64bit=1
kernel=Image
disable_splash=1
disable_fw_kms_setup=1
dtparam=audio=on
dtoverlay=vc4-kms-v3d,composite
core_freq_fixed=1

[pi02]
enable_tvout=1

[all]
```

- `dtoverlay=vc4-kms-v3d,composite` turns on the KMS driver and the VEC (VERIFIED: fragment 11 of `vc4-kms-v3d-overlay.dts` is enabled by `composite`, Pi fork 6.12). It does not turn HDMI off on a Pi 3: only `nohdmi` does (VERIFIED: the same overrides).
- `disable_fw_kms_setup=1` stops the firmware passing a `video=` mode on the command line (VERIFIED: `config_txt/video.adoc`). With no fbdev the kernel would set no mode anyway; this keeps the command line plain.
- `dtparam=audio=on` turns on the analog audio driver of the jack (VERIFIED: overlay README, base parameter `audio`, default off). It comes before every `dtoverlay` line: after an overlay it sets the `audio` parameter of `vc4-kms-v3d`, and the jack gets no sound card (seen on a Pi 3 B, 2026-11-14).
- `core_freq_fixed=1` is the definition's choice (`07-hardware-support-matrix.md`) and keeps the core clock constant (VERIFIED: [`config_txt/overclocking.adoc`](https://github.com/raspberrypi/documentation/blob/master/documentation/asciidoc/computers/config_txt/overclocking.adoc)).
- `[pi02]` with `enable_tvout=1` is the Zero 2 W cost: composite on, HDMI off (VERIFIED: model filters in `config_txt/conditional.adoc`, and `enable_tvout`).
- A later `include neptron-display.txt` line can carry a GPIO DAC overlay (VERIFIED: `include` exists in `config.txt`; settings that the bootloader handles must stay in `config.txt` itself, per `what_is_config_txt.adoc`). Not in the first image.
- The definition says that `config.txt` is generated from the settings (`10-os-layer.md`). In the first image it is a fixed file: no setting needs it yet.
- The values of `gpu_mem` and the CMA size stay at their defaults until the bench measures them (UNVERIFIED).

### Kernel

- Tree: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `ea17ae02ca28483e508b05c7c44190c01a1ef8eb`, the tarball's SHA-256 in `os/board/rpi/patches/linux/linux.hash`.
- Configuration: `bcm2711_defconfig` (VERIFIED: the Pi docs give it for 64-bit Pi 3, Zero 2 W, 4 and 5; Buildroot uses it for its Pi 3 and Zero 2 W 64-bit defconfigs), 4K pages, plus the NEPTRON fragments: `vc4` and `v3d` built in, the SD host and EROFS, ext4, exFAT and FAT built in (no initramfs), no `CONFIG_VT`, no `CONFIG_FB`, no fbdev emulation, no framebuffer console, no `simpledrm`, no DRM panic screen, the USB HID and Ethernet drivers, `ksmbd`.
- The PC's post-build check of the silent kernel (layer 8) runs on the Pi kernel too (`os/board/rpi/check-kernel-config.sh`).
- The CPU governor is `performance` (`CONFIG_CPU_FREQ_DEFAULT_GOV_PERFORMANCE` in `os/board/rpi/system.config`). With `ondemand` the clock fell to 600 MHz in a light scene, and an SNES game on a Pi 3 B lost 24 to 30 fields in a burst; with `performance` it lost none in 155 s (2026-11-14, commit `d6f51ea`). The firmware still lowers the clock when the board is too hot.
- **No 15 kHz patch.** The 15 kHz patch set enters the PC build only through `BR2_LINUX_KERNEL_PATCH` in `os/configs/neptron_pc_x86_64_defconfig`. `os/patches/linux` holds only `linux.hash`, no patch file (VERIFIED: the tree on 2026-11-13). The Pi defconfig sets no `BR2_LINUX_KERNEL_PATCH` and uses its own global patch directory `os/board/rpi/patches` (with the hash of the Pi kernel tarball), so no file of the PC kernel setup is in its path. A post-build check fails the Pi build when `BR2_LINUX_KERNEL_PATCH` is not empty. The reason for the rule is that the Pi kernel needs no patch; the licence of the set (GPL-2.0-only since ADR 0018) is not the reason. The VEC needs nothing from the set: NEPTRON sets its modes through atomic KMS, and the set patches the console mode and AMD and Intel drivers.

### Mesa and the GPU

| Board | GPU | Mesa driver | API |
| --- | --- | --- | --- |
| Pi 3, Zero 2 W | VideoCore IV | Gallium `vc4` | OpenGL ES 2 |
| Pi 4, Pi 5 | VideoCore VI, VII | Gallium `v3d` (needs `vc4` for the display) | OpenGL ES 3.1 |

VERIFIED: `package/mesa3d/Config.in` of Buildroot 2026.08.1 (Mesa 26.1.8), which says "Driver for Broadcom VC4 (rpi0/1/2/3) GPUs" and "Driver for Broadcom VC6/7 (rpi4/5) GPUs (needs vc4)". Neither needs LLVM, so the Pi image builds without the host and target LLVM that take most of the PC build time. The image selects `vc4`, and softpipe for tests. A composite buffer is 720x576, well under the 2048 texture limit of `vc4` (`apps/neptron/src/kms.rs` sizes the buffers from the caps).

### The renderer on vc4

The Pi 3 runs the GLES 2 path of `crates/neptron-gfx/src/gles`. vc4 differs from Mesa's softpipe in three ways that change the picture, so the renderer does not depend on them:

- **One texel per pixel, BGRA bytes.** Every source texture holds `BGRA_EXT` bytes, one texel per source pixel. XRGB8888 goes to GL as it is. RGB565 and XRGB1555 are expanded on the CPU before the upload, with the expansion of the reference renderer (`round(v x 255 / 31)` and `round(v x 255 / 63)`, as integer steps). The texture unit of vc4 expands 5-bit and 6-bit channels by bit replication, which is 1 off for about a third of the values. The expansion of a 256x224 RGB565 frame takes about 0.5 ms on a Pi 3, and the upload about 0.5 ms (commit `ab90f9a`).
- **The destination index from a varying.** The vertex stage gives the destination index of each corner as a varying, and the fragment stage takes the floor of it. vc4 gives `gl_FragCoord` at the pixel corner, and Mesa turns its y upside down for a window-system framebuffer and not for a framebuffer object, so the renderer does not read `gl_FragCoord`.
- **Four programs, by blending axes.** The renderer compiles one program for each pair of axes that blend (`BLEND_X`, `BLEND_Y`) at setup and picks one per quad. An axis blends only with sharp bilinear at a scale that is not whole. The vc4 compiler turns a branch on a uniform into a select, so one program with branches ran every path for every pixel. Each tap of sharp bilinear is one `NEAREST` read; the blend is in the shader, because the `LINEAR` filter of vc4 works in fixed point and is up to 5 off. The quad covers only the output pixels whose near source pixel is inside the frame.

A source texture grows on each side and never shrinks, so a game that switches between sizes allocates GL storage only at the first frame of its largest size (OP-103 point 4).

`gles_compare` on a Pi 3 (VC4 V3D 2.1, Mesa 26.1.8) passes every scene through the framebuffer object and through the window path, except the two axis product cases: their 4096-pixel output is above the 2048 limit of vc4 (2026-11-14, commit `e03f6f7`).

### The frame loop on the Pi 3

The session runs each frame in this order: flip event, input, core, mute when asked, present, `end_frame` (the audio resampling, 1.3 to 1.4 ms on the Pi 3), wait for the flip (`docs/spec/pacing-audio.md`, "Frame order"). The CPU resamples while the GPU draws, and the atomic commit waits for the render fence.

The stats reply of the control protocol (version 2) gives `fields_missed`: the fields in which no new frame reached the display while the core ran. A late flip shows the frame before for one more field, and no other counter sees it. `audio_underruns` counts the times the pipeline queue ran empty in play.

Measured on a Pi 3 B on composite, NTSC 240p, with the development build of `os push` (no LTO), on 2026-11-14 (OP-103):

- NES, SNES and Mega Drive games hold 60.054 frames per second with no missed field.
- Super Mario World leaves 1.66 ms in its worst frame. The release image is not measured.
- A keep or revert dialog over a running game misses no field (P12: 36 NTSC dialogs over the SNES 240p suite). A dialog frame ends 8.35 ms after the flip event; a game frame alone, 9.87 ms.
- A change of the TV standard takes about 75 ms from the last flip of the old mode to the first flip of the new one. A mute fills the queue up to 100 ms (`MUTE_COVER_MS`), which covers the change; with the earlier 32 ms cover the underrun count went up by one. The check on the Pi is in `docs/hardware-checklist.md`.
- With the vc4 vblank sequence in 480i not checked, `fields_missed` can hide real misses in interlace (UNVERIFIED).

### SD card layout

| Partition | Filesystem | Size | Contents |
| --- | --- | --- | --- |
| 1 boot | FAT32, label `NEPTRONBOOT`, type `0x0c` | 128 MiB | `bootcode.bin`, `start.elf`, `fixup.dat`, `config.txt`, `cmdline.txt`, `Image`, the device trees, `overlays/`, `neptron.toml`, `LICENCES.txt`, `legal/` |
| 2 root | EROFS (LZ4HC), read-only | 384 MiB | As on the PC |
| 3 state | ext4, label `neptron-state` | 256 MiB | As on the PC |
| 4 share | exFAT, label `NEPTRON` | The rest of the card, made at the first boot | As on the PC |

- The firmware reads only the FAT partition 1 (VERIFIED: Buildroot's Pi `genimage.cfg.in` puts the firmware and `config.txt` there). The boot partition is twice the PC's, because the kernel `Image` is not compressed and the overlays and device trees add files (UNVERIFIED: the sizes; the build reports them).
- `cmdline.txt`: `root=PARTUUID=<signature>-02 rootfstype=erofs ro rootwait quiet console=serial0,115200 panic=10 usbcore.old_scheme_first=1`. The development variant drops `quiet` (`os/board/rpi/cmdline.sh`). The disk signature is fixed per variant, as on the PC, with new values for the Pi variants.
- **`mmcblk0p4`.** `S06neptron-storage` finds partitions by the `partition` number in sysfs and the disk from the device of `/`, never by building a name (VERIFIED: the function `partition` in `os/board/common/rootfs-overlay/etc/init.d/S06neptron-storage`). So `mmcblk0p4` needs no change in the script. `sfdisk --append` and `partx` work on `/dev/mmcblk0` the same way (UNVERIFIED on a card). An SD card is slower than a USB stick, so the first `mkfs.exfat` takes longer (UNVERIFIED).
- A/B root and RAUC with `tryboot` stay later, as on the PC.

### Licences

- **Broadcom firmware.** `boot/LICENCE.broadcom` allows redistribution "in binary form, without modification", only for "developing for, running or using a Raspberry Pi device", and redistributions "must reproduce the above copyright notice and the following disclaimer" (VERIFIED: the file at firmware commit `063bcab6`). It is not an open source licence. Buildroot labels the package `BSD-3-Clause` (VERIFIED: `RPI_FIRMWARE_LICENSE` in `rpi-firmware.mk`), which is wrong for these terms. `os/board/rpi/LICENCES.txt` names the firmware files, gives the real terms, and reproduces the notice and the disclaimer; `legal/` carries `LICENCE.broadcom`.
- The Pi kernel is GPL-2.0; its source is in `legal-info` as for the PC.
- Mesa, SDL3, the cores, the network share packages: as in `os/board/pc/LICENCES.txt`. The non-commercial cores need the same note (OP-054).
- No Wi-Fi or Bluetooth firmware is in the first image.

### Audio

| Output | Board | ALSA device | Notes |
| --- | --- | --- | --- |
| Analog jack | Pi 3 B, 3 B+, Pi 4 | The `snd_bcm2835` driver of the Pi fork, card "Headphones" (UNVERIFIED: the card name) | PWM. 11 bits by default (`pwm_sample_bits`), with the "advanced modulation scheme" of `audio_pwm_mode=2` as the default and a dither of 1 LSB (VERIFIED: [`config_txt/audio.adoc`](https://github.com/raspberrypi/documentation/blob/master/documentation/asciidoc/computers/config_txt/audio.adoc)). It shares the jack with the video |
| HDMI | Pi 3 B, 3 B+ | `vc4-hdmi` (UNVERIFIED: the card name) | Only while HDMI is the driven output. The overlay sets `snd_bcm2835.enable_hdmi=0`, so HDMI audio comes from `vc4` (VERIFIED: fragment 14 of the overlay) |
| None on the board | Zero 2 W | | No jack. HDMI audio is off when composite is on. USB audio, or PWM on GPIO through the `audremap` overlay (VERIFIED: the overlay exists in the README; UNVERIFIED: that it works with composite on) |
| USB audio | All | `snd-usb-audio` | A USB DAC is the way to better sound than PWM |

- The output list (`docs/spec/audio-settings.md`, ADR 0011) takes the analog jack under `auto`, which is right on composite. `S08neptron-mixer` (from the PC overlay, which the Pi image also uses) sets `PCM` and the other controls to 0 dB; the `snd_bcm2835` control name `PCM` is UNVERIFIED.
- The PCM of the `vc4-hdmi` card is HDMI by its card (card id `vc4hdmi`, driver `vc4-hdmi`), because its name `MAI PCM i2s-hifi-0` has no HDMI word. The list labels it `HDMI/DP 1 (vc4-hdmi)`. The card has no ELD file in `/proc`, so its monitor is unknown, and `auto` never takes it: `auto` takes the jack card, or the default device when there is no jack card (OP-102 point 2).
- SDL gets device periods of 1024 frames (`DEVICE_PERIOD_FRAMES` in `crates/neptron-audio/src/sdl.rs`). With 512 the analog output of the Pi 3 (44100 Hz) ran short of data 14 times a second; with 1024 ALSA logged no underrun in 60 s (commit `dff9f44`). The mean output latency is about 64 ms (`docs/spec/pacing-audio.md`).
- The jack carries video on its sleeve, so audio and video leave on one cable.
- Expect hiss and limited dynamic range from the 11-bit PWM (VERIFIED in principle: the bit depth; UNVERIFIED: how it sounds).

### Network

- Pi 3 B: 100 Mbit/s Ethernet on USB; Pi 3 B+: gigabit Ethernet on USB 2.0. The kernel drivers need no firmware file (UNVERIFIED for the B+ chip). The network share and DHCP work as on the PC.
- Zero 2 W: no Ethernet. With no USB adapter there is no network, so no SMB share, no SSH and no network time. Wi-Fi needs the `brcmfmac` firmware files and `iwd`, which the first image does not have. The definition's Wi-Fi keys in `neptron.toml` do nothing yet.

### Input

USB, as on the PC: SDL3 for gamepads, evdev for keyboards. The Pi 3 has four USB 2.0 ports. GPIO input modules are later (`09-input.md`).

### Boot time

The definition gives 3 s from kernel start to the first UI frame for the Pi 4 and 5 (`10-os-layer.md`). For the Pi 3 the spec proposes 3.5 s for the part NEPTRON owns (1.5 s kernel to PID 1, 2 s PID 1 to the first frame), because the Cortex-A53 at 1.4 GHz is slower. The firmware stage before the kernel (`bootcode.bin`, `start.elf`) is outside our control. All of it is UNVERIFIED until measured on the bench.

### The development variant

- `cargo xtask os build --arch rpi-arm64 --dev-key KEY` builds it in its own output, as on the PC: Dropbear with the key only, the hot folder `/state/hot`, the log, and the `control` feature. The address is in `/run/neptron/network.toml`, as on every image, and on the serial console at each change.
- `cargo xtask os push --arch rpi-arm64 --host A.B.C.D` builds the frontend, the launcher and `neptronctl` for the Pi and sends them. It must link against the glibc of the Pi output, not of the QEMU aarch64 output, because the two outputs can hold different glibc builds. The cross build uses the Buildroot toolchain of the `rpi-arm64` output.
- A serial console on GPIO 14 and 15 helps the first boot: `console=serial0,115200` on the command line, and `enable_uart=1` and `dtoverlay=disable-bt` in the development `config.txt` only (`os/board/rpi/config-dev.txt`). `disable-bt` puts the PL011 UART on GPIO 14 and 15, and the clock of the PL011 does not follow the core clock.

### `neptron.toml` on the boot partition

The definition describes the file and its keys (`05-crt-safety-model.md`, `10-os-layer.md`). It is how a user picks composite with an HDMI display connected, or PAL, before the first boot. `S07neptron-boot` mounts the partition, `apps/neptron/src/boot_file.rs` imports the file, and `apps/neptron/src/prestart.rs` runs the import before the backend reads a setting (`apps/neptron/README.md`, "Boot, before the first frame"). Both images ship the file (`os/board/rpi/neptron.toml`, `os/board/pc/neptron.toml`).

- **Place.** `/neptron.toml` at the root of the FAT32 boot partition. On the Pi the firmware's files are beside it.
- **Mount.** `S07neptron-boot` mounts partition 1 of the boot disk read-only at `/boot` (`vfat`, `ro,noatime,nodev,nosuid,noexec`) before the launcher starts. A failed mount never stops the boot. A read-write remount is only for the later writes (the Wi-Fi blanking of the definition, not built).
- **When it is read.** At each start of the frontend, before the backend reads a setting, so the first start after a change already opens the output that the file names. When its SHA-256 differs from the one stored on the state partition, its values are copied into `config/global.toml` once, through `Storage::apply` (`crates/neptron-settings/README.md`), and the new hash is stored. Otherwise the menus' settings stand. This is the definition's rule.
- **Format.** The keys of `global.toml`. For the Pi: `display.output`, `display.tv`, `display.tv_width`, `display.pi_dac`. A key the schema does not know is ignored and listed with its line, as is a value the schema refuses. There is no diagnostics page yet, so the list is notices at a new boot and log lines. A file that cannot be read or parsed changes nothing, keeps the stored hash, and is listed at each new boot until it is fixed. The hash is in `/state/neptron.toml.sha256`. The image ships a file with every key commented out, so a new image copies nothing: only a line that the user uncomments is copied. The Pi file:

```toml
# NEPTRON first-boot settings. This partition is FAT32: edit it on any PC.
# Remove the # at the start of a line and set its value. A value here is
# copied into the settings once, when this file changes.

[display]
# output   = "auto"     # auto | HDMI-A-1 | Composite-1
# tv       = "auto"     # auto (NTSC) | ntsc | ntsc_j | ntsc_443 | pal_60 | pal_m | pal | pal_n | secam | mono_60 | mono_50
# tv_width = "aspect"   # aspect | integer | fill
# pi_dac   = "none"     # none. GPIO DACs are not supported by this image.
```

- **The GPIO DAC stays off.** The image has no DAC overlay in `config.txt` and installs none of the DPI overlays (`os/board/rpi/post-image.sh` copies `vc4-kms-v3d`, and `disable-bt` in the development variant, nothing for DPI). So GPIO pins are never driven as a video bus, whatever the file says. A `pi_dac` value other than `none` gives a notice: "GPIO DACs are not supported by this image; the value is ignored". When DACs come, the rule of the definition holds: a DAC is used only when the file or a setting names it, NEPTRON then writes its overlay line into an included file and reboots once, and it is never tried automatically.

## 6. Verification

### Without hardware

| What | Where | What it proves |
| --- | --- | --- |
| The table modes, the standards, the profile format | Tier 1, `neptron-types` | The four modes and the line systems; the round trip of `kind.tv` |
| The validator | Tier 1, `neptron-safety` | Each standard accepts its two modes and refuses the other system's modes, a clock other than 13 500 kHz and every changed field; each table mode passes the 15 kHz rails; `tv_standard()` carries the standard |
| The policy | Tier 1, `neptron-modeline` | Goldens for the games of the table in section 3 on `ntsc`, `pal` and `pal_60`; property test: every TV proposal validates; the speed notice bound |
| The fake display | Tier 1, `neptron-testkit` | A standard change alone is a modeset; a display with no standard support refuses |
| The guard | Tier 1, `neptron-video` | A standard change opens the prompt, a timeout reverts, a confirm makes it the last known good, the recovery gives NTSC |
| The app | Tier 2 | The output rule (HDMI wins, composite fallback, a name wins), the settings rows, the crawler finds `display.tv` and `display.tv_width`, a scene golden of the Display panel on composite |
| The KMS backend | Tier 3, `kms-check` on `vkms` in the aarch64 QEMU image | The four table modes commit and read back as plain modes, with no standard; a mode with a standard on a connector without `TV mode` (vkms has none) is refused and changes nothing. No virtual driver has a `TV mode` property, so setting a standard is a hardware check |
| The aarch64 image | Tier 3, the existing checks | The frontend, the launcher, the cores and the boot check, unchanged |
| The Pi image | Build only, in CI and locally | The defconfig builds; the image assembles; the silent kernel check passes; the no-15-kHz check passes; `legal-info` and `LICENCES.txt` are on the boot partition; the sizes |

QEMU cannot test the Pi display: its Pi machines model the firmware framebuffer but no `vc4` display pipeline and no VEC (VERIFIED: the implemented and missing devices of [QEMU's raspi machines](https://www.qemu.org/docs/master/system/arm/raspi.html)). The Pi image is not booted in QEMU.

### The CI entry

The `rpi-arm64` entry of `.github/image-targets.json` is enabled. `docs/ci.md` describes the workflow.

- `defconfig`: `neptron_rpi_arm64_defconfig`; `fragments`: none; `build_args`: `--arch rpi-arm64`; `images_dir`: `rpi-arm64`; `image_files`: `neptron-rpi-arm64.img`; `artifact`: `neptron-rpi-arm64`; `upload_image`: true; `rebuild`: `neptron`, `neptron-launch`, `neptronctl`; `checks`: `pi-layout`; `build_minutes`: 300.
- The upload carries the image, `SHA256SUMS`, `legal/`, `LICENCES.txt` and the `README.txt` that says the image must not be sold, and the `legal-info` sources as a second artifact, as for the PC.
- The Pi image holds no 15 kHz patch, so it needs no `--no-kernel-15khz`. The build's own check (above) keeps it so.
- The runner is `ubuntu-24.04-arm`.

### Hardware checklist: first boot of a Pi 3 B+ on a 15 kHz TV

The bench session (P09) runs these checks, each `needs-hardware`, and writes the results into `docs/hardware-checklist.md`, section 6. The setup: a Pi 3 B+, a 2.5 A power supply, a TRRS-to-RCA cable with video on the sleeve, a 15 kHz TV with a composite input, a USB gamepad, Ethernet, no HDMI cable unless the line says so.

| # | Check | Expect |
| --- | --- | --- |
| 1 | The cable: a meter between the sleeve and the yellow plug | Continuity; the sleeve is video |
| 2 | Power on with no HDMI cable; watch the TV from power on | No rainbow splash, no text at any time; the first picture is NEPTRON's menu in 240p NTSC, locked, in colour on an NTSC-capable set |
| 3 | The time from power on to the first frame, and the log's kernel and frontend times | Recorded; the target is 3.5 s from kernel start to the first frame |
| 4 | The boot prompt on the first boot | None: a boot on a TV output never prompts (`docs/spec/display-guard.md`, "Trust") |
| 5 | `neptronctl selftest video` timing on 240p | Field period about 16.652 ms (60.054 Hz); line rate 15.734 kHz |
| 6 | The Display panel: switch to `pal_60`, `pal`, `ntsc_j`, `ntsc_443`, `secam`, `mono_60`, `mono_50`, `pal_m`, `pal_n` in turn | Each is a full modeset; a standard the TV shows can be kept; a standard it cannot show (no colour, roll) reverts by itself after 15 s; record what the TV does for each |
| 7 | 288p timing on `pal` | Field period about 19.968 ms (50.080 Hz) |
| 8 | The 240p test suite on the NES core: grid, overscan, colour bars, PLUGE, grey ramp, sharpness, horizontal stripes, the drop shadow | Grid squares even; the left and right edges in the overscan area as on a console; black at the setup level of the standard; record the photos |
| 9 | An NES game, `tv_width` `aspect` | 240 lines, no flicker, visible scan lines, width about that of a real NES on the same TV; columns of even width |
| 10 | A SNES game (224 lines) | Centred, 8 black lines above and below |
| 11 | A Mega Drive game that switches between H32 and H40 | No modeset at the switch (the mode stays 720x240p), only the scale changes |
| 12 | Mega Drive dithering (for example a waterfall) on NTSC | Blends as on a real console through composite |
| 13 | The same game on `mono_60` | Sharper luma, no colour; compare the stripes of the sharpness pattern |
| 14 | `tv_width` `integer` and `fill` | Narrower and wider pictures as specified |
| 15 | A PAL game on NTSC | The speed alert; the game runs about 20 % fast |
| 16 | A 480i source (SNES high-resolution interlaced, or a 480i test pattern from `neptronctl`) | 480i, stable; record the flip rate (OP-030) |
| 17 | Sound on the jack | Plays; no crackle in 30 minutes; the mixer at 0 dB; record the noise floor with the volume at 0 |
| 18 | Audio latency and pacing | 30 minutes of play with no dropped or repeated frames in the log; latency recorded (the T17 checks of the Pi 3 section) |
| 19 | HDMI display connected at boot | HDMI is the output, through the panel path; composite off |
| 20 | HDMI connected and `display.output = "Composite-1"` in `neptron.toml` | Composite is the output |
| 21 | Blind recovery on `pal` | Holding the combination 5 s gives NTSC and the sound cue |
| 22 | First boot on a new card | Partition 4 made and formatted exFAT as `mmcblk0p4`; the second boot mounts it unchanged; the SMB share over Ethernet |
| 23 | `neptron.toml` with `tv = "pal"` before the first boot | The first boot comes up on PAL |
| 24 | The time `vc4` needs to report an HDMI display after boot | Recorded; it sets `COMPOSITE_FALLBACK_WAIT_NS` |
| 25 | `dmesg \| grep -i voltage` after 30 minutes (the image has no `vcgencmd`) | No under-voltage, no throttling |

## 7. Build plan

`docs/taskboard.toml` holds the status. Merged: P01 to P08 and P10 to P12. Left: P09, the bench session.

| Task | Scope |
| --- | --- |
| P01 | This spec |
| P02 TV standard contract | ADR 0012; `TvStandard`, the four modes, `MonitorKind::Tv`; the TV arm of `validate`, `ValidatedMode::tv_standard`, `Rejection::NotATvMode`; the `set_mode` doc; `FakeDisplay` |
| P03 TV policy | `policy/tv.rs`, `PolicySettings::tv_width`, the width rule, `TV_SPEED_SILENT_PPM`, goldens |
| P04 Pi board and image | `os/board/rpi`, the defconfig, the kernel pin and fragments, `config.txt`, `cmdline.txt`, `genimage.cfg`, `LICENCES.txt`, the no-15-kHz and silent-kernel checks, the `xtask` target `rpi-arm64` (build, dev variant, push), the checklist lines |
| P05 KMS TV mode | The `TV mode` and margin properties in `set_mode`, `WireMode` with the standard, read back, `kms-check` cases |
| P06 App and settings | `display.tv` and `display.tv_width`; the TV profile on a `tv_encoder` connector; the output rule and `COMPOSITE_FALLBACK_WAIT_NS`; the safe default; the status line; the Display panel rows; the `tv_encoder` quirk in the matrix; scene goldens |
| P07 `neptron.toml` reader | The read-only mount of `/boot`; the read, hash and import of `neptron.toml`; the notices and log lines; the commented file in both images |
| P08 CI | The `rpi-arm64` entry of `.github/image-targets.json`, the upload, `docs/ci.md` |
| P09 Bench session | The checklist of section 6 on the owner's Pi 3; results in `docs/hardware-checklist.md` |
| P10, P11 | The frame budget and the renderer on vc4: the programs by blending axes, one texel per pixel with the CPU expansion, the destination index from a varying, the window path in `gles_compare`, the texture that only grows, the audio frame after the present, the underrun count and `fields_missed`, the 1024-frame audio period, the `performance` governor |
| P12 | A UI over a running game holds the field rate |

**The first composite picture** (2026-11-14, a Pi 3 B on a Samsung tube, the development image of P04 with the frontend of P06 and later through `os push`): the picture locks in NTSC 240p, the menu is sharp, and the grid of the 240p test suite fills the tube with correct geometry; the game picture reaches the edges of the visible area. NES, SNES and Mega Drive games hold the field rate after P10 and P11. OP-102 and OP-103 hold the open points of these tests. P09 runs the rest of the checklist.

Later steps, not in this plan: the pre-filter shader; picture position; a second standard for 50 Hz games; the Pi 4 (`enable_tvout` and the one reboot, `start4.elf`, `v3d`); the Pi 5 RP1 VEC with its variable clock; the GPIO DACs with `include` in `config.txt`; Wi-Fi.

## 8. Open questions for the owner

Each has a default that the build tasks took. The owner decided 1 (the default), 4 (`aspect`), 5 (later, OP-098) and 7 (the ten standards) on 2026-11-13 (OP-096, OP-098).

1. **The Zero 2 W in the first image.** Default: its device tree and the `[pi02]` lines are in the image at no extra cost, it is not tested, and the documentation calls it untested (no network share without an adapter, no analog audio).
2. **What a PAL-only TV shows on the default NTSC.** A set with no NTSC decoder shows black and white if it locks to 60 Hz, or a rolling picture if it does not (UNVERIFIED per set). The ways out: `tv = "pal"` (or `pal_60`) in `neptron.toml` on any PC before the boot, or the Display panel when the picture locks. The owner's bench TV is a European SCART set (`15-decisions-and-open-points.md`), so check 6 tells which standards it decodes. Default: NTSC, as decided. Option for later: a first-boot standard probe that shows NTSC, PAL-60 and PAL for 15 s each with a sound cue, until a button press, like the blind probe.
3. **The speed notice on composite.** Default: silent up to 5000 ppm (the audio bound), as on a flat panel. The alternative, the CRT rule (5 mHz), would show the alert for every console game, because no game matches 60.054 Hz.
4. **The width default.** Default: `aspect`, with the whole factor when it is within 3 %. Alternative: `integer` (exact pixels, but a 256-wide game is narrow).
5. **A second standard for 50 Hz games** (`display.tv_50`, for TVs that show both NTSC and PAL). Default: later. It needs a guard rule, because a game run never changes the monitor.
6. **The GPIO DAC configuration file.** Default: later, with the definition's `pi_dac` key and an `include` file in `config.txt`; the first image ignores the key with a notice.
7. **The key name and values.** Default: `display.tv`, as in the definition's `neptron.toml` sample, with the ten standards plus `auto`. The definition lists only `auto | pal | ntsc`.
8. **The Pi kernel.** Default: `rpi-6.18.y` at a pinned commit, not the 6.12.61 that Buildroot pins. The definition already decides 6.18.

## 9. Gaps against other documents

- The definition's output rule for a Pi needs the app: `choose` in `neptron-kms` never takes composite under `auto`, so the app falls back to the composite connector of a TV encoder (section 3, "The output rule").
- `data/hardware/matrix.toml` gives `pi_vc4_composite` `flip_per_field = false`, which the session reads to count missed fields; the flip cadence of 480i on `vc4` is unknown (OP-030).
- Buildroot 2026.08.1 labels `rpi-firmware` as `BSD-3-Clause`. `LICENCE.broadcom` limits use to Raspberry Pi devices and forbids modification. `os/board/rpi/LICENCES.txt` gives the real terms.

## Sources

All fetched on 2026-11-13.

- Linux mainline `v6.18`: [`drivers/gpu/drm/vc4/vc4_vec.c`](https://github.com/torvalds/linux/blob/v6.18/drivers/gpu/drm/vc4/vc4_vec.c), [`drivers/gpu/drm/drm_atomic_state_helper.c`](https://github.com/torvalds/linux/blob/v6.18/drivers/gpu/drm/drm_atomic_state_helper.c), [`drivers/gpu/drm/drm_connector.c`](https://github.com/torvalds/linux/blob/v6.18/drivers/gpu/drm/drm_connector.c), [`drivers/gpu/drm/tests/drm_modes_test.c`](https://github.com/torvalds/linux/blob/v6.18/drivers/gpu/drm/tests/drm_modes_test.c); the file at `v6.1`, `v6.2`, `v6.3`, `v6.10` and `v6.12` for the version of each feature; the [commit list of `vc4_vec.c`](https://github.com/torvalds/linux/commits/master/drivers/gpu/drm/vc4/vc4_vec.c).
- The [DRM documentation](https://docs.kernel.org/gpu/drm-kms.html#analog-tv-connector-properties), analog TV connector properties.
- Raspberry Pi fork: [`rpi-6.18.y` `vc4_vec.c`](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/drivers/gpu/drm/vc4/vc4_vec.c), [`rpi-6.18.y` `rp1_vec.c`](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/drivers/gpu/drm/rp1/rp1-vec/rp1_vec.c), [`rpi-6.12.y` `vc4_crtc.c`](https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/gpu/drm/vc4/vc4_crtc.c), [`rpi-6.12.y` overlay README](https://github.com/raspberrypi/linux/blob/rpi-6.12.y/arch/arm/boot/dts/overlays/README) and [`vc4-kms-v3d-overlay.dts`](https://github.com/raspberrypi/linux/blob/rpi-6.12.y/arch/arm/boot/dts/overlays/vc4-kms-v3d-overlay.dts); commit `21b410140c47` (`Makefile`, 6.12.61).
- Raspberry Pi documentation sources, branch `master` of [raspberrypi/documentation](https://github.com/raspberrypi/documentation): `config_txt/video.adoc`, `config_txt/audio.adoc`, `config_txt/conditional.adoc`, `config_txt/overclocking.adoc`, `config_txt/what_is_config_txt.adoc`, `legacy_config_txt/video.adoc`, `configuration/audio-display.adoc`; and the [Linux kernel page](https://www.raspberrypi.com/documentation/computers/linux_kernel.html).
- [raspberrypi/firmware](https://github.com/raspberrypi/firmware) commit `063bcab6c8a9`, `boot/LICENCE.broadcom`.
- Buildroot 2026.08.1 in `/opt/buildroot` of the `neptron-dev` image: `configs/raspberrypi*_defconfig`, `board/raspberrypi/*`, `package/rpi-firmware/*`, `package/mesa3d/Config.in`.
- [QEMU Raspberry Pi boards](https://www.qemu.org/docs/master/system/arm/raspi.html).
- [nesdev cycle reference chart](https://www.nesdev.org/wiki/Cycle_reference_chart).
- Community, UNVERIFIED: [pinouts.ru, Pi AV jack](https://pinouts.ru/HeadsetsHeadphones/Raspberry_pi_composite_vi.shtml), [The Pi Hut AV cable](https://thepihut.com/products/adafruit-a-v-and-rca-composite-video-audio-cable-for-raspberry-pi), RetroPie forum threads [91393](https://retropie.org.uk/forum/post/91393) and [165865](https://retropie.org.uk/forum/post/165865).
- This repository: `docs/definition/`, ADRs 0007, 0009, 0010, 0011, OP-030, OP-047, OP-054, OP-070, OP-096, `os/README.md` and `os/`, `crates/neptron-modeline`, `crates/neptron-safety`, `crates/neptron-kms`, `crates/neptron-video`, `crates/neptron-audio`, `apps/neptron`, `data/hardware/matrix.toml`, `docs/hardware-checklist.md`, and `.github/image-targets.json` and `docs/ci.md` on the branch `ci/setup`.
