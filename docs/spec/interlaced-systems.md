# Interlaced and 480p systems

This spec makes exact the owner's eight decisions of OP-120 (2026-11-27) for systems that send 480 lines: a PlayStation that changes between 240p and 480i and between five widths, a 480i source, and a 640x480 progressive source (DOS VGA, the Dreamcast on its VGA cable). The rules of `docs/spec/video-policy-cases.md` hold where this spec does not change them. The mission: as plug and play as possible, and an accurate, sharp picture on a CRT. Monitor safety is absolute: every mode goes through `neptron_safety::validate` and the `ValidatedMode` token, inside the monitor profile and the limits of the GPU in `data/hardware/matrix.toml`.

## 1. Terms

- **240p game**: a progressive frame of up to 288 lines, for example 320x240 at 59.826 Hz.
- **480i game**: an interlaced frame of 289 lines or more (`SourceGeometry::interlaced`), for example 640x480 at 59.940 Hz. The core sends whole frames: both fields woven, 480 lines.
- **480p game**: a progressive frame of 480 lines, for example DOS VGA or the Dreamcast VGA cable, 640x480 at 60 Hz.
- **Display classes**: a 15 kHz CRT (`generic_15`, `arcade_15`), a 25 kHz CRT (`arcade_25`), a 31 kHz CRT (`pc_31_120`, `arcade_31`), an NTSC TV and a PAL TV (the composite output), a flat panel. A CRT with ranges in more than one class (`arcade_15_25_31`, `d9800`) follows the range that the policy picks.
- **The Intel limit**: the PC GPUs of `data/hardware/matrix.toml` (`intel_gen9`, `intel_i915`, `intel_xe`, and the AMD and NVIDIA entries) have `interlace = false`. They cannot send an interlaced mode. `pi_vc4_dpi` has the same limit. `pi5_rp1_dpi` and `pi_vc4_composite` send interlaced modes.

## 2. Decision 1: a mode change at each change between 240p and 480i

On a 15 kHz CRT and on a TV, a game that changes from 240p to 480i gets the interlaced mode, and a game that changes back gets the progressive mode, as the console does. This is the engine of today: a source of another height, scan or rate is a new decision, and a new mode is a modeset after the debounce (`DEBOUNCE_FRAMES`, 3 core frames). Decision 3 adds a limit of one modeset in a second.

- 15 kHz CRT with a GPU that sends interlace: 240p in the progressive mode 1:1, 480i in the interlaced mode 1:1.
- TV: 240p in 720x240p (NTSC) or 720x288p (PAL), 480i in 720x480i or 720x576i, lines 1:1.
- 15 kHz CRT with the Intel limit: decision 5.
- 31 kHz CRT and flat panel: no interlaced mode (decisions 6 and 8). A change of the rate still makes a new mode where the rate changes (the PlayStation runs 240p at 59.826 Hz and 480i at 59.940 Hz).

## 3. Decision 2: one wide mode for a game on a CRT

**The principle (owner, 2026-11-27).** NEPTRON follows the real hardware. A change that the real console makes with no new sync (a change of its width within the same line timing: a new dot clock, the same lines) needs no mode change and shows no black moment. A change that the real console makes with a new sync (the line count, interlace, the field rate) is a mode change. Where our limits stop us (a profile or a GPU on which no wide mode is valid), NEPTRON falls back to a mode change, and this section names those cases.

A system that changes its width in play names its widths in its data. The policy then gives every frame of the game, at one line count, scan and rate, one mode of one width: the **wide mode**. A change of the width is a new placement in the same mode (`Switch::ScaleOnly`): no modeset, no black moment, no mute.

**Which systems: data, not code.** The key `widths` of `data/systems/<system>.toml` (`data/systems/README.md`) lists the widths that the games of the system draw in play with no new sync, the most used first. `neptron-library` reads and checks it (two to 8 different widths from 1 to 4096), the app passes it with the game (`GameContext::widths`), and `neptron_video::wide_strategy` makes `HorizontalStrategy::Wide` of it. The policy takes the list as an argument (`GameWidths`); no system name is in the code. A system with no key keeps the behaviour of today.

| System | `widths` | The core |
| --- | --- | --- |
| SNES | 256, 512 | Snes9x sends each frame at 256 or 512 pixels, with no geometry event. The session reads the size of each frame, so the engine sees the change. The trace of the repository has 256-pixel frames only, so a 512 frame of the core is not checked here (hardware checklist). |
| Mega Drive | 320, 256 | Genesis Plus GX sends 320 or 256 pixels (overscan `disabled`) and reports a new geometry at the change (`tests/traces/genesis_plus_gx.trace`: 256x192 at load, then 320x224). |
| PlayStation | 320, 256, 640, 512, 368 | Its system file comes with its core in task S10. Until then the list is a test fixture: `crates/neptron-modeline/tests/integration/policy_interlaced.rs`, `crates/neptron-video/tests/integration/playstation.rs` and `PLAYSTATION_WIDTHS` of the screen simulator (`crates/neptron-testkit/src/screen/case.rs`). |

The NES, the Neo Geo and the arcade boards draw one width per game and have no key.

**Which width.** `HorizontalStrategy::Wide`, resolved in the policy for the GPU:

1. The base: the least common multiple of the longest run of the widths, from the first, that is at most the widest mode of the GPU (`GpuCaps::max_width`, `HorizontalStrategy::wide_base`). SNES: 512. Mega Drive: 1280. PlayStation on a GPU of 1920 pixels: 320, then 1280 (with 256), then 1280 (with 640), then 2560 (with 512), which is too wide, so 1280.
2. The wide width: the smallest whole multiple of the base whose pixel clock reaches the lowest pixel clock of the GPU, within its widest mode.
3. Only a frame whose width the list names takes the wide mode. A frame of another width (Genesis Plus GX with its overscan option on sends 348 pixels, which no Mega Drive draws) is no width of the console: it takes the mode of today for its size, at a whole factor, and a change to it is a modeset, never a fraction inside the wide mode.
4. Where no multiple gives a mode, or the validator refuses the wide mode, or the shape is bordered (a game with fewer lines than the range holds, which a mode wider than the game in proportion to each width pads), the candidate takes its mode of today (`HorizontalStrategy::default_for`), which follows the wide proposal in the plan. The validator judges every proposal, so the wide mode never widens a limit.

**Each width at a whole factor.** Every width of the base run is a whole number of mode pixels, so the filter (sharp bilinear) is the same as nearest. Only a PlayStation width outside the run (512 and 368 in a 1280 mode) is scaled by a fraction, with sharp bilinear: one mode pixel at the edge of a game pixel blends. A 15 kHz tube resolves about 700 to 900 dots on its active line, so a blend one 1280th of the line wide does not show.

| System, GPU, monitor | Mode width | Factor of each width |
| --- | --- | --- |
| SNES, Intel or AMD, 15 kHz | 1536 | 256: 6, 512: 3 |
| SNES, Intel, 31 kHz (`pc_31_120`) | 1024 | 256: 4, 512: 2 |
| SNES, Raspberry Pi DPI (no clock floor) | 512 | 256: 2, 512: 1 |
| Mega Drive, Intel, 15 kHz and 31 kHz | 1280 | 320: 4, 256: 5 |
| Mega Drive, Raspberry Pi DPI | 1280 | 320: 4, 256: 5 |
| PlayStation, every GPU of the matrix | 1280 | 320: 4, 256: 5, 640: 2; 512: 2.5 and 368: 3.478 with sharp bilinear |

**Where the mode of today stays.** The test `every_width_of_a_system_is_a_whole_factor_of_one_mode_or_the_mode_of_today` runs every preset of `data/monitors` with `intel_gen9`, `amd_amdgpu`, `pi_vc4_dpi`, `pi5_rp1_dpi` and the unknown GPU. Each width either takes one shared mode at whole factors, with the lines, the scan, the placement height and the rate (within `REFRESH_MATCH_MHZ`) of today, or takes the mode of today. The cases that keep the mode of today, so a width change there is still a modeset:

- The SNES and the Mega Drive on `arcade_25`, with every GPU: 224 lines are padded into the 384 lines of the 25 kHz range (a bordered shape).
- The Mega Drive on `d9200`, `d9400`, `d9800`, `m3129`, `pstar` and the CRT presets `ntsc` and `pal`, with Intel, AMD and the unknown GPU: 1280 pixels at the line rate of those ranges is below the pixel clock floor of 25 MHz, and 2560 is wider than the GPU. With a GPU of no floor (the Raspberry Pi) the wide mode holds.

**What the player sees.** The same picture, at the same shape and borders: the measures of the screen simulator before and after are equal (section 12). Only the mode width changes.

| Case | Before: mode, factor | After: mode, factor |
| --- | --- | --- |
| SNES 256 on `generic_15` and `arcade_15`, Intel | 1280x224p, 5 | 1536x224p, 6 |
| SNES 512 on `generic_15` and `arcade_15`, Intel | 1536x224p, 3 | 1536x224p, 3 |
| SNES 256 on `pc_31_120`, Intel | 768x448p, 3 | 1024x448p, 4 |
| SNES 512 on `pc_31_120`, Intel | 1024x448p, 2 | 1024x448p, 2 |
| Mega Drive 320 and 256 on `generic_15` and `arcade_15`, Intel | 1280x224p, 4 and 5 | the same |
| Mega Drive 320 on `pc_31_120`, Intel | 640x448p, 2 | 1280x448p, 4 |
| Mega Drive 256 on `pc_31_120`, Intel | 768x448p, 3 | 1280x448p, 5 |
| PlayStation 256, 320, 640 on `generic_15`, Intel | 1280x240p | the same |
| PlayStation 368 and 512 on `generic_15`, Intel | 1472x240p and 1536x240p, 4 and 3 | 1280x240p, 3.478 and 2.5 |
| PlayStation on `pc_31_120`, Intel | 640 to 1024 by width | 1280x480p |

The policy goldens, the console goldens, the reference sources of Settings > Display and the source names of the screen simulator with no system (`snes`, `psx`, `psx_480i`) name no widths, so none of them changes. The names `snes_*`, `md_*` and `ps1_*` carry the widths of their system (section 12).

A TV has one mode width (720 samples) for every width already, and a flat panel has one mode. Decision 2 changes neither.

## 4. Decision 3: at most one mode change in a second

A modeset that the game's own frames ask for (`Trigger::Geometry`) waits until one second has passed since the last modeset (`MODESET_MIN_INTERVAL_NS`, 1,000,000,000 ns). The time is the core frames since the frame of the last modeset, at the rate of the source on screen: `frames x 10^12 / refresh_mhz` ns. No setting changes it. The debounce of 3 frames still holds; the hold adds to it.

- While a change is held back, `begin_frame` returns `Directive::Waiting`. The mode on screen stays. The new frame shows in it by the rule of a frame of another size (`docs/spec/video-policy-cases.md`, 8.12, rule 5): whole steps, centred, where the plan is whole steps; the middle lines when even one step is too tall. A 480i frame held in a 240p mode shows its middle 240 lines for at most one second (question W2).
- A game that changes faster than once a second keeps its mode until it settles: when it goes back to the source on screen, nothing changes. When it settles on a new source, the modeset comes at the first frame that is both debounced and one second after the last modeset.
- The launch, a change of the settings, the monitor, the orientation, a retry and a refused mode on screen are not held: the player asked for them, or safety needs them. They count as the last modeset for the next hold.
- A scale-only change (decision 2, or a winner equal to the mode on screen) is no modeset and is never held.

What changes in the tests: the switch trace golden of the video engine (`crates/neptron-video/tests/golden/switch_trace.txt`) shows the modesets one second apart where the script changed sooner.

## 5. Decision 4: the flicker filter

A setting, `display.flicker_filter`, label "Flicker filter", values `off` (default) and `on`, with the game, system and global levels (`Levels::ALL`). Safety `Live`: it changes no mode.

**The blend.** On an interlaced mode, each line of the game picture, after the scale into `dest` and before the scanlines and the UI canvas, takes a quarter of the line above and a quarter of the line below: `out(y) = (in(y - 1) + 2 x in(y) + in(y + 1) + 2) / 4`, per 8-bit channel, with the edge lines of `dest` repeated outside it. The lines are the lines of the interlaced frame (both fields). A detail one line high then shows in both fields, at half its strength, so it does not flash at 30 Hz. The UI canvas is not filtered: it is line-doubled on an interlaced mode and does not flicker.

**Where.** A CRT and a TV in an interlaced mode. On a progressive mode, on a flat panel and with `off` the picture does not change. The row shows on every display; on a flat panel it is disabled with "A flat panel shows no interlaced picture."

**What it costs.** Two more reads of the picture per pixel of `dest`: in the GLES renderer two more texture taps in the fragment shader of the game quad, in the reference renderer two more reads per pixel. A 1280x480 interlaced picture is 0.6 million pixels a frame.

**Texts.**

| Item | Text |
| --- | --- |
| Label | Flicker filter |
| Help, `off` | Thin lines on an interlaced picture can flicker. |
| Help, `on` | Blends each line with its neighbours on an interlaced picture: less flicker, a softer picture. |
| Disabled on a flat panel | A flat panel shows no interlaced picture. |

**Blocked: a contract change.** The renderer reads only the `Scene`, and the `Scene` has no field that asks for a vertical blend. The filter needs a new field in `neptron_types::Scene`: `vertical_blend: VerticalBlend`, an append-only enum (`None`, `Flicker`), which `build_scene` sets to `Flicker` when the setting is `on` and the mode is interlaced. The alternative is a call on the `Display` trait (`Display::set_flicker_filter(bool)`), which keeps the `Scene` but adds a state outside the one value that a flip shows, so the preview and the run could differ. The recommendation is the `Scene` field. Until the owner of the contract approves it, the setting, its row and the renderer stage are not built.

## 6. Decision 5: the Intel limit on a 15 kHz CRT

A GPU with `interlace = false` on a 15 kHz CRT cannot send 480i. A 480i game there shows all its lines pressed into the progressive lines of the tube, by the Fit rule (`docs/spec/video-policy-cases.md`, 8.12): every line reaches the screen, at a scale of 0.5 (240 lines) to 0.517 (248 lines), with sharp bilinear, which blends each pair of lines. The shape stays 4:3. This is the picture of today (the fitted rung, `SoftReason::TooTall`).

The notice reason is new: `SoftReason::InterlaceNotSent { lines }`. The policy gives it when the source is interlaced, the winner is the fitted rung on a mode of the 15 kHz class (line rate below `CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ`), a range of the profile declares interlaced lines, and the GPU sends no interlaced mode (`Plan::interlace_held_by_gpu`). On a profile with no interlaced range the reason stays the one of today (`SoftReason::TooTall`): the monitor, not the PC, stops the interlaced mode. Text:

> This game is interlaced. This PC cannot send an interlaced picture to a 15 kHz monitor, so the picture is soft.

A 240p game on the same PC is not touched: 240p at 1:1.

## 7. Decision 6: a 31 kHz CRT

- A 240p game shows as 480p, each line twice (2x), with the scanline rule of today (`neptron_types::scanlines_for`: drawn at 2x above 15 kHz, scanlines `auto`).
- A 480i game shows as 480p, full frames, 1:1: the 480 lines of the woven frame on 480 progressive lines. No flicker, no scanlines (1:1).
- A 480p game shows 1:1 as 480p.

This is the policy of today. No new setting.

## 8. Decision 7: the Dreamcast and Naomi cable

`cable_signal(profile)` in `neptron-modeline` says which signal the monitor takes, with no setting:

| Display | Signal |
| --- | --- |
| Flat panel | `Vga`: 640x480 progressive |
| CRT with a range that holds 480 progressive lines at 60 Hz (31 kHz, multi-sync, and a 15/25/31 kHz tube) | `Vga` |
| CRT without such a range (15 kHz, 25 kHz) | `Interlaced`: 640x480 interlaced |
| TV | `Interlaced` |

On a 15 kHz CRT with the Intel limit, the 480i picture of the Dreamcast then follows decision 5. The core option, the fall back of a game that refuses VGA, and its notice are task S11.

## 9. Decision 8: a flat panel

A 480i game shows the frames of the core, as they come, scaled by the panel rule: 2x in 960 lines on a 1080p panel, nearest. No deinterlace filter. This is the policy of today.

## 10. The cases

The simulator sources of this spec: `snes_256` and `snes_512` (224 lines at 60.099 Hz) and `md_320` and `md_256` (224 lines at 59.922 Hz) with the widths of their system data; `ps1_256`, `ps1_320`, `ps1_368`, `ps1_512` and `ps1_640` (240 lines at 59.826 Hz) and `ps1_480i` (640x480 interlaced at 59.940 Hz) with the PlayStation widths; and `vga_480p` (640x480 progressive at 60.000 Hz, no widths). The GPU is the default of the simulator (`intel_gen9` on a CRT and a panel, `pi_vc4_composite` on a TV); the column "interlace GPU" is `pi5_rp1_dpi`.

| Display | PlayStation 240p, every width | PlayStation 480i | 480p (VGA) |
| --- | --- | --- | --- |
| `generic_15`, Intel | 1280x240p, 1:1, 59.826 Hz | 1280x240p, 480 lines at 0.5, smoothed, notice of decision 5 | 1280x240p, fit at 0.5, soft (`TooTall`) |
| `generic_15`, interlace GPU | 1280x240p, 1:1 | 1280x480i, 1:1 | 640x480i, 1:1 |
| `arcade_15`, Intel | 1280x240p, 1:1 | 1280x248p, at 0.517, notice of decision 5 | 1280x248p fit |
| `pc_31_120`, Intel | 1280x480p, 2x, scanlines | 1280x480p, 1:1, full frames | 640x480p, 1:1 |
| `ntsc` | 720x240p, console width | 720x480i, 1:1 | 720x480i, 1:1 |
| `pal` | 720x288p, console width, 50 Hz | 720x576i, 480 lines 1:1, 50 Hz | 720x576i |
| 1080p panel | 1920x1080p, fill 4.5 | 1920x1080p, 2x in 960 lines | 1920x1080p, 2x |

The measures of each case are in section 12.

## 11. Questions for the owner

| Id | Question | Options | Default |
| --- | --- | --- | --- |
| W1 | Which systems get the wide mode? | Answered by the owner on 2026-11-27: every system whose real console changes its width with no new sync, from its data (section 3). | SNES, Mega Drive; PlayStation in task S10 |
| W2 | What a held-back frame shows (decision 3), for at most one second: | (a) The rule of today: whole steps, centred, the middle 240 lines of a 480i frame in a 240p mode. (b) A 480i frame pressed into the 240p mode, smoothed, every line shown. | (a), as built |
| W3 | The text of decision 5 says "This PC". A Raspberry Pi on DPI has the same limit. | (a) The text as decided on every machine. (b) "This Raspberry Pi" on a Pi. | (a), as built; the Pi 3 DPI with a 480i game is rare |
| W4 | The flicker filter on a TV (decision 4 names a CRT; a TV in 480i flickers the same way). | (a) Every interlaced mode, CRT and TV. (b) A CRT monitor only. | (a) |

## 12. The measures

`cargo xtask screen golden` writes the glass of each case to `target/screen/golden/<name>.png`; the goldens are in `crates/neptron-testkit/tests/golden/screen/`. The unit tests `the_interlaced_cases_match_their_goldens_and_measures`, `the_width_cases_match_their_goldens_and_measures` and `the_widths_of_a_system_share_one_mode_on_a_crt` assert the measures below, and that every case shows every line, drops none, and stands upright. Fill is the share of the visible glass, width by height, in per mille; shape is the error of the placement in per mille. On a TV the placement error is measured against the policy's own width rule, so it cannot prove the shape; the tables give the error on the glass model too (`glass_aspect_error_permille`). Those glass values record the picture of a TV as it is on `main` (a console picture keeps its width in the field). They are not a claim that the shape is right; the question is with the owner, and this task changes nothing in the policy for a TV.

| Case (`INTERLACED_CASES`) | Mode | `dest` | Scale x / y | Fill | Shape | Notice |
| --- | --- | --- | --- | --- | --- | --- |
| `ps1_320-generic_15` | 1280x240p, 59.826 Hz | 1280x240 | 4 / 1 | 1000 / 995 | 0 | none |
| `ps1_512-generic_15` | 1280x240p | 1280x240 | 2.5 / 1 | 1000 / 995 | 0 | none |
| `ps1_480i-generic_15` (Intel) | 1280x240p, 59.940 Hz | 1280x240 | 2 / 0.5 | 1000 / 997 | 0 | decision 5 |
| `ps1_480i-generic_15-pi5` | 1280x480i | 1280x480 | 2 / 1 | 1000 / 995 | 0 | none |
| `ps1_320-arcade_15` | 1280x240p | 1280x240 | 4 / 1 | 1000 / 995 | 0 | none |
| `ps1_480i-arcade_15` (Intel) | 1280x248p | 1280x248 | 2 / 0.517 | 1000 / 998 | 0 | decision 5 |
| `ps1_320-pc_31_120` | 1280x480p, scanlines 75 % | 1280x480 | 4 / 2 | 1000 / 947 | 0 | none |
| `ps1_480i-pc_31_120` | 1280x480p | 1280x480 | 2 / 1 | 1000 / 948 | 0 | none |
| `vga_480p-pc_31_120` | 640x480p, 60.000 Hz | 640x480 | 1 / 1 | 999 / 949 | 0 | none |
| `ps1_320-ntsc` | 720x240p | 640x240 at 40 | 2 / 1 | 1000 / 1000 | placement -6, glass -91 | none |
| `ps1_480i-ntsc` | 720x480i | 640x480 at 40 | 1 / 1 | 1000 / 1000 | placement -6, glass -91 | none |
| `ps1_320-pal` | 720x288p, 50.080 Hz | 640x240 at 40, 24 | 2 / 1 | 1000 / 926 | placement -15, glass 91 | speed |
| `ps1_480i-pal` | 720x576i, 50.000 Hz | 640x480 at 40, 48 | 1 / 1 | 992 / 926 | placement -15, glass 91 | speed |
| `ps1_480i-panel_1080` | 1920x1080p | 1280x960 at 320, 60 | 2 / 2 | 667 / 889 | 0 | none |

The placement error on a TV is the console width rule of today (640 samples against 644 on NTSC and 650 on PAL).

| Case (`WIDTH_CASES`) | Mode after | `dest` after | Factor | Fill before and after | Shape before and after (placement; glass on a TV) |
| --- | --- | --- | --- | --- | --- |
| `snes_256-generic_15`, `snes_256-arcade_15` | 1536x224p (before 1280x224p) | 1536x224 | 6 (before 5) | 1000 / 932 | 0 |
| `snes_512-generic_15`, `snes_512-arcade_15` | 1536x224p (no change) | 1536x224 | 3 | 1000 / 932 | 0 |
| `md_320-generic_15`, `md_320-arcade_15` | 1280x224p (no change) | 1280x224 | 4 | 1000 / 931 | 0 |
| `md_256-generic_15`, `md_256-arcade_15` | 1280x224p (no change) | 1280x224 | 5 | 1000 / 931 | 0 |
| `snes_256-ntsc`, `md_256-ntsc` | 720x240p (no change) | 640x224 at 40, 8 | 2.5 | 1000 / 1000 | placement -6, glass -26 |
| `snes_512-ntsc` | 720x240p (no change) | 644x224 at 38, 8 | 1.258 | 1000 / 1000 | placement 0, glass -20 |
| `md_320-ntsc` | 720x240p (no change) | 640x224 at 40, 8 | 2 | 1000 / 1000 | placement -6, glass -26 |
| `snes_256-pal`, `md_256-pal` | 720x288p (no change) | 640x224 at 40, 32 | 2.5 | 1000 / 864 | placement -15, glass 169 |
| `snes_512-pal` | 720x288p (no change) | 650x224 at 35, 32 | 1.270 | 1000 / 864 | placement 0, glass 187 |
| `md_320-pal` | 720x288p (no change) | 640x224 at 40, 32 | 2 | 1000 / 864 | placement -15, glass 169 |

On `pc_31_120` with Intel (no golden; section 3): the SNES at 256 goes from 768x448p (3) to 1024x448p (4), the Mega Drive at 320 from 640x448p (2) and at 256 from 768x448p (3) to 1280x448p (4 and 5); the fill (1000 / 887 and 1000 / 884) and the shape (0) stay.
