## CRT video pipeline

NEPTRON's DynaRes equivalent is a closed loop: the core states what it draws, a solver turns that into a modeline for the active monitor profile, a validator approves it, and one atomic KMS commit applies it. It is called the Open Dynamic Resolution Engine, **ODREN**.

&#91;embedded content: mode-switch flow · 5 steps, 1 decision\]

A rejected candidate loops back to the solver, which offers the next-best mode or the profile's safe fallback. The refresh actually achieved drives audio resampling and frame pacing.

**Inputs**

- `retro_system_av_info` at load: base width, height, aspect and frames per second.
- `SET_GEOMETRY` and `SET_SYSTEM_AV_INFO` callbacks, plus the real size of every frame the core submits. Many cores change resolution mid-game without announcing it.
- The settings stack: refresh policy, interlace policy, geometry offsets, per-game overrides.

**Mode solver** (`neptron-modeline`, a pure function with no I/O)

`(source geometry, refresh, monitor profile, GPU capabilities) -> ranked list of modelines`

- Monitor profiles use the Switchres [`crt_range`](https://github.com/antonioginer/switchres/blob/master/switchres.ini) model: horizontal and vertical frequency windows, horizontal porches and sync in microseconds, vertical ones in milliseconds, and line-count limits for progressive and interlaced. The preset names (`generic_15`, `ntsc`, `pal`, `arcade_15`, `arcade_15_25_31`, `vesa_480` and so on) stay compatible so existing community knowledge transfers.
- Vertical is exact whenever the source fits the profile: one emulated line is one scanline. Sources that do not fit follow the table below. Refresh matches the core to two decimals when the profile allows it.
- Horizontal follows one of three strategies, chosen per GPU and per system; see the table below. A tube has no horizontal pixel grid, so horizontal scaling costs far less than vertical scaling would.
- When the source does not fit (for example 480 lines on a 15 kHz profile) the solver picks interlace. When nothing fits it returns a scored fallback with the reason, never an out-of-range mode.
- Modelines are calculated by Switchres itself, linked as a library behind a small Rust wrapper, so NEPTRON does not re-derive maths that years of real monitors have tested. ODREN's own code is the policy on top: which strategy, which picture style, which fallback. The Rust validator shares no code with Switchres, which makes it a true second opinion.

**Horizontal strategy**

| Strategy | How it works | Width changes mid-game | Default for |
| --- | --- | --- | --- |
| Native | Mode width equals the game's width | Modeset | GPUs with no minimum pixel clock: Pi GPIO DPI, AMD analog |
| Dynamic super width | Smallest whole multiple of the game's width that clears the GPU's minimum pixel clock; exact integer scale | Modeset | GPUs with a pixel-clock floor |
| Fixed super width | One wide mode (1920, 2560 or 3840) for everything; fractional horizontal scale with a sharp filter | No modeset | Systems that change width during play or mid-frame: PlayStation, SNES, Mega Drive, Amiga |

The minimum pixel clock comes from the GPU capability table and can be raised in an expert menu, as Switchres does with `dotclock_min`. Each system definition names its preferred strategy.

**What each monitor class gets**

| Monitor profile | 240p-class games | 480i-class games | Menu |
| --- | --- | --- | --- |
| 15 kHz TV or arcade | Native 240p or 288p | Native 480i or 576i, optional flicker filter | 240p, with a 288p 50 Hz option for PAL-only sets |
| 31 kHz VGA, 60 Hz only | Line-doubled to 480p, optional black scanlines | Shown as 480p progressive | 480p |
| 31 kHz VGA, 120 Hz capable | **240p at twice the game's rate** (120 Hz for 60 Hz games), each frame shown twice: real scanlines on a PC monitor. Opt-in; the default is line-doubled 480p with scanlines | 480p progressive | 240p at 120 Hz, or 480p |
| Multisync arcade (15/25/31 kHz) | Native 240p at 15 kHz | Native 480i, or 480p at 31 kHz | 240p |
| High-resolution PC CRT | 240p at 120 Hz, or integer scaling with scanline shaders at 1024x768 and above | 480p | 480p |

RetroArch offers the 120 Hz path as its ["31 kHz, 120 Hz" Switchres mode](https://docs.libretro.com/guides/crtswitchres/), and Recalbox uses 240p at 120 Hz for its menu on 31 kHz screens per its [release notes](https://gitlab.com/recalbox/recalbox/-/raw/master/RELEASE-NOTES.md). DOS games get their native 70 Hz VGA mode on 31 kHz profiles.

**Picture style: the one setting that chooses among these**

The table above lists what is possible; the user picks with a single "Picture style" setting in the Display panel, overridable per system and per game and changeable live from the quick menu. Refresh is independent of the style: every CRT mode is generated, so each style runs at the game's exact rate, or exactly twice it, whenever the monitor's range allows.

| Picture style | Signal sent | Scanlines | Needs |
| --- | --- | --- | --- |
| Auto (default) | Native on 15 kHz; 2x on 31 kHz | Real on 15 kHz; shader on 31 kHz | Nothing |
| Native | 240p or 288p at the game's rate | Real | 15 kHz capable monitor |
| Native, double refresh | 240p at twice the game's rate, frame repeated or black frame inserted | Real | 31 kHz monitor that accepts about 100 to 120 Hz |
| 2x | 480 lines progressive at the game's rate (448 for a 224-line game) | Off, light, medium, strong | Any 31 kHz monitor |
| 3x | 720 lines, for example 960x720 | Shaped scanlines, one dark line in three | About 45 kHz |
| 4x | 960 lines, for example 1280x960 | Shaped scanlines | About 60 kHz |
| Fixed mode | A mode the user picks, such as 1024x768; game integer-scaled inside it | Shader | For monitors that dislike custom modes |

- The line count is the game's own height times the factor, so a 224-line game at 3x is a 672-line mode with no borders.
- Each row shows the resulting line rate and field rate before it is applied, for example "45.3 kHz, 59.94 Hz, in range". Styles the active monitor profile cannot display are greyed out with the reason.
- Double refresh has a trade-off the menu states: repeating frames keeps brightness but doubles moving edges; inserting black frames gives correct motion at half brightness with 60 Hz flicker. That is why it is not the default.
- Two companion settings sit beside it: scanline strength, and refresh policy (`exact`, `exact_x2`, `force_50`, `force_60`). In the settings files the picture style takes `auto`, `native`, `2x`, `3x` or `4x`.

**Decision procedure.** ODREN runs these steps at launch and whenever the core's geometry changes. This list is the specification the policy tests are written from.

1. Take the source: width, height, refresh, interlaced or not, orientation after the rotation setting.
2. Read the effective settings: picture style, refresh policy, horizontal strategy, monitor profile, GPU capabilities.
3. Build candidates by walking the pixel-perfect ladder. Auto walks every rung in order; an explicit picture style starts at its own rung and falls through the ones below.
4. For each candidate set lines to height times the factor, refresh to the game's rate (times two for double refresh, or the forced rate), and width by the horizontal strategy.
5. Ask Switchres for the modeline for that width, line count, refresh and interlace flag under the profile.
6. Validate it. The first candidate that validates wins; the reasons the earlier ones failed are kept for the log and the notice.
7. If the winner equals the current mode, update only scale and crop. Otherwise wait until the request has been stable for a few frames, then commit.
8. If the winner is the fitted rung, or its refresh differs from the game's, show the notice.

**Sources that do not fit the monitor**

| Source | Example | What ODREN does |
| --- | --- | --- |
| Fewer lines than the profile's minimum | Game Boy (144), Game Gear, Nintendo DS (192) | Centre it in a 224 or 240-line mode with borders; optional integer or fitted scaling |
| More lines than progressive allows | 480 lines on a 15 kHz set | Interlace, one page flip per field so 60 Hz motion is kept |
| Medium resolution | 384-line arcade boards at 24 kHz | Native on a multisync profile; interlaced fit on 15 kHz; 480p fit on 31 kHz |
| Refresh outside the vertical range | 70 Hz DOS on a TV | Nearest in-range rate, with the mismatch shown in the quick menu |
| Vertical game on a horizontal screen | Arcade shooters | See the vertical games section |

**Signal options** (Display panel, per monitor profile)

- Sync: separate H/V or composite sync, with polarity, where the output supports it. A TV on SCART needs composite sync; some outputs can only provide it through an external sync combiner.
- Colour: full or limited RGB range for HDMI and DisplayPort DACs, per-channel gain and gamma, and ordered dithering for 18-bit GPIO DACs such as VGA666 to hide banding.
- Interlace flicker filter strength.
- Launch prediction: system, ROM region and recorded core traces decide the first mode before the core starts, so a PAL game does not open at 60 Hz and switch a second later.

**Switching**

- The mode family for a game is computed at launch, so the first frame already appears at the right timing.
- With a fixed super width a horizontal change (PlayStation moving between 256, 320, 512 and 640) needs no modeset at all, only a new scale factor. Only line count, refresh or interlace changes cause a real switch.
- Switches are debounced, applied with an atomic commit at vertical blank, and audio is muted for the frames the monitor needs to resync.

* Scanout buffers are allocated once at the largest size any mode needs, and each mode shows a cropped region of them. A switch changes only timings, never the GL or buffer setup, which is what keeps it to a few frames. RePlayOS reports 1 to 3 frames with the same approach in its [changelog](https://www.replayos.com/changelog/).
* Software-rendered cores can draw straight into those buffers through libretro's software-framebuffer callback, saving a copy per frame. libretro.h marks that callback experimental, so it is an optimisation and never a requirement.

**Timing**

- The core runs one frame per page-flip event. The display is the master clock.
- The solver reports the refresh actually achievable with the GPU's clock granularity. Audio is resampled with dynamic rate control against that real figure, not the nominal one.
- Target: zero dropped or duplicated frames over a 30-minute run on every tier-1 output. This is an automated test, not a hope.

**Force 50 Hz / 60 Hz**

A `refresh_policy` setting takes `exact`, `exact_x2`, `force_60` or `force_50` at global, system or game level. It is independent of the picture style: the style `native` with the policy `exact_x2` gives the "Native, double refresh" row of the picture style table. Where a core exposes a region option, NEPTRON sets it to match. Where it does not, the game runs at the forced rate, speed changes by the ratio, and the quick menu says so. RePlayOS removed its forced 50/60 modes for stability reasons, so forcing is only offered where the profile's vertical range covers the target rate.

**Calibration**

- A built-in pattern generator, drawn by the frontend itself, so every mode family (240p60, 288p50, 480i, 31 kHz) can be adjusted with its exact modeline: grid, overscan markers, colour bars, grey ramp, convergence dots, a scrolling test and a 50/60 flash test.
- Two kinds of geometry control. Signal geometry moves porches inside the profile's limits to shift or resize the raster; it is stored per monitor profile and mode family and uses confirm-or-revert. Image position shifts the picture inside the active area in software; it carries no risk and can be set per system or per game.
- The [240p Test Suite](https://github.com/ArtemioUrbina/240pTestSuite) ships as a homebrew ROM launched through a regular core, listed under Settings > Calibration. The suite is GPL-2.0, so an image that bundles it must carry its source offer.
