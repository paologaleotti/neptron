# NEPTRON: Review notes

The review found no reason to change the architecture, but eleven corrections and additions, all now applied to the Definition tab. No sub-agent tool exists in this session, so this is a second pass by the same model, done by re-reading each claim against a source.

## Corrected or added

| Area | Finding | Evidence | Change in Definition |
| --- | --- | --- | --- |
| Networking | `ksmbd` is not daemon-free: it needs the `ksmbd.mountd` userspace helper for shares and users | [Kernel ksmbd documentation](https://docs.kernel.org/next/filesystems/cifs/ksmbd.html) | Technology row reworded |
| Networking | `iwd` and BlueZ both require a D-Bus daemon, which the footprint list did not count | General knowledge, not re-verified | Technology row reworded; started only on demand |
| Updates | Pi has a firmware A/B mechanism (`tryboot` with `autoboot.txt`), which wants one boot partition per slot | [Raspberry Pi config.txt documentation](https://raspberrypi.com/documentation/computers/config_txt.html) | Updates paragraph names tryboot on Pi and GRUB on x86; USB-file updates added |
| Updates | Whether tryboot works on Pi 3 and Zero 2 W firmware is not confirmed | Not found in the pages read | Added as a risk with a U-Boot fallback |
| Composite | Composite encoders have fixed timings, so ODREN cannot generate modelines there | [raspberrypi/linux PR #4406](https://github.com/raspberrypi/linux/pull/4406) adds fixed 240p/288p modes | Hardware note: ODREN selects among four modes and scales |
| Input | Without udev nothing loads kernel modules on hotplug | Follows from the no-udev decision | Input note: built-in drivers or modalias loading by `neptron-init` |
| Emulation | In-process cores can crash the frontend, and some cores do not unload cleanly | Known libretro frontend behaviour, not re-verified | New risk row; re-exec between games |
| Platform | Pi 3 and Zero 2 W as tier 1 make OpenGL ES 2 and 512 MB hard limits | Your decision in the comments | New risk row; memory-budget test planned |
| Licensing | GPU and Wi-Fi firmware blobs are not open source | General knowledge | New risk row |
| Scope | Missing basics: safe shutdown, fast-forward, zipped ROMs | Compared with the [RePlayOS feature list](https://www.replayos.com/) | Three feature rows added |
| Testing | No way for a coding agent to verify its own work was defined | Your request | Developer feature row, M0 scope, and the Implementation plan tab |

## Checked and kept

- **Intel gen9 on your CPU.** The i3-6100T / HD 530 measurements and the interlace cause stand as cited. The claim that NEPTRON avoids the kernel patch by choosing its own buffer modifiers remains a hypothesis until run on your machine.
- **Pi 5 interlace and composite sync** through the RP1 PIO block, with GPIO 1 to 3 reserved.
- **librashader** supports OpenGL 3.3+, 4.6 and Vulkan but not OpenGL ES, so slang presets cannot run on Pi 3 or Zero 2 W. The built-in GLES 2 shader set covers those boards.
- **Virtual KMS (`vkms`)** provides hrtimer-simulated vblanks, per-frame CRCs and a writeback connector, per the [kernel documentation](https://www.kernel.org/doc/html/latest/gpu/vkms.html). The plan relies on these.
- **GunCon 2** reports raw coordinates that need per-display calibration, as the driver README states.

## Still unverified

- Whether `vkms` accepts arbitrary user modelines and interlaced modes. The plan has a task to test this first and a fallback.
- Exact crate choices (`drm`, `gbm`, `khronos-egl`, `glow`, `evdev`, `rubato`, `ureq`) were named from memory.
- The libretro thumbnails set as the box-art source.
- Boot-time and memory figures are targets, not measurements.

## Second review: CRT video, resolutions and scaling

The CRT section had one real contradiction and four gaps; all are fixed in the Definition tab. The comparison with RetroArch's Switchres integration, RePlayOS, Recalbox and the Batocera CRT Script also produced nine improvements worth taking.

### What was wrong or missing

| Problem in the Definition | Why it was wrong | Fix |
| --- | --- | --- |
| Super resolutions were described as an integer scale that also never needs a modeset on width changes | Those are two different schemes. A fixed 2560 width is not a whole multiple of 384 or 352, and an exact multiple changes when the game's width changes | Three named horizontal strategies (native, dynamic super width, fixed super width) with a default per GPU and per system |
| "Vertical is always exact" | Not true for handhelds under the monitor's minimum line count, medium-resolution arcade boards, or anything taller than the progressive limit | New table: sources that do not fit, with what ODREN does for each |
| 31 kHz monitors only got line doubling | The better option on a VGA CRT is 240p at 120 Hz, which gives real scanlines. This matters most for your VGA focus | New table by monitor class, including 240p at 120 Hz, 480i shown as 480p, and 70 Hz DOS |
| Geometry was adjusted only by moving porches | Changing timings is the risky way to nudge a picture and cannot be set per game | Split into signal geometry (per monitor, confirm-or-revert) and software image position (per game) |
| Mode switches did not say how buffers are handled | Rebuilding GL and buffers on each switch is what makes switching slow | Buffers allocated once at maximum size; a switch changes timings only |

### What the others do, and what NEPTRON takes

| Source | What it does | Taken into NEPTRON |
| --- | --- | --- |
| [RetroArch CRT SwitchRes](https://docs.libretro.com/guides/crtswitchres/) | Native or fixed super widths of 1920, 2560, 3840; dynamic super widths when `dotclock_min` is set; a 31 kHz 120 Hz mode for 240p; per-core and per-folder overrides | The three horizontal strategies; the 120 Hz path; overrides map onto our game, system, global settings |
| RetroArch | Notes that Pi composite has fixed resolution and cannot take custom modelines | Already corrected in the first review |
| [RePlayOS](https://www.replayos.com/changelog/) | Mode switches in 1 to 3 frames; live horizontal size and shift with confirmation and exact rollback, applied without rebuilding the video stack | Buffer-once design; signal geometry under confirm-or-revert |
| RePlayOS | 31 kHz profile plays 15 kHz content, shows interlaced games progressive, optional scanlines, native 70 Hz DOS | Monitor-class table |
| RePlayOS | Handheld low-resolution support on CRT; 192-line sources placed in a 224-line mode | Sources-that-do-not-fit table |
| RePlayOS | Separate software screen position, per system and per game | Software image position |
| RePlayOS | Sync mode selector; RGB range for DACs; gamma and RGB gain; interlace flicker filter | Signal options list |
| RePlayOS | Display filter with brightness-adaptive scanlines at fractional scales; analog filter for dither blending | Flat-panel shader rules relaxed; analog filter added to the built-in set |
| RePlayOS | Fixed core frequency and a minimum bootloader EEPROM version for stable CRT timing on Pi | Hardware notes |
| [Recalbox](https://www.recalbox.com/blog/2022-01-05-recalbox-rgb-dual-the-story) | A database of resolutions and frequencies per console and per game; mode switching by ROM region; multiple-resolution calibration screens | Launch prediction from region and core traces; calibration per mode family was already planned |
| [Batocera CRT Script](https://github.com/ZFEbHVUE/Batocera-CRT-Script/wiki/How-to-CRT-Script-Configuration) | Full Switchres preset list including named arcade chassis; recommended trial order for TVs; minimum pixel clock tiers; interlace force-even option; no 240p menu | Wizard order; expert pixel-clock override; quirks as capability flags. A 240p menu is a NEPTRON advantage, kept |

### Considered and not taken

- **Dual-screen modes** (RePlayOS): stays after alpha.
- **Dropping forced 50/60 Hz.** RePlayOS removed it for stability. You asked for it, so it stays, limited to profiles whose vertical range covers the target rate.
- **NVIDIA proprietary driver** (Batocera): closed source, conflicts with the project's rules.

### Could not check this time

- Batocera's "How Mode Switching Works" page and RePlayOS's video configuration page could not be opened (fetch limit). Their content is inferred from the pages that could be read.
- Whether KMS drivers deliver one page flip per field in interlaced modes varies; it is now a capability flag to measure per GPU, not an assumption.
- Composite-sync support on x86 VGA outputs through DRM is uncertain; the Definition says "where the output supports it".

## Third review: complexity, build or reuse

The plan hand-rolled six things that proven libraries already solve, and left four complex areas too thin to implement. Both are fixed. The core idea (one Rust frontend owning the display, with ODREN and a type-enforced safety gate) is unchanged, and several of these reversals undo choices I made earlier under a stricter reading of "all Rust".

### Hand-rolled without good reason, now reused

| Was | Now | Why the change |
| --- | --- | --- |
| Own Rust modeline solver, checked against Switchres fixtures | Switchres linked as a library; our Rust keeps the policy and the validator | The maths is safety-critical and already proven on real monitors by RetroArch, GroovyMAME and RePlayOS. A port would spend weeks re-earning that. Depends on a licence and build check |
| Own evdev layer and controller-mapping parser | SDL3 gamepad API | Controller quirks are a long tail. RePlayOS uses SDL3 for the same job |
| Own ALSA output and possibly own resampler | SDL3 audio and the `rubato` crate; rate control modelled on RetroArch's | Device handling and resampling are solved problems |
| Own Rust PID 1 | BusyBox init with respawn | Supervision comes free; a custom init saves milliseconds and adds a way to fail at boot |
| Own A/B updater | RAUC | A buggy updater bricks devices |
| Two UI renderers (CPU for tests, GPU for hardware) | One CPU renderer everywhere, composited as a texture | At 320x240 the CPU cost is negligible, and it removes a whole class of renderer-difference tests |

### Over-engineered, now cut or deferred

| Item | Decision |
| --- | --- |
| Virtual CRT oracle (a second monitor model) and mutation testing | Dropped. Switchres and the independent validator already check each other |
| Kernel `video=` string and generated EDID blob | Dropped. The kernel is kept from driving the display at all; to be confirmed in the M1 spike |
| GunCon 2 calibration converted mathematically across modes | Demoted to a later experiment; calibration per mode family for now |
| Automatic merging of community self-test reports into the matrix | Manual pull requests for now |
| Bluetooth pairing, web uploader, variable refresh on flat panels | After alpha |
| General layout engine for skins | Five fixed screen templates |
| Seventeen crates from day one | About ten to start; split when a boundary proves real |
| Eight M0 packages including Buildroot and QEMU before any product code | Six M0 packages with no hardware, Buildroot or QEMU; the emulated-OS tier arrives in M1 when the display code needs it |

### Complex but under-specified, now deepened

| Area | What was added |
| --- | --- |
| ODREN policy | An eight-step decision procedure in the Definition, which the policy tests are written from |
| Libretro host | Callback tiers and a spec task in the plan |
| KMS layer | A hardware spike as the first M1 task, before any KMS crate is written |
| Frame pacing and audio | A spec task that reuses RetroArch's rate-control method |

### Kept hand-rolled on purpose

- The KMS display layer, the libretro host, the ODREN policy, the safety validator: these are the product.
- The UI: small at this resolution, and no existing toolkit targets 320x240 pixel-exact output.
- The agent control API and test fakes: specific to this project.

### Open after this review

- Switchres's licence is reported as GPL-2.0-only by one packager; this was not confirmed against the source. It decides whether NEPTRON can link it and under which licence NEPTRON itself ships.
- SDL3, `rubato`, `embedded-graphics` and RAUC were chosen from general knowledge, not from pages read in this session.
- RAUC and SDL3 bring C dependencies and RAUC needs D-Bus, so the image grows. The size target of 300 MB should be rechecked in M1.

## Fourth review: fact check before the build (2026-11-10)

Three sub-agents opened the primary sources for every claim and link. The architecture stands. Two claims were wrong in ways that change the design, and seventeen more were corrected. All changes are applied in the Definition and Implementation plan tabs. Where this section contradicts an earlier review, this section is right.

### Wrong, with a design change

| Claim | What is true | Change |
| --- | --- | --- |
| The kernel keeps the CRTC running when the frontend dies | The kernel removes the framebuffers when the last holder closes the DRM descriptor, and amdgpu then switches the CRTC off ([`drm_framebuffer.c`](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/drm_framebuffer.c)) | New `neptron-launch` process holds the descriptor and passes it to the frontend; safety layer 7 rewritten |
| QEMU in the container uses the Mac's hypervisor | The hypervisor is available only to a native macOS process. An x86\_64 guest on Apple Silicon is always emulated | QEMU runs natively on a Mac with the aarch64 guest; the x86\_64 guest runs in CI with KVM |

### Corrected

| Area | Was | Now |
| --- | --- | --- |
| Switchres licence | "GPL-2.0-only per one packager", open question | GPL-2.0-or-later in the README and source headers; GPL-3.0-or-later for NEPTRON fits. The makefile has no macOS branch, so NEPTRON compiles the sources with the `cc` crate |
| Switchres `crt_range` | All values in microseconds; link to a gist | Vertical values are in milliseconds; link is now `switchres.ini` in the repository |
| Silent kernel | First signal is always NEPTRON's | True on Pi GPIO only. On x86 i915 inherits the firmware's mode until the first commit |
| Buildroot | "Current LTS" | The LTS (2025.02) has no SDL3 package and Rust 1.82. Use stable 2026.08 or later, pinned |
| RAUC | Needs D-Bus; uses Pi tryboot; GRUB boot counter | Builds without D-Bus. No Pi backend yet ([PR #1599](https://github.com/rauc/rauc/pull/1599)), so a custom-backend script. GRUB uses a one-shot try flag |
| Pi tryboot | Unconfirmed on Pi 3 and Zero 2 W | The documentation says all models support it; still to test |
| Input | SDL3 for all devices | SDL3 for gamepads only. Keyboards, mice and guns need the `evdev` crate, because SDL3 reads them only through its video driver |
| Module loading | By the removed `neptron-init` | BusyBox `mdev`, as in stock Buildroot |
| librashader | Pure Rust, MPL-2.0, no OpenGL ES | MPL-2.0 or GPL-3.0; builds C++ glslang and SPIRV-Cross; has an undocumented OpenGL ES 3 path; no OpenGL ES 2 |
| Pi composite | `enable_tvout` slows the system; boards 3, 4, 5 | It disables HDMI; Zero 2 W also has composite |
| Pi 5 DPI | "GPIO 1 to 3 reserved" | GPIO 1 is DE or composite sync, 2 is VSYNC, 3 is HSYNC; the PIO block is then in use |
| Pi 3 and 4 DPI interlace | Tier 1 with a small patch | Still needs the patch in rpi-6.18.y, and RePlayOS supports GPIO DPI on Pi 5 only, so there is no reference. Test in the M1 spike |
| Intel gen9 | Measurements cited as settled | One author, tested on a DisplayPort LCD; the Batocera CRT Script still lists Intel as unsupported. WP 1.1 decides |
| AMD | amdgpu for analog outputs | amdgpu supports analog outputs from kernel 6.19 only; on 6.18 use `radeon` or the legacy display path |
| Links | Recalbox GitHub mirror, arcadecontrols forum | Dead or unreachable; replaced with the GitLab file and a Wayback copy |
| Wheels, guns | `hid-fanatec`; GunCon "beam-timing values" | The module is `hid-fanatecff`; the README says raw values. Both GunCon modules are unmaintained |
| USB polling | 1000 Hz for all pads | `usbhid.jspoll=1` covers HID pads only; `xpad` uses the pad's own rate |

### Internal contradictions removed

- The monorepo tree still listed `neptron-init` and the virtual CRT oracle, both dropped in the third review.
- `neptron-modeline` and `neptron-safety` could not share a modeline type while depending on nothing. New `neptron-types` and `neptron-backend` crates hold the shared types and traits.
- The feature table listed Sinden for M4 while the input table left it open.
- Two risk rows described the own-solver design that Switchres replaced.

### Confirmed

Kernel 6.18 is the current longterm line and the Raspberry Pi tree tracks it. The D0023R patch set supports it. Intel interlace is absent from gen12 on. Connector `mode_valid` does not apply to user modes. `vkms` has hrtimer vblanks, CRCs and writeback. SDL3 hotplug works without udev. libretro has rumble but no force feedback. Every crate named exists and is maintained; `libretro-sys` crates are stale, so bindgen is right. All RePlayOS changelog claims match its text.

### Still open

- Whether `vkms` accepts interlaced user modes (T14 tests it).
- Whether the i3-6100T's DP-to-VGA bridge passes a 6.5 MHz clock (T21 and WP 1.1).
- Whether librashader's OpenGL ES 3 path works on Pi 4 and 5.
- Non-commercial cores in the default image: decided on 2026-11-10, accepted with notices. All decisions and open points are now at the end of the Definition tab.
