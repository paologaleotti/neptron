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
| PAL | 15 kHz | 45 | ok: 43, ok: one field rate only: 1, ok: overscan crop: 1 |
| PAL | 25 kHz | 5 | ok: 5 |
| PAL | 31 kHz | 10 | ok: 10 |
| PAL | TV | 10 | ok: console width rule: 10 |
| PAL | multi-range preset | 65 | ok: 65 |
| PAL | multi-sync EDID | 15 | ok: 15 |
| PAL | multi-sync preset | 5 | ok: 5 |
| PAL | panel | 25 | ok: 25 |
| computer | 15 kHz | 81 | ok: 77, ok: no exact mode exists on this profile: 1, ok: one field rate only: 2, ok: overscan crop: 1 |
| computer | 25 kHz | 9 | ok: 7, ok: no exact mode exists on this profile: 2 |
| computer | 31 kHz | 18 | ok: 17, ok: overscan crop: 1 |
| computer | TV | 18 | ok: 11, ok: console width rule: 7 |
| computer | multi-range preset | 117 | ok: 114, ok: no exact mode exists on this profile: 1, ok: overscan crop: 2 |
| computer | multi-sync EDID | 27 | ok: 27 |
| computer | multi-sync preset | 9 | ok: 9 |
| computer | panel | 45 | ok: 45 |
| console | 15 kHz | 108 | ok: 108 |
| console | 25 kHz | 12 | ok: 10, ok: no exact mode exists on this profile: 2 |
| console | 31 kHz | 24 | ok: 24 |
| console | TV | 24 | ok: 3, ok: console width rule: 21 |
| console | multi-range preset | 156 | ok: 156 |
| console | multi-sync EDID | 36 | ok: 36 |
| console | multi-sync preset | 12 | ok: 12 |
| console | panel | 60 | ok: 60 |
| console hi-res | 15 kHz | 99 | ok: 98, ok: one field rate only: 1 |
| console hi-res | 25 kHz | 11 | ok: 9, ok: no exact mode exists on this profile: 2 |
| console hi-res | 31 kHz | 22 | ok: 22 |
| console hi-res | TV | 22 | ok: 2, ok: console width rule: 20 |
| console hi-res | multi-range preset | 143 | ok: 143 |
| console hi-res | multi-sync EDID | 33 | ok: 33 |
| console hi-res | multi-sync preset | 11 | ok: 11 |
| console hi-res | panel | 55 | ok: 55 |
| handheld | 15 kHz | 54 | ok: 48, ok: one field rate only: 6 |
| handheld | 25 kHz | 6 | ok: 3, ok: no exact mode exists on this profile: 3 |
| handheld | 31 kHz | 12 | ok: 12 |
| handheld | TV | 12 | ok: 11, ok: console width rule: 1 |
| handheld | multi-range preset | 78 | ok: 76, ok: no exact mode exists on this profile: 2 |
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
| vertical arcade | 15 kHz | 90 | ok: 80, ok: no exact mode exists on this profile: 7, ok: one field rate only: 3 |
| vertical arcade | 25 kHz | 10 | ok: 8, ok: no exact mode exists on this profile: 2 |
| vertical arcade | 31 kHz | 20 | ok: 18, ok: the profile declares no interlace: 2 |
| vertical arcade | TV | 20 | ok: 17, ok: no exact mode exists on this profile: 3 |
| vertical arcade | multi-range preset | 130 | ok: 121, ok: no exact mode exists on this profile: 8, ok: the profile declares no interlace: 1 |
| vertical arcade | multi-sync EDID | 30 | ok: 30 |
| vertical arcade | multi-sync preset | 10 | ok: 10 |
| vertical arcade | panel | 50 | ok: 50 |

The allowed reasons:

- No exact mode exists on this profile: the game's lines fit no window of any range at its rate (the 480x640 board on 15 kHz, a 576i or 512i source on 525 lines).
- The profile declares no interlace: a 31 kHz or EDID profile, for a game that needs an interlaced mode.
- One line rate or one field rate only: the `ntsc`, `pal` and `arcade_25` presets (section 5, V4).
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

- 75 Hz handhelds (WonderSwan 75.47 Hz, Lynx 75 Hz): a 15 kHz preset covers 49.5 to 65 Hz, so the game runs at 65 Hz, 13 to 14 % slow, with the speed notice (`NoticeReason::SpeedChange`). A multi-sync tube covers 75 Hz and shows it exactly. A TV runs it at 59.94 Hz (-20 %) with the speed alert. A 60 Hz panel runs it at 60 Hz (-20 %); a panel shows the speed in the status only (decided before: `PANEL_SPEED_SILENT_PPM`). Double refresh does not help: twice 75 Hz is above every range.
- 30 Hz (Little Robin): an arcade board gets double refresh (decision A11). A console below every range keeps the nearest rate (section 5, V5).
- 70 Hz DOS VGA: a 15 kHz tube runs it at 65 Hz; a 31 kHz arcade tube at 65 Hz; a PC tube shows it exactly.

## 5. The invariant sweep

`crates/neptron-modeline/tests/integration/policy_sweep.rs` runs a fixed grid with no random numbers: widths 160, 320, 512 and 800; heights 100, 144, 224, 240, 288, 320, 480 and 640; both orientations (a turned 4:3 frame shows a 3:4 picture); progressive, and interlaced for 480 and 640 lines; at 30, 50, 55, 60, 70 and 80 Hz and at every edge of each range of the display, one millihertz inside and one outside; on the 36 displays, with the default settings. For each case it also runs the source one line taller and one millihertz faster. That is 82,296 decisions in about 8 s in the dev profile.

| Invariant | Cases | Failures at the first run | After the fixes |
| --- | --- | --- | --- |
| A decision without a panic | 82,296 | 0 | 0 |
| The winner passes the validator | 82,161 | 0 | 0 |
| No interlaced mode on a profile that declares none | 73,089 | 0 | 0 |
| The shape is kept (2 % and one pixel per axis; the overscan crop) | 79,569 | 0 (4:3 frames); 29,614 with square pixels before the wide-game fix | 0 |
| The shape on a TV (the console width rule) | 2,592 | stated exception | stated exception |
| An exact picture has a whole vertical scale | 51,772 | 0 | 0 |
| No soft fit when an exact mode exists (the test searches the whole multiples itself) | 23,141 | 209 | 0 |
| No mode carries an allowed reason | 135 | 586 | 0 |
| The rate is exact where a range covers it | 31,604 | 0 | 0 |
| Continuity: one line or one millihertz more changes the class only across a named limit | 54,864 | 227 | 0 |

The figures of the first run counted a grid with slightly different sources while the test was written; the faults they found:

1. A Switchres mode above the top line rate of a band range (16.217 kHz on `arcade_15`, limit 16.200): the validator refused every mode, so no mode. Fixed: the mode moves into the band by lines of the back porch (`onto_fixed_line_rate`).
2. A field rate 1 to 3 mHz off a range of one field rate (`ntsc`): the clock aimed for Switchres's rate. Fixed: the clock aims inside the range and ranks a mode inside first. A narrow mode can still miss (exception, V4).
3. A game wider than the 4:3 face (aspect 1.6 or more) filled the height and was squeezed. Fixed: such a game is bordered with the lines its shape needs, and takes no crop.
4. An interlaced game on ranges with no interlaced window took no border. Fixed: it takes the progressive border.
5. The whole profile gave a mode that the validator refused (a sync pulse too long on a 171-pixel mode on `pstar`), so the per-range shapes never ran. Fixed: each covering range adds its candidates whenever two or more ranges cover the rate.

The named limits of the continuity check: the progressive and interlaced windows of each range at the rate (their least and most lines, and whether the least fits), the crop reach of 8 lines per edge, the double rate, the TV modes and their crop, the panel scale of 4, and a rate within `REFRESH_MATCH_MHZ` of a range edge (a mode there may miss the rate by the match).

## 6. Open questions

Each has a recommended default. None is built: each changes existing console goldens or adds a mechanism.

| Id | Question | Present result | Recommended default |
| --- | --- | --- | --- |
| V1 | Handheld size on a 15 kHz CRT: is a 1x picture with borders (Game Boy: 144 of 242 picture lines, 59.5 %) right? | 1x, centred, small-picture notice | Keep: pixel exact before soft. The `fit` choice of a later per-system setting can give a bigger soft picture |
| V2 | A source above every range of the display (75 Hz handhelds on 15 kHz, on a TV, on a 60 Hz panel) | The nearest covered rate, the speed notice on a CRT and a TV, the status on a panel | Keep, and show the speed notice on a panel too when the change is above 5 % |
| V3 | A console taller than the interlaced TV mode (576i on NTSC) fits the progressive mode: 0.42 of its lines | `pal_576i tv_ntsc`: 644x240, sharp | Extend W5 to every source: the interlaced fit keeps 0.83. It changes the three `pal_576i tv_ntsc` goldens |
| V4 | A narrow mode on a range of one field rate (`ntsc`, `pal`) cannot hit the rate to the millihertz with a whole-kHz clock | The fit, or no mode, for sources under about 300 pixels | Widen such a mode by a whole multiple of the game width until a whole-kHz clock gives the rate, as the dynamic super width does |
| V5 | A console below every range of a CRT (Mega Drive PAL, 49.701 Hz, on a 50 to 160 Hz tube) | The nearest rate: 2x at 50.000 Hz, 0.6 % fast | Extend the double refresh of A11 to every source: 1x at 99.402 Hz keeps the speed. It changes one golden (`md_pal_h40 edid_npt_0071`) |
