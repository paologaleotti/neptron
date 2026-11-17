# Video policy cases

This spec holds the cases of the video policy (`crates/neptron-modeline`, the ODREN policy) beyond arcade: consoles, handhelds and computers, the invariant sweep, and the open questions they raise. The rule is the one of `docs/spec/arcade.md`, section 11, as the owner decided it on 2026-11-16. That section holds the arcade boards, the decisions and the notices. This file holds the rest because the content is no longer only arcade.

## 1. The sources

`crates/neptron-modeline/tests/integration/arcade_table.rs` runs two lists through `decide` and `choose` with `neptron_safety::validate`, on the 36 displays of the arcade table (every preset of `data/monitors`, four EDID tubes, five flat panels, NTSC and PAL):

- `BOARDS`: the 18 arcade boards of `docs/spec/arcade.md`.
- `SOURCES`: 44 more sources. Each one names where its size and rate come from in a comment. A value marked UNVERIFIED is the common figure of the core, not checked: the PlayStation, Saturn and N64 rates, the Mega Drive 448-line interlace, the Lynx, Neo Geo Pocket and Virtual Boy rates, the C64 border size and the Amiga rates. A system that also has goldens (NES, SNES, Mega Drive NTSC and the others of `tests/golden/systems.toml`) keeps their geometry and 4:3, and the test `the_wider_table_agrees_with_the_console_goldens` holds it to `policy.toml` on the profiles they share. A handheld comes with its own display aspect (Game Boy 10:9, Game Boy Advance 3:2, WonderSwan 14:9, Lynx 160:102, Neo Geo Pocket 20:19, Virtual Boy 384:224).

To print every table and the summary: `cargo xtask test -p neptron-modeline --no-capture sources_table`. The full table has 1,584 rows; it is the output of the test, not a part of this file. Every cell that follows the rule or carries an allowed reason is a golden in `tests/golden/policy_sources.toml`.

## 2. The summary by class

The verdicts of the rule after the change of 2026-11-16, for every source of both lists. "ok" follows the rule; "ok:" and a reason is an allowed exception. No cell breaks the rule.

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
| computer | 15 kHz | 81 | ok: 78, ok: no exact mode exists on this profile: 1, ok: overscan crop: 1, ok: the exact mode changes the speed more: 1 |
| computer | 25 kHz | 9 | ok: 7, ok: overscan crop: 2 |
| computer | 31 kHz | 18 | ok: 17, ok: overscan crop: 1 |
| computer | TV | 18 | ok: 11, ok: console width rule: 7 |
| computer | multi-range preset | 117 | ok: 114, ok: overscan crop: 2, ok: the exact mode changes the speed more: 1 |
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
- No window at the game's rate: no exact mode exists at the game's rate, and the exact rung at the nearest rate where a range holds the game changes the speed no more than the fit.
- The exact mode changes the speed more: an exact rung exists only at a rate that changes the speed more than the fit does (section 7, V6).
- Overscan crop: the crop hides up to 8 lines per edge and keeps the width, so the shown lines lose their shape by the share of the hidden lines.
- Console width rule: a console on a TV keeps the console width of `display.tv_width = aspect` (a console does not fill the line).

## 3. Handhelds

A handheld has 102 to 160 lines. The rule (the smallest whole multiple that fills the screen; below 60 % of the height an interlaced mode that fills more, or else the small exact picture with a notice) gives:

| Source | 15 kHz (`generic_15`) | 31 kHz (`arcade_31`) | Multi-sync (`edid_30_96`) | Panel 720p | Panel 1080p | NTSC |
| --- | --- | --- | --- | --- | --- | --- |
| Game Boy 160x144 | 1x in 224 lines, 64 % of the active lines, 59.5 % of the picture lines: small-picture notice | 3x, 432 lines | 3x, 432 lines | Fills, 5.0, sharp | Fills, 7.5, sharp | 1x in 240p, 60 % |
| Game Boy Advance 240x160 | 1x in 224, 71 % | 2x, 320 in 448 | 3x, 480 in 540 | Fills, 4.5 | Fills, 6.75 | 1x, 67 % |
| WonderSwan 224x144, 75.47 Hz | 1x in 221 at 65 Hz: -13.9 % speed | 2x at 65 Hz: -13.9 % | 2x at 75.47 Hz, exact | Fills, 60 Hz: -20.5 % | Fills: -20.5 % | 1x, 59.94 Hz: -20.4 % |
| Lynx 160x102, 75 Hz | 1x in 221, 46 %: small; 65 Hz: -13.3 % | 3x, 306 lines | 3x, 306 lines, exact | Fills, 7.06 | Fills, 10.6 | 1x, 42.5 %: small |
| Neo Geo Pocket 160x152 | 1x in 224, 68 % | 3x, 456 lines | 3x | Fills, 4.74 | Fills, 7.11 | 1x, 63 % |
| Virtual Boy 384x224, 50.27 Hz | 1x in 288 (the lines its 1.71 aspect needs) | 1x in 512, 44 %: small | 2x, 448 in 577 | 3x, nearest, 60 Hz: +19.4 % | Fills, 4.82: -0.5 % (50 Hz mode) | 1x, console width: +19.5 % |

What it means:

- On 15 kHz a handheld shows at 1x with black borders: 2x needs 288 to 320 lines, more than a 15 kHz tube holds progressive, and 2x interlaced would flicker for no gain in sharpness. The picture is the size of the handheld's own pixels on a tube; the small-picture notice shows below 60 %. This is the "pixel exact before soft" rule.
- On 31 kHz and on a multi-sync tube the smallest multiple that fills the screen gives 2x or 3x, full height.
- On a panel the scale is 4.5 to 10.6 on 1080p, so the panel rule fills the height with sharp bilinear.

## 4. Sources above or below the display's rates

- 75 Hz handhelds (WonderSwan 75.47 Hz, Lynx 75 Hz): a 15 kHz preset covers 49.5 to 65 Hz, so the game runs at 65 Hz, 13 to 14 % slow, with the speed notice (`NoticeReason::SpeedChange`). A multi-sync tube covers 75 Hz and shows it exactly. A TV runs it at 59.94 Hz (-20 %) with the speed alert. A 60 Hz panel runs it at 60 Hz (-20 %), with the speed notice since the change is above 5 % (V2). Double refresh does not help: twice 75 Hz is above every range.
- 30 Hz (Little Robin) and any source below every range: twice the rate where a range covers it, else the covered rate nearest twice it with each frame twice when that is nearer (60 Hz on 15 kHz; 59.94 Hz on `ntsc`, -0.1 %; 50 Hz on `pal`, -16.7 %). A Mega Drive PAL game (49.701 Hz) on a tube from 50 Hz shows at 99.40 Hz (V5).
- 70 Hz DOS VGA: a 15 kHz tube runs it at 65 Hz; a 31 kHz arcade tube at 65 Hz; a PC tube shows it exactly.

## 5. The invariant sweep

`crates/neptron-modeline/tests/integration/policy_sweep.rs` runs a fixed grid with no random numbers: widths 160, 320, 512 and 800; heights 100, 144, 224, 240, 288, 320, 480 and 640; both orientations (a turned 4:3 frame shows a 3:4 picture); progressive, and interlaced for 480 and 640 lines; at 30, 50, 55, 60, 70 and 80 Hz and at every edge of each range of the display, one millihertz inside and one outside; on the 36 displays. With the default settings it also runs each source one line taller and one millihertz faster (82,296 decisions). Then it runs four more settings on the fixed rates with no continuity pairs (15,552 decisions each): `arcade`, `vertical = fit`, `panel_scale = integer` and scanlines on a panel. In all 144,504 decisions in about 17 s in the dev profile.

| Invariant | Cases (default settings) | Failures |
| --- | --- | --- |
| A decision without a panic | 82,296 | 0 |
| The winner passes the validator | 82,278 | 0 |
| No interlace on a profile that declares none | 73,206 | 0 |
| The shape is kept (2 %, one pixel per axis, the overscan crop, the TV snap within 3 %) | 81,342 | 0 |
| The shape on a TV for a 4:3 console of 224 lines per field or more (console width rule) | 936 | stated exception |
| An exact picture has a whole vertical scale | 54,819 | 0 |
| No soft fit when an exact mode changes the speed no more (the test searches the whole multiples itself, at the game's rate or the nearest covered one, at twice it or the nearest covered rate to twice it, and at the highest window rate of each range) | 20,211 | 0 |
| No mode only on the range of one line rate that covers the rate, or under the display's pixel clock limit | 18 | 0 |
| The rate is exact where a range covers it, or the exact rung at the window rate when no exact mode exists at the rate and no fit keeps the speed better | 32,933 | 0 |
| A CRT takes the smallest whole multiple that fills (style `auto`) | 26,147 | 0 |
| The crop stays within 8 source lines and 8 of 240 screen lines | 4,511 | 0 |
| Scanlines on a panel imply a whole vertical scale (scanline run) | 2,160 | 0 |
| Continuity: one line or one millihertz more changes the class only across a named limit | 54,864 | 0 |

The faults the sweep found and that are fixed, each with its test: a Switchres mode above the top line rate of a band range; a field rate off a range of one field rate; a game wider than the 4:3 face squeezed; a deinterlaced game with no border; a whole-profile mode the validator refuses that hid a range alone; a 224-line game on m3129 at 80 Hz that fell to a fit where an exact rung at 71.75 Hz changes the speed less; and with the arcade setting, a TV width snap past the 2 % shape bound (now the decided 3 % snap).

The named limits of the continuity check: the progressive and interlaced windows of each range at the rate and at twice it (their least and most lines, and whether the least fits), the crop reach, the TV modes and their crop, the panel scale of 4, a rate within `REFRESH_MATCH_MHZ` of a range edge, and the speed rule of the window rate (an exact rung there against the fit, within the match).

## 6. Decided questions

The owner decided V1 to V3 and V5 on 2026-11-16, the orchestrator V4. Each is built:

| Id | Question | Decision |
| --- | --- | --- |
| V1 | Handheld size on a 15 kHz CRT (Game Boy: 144 of 242 picture lines, 59.5 %) | Keep: 1x with borders and the small-picture notice |
| V2 | A source above every range of the display | The nearest covered rate, and on a flat panel the speed notice too when the change is above 5 % (`PANEL_SPEED_NOTICE_MIN_PPM`) |
| V3 | A game taller than the interlaced TV mode | The fit into the interlaced mode for every source: `pal_576i tv_ntsc` keeps 0.83 of its lines, not 0.42 |
| V4 | A narrow mode on a range of one field rate (`ntsc`, `pal`) | The next whole multiples of the game width until a whole-kHz clock gives the rate to the millihertz; the validator decides |
| V5 | A source below every range of a CRT | Double refresh for every source, only when no range covers the game's rate; a display that covers it keeps the rate once per frame |

## 7. Questions that stay open

| Id | Question | Present result | Recommended default |
| --- | --- | --- | --- |
| V6 | An exact rung at the window rate that changes the speed a little more than the fit (DOS 640x400 at 70 Hz on a 15 kHz tube: interlaced 1:1 at 64.46 Hz, -8.0 %, against the fit at 65 Hz, -7.3 %) | The fit wins: the exact rung changes the speed more | Keep the rule "exact wins when its speed error is not larger than the fit's". A limit of about 1 % would let the exact rung win here; it would be a named constant |
| V7 | A wide handheld (WonderSwan, 14:9) on a tube whose range holds its 2x lines only squeezed (288 lines in 320, d9400 at 75.47 Hz) | 1x in 320 lines, 45 %, the small-picture notice | Keep: the 2x picture would be 5 % too narrow |
| V8 | A game wider than the face with too few progressive lines for its shape (Virtual Boy, 1.71, on `ntsc`; a 3:2 source of 224 lines on `generic_15` at 60 Hz, which needs 252 lines where 241 fit) | Interlaced 1:1 in 448i with the shape kept: 224 of 448 lines, about 50 % of the height | Keep as built until the owner decides. The other choice is the progressive fit, which fills more of the height but squeezes the game |
