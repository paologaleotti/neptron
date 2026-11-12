# neptron-modeline

Safe Switchres wrapper, monitor profiles and the ODREN policy: from source geometry, profile and GPU capabilities to a list of candidate modelines. No I/O.

## Public API sketch

- `fn modelines(&SourceGeometry, &MonitorProfile, &GpuCaps) -> Result<Vec<Candidate>, ModelineError>`: the modelines of Switchres with the default settings.
- `fn modelines_with(&SourceGeometry, &MonitorProfile, &GpuCaps, &Settings) -> Result<Vec<Candidate>, ModelineError>`
- `Candidate`: a `Modeline`, the index of the range that made it, and the score of Switchres (stretched, refresh off, scale, refresh difference) in integers.
- `Settings`: doublescan, refresh tolerance, super width, monitor aspect.
- `policy` module: the ODREN policy. See "The policy" below.

## The policy

`neptron_modeline::policy` decides which mode a CRT or a flat panel gets for a game, and where the game sits in it. It is pure: no I/O, no clock, and no global state except the lock of the Switchres calls. Switchres calculates each modeline. The policy decides which rung, which refresh, which width and which fallback.

### API

- `fn decide(&SourceGeometry, &PolicySettings, &MonitorProfile, &GpuCaps) -> Result<Plan, PolicyError>`: steps 1 to 5.
- `PolicySettings`: `style` (`auto`, `native`, `2x`, `3x`, `4x`), `refresh` (`exact`, `exact_x2`, `force_50`, `force_60`), `horizontal` (`None` for the GPU default, or a strategy that the system names), and the Switchres `Settings`.
- `Plan`: the source after the rotation, the effective settings, the refresh targets that the profile does not cover (`skipped`), and `considered`: every candidate in preference order. Each `Considered` has its `Rung`, its `RefreshTarget` and an outcome: a `Proposal`, or a `Dropped` reason with the rule name and the numbers.
- `Proposal`: the `Modeline` to validate and set, the `Placement` (crop, destination, filter, UI destination, the whole-scale claims), the `Timing` (game rate, target, line rate, field rate, frames per game frame), the width rule (`HorizontalScale`), and the padding when the mode has one.
- `fn choose(&Plan, validate) -> Choice`: step 6. The first proposal that the validator accepts wins. The refusals before it are kept.
- `fn switch_for(Option<&Modeline>, &Modeline) -> Switch`: step 7. `ScaleOnly` or `Modeset`.
- `fn notice(&Plan, &Considered) -> Option<Notice>`: step 8.
- `fn style_rows(..., validate) -> Vec<StyleRow>`: the picture style rows of the Display panel, from `decide` and `choose` for each style. A row whose winner is not on its own rung is greyed out; the first candidate on its own rung holds the reason.
- `Display` on `Considered` and `Dropped` gives one line with the rule and the numbers, for the log.

### The decision procedure

| Step | Function | What it does |
| --- | --- | --- |
| 1 | `take_source` | Width, height, refresh, scan. A game turned by 90 or 270 degrees swaps its axes, so its long side becomes lines. |
| 2 | `read_settings` | No strategy gives the GPU default: dynamic super width with a pixel clock floor, native width without one. A fixed width of 0 or wider than the GPU becomes the widest of 3840, 2560 and 1920 that the GPU sends. |
| 3 | `walk_ladder` | The rungs of the style. `auto`: native, 2x, 3x, 4x, interlaced 1:1, fitted. `native`: native, interlaced 1:1, fitted, never a multiple. `2x`: 2x, interlaced, fitted. `3x` and `4x` fall to the smaller multiples first. |
| 4 | `refresh_targets`, `shape` | The refresh targets in order, and per rung the lines (height times the factor), the scan and the base width. |
| 5 | `solve` | The width by the horizontal strategy, and the modeline from Switchres. |
| 6 | `choose` | The validator walks the proposals in order. |
| 7 | `switch_for` | A winner equal to the current mode changes only scale and crop. |
| 8 | `notice` | A notice when the winner is the fitted rung, when the game runs at a rate other than its own, when the dynamic super width replaced the width strategy of the settings, when `native` took double refresh on its own, when the winner is not on the style's own rung, when an overscan crop hides lines, or when the winner is padded. |

### Rules

- **Refresh.** `exact` aims for the game's rate. A rate matches when the mode is less than `REFRESH_MATCH_MHZ` (5 mHz) from it: "matches to two decimals" means that no rounding to 0.01 Hz can tell the two apart. `exact_x2` (twice the rate, each frame shown twice), `force_50` and `force_60` come first only where a range covers the rate; the game's own rate follows them. A game rate that no range covers becomes the nearest covered rate (`Nearest`). The last resort is the fitted rung at the rate that Switchres reaches (`Unmatched`).
- **Picture styles.** `auto` takes the best exact result for the monitor: native on 15 kHz, line doubled (2x) on 31 kHz, and native at twice the rate with `exact_x2` where a range covers it. `native` shows the game's own lines 1:1 and never a whole multiple. Its walk: (a) the native rung at each refresh target, not padded; (b) the native rung at twice the game's rate (`NativeDouble`), when the refresh policy did not ask for double refresh and a range covers the rate, so a 31 kHz monitor with a 120 Hz range shows 240p at 120 Hz; (c) rung 3, interlaced 1:1, where the lines fit no progressive range; (c2) the native rung with an overscan crop, when its lines miss what the range holds by a few lines; (d) every padded exact mode, the fuller first, with its `Padding`; (e) the fitted rung. Double refresh does not change the game's speed, so `exact` allows it; the notice sets `double_refresh`. `2x`, `3x` and `4x` keep their walk: the smaller multiples, rung 3, then the fit.
- **Lines.** A rung asks for the game's height times its factor. Switchres must keep the lines, the scan and the width, and reach the target rate; else the candidate is dropped (`LinesChanged`, `ScanChanged`, `WidthChanged`, `RefreshMissed`). An interlaced game on a monitor with no interlaced range is shown progressive (480i as 480p). A timing that does not fit its `Modeline` field drops that one candidate (`TimingOutOfRange`); any other solver error is an error of the profile or the settings and stops the plan.
- **Fitted rung.** Switchres fits the source to the profile. When that misses the target rate, the policy asks again for the most progressive lines a range holds at the rate (`floor(h_max / rate)` less the vertical blanking, at most the line limit), so the game keeps its rate when the profile allows it. A 510-line source at 60.606 Hz on `pc_31_120` gets 502 lines at 60.606 Hz, not 510 lines at 59.848 Hz.
- **Borders.** Pixel exact has priority over a filtered fit. A progressive game, or its whole multiple on a multiple rung, with fewer lines than the ranges at the target rate allow is centred with black borders in a mode of any height that a range holds at the rate (`border_lines`). The standard heights come first: 224 and 240 (`BORDER_LINES`), times the factor on a multiple rung (448 and 480 for 2x). When no standard height fits, the mode gets the smallest height in range that Switchres does not pad: the game's lines, at least the range's minimum, and at least the lines that the minimum line rate forces at the rate. The black lines are then the policy's own border, centred, not a porch that moves the picture up. The mode width keeps the game's aspect. When no range holds a height at the rate, the rung is dropped (`NoBorderMode`). A Master System on `pc_31_120` gets 2x, 384 lines in 448; a WonderSwan on `generic_15` gets 144 lines in 221 at 65 Hz.
- **Padding (OP-026).** A mode is padded when the game fills less than `PADDED_FILL_MIN_PERCENT` (75 %) of the picture lines. A border mode counts its border lines as picture lines, so its fill is the game's share of the mode (`Padding::fill_permille`). The picture lines are the larger of the mode's active lines per field and `ceil(h_min / rate) - ceil(h_min * vertical blanking)`, with the range that made the mode at the target rate. On the EDID profile of a 30 to 71 kHz PC CRT, a 224-line game at 60.099 Hz has 480 picture lines: 224 is 47 %, so the mode Switchres pads to 500 lines is padded, and 2x (448 lines, 93 %) or double refresh (224 of 230 lines) wins. On 15 kHz, 224 lines fill 93 % of 240.
- **Overscan crop.** When a progressive whole multiple of the game's lines (the native rung is the multiple 1) misses the lines that the ranges hold at the target rate (`progressive_capacity`), the policy hides the fewest source lines per edge, the same number at the top and at the bottom, so that the multiple of the rest fits: `ceil((height - capacity / k) / 2)` lines per edge, at most `OVERSCAN_CROP_MAX_LINES_PER_EDGE` (8, the classic NES crop from 240 to 224 lines). The picture stays pixel exact (whole multiple, nearest), as the overscan of a CRT hides the edge lines. The width is never cropped. The uncropped candidate stays in the plan with its reason, and the cropped one follows it. `Proposal::overscan_crop` holds the numbers, the placement's crop leaves the lines out (columns of a turned frame), and the notice gives `cropped_lines`. An NES on `arcade_31` (478 lines at 60.099 Hz) shows 238 lines at 2x, 476 lines; Donkey Kong on `pc_31_120` (502 lines) shows 250 lines at 2x, 500 lines.
- **Order.** An exact mode at the asked rate (`exact_x2`, `force_50`, `force_60`) ranks before every exact mode at another rate, for every style: the asked rate is the user's choice, and no fill comparison and no crop overrides it. The double rate that `native` takes on its own (`NativeDouble`) is not an asked rate. Within each of these two bands, `native` orders its exact entries in four groups (`native_key`): (1) the native rung with no crop and no padding, at every target in target order (the asked rate, the game's rate, then `NativeDouble`); (2) interlaced 1:1 with no padding, at every target; (3) the native rung with an overscan crop, at every target; (4) every padded exact mode, native or interlaced, the fuller first, then by target. The fitted rung of every target follows both bands, then the last resort. For the other styles: every exact rung before every fitted rung: the fitted rung wins only when no exact placement exists at any refresh target, and a fitted mode at a doubled or forced rate never beats an exact mode at the game's rate. Then the refresh targets in order. Within a target: the exact rungs with no crop and no padding, in ladder order; then the cropped multiples; then the padded exact rungs, the one that fills more of its picture lines first. A padded mode on 31 kHz then loses to line doubling.
- **Width.** The policy computes the width, so the reason is its own rule: Switchres gets no pixel clock floor and never doubles the width. Native: the game width (or the border width). Dynamic super width: the smallest whole multiple whose pixel clock is at least the GPU floor, the bound the validator checks; the next multiple is estimated from the last pixel clock, at most 16 calls, and the search stops when the product does not fit `u32`. Fixed super width: the fixed width, with a sharp filter. A game wider than the GPU gets the GPU's widest mode, with a sharp filter. A native or fixed width below the GPU floor is dropped with `PixelClockFloor` (width, pixel clock, floor, widest GPU mode); the next entry is the same rung with the dynamic super width, marked `strategy_replaced`, and `notice` reports it. A source wider than the GPU has no replacement: no multiple of its width fits.
- **Pixel clock.** Every CRT proposal has a pixel clock in whole kHz (`Modeline::PCLOCK_STEP_HZ`): the kernel's mode record holds kHz, and a display backend refuses any other clock and never rounds. Switchres gives whole hertz, so step 5 changes each mode of Switchres before any check (`whole_khz`), and the rates of the proposal are the rates of the final clock and totals: the proposal is the mode that the validator judges and that the kernel reads back. Rounding alone fails twice. A step of 1 kHz moves the field rate by `1000 / (htotal * vtotal)` Hz, about 9 mHz for a 320-wide 15 kHz mode, more than `REFRESH_MATCH_MHZ`; and a mode that Switchres puts at the edge of a range (`neogeo` at 15.625 kHz, a rate of 65.000 Hz at `v_max`) crosses the edge. So the policy tries Switchres's horizontal total, then one pixel more, one less, and so on up to 8 pixels (the pixels go to or come from the back porch), and for each total the whole-kHz clock below and above the one that keeps Switchres's line time, the nearer first. The first mode whose field rate equals Switchres's to the millihertz wins, else the nearest. A mode must keep each rate inside the range where Switchres has it inside, each horizontal porch and the sync pulse at least 90 % of the nominal time (the share of Switchres) or no shorter than Switchres's, and the sync pulse at most 110 % of the nominal time or no longer than Switchres's. When no mode passes, the nearest whole kHz at Switchres's total is the proposal, and the validator judges it. On the golden list every winner keeps its rung and its mode size, and its field rate is at most 3 mHz from the rate before the change. A flat panel's listed modes are not changed.
- **Placement.** The crop is the whole frame, less the lines of an overscan crop. On a CRT the game fills the width at a whole multiple, or the fixed width, and its lines times the factor, centred when bordered; the fitted rung fills the active area. The UI fills the active area (`ui_dest_fill`). On a flat panel the game gets the largest whole vertical factor at which its display aspect fits, centred; the UI gets one whole scale (`ui_dest_uniform`). `ScaleClaim` gives the whole factors that hold exactly; the filter is `Nearest` when both axes are whole, else `SharpBilinear`. On a panel only, an axis whose scale is at least 1, below 2 and not whole takes `Nearest` (`panel_filter`): sharp bilinear has a prescale of 1 there, which is plain bilinear, and a panel picture must never be blurry. Nearest keeps it sharp, with pixels of uneven width. SNES hires 512x224 on 720p (896 pixels, 1.75) and PAL 576i on 1080p (768 for 720, 1.07) are nearest. A scale below 1 (a game taller than the panel's mode, the fitted rung on a panel) keeps sharp bilinear.
- **Flat panel.** Each listed mode is a proposal. Order: modes with a whole vertical scale first, then the preferred size (the first listed mode), then the rate per game frame nearest the game's, then the list order.

### Readings of the Definition

Where the Definition allows two readings, the policy takes the one below. Each is one named constant or function. The product choices among them (marked *owner*) wait for the owner's decision after the wave; change only the named item.

- *Owner (decided):* pixel exact first. The policy loses a few edge lines before it uses any filter. A filtered picture (the fitted rung) is the last resort: it wins only when no exact placement exists at any refresh target that the policy allows, and it always carries a notice (`Notice::fitted`). The order for every style: exact with no crop and no padding; exact with an overscan crop; exact but padded, the fuller first; the fit.
- "Falls through the ones below" for `3x` and `4x`: the smaller multiples first, never the native rung (`walk_ladder`).
- *Owner (decided, OP-046 point 1):* `native` means the game's lines 1:1 and never a multiple; `auto` takes the best exact result for the monitor (`walk_ladder`, `crt_plan`). The Display panel shows a difference on 31 kHz: `auto` is 2x, `native` is double refresh or the padded native mode.
- `native` with the refresh policy `exact` takes double refresh (`NativeDouble`) when the game's lines do not fit at the game's rate without padding. The Definition lists "Native, double refresh" as `native` with `exact_x2`, and "Native" as needing a 15 kHz monitor. The Definition also says that each style runs "at the game's exact rate, or exactly twice it", and double refresh keeps the game's speed, so this reading takes it. The notice (`double_refresh`) tells the player, because the policy did not ask for it.
- "Never show something the user did not choose with no notice": a style that falls to another rung (4x to 2x, `native` to rung 3 or the fit) sets `style_fallback` in the notice. The Display panel greys the row out as before.
- *Owner (decided, OP-046 point 2):* a border mode can have any height that a range holds at the rate, and the fit is the last choice. The sources table says "a 224 or 240-line mode" and "480p fit on 31 kHz" for a medium-resolution board; the standard heights still come first, and a 384-line board gets its own lines centred, not a fit (`border_lines`). System 24 (57.524 Hz) on `pc_31_120` is 384 lines in 512, of 528 picture lines (72.7 %, padded); on `arcade_31` it is 384 lines in 501 (76.6 %, not padded); the 384-line board at 55 Hz of the table is 384 lines in 512 on `arcade_31`, of 526 picture lines (73.0 %, padded).
- *Owner (decided):* the overscan crop hides at most `OVERSCAN_CROP_MAX_LINES_PER_EDGE` (8) source lines per edge: the classic NES crop from 240 to 224 lines that emulator cores apply by default. The crop is the same at the top and the bottom, so an odd miss in output lines loses one line more than needed (NES on `arcade_31`: 238 lines at 2x, 476 of the 478 lines that the range holds). The picture then sits about 1 line off the centre of the range, which is not visible.
- "The smallest height in range that holds the picture" is at least the lines that the minimum line rate forces at the rate, less the blanking (`forced_picture_lines`), because Switchres puts the lines below that number into the front porch, and the picture then sits high on the screen.
- How the border rule meets the padded rule: a border mode is not a mistake, but its fill is honest. Among the exact placements at one target, the ones that fill at least 75 % of the picture lines come first, in ladder order; then the padded ones, the one that fills more first (`crt_plan`). A ZX Spectrum on `pc_31_120` at 50.08 Hz gets 2x (384 lines in 448, 63 % of 609 picture lines) before native (192 lines in 512, 31 %).
- *Owner:* a medium-resolution board (384 lines) on 15 kHz gets rung 3, interlaced 1:1 with borders, where its lines fill the field; the sources table says "interlaced fit". The ladder puts rung 3 above the fitted rung, and the fit stays the fallback.
- *Owner:* "matches to two decimals" is less than 5 mHz (`REFRESH_MATCH_MHZ`).
- *Owner:* `force_60` is 60.000 Hz and `force_50` is 50.000 Hz (`FORCE_60_MHZ`, `FORCE_50_MHZ`).
- *Owner:* a mode is padded below 75 % (`PADDED_FILL_MIN_PERCENT`).
- *Owner:* a flat panel gets no notice for its rate; audio rate control absorbs the difference (`notice`).
- "When the lines only fit that way" (rung 3): the game's lines do not fit a progressive range at the target rate, that is the range's line limit or `floor(h_max / rate)` less the vertical blanking (`progressive_capacity`). A short game is centred in the interlaced minimum, as a vertical game on a 15 kHz set is.
- A padded exact mode ranks before every fitted rung, so a handheld on 15 kHz is centred with borders, as the sources table says; it ranks after every exact mode that is not padded at its target (`crt_plan`).
- A width strategy below the GPU floor is replaced, not left without a mode: the screen must get a mode, and only the dynamic super width can reach the floor.
- The dynamic super width needs a pixel clock of at least the floor, the bound of the validator; Switchres doubles while the clock is at most the floor.
- A flat panel: a whole vertical scale, the width at the core's aspect (`panel_placement`).

### What the engine does

1. On launch and on each geometry change, call `decide`, then `choose` with `|m| neptron_safety::validate(m, profile, caps)`. The winner's token is the `ValidatedMode` to set. The policy does not depend on `neptron-safety` (the dependency rules forbid it), so the engine validates every proposal itself; a proposal the validator refuses is skipped, never set.
   Commit `token.mode()` of the winner, never `proposal.mode`: only the token is the mode the validator approved.
2. Call `switch_for` with the current mode. `ScaleOnly`: change only crop, destination and filter. `Modeset`: commit after the debounce.
3. Build each `Scene` from the winner's `Placement`, with its crop: a cropped placement is a part of the frame, not a frame of another size. Run the core one frame per `Timing::repeat()` flips. When `Timing::changes_speed()`, the game runs at `field_rate_mhz / repeat`; audio rate control follows the field rate.
4. Show `notice` in the quick menu. A `Notice` holds flags and numbers only, no text: it carries every alert the app must show. The texts it supports:
   - `fitted`: "Filtered picture".
   - `cropped_lines`: "N edge lines hidden".
   - `padded_fill_permille`: "Small picture" with the percentage of the picture lines that the game fills.
   - `double_refresh`: double refresh, each frame shown twice, at the same game speed.
   - `style_fallback`: the picture style could not be shown, and another rung is shown.
   - `game_mhz` and `shown_mhz`: the game runs at a rate other than its own.
   - `strategy_replaced`: the width strategy was replaced.

   The reasons are in the plan: each `Considered` before the winner has its `Dropped` reason or its `Padding`, `Choice::refused` has the validator's reason for each refused proposal, and `Plan::skipped` has the refresh targets no range covers. The UI builds its "why" text from those, and logs each `Considered` with its `Display` line.
5. Apply the EDID pixel clock limit to the GPU caps first (`EdidProfile::limit_caps`, OP-013).
6. Cache the plan by source geometry and settings. A decision takes tens of microseconds in the dev profile, so it can run on the frame thread, but a cache keeps the Switchres lock off the frame for a repeated geometry.

### Known limits

- A monitor aspect other than 4:3 in the Switchres settings makes Switchres widen every mode by the ratio of the aspects (256 becomes 341 at 16:9). Every exact rung then drops with `WidthChanged`, and the fitted rung wins. The policy computes the width itself, so it could pass 4:3 to Switchres and keep the aspect for the border width only; that is an open question for the owner.
- A GPU with a fixed pixel clock (`pi_vc4_composite`, 13.5 MHz) gets no mode: Switchres does not calculate for a fixed clock.
- At the crop bound, an overscan crop can hide real game content: arcade boards draw to the edge of the frame, and a vertical game shown unrotated loses columns of its upright picture (Donkey Kong on `arcade_15` loses 5 columns at each side). The bound is not checked against real game pictures.
- Some `auto` results stay small after the overscan crop, because no whole multiple fits within 8 lines per edge, and the fit is the last resort. The game is then pixel exact, centred with black borders, smaller than the screen. Fill is the game's share of the picture lines:
  - Vertical arcade games, which the MVP cannot rotate: Pac-Man on `pc_31_120`, 288 of 501 (57.4 %), and on `arcade_31`, 288 of 474 (60.7 %); Donkey Kong on `arcade_31`, 256 of 474 (54.0 %); Pac-Man and Donkey Kong on `generic_15` and `arcade_15`, interlaced 1:1, 144 or 128 lines per field of 238 (60.5 % and 53.7 %).
  - Handhelds on 15 kHz, centred in 224 lines as the sources table asks: Game Boy, 144 of 242 (59.5 %); Game Gear, 144 of 241 (59.7 %); WonderSwan, 144 of 221 on `generic_15` (65.1 %); Game Boy Advance, 160 of 242 (66.1 %).
  - 50 Hz computers on 31 kHz, where a range forces many lines at 50 Hz: ZX Spectrum at 2x, 384 of 609 on `pc_31_120` (63.0 %) and 384 of 582 on `arcade_31` (65.9 %); C64 PAL at 2x, 400 of 609 (65.6 %) and 400 of 582 (68.7 %).

### Rotation

The policy swaps the axes of a turned game and returns the crop in frame pixels. `Scene` has no rotation field, and the renderer does not rotate in the MVP, so the engine must not show a turned game until the renderer rotates it.

## What the solver does

- It accepts a CRT profile with 1 to 10 ranges. A flat-panel profile is an error: it lists its modes, and Switchres does not calculate for it.
- A range that Switchres refuses, or whose blanking is outside the limits of `neptron-switchres-sys` (Switchres would loop forever on it), is an error with the index of the range and the values in nanoseconds. Every shipped preset passes.
- It returns at most one candidate per range, best first, in the order of Switchres. The first candidate is the mode that Switchres selects. An empty list means that no range can show the source.
- From `GpuCaps`, Switchres uses `pclock_min_hz` (its `dotclock_min`) and `interlace`. The highest pixel clock and the largest size are not checked here: the safety validator judges every candidate.
- A rotation of 90 or 270 degrees sets the "rotated" flag of Switchres, which swaps width and height. Switchres takes every source as 4:3, so `aspect_milli` is not used.
- The source refresh reaches Switchres as a `float`, as `display_manager::get_mode` takes it.

## Settings

`Settings::default()` holds the defaults of `switchres_manager`, except doublescan:

| Setting | Default | Switchres default |
| --- | --- | --- |
| `doublescan` | off | on |
| `refresh_tolerance_mhz` | 2,000 | 2.0 Hz |
| `super_width` | 2,560 | 2560 |
| `monitor_aspect` | 4:3, from 1:4 to 4:1 | 4.0/3.0 |

Doublescan is off because not every DRM driver accepts a doublescan mode. The fixed values are the Switchres defaults: pixel precision on, interlace not forced to even, proportional scale on, vertical shift correction off, no geometry adjustment.

## Conversion

`src/convert.rs` is the one place where integer values become Switchres doubles and back.

- Into Switchres: each integer becomes an `f64` without loss, then one division by a power of ten gives the unit of `switchres.ini` (Hz, µs, ms), rounded to the nearest `f64`. A correctly rounded parser gives the same `f64` for the decimal text of a Switchres preset, so a profile from `data/monitors` reaches Switchres bit for bit as the preset text does. The unit tests check this against the preset text.
- Out of Switchres: each timing is checked against its field (`u16`, and `u32` for the pixel clock). The pixel clock is whole hertz: Switchres truncates it. A score is multiplied by 1,000 and rounded to the nearest integer, a half away from zero. A value that does not fit is a `ConvertError` with the field, the value and the limit.

## Must never depend on

- Any NEPTRON crate except `neptron-types` and `neptron-switchres-sys` (normal dependencies)
- Any other crate as a normal dependency: the error types implement `Display` and `Error` by hand, not with `thiserror`
- Any hardware crate
- I/O of any kind
- `unsafe`: the raw calls live in `neptron-switchres-sys`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Licence

The source is `MIT OR Apache-2.0`. The crate links Switchres (GPL-2.0-or-later) through `neptron-switchres-sys`, so a binary that uses it is a combined work. See `docs/adr/0002-switchres-link-licence.md`.

## Tests

- `src/convert.rs`: every conversion, its rounding and its errors.
- `tests/integration/solver.rs`: the solver on presets from `data/monitors` (results compared with the upstream standalone program), every error, the settings, and a grid of 7 widths, 9 heights, 6 rates and both scan types on all 25 presets, in which every candidate has its timings in order.
- `tests/integration/golden.rs`: timing check 1, the golden list in `tests/golden/systems.toml`. It has 30 real systems and 61 cases that cover all 25 presets. On a difference, the test prints every case that changed, with the expected and the actual modeline.

- `src/policy/*`: a unit test per rule of each step, at its limits. `src/policy/clock.rs` tests the whole-kHz clock on the `ntsc` range of one field rate, at the top of a range, at a porch on its 90 % limit, and the fallback to the nearest kHz.
- `tests/integration/policy_table.rs`: timing check 3, one row per rule of the decision procedure: the rung and the mode family that win after the validator; a drop table with one row per `Dropped` reason, values from the Definition or a hand calculation; and steps 6 to 8, the padding rule, the width strategies at the GPU floor and the Display panel rows. `NoMode` cannot happen through `decide`, because Switchres returns one mode per range for every request with editable lines, width and rate; a unit test of step 5 covers it.
- `tests/integration/policy_golden.rs`: the golden list through the policy on `generic_15`, `arcade_15`, `pc_31_120`, `arcade_31`, the EDID profile and a 1080p panel (`tests/golden/policy.toml`), and timing check 4, each entry of the GPU capability table (`tests/golden/policy_gpus.toml`). The file headers say where the values come from. Every winner has a pixel clock in whole kHz, the check that each display backend applies.
- `tests/integration/policy_property.rs`: 2,160 random sources with a fixed seed over every preset, the EDID profile and the panel, each with every picture style and every refresh policy: 43,200 decisions in about 3 s in the dev profile. One source in eight has a value of 0, 1 or the type's largest on an axis: a zero value gives `EmptySource`, every other source a plan. No panic, every rectangle lies inside its mode, the crop is the frame less the overscan crop (lines only, within the bound), every whole-scale claim holds against the crop, no fitted proposal comes before an exact one, every exact proposal of `native` on a CRT has a vertical scale of 1, two decisions are equal, every CRT proposal has a pixel clock in whole kHz and the rates of that clock and its totals, and the winner's proposal is the mode the validator approved. On the winner: with `exact_x2`, `force_50` or `force_60`, the asked rate wins whenever an exact mode at it validates; a progressive source never gets an interlaced fit while a progressive exact mode validates; `Notice::fitted` is set exactly for a fitted winner and `Notice::cropped_lines` exactly when the winner has a crop.
- `examples/policy_timing.rs`: a program, not a test, that prints the time of one decision. Tier 1 builds it but does not run it, because a wall clock gives no stable result.
- `tests/integration/safety.rs`: the second opinion. Every golden modeline validates with `neptron_safety::validate` against its preset, and so do 640x480 at 60 Hz, 320x240 at 120 Hz and 256x224 at 60.099 Hz on the profile that `neptron-platform` builds from the EDID of a 30 to 71 kHz, 50 to 160 Hz PC CRT (values copied from its tests). `neptron-safety` is a dev-dependency only.

## The golden list

The expected modelines are recorded from Switchres at the vendored commit. No source independent of Switchres gives them. Each one is the output of the upstream standalone program, built from the same commit and not from this crate. That program reads the presets from its own table and selects the mode in its own `get_mode`, so a match also checks the shim and the conversion. The source geometries and where they come from are in the file.

To check a value against the standalone program:

1. Get the Switchres archive of the vendored commit (`vendor/switchres/README.md` has the URL) and unpack it into a new directory.
2. Build the program there: `c++ -std=c++11 -ffp-contract=off -I. switchres_main.cpp switchres.cpp display.cpp custom_video.cpp monitor.cpp modeline.cpp log.cpp edid.cpp -o switchres`. On Linux, add `display_linux.cpp`.
3. Run it from an empty directory with the command line in the comment of the case. The program reads a `switchres.ini` from the working directory and from `/etc` (or `./ini`) if one exists, and that changes the result.
4. `expect` is the timing part of the "Modeline" line, after the quoted label, with the spaces collapsed.

A changed modeline is a change in what NEPTRON sends to a CRT. Update the file only after the standalone program gives the same value.
