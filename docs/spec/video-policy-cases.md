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

## 8. Picture mode (planned, not built)

The owner wants more control for the player beside good defaults: one setting, "Picture mode", with a flat list of named modes. The speed is part of the mode, not a second setting. This is a proposal. Nothing of it is built.

**Key and storage.** The key stays `display.style`, because settings on disk are append-only. Its label becomes "Picture mode". The new values are appended to its list; `auto`, `native`, `2x`, `3x` and `4x` keep their meaning. Levels: global, system and game (`Levels::ALL`), as today.

**The modes.** Each row has a one-line meaning. The help line under the list names the compromises of the selected row.

| Value (label) | Meaning | 15 kHz CRT | 31 kHz CRT | TV | Flat panel | Compromises it can cause |
| --- | --- | --- | --- | --- | --- | --- |
| `auto` (Auto) | The decided rule: exact lines first, the game's speed within the margin of V6 | As decided | As decided | As decided | As decided | Each, with its notice |
| `sharp` (Sharp lines) | Every line exact, the smallest whole multiple that fills. Another rate or interlace when needed. Never soft | Interlaced 1:1 for a tall game | 2x line doubling; 1:1 in about 500 lines | 480i or 576i 1:1 | The largest whole multiple | Small picture, interlace, speed change, crop |
| `speed` (Game speed) | The game's own rate first, or each frame twice. Exact lines at that rate, else the fit at that rate | The fit at the game's rate where exact lines need another rate | The same | Not available: one field rate | The listed mode nearest the game's rate (1080p 50 Hz for a 50 Hz game) | Soft scale, frames shown twice |
| `native` (Native) | The game's own lines once, with borders | 1x | 1x in about 480 lines | 1x | 1x | Small picture, interlace |
| `2x`, `3x`, `4x` | That whole multiple | 2x of a short game only | 2x | Not available | That multiple | Crop, small picture |
| `fit` (Fill) | Fill the height with the shape kept, progressive where possible, soft | The 240p or 288p fit | The fit into about 500 lines | The fit into 240p or 288p | The fill, sharp bilinear | Soft scale |

**The list on a 240-line screen.** A UI row is 12 to 24 pixels (`ROW_HEIGHT_MIN_PX` to `ROW_HEIGHT_MAX_PX`). At 24 pixels a screen holds 10 rows: a title, 7 modes and a help line of 2 rows. At 16 pixels all 8 values fit.

**Which modes show.** A mode that the display class can never give is hidden: `3x` and `4x` on a 15 kHz CRT and on a TV, `speed` on a TV of one field rate. So the list stays short and the same for every game on that screen. A mode that this game cannot get on this screen shows greyed, with the reason in the help line. The Display panel has the data already (`style_rows`: `available` and the dropped reason).

**Notices.** Every mode keeps the notice of the quick menu, one sentence per compromise (`Notice::reasons`). One reason is new: interlace, for a progressive game shown interlaced.

**Old values.** `display.vertical` and `display.panel_scale` stay in the schema, hidden from the menus. They keep their meaning at the level that holds them, below `display.style` of the same level: `display.vertical = fit` reads as `fit` for a turned game, and `display.panel_scale = integer` reads as `sharp` on a panel. When the player sets the picture mode at a level, the editor writes `display.style` only. `display.refresh_policy` stays as an expert key, hidden from the menu: `speed` covers `exact_x2`, and a file that holds `force_50` or `force_60` keeps them.

**Where the player changes it.** Settings > Display > Picture mode writes the global level. The Video page of the quick menu, in a game, writes the game level; its keep prompt moves the change to the system or the global level, as the picture style does today.

**Why the speed is part of the mode.** One list answers one question: what does the player give up. Each row names its compromise. Two settings would give combinations that mean nothing on some displays (a speed switch on a TV), and the player would have to read both to know the result.
