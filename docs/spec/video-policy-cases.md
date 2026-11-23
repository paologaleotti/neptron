# Video policy cases

This spec holds the cases of the video policy (`crates/neptron-modeline`, the ODREN policy) beyond arcade: consoles, handhelds and computers, the invariant sweep, and the open questions they raise. The rule is the one of `docs/spec/arcade.md`, section 11, as the owner decided it on 2026-11-16 and 2026-11-17. That section holds the arcade boards, the decisions and the notices. This file holds the rest because the content is no longer only arcade.

## 1. The sources

`crates/neptron-modeline/tests/integration/arcade_table.rs` runs two lists through `decide` and `choose` with `neptron_safety::validate`, on the 36 displays of the arcade table (every preset of `data/monitors`, four EDID tubes, five flat panels, NTSC and PAL):

- `BOARDS`: the 18 arcade boards of `docs/spec/arcade.md`.
- `SOURCES`: 44 more sources. Each one names where its size and rate come from in a comment. A value marked UNVERIFIED is the common figure of the core, not checked: the PlayStation, Saturn and N64 rates, the Mega Drive 448-line interlace, the Lynx, Neo Geo Pocket and Virtual Boy rates, the C64 border size and the Amiga rates. A system that also has goldens (NES, SNES, Mega Drive NTSC and the others of `tests/golden/systems.toml`) keeps their geometry and 4:3, and the test `the_wider_table_agrees_with_the_console_goldens` holds it to `policy.toml` on the profiles they share. A handheld comes with its own display aspect (Game Boy 10:9, Game Boy Advance 3:2, WonderSwan 14:9, Lynx 160:102, Neo Geo Pocket 20:19, Virtual Boy 384:224).

To print every table and the summary: `cargo xtask test -p neptron-modeline --no-capture sources_table`. The full table has 1,584 rows; it is the output of the test, not a part of this file. Every cell that follows the rule or carries an allowed reason is a golden in `tests/golden/policy_sources.toml`.

## 2. The summary by class

The verdicts of the rule after the changes of 2026-11-16 and 2026-11-17, for every source of both lists. "ok" follows the rule; "ok:" and a reason is an allowed exception. No cell breaks the rule.

| Source class | Display class | Cells | Verdicts |
| --- | --- | --- | --- |
| PAL | 15 kHz | 45 | ok: 43, ok: no exact mode exists on this profile: 1, ok: overscan crop: 1 |
| PAL | 25 kHz | 5 | ok: 5 |
| PAL | 31 kHz | 10 | ok: 10 |
| PAL | TV | 10 | ok: console width rule: 10 |
| PAL | multi-range preset | 65 | ok: 65 |
| PAL | multi-sync EDID | 15 | ok: 15 |
| PAL | multi-sync preset | 5 | ok: 5 |
| PAL | panel | 25 | ok: 25 |
| computer | 15 kHz | 81 | ok: 78, ok: no exact mode exists on this profile: 1, ok: no window at the game's rate: 1, ok: overscan crop: 1 |
| computer | 25 kHz | 9 | ok: 7, ok: overscan crop: 2 |
| computer | 31 kHz | 18 | ok: 17, ok: overscan crop: 1 |
| computer | TV | 18 | ok: 11, ok: console width rule: 7 |
| computer | multi-range preset | 117 | ok: 114, ok: no window at the game's rate: 1, ok: overscan crop: 2 |
| computer | multi-sync EDID | 27 | ok: 27 |
| computer | multi-sync preset | 9 | ok: 9 |
| computer | panel | 45 | ok: 45 |
| console | 15 kHz | 108 | ok: 108 |
| console | 25 kHz | 12 | ok: 10, ok: no window at the game's rate: 2 |
| console | 31 kHz | 24 | ok: 24 |
| console | TV | 24 | ok: 3, ok: console width rule: 21 |
| console | multi-range preset | 156 | ok: 156 |
| console | multi-sync EDID | 36 | ok: 36 |
| console | multi-sync preset | 12 | ok: 12 |
| console | panel | 60 | ok: 60 |
| console hi-res | 15 kHz | 99 | ok: 99 |
| console hi-res | 25 kHz | 11 | ok: 9, ok: no window at the game's rate: 2 |
| console hi-res | 31 kHz | 22 | ok: 22 |
| console hi-res | TV | 22 | ok: 2, ok: console width rule: 20 |
| console hi-res | multi-range preset | 143 | ok: 143 |
| console hi-res | multi-sync EDID | 33 | ok: 33 |
| console hi-res | multi-sync preset | 11 | ok: 11 |
| console hi-res | panel | 55 | ok: 55 |
| handheld | 15 kHz | 54 | ok: 54 |
| handheld | 25 kHz | 6 | ok: 3, ok: no window at the game's rate: 3 |
| handheld | 31 kHz | 12 | ok: 12 |
| handheld | TV | 12 | ok: 11, ok: console width rule: 1 |
| handheld | multi-range preset | 78 | ok: 76, ok: no window at the game's rate: 2 |
| handheld | multi-sync EDID | 18 | ok: 18 |
| handheld | multi-sync preset | 6 | ok: 6 |
| handheld | panel | 30 | ok: 30 |
| horizontal arcade | 15 kHz | 81 | ok: 79, ok: overscan crop: 2 |
| horizontal arcade | 25 kHz | 9 | ok: 9 |
| horizontal arcade | 31 kHz | 18 | ok: 18 |
| horizontal arcade | TV | 18 | ok: 16, ok: overscan crop: 2 |
| horizontal arcade | multi-range preset | 117 | ok: 117 |
| horizontal arcade | multi-sync EDID | 27 | ok: 27 |
| horizontal arcade | multi-sync preset | 9 | ok: 9 |
| horizontal arcade | panel | 45 | ok: 45 |
| vertical arcade | 15 kHz | 90 | ok: 80, ok: no exact mode exists on this profile: 10 |
| vertical arcade | 25 kHz | 10 | ok: 8, ok: no window at the game's rate: 2 |
| vertical arcade | 31 kHz | 20 | ok: 18, ok: the profile declares no interlace: 2 |
| vertical arcade | TV | 20 | ok: 17, ok: no exact mode exists on this profile: 3 |
| vertical arcade | multi-range preset | 130 | ok: 121, ok: no exact mode exists on this profile: 8, ok: the profile declares no interlace: 1 |
| vertical arcade | multi-sync EDID | 30 | ok: 30 |
| vertical arcade | multi-sync preset | 10 | ok: 10 |
| vertical arcade | panel | 50 | ok: 50 |

The allowed reasons:

- No exact mode exists on this profile: the game's lines fit no window of any range at its rate (the 480x640 board on 15 kHz, a 576i or 512i source on 525 lines).
- The profile declares no interlace: a 31 kHz or EDID profile, for a game that needs an interlaced mode.
- No window at the game's rate: no exact mode exists at the game's rate, and the exact rung at the nearest rate where a range holds the game changes the speed at most `EXACT_SPEED_MARGIN_PPM` (2 percentage points) more than the fit (V6).
- The exact mode changes the speed beyond the margin: an exact rung exists only at a rate that changes the speed more than 2 points more than the fit does (V6). No cell of the table has this reason.
- Overscan crop: the crop hides up to 8 lines per edge and keeps the width, so the shown lines lose their shape by the share of the hidden lines.
- Console width rule: a console on a TV keeps the console width of `display.tv_width = aspect` (a console does not fill the line).

## 3. Handhelds

A handheld has 102 to 160 lines. The rule (the smallest whole multiple that fills the screen; below 60 % of the height an interlaced mode that fills more, or else the small exact picture with a notice) gives:

| Source | 15 kHz (`generic_15`) | 31 kHz (`arcade_31`) | Multi-sync (`edid_30_96`) | Panel 720p | Panel 1080p | NTSC |
| --- | --- | --- | --- | --- | --- | --- |
| Game Boy 160x144 | 1x in 224 lines, 64 % of the active lines, 59.5 % of the picture lines: small-picture notice | 3x, 432 lines | 3x, 432 lines | Fills, 5.0, sharp | Fills, 7.5, sharp | 1x in 240p, 60 % |
| Game Boy Advance 240x160 | 1x in 224, 71 % | 2x, 320 in 448 | 3x, 480 in 540 | Fills, 4.5 | Fills, 6.75 | 1x, 67 % |
| WonderSwan 224x144, 75.47 Hz | 1x in 221 at 65 Hz: -13.9 % speed | 2x at 65 Hz: -13.9 % | 2x at 75.47 Hz, exact | Fills, 60 Hz: -20.5 % | Fills: -20.5 % | 1x, 59.94 Hz: -20.4 % |
| Lynx 160x102, 75 Hz | 1x in 221, 46 %: small; 65 Hz: -13.3 % | 3x, 306 lines | 3x, 306 lines, exact | Fills, 7.06 | Fills, 10.6 | 2x, 204 of 240 lines, 85 %; 59.94 Hz: -20.1 % |
| Neo Geo Pocket 160x152 | 1x in 224, 68 % | 3x, 456 lines | 3x | Fills, 4.74 | Fills, 7.11 | 1x, 63 % |
| Virtual Boy 384x224, 50.27 Hz | 1x in 288 (the lines its 1.71 aspect needs) | 1x in 512, 44 %: small | 2x, 448 in 577 | 3x, nearest, 60 Hz: +19.4 % | Fills, 4.82: -0.5 % (50 Hz mode) | 1x, console width: +19.5 % |

What it means:

- On 15 kHz a handheld shows at 1x with black borders: 2x needs 288 to 320 lines, more than a 15 kHz tube holds progressive, and 2x interlaced would flicker for no gain in sharpness. The picture is the size of the handheld's own pixels on a tube; the small-picture notice shows below 60 %. This is the "pixel exact before soft" rule.
- On 31 kHz and on a multi-sync tube the smallest multiple that fills the screen gives 2x or 3x, full height.
- A handheld wider than the 4:3 face needs more mode lines than its own for its shape. When no range holds that height rounded up, the tallest height a range holds that is short of it by less than one pixel of the game is used (V7). So the WonderSwan on `d9400` and `d9800` at 75.47 Hz is 2x, 288 of 335 lines (86 %), at full speed, with its shape within 0.3 %: the shape needs 336.1 lines. The Lynx on `ntsc` is 2x in 240 lines. The Lynx on a 15 kHz tube stays 1x with the small-picture notice: 2x needs 240 lines where 221 fit at 65 Hz, 8 % short, and no mode squeezes a game.
- On a panel the scale is 4.5 to 10.6 on 1080p, so the panel rule fills the height with sharp bilinear.

## 4. Sources above or below the display's rates

- 75 Hz handhelds (WonderSwan 75.47 Hz, Lynx 75 Hz): a 15 kHz preset covers 49.5 to 65 Hz, so the game runs at 65 Hz, 13 to 14 % slow, with the speed notice (`NoticeReason::SpeedChange`). A multi-sync tube covers 75 Hz and shows it exactly. A TV runs it at 59.94 Hz (-20 %) with the speed alert. A 60 Hz panel runs it at 60 Hz (-20 %), with the speed notice since the change is above 5 % (V2). Double refresh does not help: twice 75 Hz is above every range.
- 30 Hz (Little Robin) and any source below every range: twice the rate where a range covers it, else the covered rate nearest twice it with each frame twice when that is nearer (60 Hz on 15 kHz; 59.94 Hz on `ntsc`, -0.1 %; 50 Hz on `pal`, -16.7 %). A Mega Drive PAL game (49.701 Hz) on a tube from 50 Hz shows at 99.40 Hz (V5).
- 70 Hz DOS VGA: a 31 kHz arcade tube runs it at 65 Hz; a PC tube shows it exactly. On a 15 kHz tube the 640x400 game is interlaced 1:1, 400 of 448 lines, at the window rate of 64.29 Hz (-8.3 %): the fit at 65 Hz (-7.3 %) would show the 400 lines at a scale of 0.55 (V6).

## 5. The invariant sweep

`crates/neptron-modeline/tests/integration/policy_sweep.rs` runs a fixed grid with no random numbers: widths 160, 320, 512 and 800; heights 100, 144, 224, 240, 288, 320, 480 and 640; both orientations (a turned 4:3 frame shows a 3:4 picture); progressive, and interlaced for 480 and 640 lines; at 30, 50, 55, 60, 70 and 80 Hz and at every edge of each range of the display, one millihertz inside and one outside; on the 36 displays. With the default settings it also runs each source one line taller and one millihertz faster (82,296 decisions). Then it runs four more settings on the fixed rates with no continuity pairs (15,552 decisions each): `arcade`, `vertical = fit`, `panel_scale = integer` and scanlines on a panel. In all 144,504 decisions in about 17 s in the dev profile. The numbers below are those after V6 and V7.

| Invariant | Cases (default settings) | Failures |
| --- | --- | --- |
| A decision without a panic | 82,296 | 0 |
| The winner passes the validator | 82,278 | 0 |
| No interlace on a profile that declares none | 73,206 | 0 |
| The shape is kept (2 %, one pixel per axis, the overscan crop, the TV snap within 3 %) | 81,342 | 0 |
| The shape on a TV for a 4:3 console of 224 lines per field or more (console width rule) | 936 | stated exception |
| An exact picture has a whole vertical scale | 55,958 | 0 |
| No soft fit when an exact mode changes the speed at most `EXACT_SPEED_MARGIN_PPM` more (the test searches the whole multiples itself, at the game's rate or the nearest covered one, at twice it or the nearest covered rate to twice it, and at the highest window rate of each range) | 19,072 | 0 |
| No mode only on the range of one line rate that covers the rate, or under the display's pixel clock limit | 18 | 0 |
| The rate is exact where a range covers it, or the exact rung at the window rate when no exact mode exists at the rate and no fit keeps the speed better by more than the margin | 33,398 | 0 |
| A CRT takes the smallest whole multiple that fills (style `auto`) | 26,459 | 0 |
| The crop stays within 8 source lines and 8 of 240 screen lines | 4,619 | 0 |
| Scanlines on a panel imply a whole vertical scale (scanline run) | 2,160 | 0 |
| Continuity: one line or one millihertz more changes the class only across a named limit | 54,864 | 0 |

The faults the sweep found and that are fixed, each with its test: a Switchres mode above the top line rate of a band range; a field rate off a range of one field rate; a game wider than the 4:3 face squeezed; a deinterlaced game with no border; a whole-profile mode the validator refuses that hid a range alone; a 224-line game on m3129 at 80 Hz that fell to a fit where an exact rung at 71.75 Hz changes the speed less; and with the arcade setting, a TV width snap past the 2 % shape bound (now the decided 3 % snap).

The named limits of the continuity check: the progressive and interlaced windows of each range at the rate and at twice it (their least and most lines, and whether the least fits), the crop reach, the TV modes and their crop, the panel scale of 4, a rate within `REFRESH_MATCH_MHZ` of a range edge, and a winner at a window rate. The limits are those of the game's rate (or the nearest covered one), and a winner at a lower window rate meets the limits of that rate: 240 lines on `generic_15` at 70 Hz take the progressive window mode at 64.29 Hz with 8 lines cropped per edge, and 241 lines need 9 and take interlaced 1:1. Before V6 this exception held for any plan with a mode at a window rate, because the speed rule could put the fit before it by a few millihertz (`ms929` at 288 and 289 lines). With the margin that cause is gone, and the exception holds only when the winner runs at a window rate.

## 6. Decided questions

The owner decided V1 to V3 and V5 on 2026-11-16, and V6 and V7 on 2026-11-17 (the margin of V6 from the table of every golden cell); the orchestrator decided V4. Each is built:

| Id | Question | Decision |
| --- | --- | --- |
| V1 | Handheld size on a 15 kHz CRT (Game Boy: 144 of 242 picture lines, 59.5 %) | Keep: 1x with borders and the small-picture notice |
| V2 | A source above every range of the display | The nearest covered rate, and on a flat panel the speed notice too when the change is above 5 % (`PANEL_SPEED_NOTICE_MIN_PPM`) |
| V3 | A game taller than the interlaced TV mode | The fit into the interlaced mode for every source: `pal_576i tv_ntsc` keeps 0.83 of its lines, not 0.42 |
| V4 | A narrow mode on a range of one field rate (`ntsc`, `pal`) | The next whole multiples of the game width until a whole-kHz clock gives the rate to the millihertz; the validator decides |
| V5 | A source below every range of a CRT | Double refresh for every source, only when no range covers the game's rate; a display that covers it keeps the rate once per frame |
| V6 | An exact rung at a window rate that changes the speed more than the fit (DOS 640x400 at 70 Hz on a 15 kHz tube: interlaced 1:1 at 64.29 Hz, -8.28 %, against the fit at 65 Hz, -7.26 %, at a scale of 0.55) | The exact lines win while the exact rung changes the speed at most `EXACT_SPEED_MARGIN_PPM` (20,000 ppm, 2 percentage points) more than the best fit. A speed and pitch change of that size is small against the lines or the sharpness that a fit loses, and the games that reach this rule already run several percent from their own rate. Above the margin the speed counts more: a 50 Hz game in a 60 Hz panel mode (19 to 21 points) stays a fit. In the golden tables two cells change: `dos_640x400` on `generic_15` (1.02 points) and on `ms2930` (0.77 point) |
| V7 | A wide handheld whose 2x picture needs a few more lines than the range holds (WonderSwan, 14:9, on `d9400` and `d9800` at 75.47 Hz) | No squeeze rule. A squeeze is a stretch, which the first rule (pixel exact first, no stretch) excludes by default; the picture mode setting is the place for a player who wants it (section 8). The WonderSwan case was a rounding: the shape needs 336.1 lines, and a height short of it by less than one pixel of the game keeps the shape within R1. The policy takes that height now: 2x, 288 of 335 lines, full speed. The Lynx on a 15 kHz tube stays 1x: 2x would need an 8 % squeeze |

## 7. Questions that stay open

| Id | Question | Present result | Recommended default |
| --- | --- | --- | --- |
| V8 | A game wider than the face with too few progressive lines for its shape (Virtual Boy, 1.71, on `ntsc`; a 3:2 source of 224 lines on `generic_15` at 60 Hz, which needs 252 lines where 241 fit) | Interlaced 1:1 in 448i with the shape kept: 224 of 448 lines, about 50 % of the height | Keep as built until the owner decides. The other choice is the progressive fit, which fills more of the height but squeezes the game |

Known case, not open: Mortal Kombat (54.70 Hz) on a panel with a 720p 60 Hz mode and a 1080p 50 Hz mode (`panel_1080`). The panel rule takes the 1080p fill at 50 Hz, -8.6 %; the whole multiple at 720p 60 Hz would be +9.7 %. Neither is close to the board's rate. The default does not change.

## 8. Picture mode

The player gets two rows for the picture of a game: "Picture mode", one setting with a flat list of named modes, and "Refresh rate", the rate target that every mode works with. Each option names its compromises, so the player knows the trade before the choice. `auto` is the default and is the built policy of sections 1 to 7, unchanged: every golden of those sections passes as it was.

### 8.1 Keys, values and labels

`display.style` keeps its key, because the settings formats on disk are append-only. Its label is "Picture mode". The values `sharp`, `speed` and `fit` are appended to the end of its list; `auto`, `native`, `2x`, `3x` and `4x` keep their names and their meaning on a CRT and on a TV. The menus show the modes in their own order, Auto, Sharp lines, Game speed, Fill, Native, 2x, 3x, 4x (`PICTURE_MODE_MENU_ORDER`); a menu option maps to its value through that list (`menu_choices`).

| Value | Label | Help line |
| --- | --- | --- |
| `auto` (default) | Auto | Exact lines first, at the game's speed. A notice names each compromise. |
| `sharp` | Sharp lines | Exact lines over the game's speed. The picture can flicker or be small. |
| `speed` | Game speed | The game's own speed first. The picture can be soft. |
| `fit` | Fill | The largest picture with no flicker where the screen allows it. The picture can be soft. |
| `native` | Native | Each game line on one screen line. The picture can be small, with borders. |
| `2x` | 2x | Each game line on two screen lines. |
| `3x` | 3x | Each game line on three screen lines. |
| `4x` | 4x | Each game line on four screen lines. |

`display.refresh_policy` keeps its key and its values. Its label is "Refresh rate". It is no expert key: its row stays in Settings > Display and in the Video page of the quick menu, next to Picture mode, with the levels of every picture setting.

| Value | Label | Help line |
| --- | --- | --- |
| `exact` (default) | Auto | The game's own rate, or the nearest rate the monitor takes. |
| `exact_x2` | Double rate | Each frame shows twice, at twice the game's rate. The game speed does not change. |
| `force_50` | 50 Hz | Settings > Display: "Games run at 50 Hz. A game made for 60 Hz runs slower." The quick menu: "This game at 50 Hz. If it is made for 60 Hz, it runs slower." |
| `force_60` | 60 Hz | Settings > Display: "Games run at 60 Hz. A game made for 50 Hz runs faster." The quick menu: "This game at 60 Hz. If it is made for 50 Hz, it runs faster." |

`exact_x2` has a real use: a 240p game on a 31 kHz monitor that takes 120 Hz shows its own 240 lines at 120 Hz, each frame twice, in place of 480 lines at 60 Hz. It is the "Native, double refresh" of the Definition (`native` with `exact_x2`).

Both keys: levels game, system and global (`Levels::ALL`); safety `Validated`; live. `display.style` applies to every display; `display.refresh_policy` applies to a CRT (a TV standard fixes the rate, and a flat panel runs at the rates of its modes), so its row is disabled on a TV and on a panel with that reason.

### 8.2 Lines shown twice and frames shown twice

Two different things can be "twice", and the texts never mix them up. Lines twice: each game line is drawn on two screen lines, so a 240-line game fills a 480-line screen. This is how a 31 kHz VGA monitor shows a 240p game by default: 480p, every pixel exact, at the game's own speed, with scanlines. Frames twice: each game frame is shown for two screen refreshes, at twice the game's rate, for example 120 Hz for a 60 Hz game. The speed does not change. Only the refresh choice Double rate asks for it, and the policy takes it on its own only where `main` already does: for `native` where the game's lines do not fit at its rate, and for a game below every rate of the monitor (a 30 Hz board). No help line says "twice" without saying lines or frames.

### 8.3 The display classes

The policy knows three kinds of display (`MonitorKind`): a CRT with ranges, a TV with the fixed modes of its standard, and a flat panel with its listed modes. The rules of a CRT are the same for every CRT class of the tables above (15 kHz, 25 kHz, 31 kHz, multi-range and multi-sync presets, EDID tubes); the ranges of the profile and the limits of the video output (`GpuCaps`) give the different results.

The Raspberry Pi 3 is a first target. Its outputs, with the limits of `data/hardware/matrix.toml` that the policy reads: HDMI to a flat panel (the panel path, with the modes the panel lists); composite to a TV (`pi_vc4_composite`: the 13.5 MHz clock of the encoder, interlace, 720 samples); GPIO DPI to a CRT (`pi_vc4_dpi`: no interlace, 165 MHz). A Pi on DPI has no interlaced mode, so a rung that needs one fails for the output, not the monitor, and the menu greys it with "Not possible with this screen and video output" (8.7).

### 8.4 Each mode on each display

"The auto ladder" is the ladder of `auto`: native, 2x, 3x, 4x, interlaced 1:1, then the fit. "The V6 margin" is `EXACT_SPEED_MARGIN_PPM`.

**`auto`.** The built policy, unchanged on every display wherever it has a picture. Where the plan of `auto` has no proposal that the validator accepts, the decision takes the picture of `fit` for the same inputs (owner, 2026-11-19: "if no auto can find a decent mode, fill is always the default"; `decide_shown`, the one place of this rule), with the notice "Fill picture: Auto has no picture for this game here." in place of the soft reason of the Fill mode, and the status result "fill: auto has no picture". A stored mode with no picture whose `auto` has none either says "Fill picture: {mode} and Auto have no picture for this game here." (status "fill: {key} and auto have no picture"). The text names only the modes that the decision tried: the stored mode, then `auto`, then `fit`; another mode, such as a whole multiple, can still have a picture there. Where `fit` has no picture either, nothing changes: the engine keeps the mode on screen. This changes `auto` only where it had no picture: every golden, `policy_vertical_fit.toml` and every exhaustive run pass unchanged, and the cells that change are named in the tests (8.7). A stored mode with no picture of its own (Sharp lines, Native, Game speed, a multiple, Fill) shows the picture of `auto`, and so the Fill picture where `auto` has none, and is declined with `NoPicture`.

**`native`, `2x`, `3x`, `4x` on a CRT and on a TV.** Unchanged: the ladders of `walk_ladder`. Where the winner is not the first rung of the mode, the option is declined (8.7): `NotShownHere` where the rung is not reachable whole at the rate of the winner, `SmallerFillsMore` where it is reachable at the winner's target (its kind and its rate) and the winner, a smaller whole multiple, fills more of the screen height; `InterlacedFillsMore` where it is reachable and the winner, the interlaced picture, fills more; a stored value falls through the ladder as before, with the notice of 8.9.

**`native`, `2x`, `3x`, `4x` on a flat panel.** The game is centred at that whole factor of its lines, at its display aspect, with the filter of `panel_filter` (nearest at a whole scale and from 1 to below 2, sharp bilinear below 1). When the factor does not fit the mode, the option is declined with `TooLargeForScreen` (8.7: greyed for the game of the quick menu, "Not all games" on Settings > Display when some games fit); a stored value gives the largest smaller factor that fits, or the fit for a game larger than the panel.

**`sharp` on a CRT.** The auto ladder, with one difference: every exact mode ranks before the fit, at any speed change. Example: 640x500 at 60 Hz on `generic_15` has no exact mode at 60 Hz; interlaced 1:1 holds the lines only at a slower window rate, more than 2 points slower than the fit. `auto` and `speed` take the fit at 60 Hz, `sharp` the interlaced 1:1 mode. DOS 640x400 at 70 Hz is within the margin, so `auto` and `sharp` both take interlaced 1:1 at 64.29 Hz. `display.vertical` reads as `exact` (`PolicySettings::read_vertical`, the one place of this rule). Where only a fit validates, the option is greyed.

**`sharp` on a TV.** The TV walk of `auto` with `display.vertical` read as `exact`.

**`sharp` on a flat panel.** The largest whole multiple that fits, at every scale, with black borders (`display.panel_scale` read as `integer`, `PolicySettings::read_panel_scale`).

**`speed` on a CRT.** The auto ladder. An exact mode at a slower window rate wins over the fit only when it changes the speed no more than the fit (a margin of 0, within `REFRESH_MATCH_MHZ`). Example: DOS 640x400 at 70 Hz on `generic_15` is the fit at 65 Hz (-7.26 %), not 1:1 at 64.29 Hz (-8.28 %). `speed` takes no frame shown twice that `auto` does not take (8.2). Where the winner still changes the speed by more than the alert threshold (section 9, 0.5 %), Game speed does not give what it names (`SpeedNotHeld`, "This screen cannot hold the game's speed"): the menus judge it by the rules of 8.7, and a stored `speed` gives that winner with the notice.

**`speed` on a TV.** A TV standard has one field rate, so the option is greyed: "A TV has one rate". A stored value gives the result of `auto`.

**`speed` on a flat panel.** A mode that holds the game comes before a mode that must shrink it, as in `auto`. Where no listed mode keeps the speed within 0.5 % (a 50 Hz game on a panel with only 60 Hz modes), Game speed is declined with `SpeedNotHeld`, as on a CRT. Then the listed mode whose rate is nearest the game's rate, in steps of 0.1 % (1,000 ppm). Then the order of `auto`. Example: a 50 Hz game on a panel that lists 1080p at 60 Hz (preferred) and 720p at 50 Hz takes 720p at 50 Hz, where `auto` takes 1080p at 60 Hz.

**`fit` on a CRT.** These rules hold for the mode `fit` only; `auto` with the expert key `display.vertical = fit` decides exactly as before the picture modes (the golden `policy_vertical_fit.toml`, checked against the `crt.rs` of `main`). For a progressive game: every interlaced 1:1 mode and every exact picture below `SMALL_PICTURE_MIN_PERCENT` (60 %) of the picture lines drop (`Dropped::VerticalFit`). The fitted rung then takes the most progressive lines that a range holds at the game's rate; where the game's rate holds none, the fit at the window rate where a range holds progressive lines comes before the last resort. So a progressive game never gets an interlaced mode under `fit` where the monitor holds progressive lines at the game's rate or at the rate the policy takes (`md_pal_h40` on `arcade_25` is a 400-line progressive fit, not 800i). An exact progressive picture of 60 % or more stays: it keeps every pixel, and the fit would be at most a little larger and soft. An interlaced game keeps its interlaced mode: its lines need it. A monitor with no progressive lines at either rate gives the interlaced fit of `auto`; no cell of the goldens has this case. The last resort, where the solver picks the rate and the scan itself, can also be interlaced: the generated run finds it on moved ranges only.

**`fit` on a TV.** A progressive game that `auto` puts in 480i or 576i 1:1 gets the fit into 240p or 288p. A game whose lines fit the progressive mode keeps them 1:1. An interlaced game keeps the interlaced mode.

**`fit` on a flat panel.** The fill at every scale, the shape kept. Below the scale of 4 (`PANEL_FILL_MIN_SCALE_MILLI`) the fill is soft, and the notice names it: "Soft picture: Picture mode is set to Fill." The same holds for `display.vertical = fit` on a panel, with its own sentence. No `auto` result with the default keys changes.

### 8.5 The refresh choice with each mode

The refresh choice is the rate target that every mode works with, as on `main`. On a CRT, an exact mode at the asked rate (`exact_x2`, `force_50`, `force_60`) ranks before every exact mode at another rate, for every mode: the asked rate is the player's choice, so it wins over the speed rule of `speed` and over the margin of `sharp`. An option of the refresh row is offered when the winner runs at the asked rate, a soft fit at that rate too: the option gives the rate it names, and the notice names the soft picture (640x500 at 60.000 Hz on `generic_15` with `force_60` is the fit at 60 Hz). It is declined when no range covers the asked rate (`RateNotCovered`), or when a range covers it but the winner runs at another rate (`RateNotHeld`): no mode at the asked rate validates, or a mode at another rate, an exact picture or a fit, ranks before every mode at the asked one. On a TV and on a flat panel the display does not read the refresh choice: the row is disabled, and a stored choice other than Auto greys nothing and gives no notice. A pick of a picture mode changes the refresh choice only by the pair rule of 8.7, and the tag of the option says so. A pick of a refresh choice that also sets the picture mode to Auto keeps the expert keys of that level (`display.vertical`, `display.panel_scale`): the pair that the menu judged had them, so the write runs the very pair that was judged.

### 8.6 Scanlines with each mode

The scanline rule is the one of `neptron_types::scanlines_for`, unchanged: `auto` draws them on a CRT above the 15 kHz class (`CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ`, so a 25 kHz or 31 kHz mode) where each game line is on two or more screen lines; `on` draws them on every screen where each game line is on two or more screen lines; no scanline is drawn on an interlaced mode or at a strength of 0. On a flat panel `on` keeps a whole vertical multiple (`panel_scanlines`, the one place of this rule), so a fill becomes whole steps. `offer` reads the rule back for the winner (`ScanlineOutcome`), and the notice says why scanlines that are on do not show.

| Display | Scanlines | Auto, Sharp lines, Game speed | Fill | Native | 2x, 3x, 4x |
| --- | --- | --- | --- | --- | --- |
| 15 kHz CRT | Auto | None: the tube makes its own | None | None | Greyed |
| 15 kHz CRT | On | None at 1x, with the notice | None on a fit, with the notice | None, with the notice | Greyed |
| 31 kHz CRT | Auto | Drawn at 2x: 240p shows as 480p with scanlines | Drawn where the result is 2x | None at 1x | Drawn |
| 31 kHz CRT | On | As Auto | As Auto | None at 1x, with the notice | Drawn |
| TV | Auto | None: the TV makes its own | None | None | Greyed |
| TV | On | None (1x or interlaced), with the notice | None, with the notice | None, with the notice | Greyed |
| Flat panel | Auto | None | None | None | None |
| Flat panel | On | Whole steps in place of the fill, drawn, with the notice of whole steps | Whole steps, drawn, with that notice | None at 1x, with the notice | Drawn |
| Every display | Off | None | None | None | None |

The help line of the scanline row names what Auto does on the connected display (8.9).

### 8.7 The one decision: `offer`

One pure function in `crates/neptron-modeline/src/policy/offer.rs` decides what the display gives: `offer(source, settings, scanlines, profile, caps, validate) -> Result<Offer, PolicyError>`. A candidate is the picture mode and the refresh choice of `settings` with the scanline settings (`ScanlineChoice`). It runs the same `decide` and `choose` as the engine, with the engine's validator, and returns `Offer = Result<Offered, Declined>`:

- `Offered`: the plan, the winner the validator accepted, its notice, and what the scanlines give (`ScanlineOutcome`).
- `Declined`: the reason (`DeclineReason`), the plan, and the winner the display shows in its place (for a stored candidate), or none.

`picture_choices` judges each option of the two rows as the pair that a pick stores and the engine then runs: the option with the stored value of the other row. The answer is one type, `RowAnswer`, with three cases:

- `Plain`: `offer` gives that pair. The option is selectable; a pick stores this row with the stored value of the other row.
- `ResetsOther`: `offer` declines that pair, and gives the pair with the other row at Auto. The option is selectable; a pick stores this row with the other row at Auto (see "How the app stores the pair" below), and a revert restores both. The row shows the tag "Rate to Auto too" (or "Mode to Auto too") in place of the rates; the help line keeps the option's own text, and the notice after the pick says it in full: "Refresh rate is set to Auto too." ("..., for all games." or "..., for all games of this system." at those levels).
- `Declined`: `offer` declines the pair with the other row at Auto too. The option is greyed with the reason of that pair, which is a reason of the row itself.

On a TV and a flat panel the pair is the mode alone, because the display does not read the refresh choice; a refresh choice other than Auto is declined there with the rate of the TV or of the panel. Auto of a row is offered with the other row at Auto wherever the display shows a picture, so it is never greyed there. `offer` judges a stored pair: the refresh reasons where the display reads the refresh choice (a CRT), then the mode reasons. The engine asks both with its own validator and veto. The menus build every option of the two rows from the answers and from nothing else; with no answer (no engine) every option is greyed. The way from an offered candidate to the screen stays: the decision, the validator, `ValidatedMode`.

**Which values judge a write.** A write at a level is judged with the values resolved from the defaults up to that level (`Config::policy_up_to`, `Config::scanlines_up_to`): the game level of the quick menu with the values of the run, for the running game; the system level in a game with no game layer, for the running game; the global level, and every level in the game list, with the global values, for every game: the running game, if any, then the reference sources. The control protocol takes the same rule at each level, with and without a running game.

**Settings > Display for every game.** `REFERENCE_SOURCES` in `offer.rs` is a named list of real shapes, each with the reason it is there: the Game Boy, the SNES, the PlayStation at 240 lines, the NES PAL, the Amiga PAL at 256 lines, a 480i source, the Neo Geo at 59.180 Hz, Mortal Kombat at 54.700 Hz, DoDonPachi turned, and a 480-line progressive source. `for_every_game` makes one answer of the answers of the sources (`RowChoice`): an option declined for every source is greyed with the reason of the first; an option that some sources do not get is selectable, with the tag "Not all games" in its row, and `RowChoice::not_for_every_game` says why (`SomeGames`): a reason of its own row, or `NeedOtherAtAuto`, where some games get it only with the other row at Auto; a reason of the other row never shows on a row. The notice after the pick of such an option says it in full: "Some games need 99.840 Hz, which this monitor does not take." for a rate of one game, "Not all games: {reason}." for another reason, "Some games need Refresh rate at Auto for it." for the other row. A rate that one game needs never shows as the reason of a greyed option for every game: Double rate that no game gets reads "This monitor does not take a doubled rate". The page shows no rate detail for every game: no rate holds for all, and the status block shows the mode on screen. The quick menu shows the mode of the running game in the detail.

**How the app stores the pair.** The app writes `display.style` and `display.refresh_policy` together, at one level, in one write (`Config::write_pair`). A pick in a menu, a keep and the control protocol write the picked value and the value of the other row that the pick was judged with: the value stored up to that level for a `Plain` answer, Auto for a `ResetsOther` answer. A pick of the picture mode also removes, in the same write, the expert keys that the mode covers (8.8). The Menu button on either row removes both keys at that level, in one write. So after any write by the app a level holds both keys or neither, and the pair that a game resolves to comes whole from one level: the pair that was judged there. An old file or a file edited by hand can hold one key at a level: the keys then resolve one by one, as before, a pair that the display declines falls back with its notice (8.9), and the first write of the app at that level makes it a full pair.

A write at a level is judged with the values up to that level. In a game, a level above it can hold a pair for this game: the game level, for a change of Settings > Display, or the system level, for a write for every game. The write is still made when the level offers it, and a notice says at once that the higher level keeps its own pair: "This game has its own picture settings and keeps them." (or "This system has ...").

A reset is judged too. After the pair at a level goes, the pair that resolves comes from a level below: the player's stored choice. The reset removes the pair also when the display does not give the pair below; then the notice of a stored choice (8.9) says what shows.

**The keep prompt.** The keep prompt of the quick menu moves the pair of the game: one write of both keys at the level it names, then one write that removes both keys at the game level. The move is judged with the values of that level after the move: the option of the picture mode of the pair, with the pair stored, must be `Plain`. A move that the level refuses removes nothing at the game level, and the notice says "Not kept for all games of this system: {reason}. The choice stays for this game." (or "for all games"). "All games" while the system level holds a pair of its own is refused: the player asked for the running game to keep its picture, and the system pair would win. The text: "Not kept for all games: this system has its own picture settings. The choice stays for this game." The prompt sends the set at the new level before the reset at the game level, so a refused set stops the reset of the same batch.

**The cost of the answers.** `policy_cost` (an ignored test that prints its numbers) measures each display class on the development machine (dev profile). One plan: 0.002 ms on a 1080p panel, 0.03 ms on NTSC, 0.11 ms on `generic_15`, 0.18 ms on `pc_31_120`. The answers of one game: 0.03 ms on the panel, 0.7 to 2.1 ms elsewhere; of every reference game: 0.3 ms on the panel, 7 to 13 ms elsewhere. The dearest decision, the fallback with every plan walked to its end (the stored mode, then `auto`, then `fit`): 0.75 ms on `pc_31_120`. Two decisions a frame therefore take at most 0.35 ms in the common case and 1.5 ms in the worst case on the development machine; with the estimate of a Raspberry Pi 3 at 15 times slower, about 5 ms in the common case, inside the 8 ms budget, and about 22 ms in the worst case, outside it. That worst case needs a stored mode with no picture and an `auto` with no picture on a 31 kHz monitor, a fault case; the measure on a Pi is in the hardware checklist. A page of Settings > Display judges about 100 decisions (eleven sources), the quick menu up to 24. The engine keeps the answers of the last requests, keyed by the request (the sources, the settings and the scanlines of the level) and by the generation of its inputs (the monitor profile, the caps, the veto and the driver refusals); a change of any of those makes new work, and a page that loads again with the same inputs costs nothing. A new request starts its work, and the frame makes at most `PICTURE_DECISIONS_PER_FRAME` (2) decisions of it, inside the 8 ms that the frame thread gives to work of its own on a Pi 3; the request that the page on show or the keep prompt waits for goes first, and the cache never drops a request in work or in use. The answers have a generation of their own: the monitor (its profile and caps), the veto and the driver refusals. A pick changes the settings, which are in the request, so it makes a new request and keeps every other answer. While the answers of a page are in work, the page keeps its last settled answers in part: the declined options keep their reasons, and the offered options show "..." in place of their rates or tags and take no pick; a page with no settled answers yet shows every option as "...". The help line of an option in work shows "..." alone. A pick in a menu takes the answers of its page and makes none at once. While the quick menu is open in a game whose game level holds a pair, the frame makes the answers of the keep prompt (System, and All games when the system holds no pair); a button whose answers are in work is dim and takes no choice until they are there. The control protocol is no player path: it makes the answers of its level at once.

The app sets a picture mode or a refresh choice only by this rule (`pages::gate`), for an input from the menus and from the control protocol alike. A greyed value changes nothing on disk or on the engine, and a short notice gives the reason: "Not changed: {reason}." The protocol sets one key: a value that is only given with the other row at Auto is refused with a reason that says so ("display.style = speed is only possible with display.refresh_policy = exact: set it first"), and the protocol never resets another key itself.

`DeclineReason`, in the order the function checks them:

| Reason | When | Text in the menu |
| --- | --- | --- |
| `NoPicture` | The plan of the candidate has no proposal that validates. A stored candidate shows the picture of `auto`, or of `fit`, where one has a picture | No picture with this choice |
| `RateFixedByTv` | A refresh choice other than `exact` on a TV | A TV has one rate |
| `RateOfPanel` | A refresh choice other than `exact` on a flat panel | A flat panel uses its own rates |
| `RateNotCovered` | No range of the CRT covers the asked rate | This monitor does not take {rate} Hz |
| `RateNotHeld` | A range covers it, but the winner runs at another rate: no mode at the asked rate validates, or a mode at another rate (an exact picture or a fit) ranks first | No exact picture at this rate |
| `NotShownHere` | `native` or a multiple on a CRT or a TV: the winner is another rung, and the display (the monitor with its video output) cannot show the asked rung whole at the rate of the winner: no proposal of it there validates with no crop. The policy does not tell the limit of the monitor from the limit of the output: that would need a second decision with other limits | Not possible with this screen and video output |
| `SmallerFillsMore` | A multiple on a CRT or a TV: the asked rung is reachable whole at the target of the winner (its kind and its rate), and the winner, a smaller whole multiple, fills more of the screen height: the height of the game on the screen over the lines the screen scans, the active lines or the lines that a padded range forces (the Lynx in 3x at 50 Hz on `d9200` shows a padded 2x). `offer` sets it where the validator's answer is known; the order of the plan does not change | {2x} fills more of this screen |
| `InterlacedFillsMore` | `native` or a multiple on a CRT or a TV: the asked rung is reachable whole at the target of the winner, and the winner, the interlaced picture, fills more of the screen height (a turned 288x224 board at 60 Hz in Native on `d9200`). The exhaustive runs of the tests find no other winner that fills more than a reachable asked rung, so these two reasons cover every reachable case | The interlaced picture fills more of this screen |
| `TooLargeForScreen` | `native` or a multiple on a flat panel: the factor does not fit | Too large for this screen |
| `NoExactMode` | `sharp`: only a fit validates | No exact mode on this screen |
| `OneRateOnTv` | `speed` on a TV | A TV has one rate |
| `SpeedNotHeld` | `speed`: the winner still changes the speed by more than 0.5 % | This screen cannot hold the game's speed |

For a stored pair the refresh reasons come first, because the refresh choice holds for every mode. The rule of the mode reasons is one function (`mode_refusal`), which the notice reads too (`style_fallback`).

A stored candidate that the display declines (the settings came from another display, or a file edited by hand) is never applied as stored: the engine shows the winner of its plan, which the validator accepted, inside the profile; where its plan has no picture (`NoPicture`), the picture of `auto`, or of `fit` (8.4). The notice names what was asked, why, and what shows, and why scanlines that are on do not show (8.9). Where no plan has a picture, the engine keeps the mode on screen; the runs below name every cell where that happens (none on the presets, the EDID tubes, the panels and the TV standards) and every cell where `auto` takes the Fill picture (one generated cell: a source one millihertz above a range of `pc_70_120`, with the ranges moved by the seed), each with the rate of its source.

`crates/neptron-modeline/tests/integration/policy_offer.rs` runs every candidate (every picture mode, refresh choice and the scanline settings a plan reads) on every preset of `data/monitors` with an open GPU and with each GPU entry of `data/hardware/matrix.toml` that drives a CRT, the EDID tubes and panels of the arcade table, and every TV standard on the composite output (`pi_vc4_composite`), for every source of the arcade table. An offered candidate keeps the promise of its option (sharp is no fit, a multiple is that multiple, native is 1:1, Fill is not interlaced where progressive lines exist, Game speed holds the speed, a forced rate runs at that rate) and shows a validated mode inside the profile; a declined one has a reason that the test finds true by itself, from the profile, the caps, the plan and the validator, and the mode it shows in its place validates and lies inside the profile. For every stored pair of every screen and source, and on a panel for every scanline setting, the test builds the rows by the pair rule from the answers of `offer` and holds: Auto of each row is selectable where (Auto, Auto) has a picture, and no option is greyed with a reason of the other row. In the runs with an open GPU and in the other runs but the GPU groups (which repeat the same code with other limits), it compares `picture_choices` with those rows for one stored pair of each screen, source and scanline setting, a different one from source to source, and each selectable option with a direct call of `offer` for the very pair that a pick stores. The cells with no picture, and the cells where `auto` takes the Fill picture, are asserted against named lists in every run, each with the rate of its source. A last run moves the rate limits of every CRT preset, takes sources at each field rate edge, one millihertz inside and outside, and generated sources; a failure prints the seed and the case. The GPU entries with the same limits give the same decisions, so one run covers each group of them, and a test holds the groups against the matrix. The runs are split into parts: the presets with an open GPU into twelve parts of the screen list, each GPU group into three, and the generated run into twelve parts of the source list (so a profile that the solver refuses spreads over every part, as over the whole run). On the development machine (dev profile, 2026-11-23) the longest part takes 11.5 s alone and with the other tests; the aim is less than 20 s each on a loaded machine. `policy_property` and `policy_sweep`, older tests, take 26 to 32 s.

The table below is the output of `offer` for the SNES (256x224 at 60.099 Hz) with scanlines Auto, for each stored pair: a row per picture mode and a column per refresh choice, the mode shown, or the reason. On a TV and a panel a stored refresh choice declines nothing. The test `the_spec_table_of_offered_combinations_is_the_output_of_the_function` compares it with the function, so it cannot drift.

<!-- offer-table:begin -->
**15 kHz CRT (`generic_15`)**

| Mode | `exact` | `exact_x2` | `force_50` | `force_60` |
| --- | --- | --- | --- | --- |
| `auto` | 256x224p 60.09 Hz | no: rate not covered | 256x224p 50.00 Hz | 256x224p 59.99 Hz |
| `native` | 256x224p 60.09 Hz | no: rate not covered | 256x224p 50.00 Hz | 256x224p 59.99 Hz |
| `2x` | no: not shown here | no: rate not covered | no: not shown here | no: not shown here |
| `3x` | no: not shown here | no: rate not covered | no: not shown here | no: not shown here |
| `4x` | no: not shown here | no: rate not covered | no: not shown here | no: not shown here |
| `sharp` | 256x224p 60.09 Hz | no: rate not covered | 256x224p 50.00 Hz | 256x224p 59.99 Hz |
| `speed` | 256x224p 60.09 Hz | no: rate not covered | no: speed not held | 256x224p 59.99 Hz |
| `fit` | 256x224p 60.09 Hz | no: rate not covered | 256x224p 50.00 Hz | 256x224p 59.99 Hz |

**31 kHz CRT (`arcade_31`)**

| Mode | `exact` | `exact_x2` | `force_50` | `force_60` |
| --- | --- | --- | --- | --- |
| `auto` | 256x448p 60.09 Hz | no: rate not covered | 256x448p 50.00 Hz | 256x448p 60.00 Hz |
| `native` | 546x478p 60.09 Hz | no: rate not covered | 585x512p 50.00 Hz | 548x479p 60.00 Hz |
| `2x` | 256x448p 60.09 Hz | no: rate not covered | 256x448p 50.00 Hz | 256x448p 60.00 Hz |
| `3x` | no: not shown here | no: rate not covered | no: not shown here | no: not shown here |
| `4x` | no: not shown here | no: rate not covered | no: not shown here | no: not shown here |
| `sharp` | 256x448p 60.09 Hz | no: rate not covered | 256x448p 50.00 Hz | 256x448p 60.00 Hz |
| `speed` | 256x448p 60.09 Hz | no: rate not covered | no: speed not held | 256x448p 60.00 Hz |
| `fit` | 256x448p 60.09 Hz | no: rate not covered | 256x448p 50.00 Hz | 256x448p 60.00 Hz |

**31 kHz PC CRT up to 120 Hz (`pc_31_120`)**

| Mode | `exact` | `exact_x2` | `force_50` | `force_60` |
| --- | --- | --- | --- | --- |
| `auto` | 256x448p 60.09 Hz | 256x224p 120.19 Hz | 256x448p 50.00 Hz | 256x448p 60.00 Hz |
| `native` | 256x224p 120.19 Hz | 256x224p 120.19 Hz | 585x512p 50.00 Hz | 578x506p 60.00 Hz |
| `2x` | 256x448p 60.09 Hz | no: rate not held | 256x448p 50.00 Hz | 256x448p 60.00 Hz |
| `3x` | no: not shown here | no: rate not held | no: not shown here | no: not shown here |
| `4x` | no: not shown here | no: rate not held | no: not shown here | no: not shown here |
| `sharp` | 256x448p 60.09 Hz | 256x224p 120.19 Hz | 256x448p 50.00 Hz | 256x448p 60.00 Hz |
| `speed` | 256x448p 60.09 Hz | 256x224p 120.19 Hz | no: speed not held | 256x448p 60.00 Hz |
| `fit` | 256x448p 60.09 Hz | 256x224p 120.19 Hz | 256x448p 50.00 Hz | 256x448p 60.00 Hz |

**TV (`tv_ntsc`)**

| Mode | `exact` | `exact_x2` | `force_50` | `force_60` |
| --- | --- | --- | --- | --- |
| `auto` | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz |
| `native` | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz |
| `2x` | no: not shown here | no: not shown here | no: not shown here | no: not shown here |
| `3x` | no: not shown here | no: not shown here | no: not shown here | no: not shown here |
| `4x` | no: not shown here | no: not shown here | no: not shown here | no: not shown here |
| `sharp` | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz |
| `speed` | no: one rate on TV | no: one rate on TV | no: one rate on TV | no: one rate on TV |
| `fit` | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz | 720x240p 60.05 Hz |

**Flat panel (`panel_1080`)**

| Mode | `exact` | `exact_x2` | `force_50` | `force_60` |
| --- | --- | --- | --- | --- |
| `auto` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |
| `native` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |
| `2x` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |
| `3x` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |
| `4x` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |
| `sharp` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |
| `speed` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |
| `fit` | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz | 1920x1080p 60.00 Hz |

<!-- offer-table:end -->

### 8.8 Old keys and old files

| Key | What changes |
| --- | --- |
| `display.style` | Label "Picture mode". Three values appended. It now applies to a flat panel: `native`, `2x`, `3x` and `4x` give that factor there (before, the panel ignored them) |
| `display.refresh_policy` | Label "Refresh rate". Its row and its values stay; a pick of a picture mode sets it to Auto only by the pair rule of 8.7, and says so |
| `display.vertical` | An expert key (`Entry::expert`): it stays in the schema and in the files, and the menus do not show it. It keeps its meaning under `auto`, `native`, `2x`, `3x`, `4x` and `speed`. `sharp` reads it as `exact`; `fit` gives its rule to every progressive game |
| `display.panel_scale` | An expert key. It keeps its meaning under `auto` and `speed`. `sharp` reads it as `integer`; `fit` fills; `native`, `2x`, `3x`, `4x` name their own factor |

When the player sets the picture mode at a level (the Settings panel writes the global level; the quick menu writes the game level, and its keep prompt may move the change to the system or the global level), the expert keys that the mode covers (`PICTURE_MODE_COVERS`: `display.vertical` and `display.panel_scale`) are removed at that same level, in the same write. So the mode the player picks is what the player gets. A value that still acts from another level, or from a file edited by hand, keeps its effect, and the notice names it whenever it changes the picture: "the setting display.vertical is set to fit", "the setting display.panel_scale is set to integer". A schema test holds the expert keys equal to `PICTURE_MODE_COVERS`, so no expert key is a dead entry.

An old settings file keeps its meaning on a CRT and on a TV: it can hold only the old values, and they give the old results. On a flat panel, a value `native`, `2x`, `3x` or `4x` now gives that factor. Known limit: one settings folder serves every display, and with `display.output = auto` a machine can drive a CRT one day and a panel the next, so a mode set for a game on one display acts on the other. A mode per display class is a later choice for the owner.

### 8.9 Notices

Each picture keeps the notice of the quick menu, one sentence per compromise (`Notice::reasons`). The notice shows at the first picture of a game, and again after a change of the picture mode or the refresh choice in a running game: the app waits until the engine has the picture of the new settings on screen (`VideoEngine::settled`), however long the modeset takes, and shows the notice when that picture differs, when it is not exact or changes the speed.

| Reason | Text |
| --- | --- |
| `FillFallback` | "Fill picture: Auto has no picture for this game here.", or "Fill picture: {mode} and Auto have no picture for this game here." for a stored mode It takes the place of `Soft(FillMode)`: the player did not choose Fill |
| `Soft(FillMode)` | "Soft picture: Picture mode is set to Fill." |
| `Soft(Chosen)` | "Soft picture: the setting display.vertical is set to fit." |
| `WholeSteps(Scanlines)` | "Whole steps, with borders: scanlines are on, and they need whole steps on a flat panel." |
| `WholeSteps(PanelScaleKey)` | "Whole steps, with borders: the setting display.panel_scale is set to integer." |
| A stored candidate that `offer` declines | "{Picture mode X / Refresh rate X / Picture mode X with Refresh rate Y}: {reason, first letter in lower case}. Shown: {the mode of the status line}." The pair names both rows for `NoPicture`, and for a reason of the picture mode while the refresh rate is not Auto: the rate can be the cause, and it can come from another level. Game speed with a forced rate says "the refresh rate changes the game's speed", not that the screen cannot hold it |
| A value of the picture mode or refresh row that the display does not give, set from the menus or the control protocol | "Not changed: {reason, first letter in lower case}." |
| A pick that also sets the other row to Auto (8.7) | "Refresh rate is set to Auto too." / "Picture mode is set to Auto too.", with ", for all games of this system" or ", for all games" at those levels |
| A pick on Settings > Display that some games do not get (8.7) | "Some games need {rate} Hz, which this monitor does not take." / "Not all games: {reason}." / "Some games need {Refresh rate / Picture mode} at Auto for it." |
| A keep that the gate of its level refuses (8.7) | "Not kept for {all games of this system / all games}: {reason}. The choice stays for this game." |
| Scanlines on, none drawn | "No scanlines: they need each game line on two or more screen lines." / "No scanlines: they are not drawn on an interlaced picture." / "No scanlines: the scanline strength is 0." |

The help lines of the scanline row, by display class:

| Value | CRT | TV | Flat panel |
| --- | --- | --- | --- |
| Auto | Drawn above 15 kHz when each game line is on two or more screen lines, with no interlace. | None on a TV. | None on a flat panel. Choose On to draw them. |
| On | Dark lines where each game line is on two or more screen lines, with no interlace. | The same as on a CRT | Dark lines at a whole multiple of 2 or more. The picture then uses whole steps. |
| Off | No scanlines. | No scanlines. | No scanlines. |

The one-time notice of a vertical game names the place of the choice:

> This is a vertical game on a horizontal screen. It shows interlaced, with every pixel exact. Picture mode Fill, in Settings > Display or in Video in the quick menu, shows it as a soft picture with no flicker. A screen turned on its side (TATE) comes in a later version.

### 8.10 The cases

`crates/neptron-modeline/tests/golden/policy_modes.toml` holds the winner of each mode for nine sources (the NES, the SNES, the Mega Drive in PAL and in 448i, the Game Boy, the WonderSwan, DoDonPachi on its side, DOS 640x400 at 70 Hz and Mortal Kombat at 54.70 Hz) on one display of each class: `generic_15`, `arcade_25`, `arcade_31`, `d9800`, `edid_30_70`, `tv_ntsc`, `tv_pal` and `panel_1080`. The test `every_picture_mode_on_each_display_class_gives_its_golden_decision` checks it, and checks every `auto` cell against `policy_sources.toml`. The cases that show the differences:

| Source | Display | `auto` | `sharp` | `speed` | `fit` | `native` / `2x` on the panel |
| --- | --- | --- | --- | --- | --- | --- |
| DOS 640x400, 70 Hz | `generic_15` | Interlaced 1:1, 64.29 Hz | The same | The fit, 221p at 65 Hz | The same as `speed` | |
| DOS 640x400, 70 Hz | `tv_ntsc` | 480i 1:1 | The same | The same | The fit into 240p | |
| Mega Drive PAL | `arcade_25` | 1:1 in 400p, 49.70 Hz | The same | The same | The fit into 400p | |
| SNES | `arcade_25` | 1:1 in 384p at 60.00 Hz | The same | The same | The fit into 384p at 60.00 Hz | |
| DoDonPachi, turned | `generic_15` | Interlaced 1:1, 448i | The same | The same | 252p fit, soft | |
| DoDonPachi, turned | `panel_1080` | 3x, 960 lines | The same | The same | Fill, 1080 lines, soft, with the notice | 1x, 320 lines; 2x, 640 lines |
| Game Boy | `generic_15` | 1x, 144 of 224 lines | The same | The same | The fit into 242p, soft | |
| Game Boy | `panel_1080` | Fill, 7.5 | 7x, 1008 lines | The same as `auto` | The same as `auto` | 1x, 144 lines; 2x, 288 lines |
| Mega Drive PAL | `panel_1080` | 1080p at 50 Hz, fill | 1080p at 50 Hz, 4x | The same as `auto` | The same as `auto` | 1x; 2x; at 50 Hz |
| Mega Drive 448i | `tv_ntsc` | 480i, 448 lines 1:1 | The same | The same | The same: an interlaced game keeps it | |

### 8.11 Decisions and questions for the owner

Decided by the owner on 2026-11-19: the menu order of 8.1; the notice after a change of mode in a running game (8.9); old files on a panel (8.8); the refresh choice stays in the menus (8.1, 8.5); `speed` takes no frames shown twice of its own (8.2, 8.4); the scanlines are named in the help line and in the notice (8.6, 8.9); the Pi 3 is a first target (8.3, 8.7).

| Id | Question | Default as built |
| --- | --- | --- |
| P1 | The texts of 8.1, 8.7 and 8.9. | As written, waiting for approval |
| P2 | V7 named the picture mode as the place for a squeeze of a wide handheld. | No mode squeezes |
| P3 | A notice reason for a progressive game shown interlaced. | Not built: the one-time notice of a vertical game covers the main case |
| P4 | A mode per display class, for one settings folder that serves a CRT and a panel. | One mode for every display |
| P5 | Options that the display gives for no reference game (2x, 3x and 4x on 15 kHz and on a TV, Game speed on a TV) are greyed, not hidden. | Greyed with the reason |
| P6 | A whole multiple that ranks after a padded smaller multiple of a lower line rate range (the Lynx in 3x at 50 Hz on `d9200`). The order of the rungs is the policy of `main` for these modes and does not change. | Declined with its own reason, `SmallerFillsMore` ("2x fills more of this screen") |
| P7 | The reference sources of Settings > Display (8.7). | The ten shapes of `REFERENCE_SOURCES` |

## 9. The speed alert

The status line of the Video page marks an alert with `[!]` and its colour, and the picture notice names a speed change in a sentence. Before this rule a CRT set the alert from `REFRESH_MATCH_MHZ` (5 mHz): a NES at 59.940 Hz in place of 60.099 Hz (0.26 %) showed `[!] speed (60.099 to 59.940 Hz)`, where a panel and a TV stayed silent below 5000 ppm.

The rule (owner, 2026-11-19): one threshold for the alert on every display class, `SPEED_ALERT_MIN_PPM` (5000 ppm of the shown rate, 0.5 %), in `crates/neptron-modeline`. It is the bound of the audio rate control (`neptron_audio::MAX_RATE_DEVIATION_PPM`): below it the change of pitch and speed cannot be heard or seen in play. `TV_SPEED_SILENT_PPM` and `PANEL_SPEED_SILENT_PPM` are this constant. One pure function decides (`speed_alert`), and the status line and the notice both read it. Every speed comparison (the alert, the speed word of a TV and a panel, the speed sentence of a panel's notice) is one function, `speed_beyond`, with the shown rate (the field rate per game frame) as the one denominator. The speed sentence of a panel's notice keeps its own bound, `PANEL_SPEED_NOTICE_MIN_PPM` (5 %, V2): a panel runs every game at its own rate, so the status line alerts from 0.5 % and the sentence waits for 5 %. That bound compared the change with the game's rate before; it now compares it with the shown rate, like every other bound. Only a change near 5 % sees a difference: 5 % of the game's rate is 4.76 % of the shown rate when the panel runs faster.

| Speed difference | CRT | TV | Flat panel |
| --- | --- | --- | --- |
| Below `REFRESH_MATCH_MHZ` | `exact` | `exact` | `exact` |
| From `REFRESH_MATCH_MHZ` up to 5000 ppm | `speed (X to Y Hz)`, no `[!]`, no speed sentence | `exact`, silent | `exact`, silent |
| Above 5000 ppm | `[!] speed (X to Y Hz)` and the speed sentence | The same | `[!] speed (X to Y Hz)`; the sentence from 5 % (V2) |

On a CRT the two rates still show below the threshold: they are true information for a CRT player. Another alert reason on the same line (`filtered`) still sets the mark. With the picture modes: `sharp` can change the speed by more than the threshold (an exact rung at a slow window rate), and then it alerts; `speed` takes the smallest change the display gives, so it alerts only where even that change is above the threshold (a 75 Hz handheld on a 15 kHz tube). The margin of V6 (`EXACT_SPEED_MARGIN_PPM`, 20,000 ppm) orders the candidates; it does not change the alert.
