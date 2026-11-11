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
- `fn notice(&Considered) -> Option<Notice>`: step 8.
- `fn style_rows(..., validate) -> Vec<StyleRow>`: the picture style rows of the Display panel, from `decide` and `choose` for each style. A row whose winner is not on its own rung is greyed out; the first candidate on its own rung holds the reason.
- `Display` on `Considered` and `Dropped` gives one line with the rule and the numbers, for the log.

### The decision procedure

| Step | Function | What it does |
| --- | --- | --- |
| 1 | `take_source` | Width, height, refresh, scan. A game turned by 90 or 270 degrees swaps its axes, so its long side becomes lines. |
| 2 | `read_settings` | No strategy gives the GPU default: dynamic super width with a pixel clock floor, native width without one. A fixed width of 0 or wider than the GPU becomes the widest of 3840, 2560 and 1920 that the GPU sends. |
| 3 | `walk_ladder` | The rungs of the style. `auto` and `native`: native, 2x, 3x, 4x, interlaced 1:1, fitted. `2x`: 2x, interlaced, fitted. `3x` and `4x` fall to the smaller multiples first. |
| 4 | `refresh_targets`, `shape` | The refresh targets in order, and per rung the lines (height times the factor), the scan and the base width. |
| 5 | `solve` | The width by the horizontal strategy, and the modeline from Switchres. |
| 6 | `choose` | The validator walks the proposals in order. |
| 7 | `switch_for` | A winner equal to the current mode changes only scale and crop. |
| 8 | `notice` | A notice when the winner is the fitted rung, or when the game runs at a rate other than its own. |

### Rules

- **Refresh.** `exact` aims for the game's rate. A rate matches when the mode is less than `REFRESH_MATCH_MHZ` (5 mHz) from it: "matches to two decimals" means that no rounding to 0.01 Hz can tell the two apart. `exact_x2` (twice the rate, each frame shown twice), `force_50` and `force_60` come first only where a range covers the rate; the game's own rate follows them. A game rate that no range covers becomes the nearest covered rate (`Nearest`). The last resort is the fitted rung at the rate that Switchres reaches (`Unmatched`).
- **Lines.** A rung asks for the game's height times its factor. Switchres must keep the lines, the scan and the width, and reach the target rate; else the candidate is dropped (`LinesChanged`, `ScanChanged`, `WidthChanged`, `RefreshMissed`). An interlaced game on a monitor with no interlaced range is shown progressive (480i as 480p).
- **Fitted rung.** Switchres fits the source to the profile. When that misses the target rate, the policy asks again for the most progressive lines a range holds at the rate (`floor(h_max / rate)` less the vertical blanking, at most the line limit), so the game keeps its rate when the profile allows it. Donkey Kong (256 lines, 60.606 Hz) on `pc_31_120` gets 502 lines at 60.606 Hz, not 512 lines at 59.623 Hz.
- **Rung 3.** Interlaced 1:1 applies only when the game's lines do not fit a progressive range at the target rate: the range's line limit, or `floor(h_max / rate)` less the vertical blanking. A short game is centred in the interlaced minimum, as a vertical game on a 15 kHz set is.
- **Borders.** A progressive game with fewer lines than the ranges at the target rate allow is centred in a 224 or 240-line mode (`BORDER_LINES`). The mode width keeps the game's aspect. When neither height reaches the minimum, the rung is dropped (`NoBorderMode`).
- **Padding (OP-026).** A mode is padded when the game fills less than `PADDED_FILL_MIN_PERCENT` (75 %) of the picture lines. The picture lines are the larger of the mode's active lines per field and `ceil(h_min / rate) - ceil(h_min * vertical blanking)`, with the range that made the mode at the target rate. On the EDID profile of a 30 to 71 kHz PC CRT, a 224-line game at 60.099 Hz has 480 picture lines: 224 is 47 %, so the mode Switchres pads to 500 lines is padded, and 2x (448 lines, 93 %) or double refresh (224 of 230 lines) wins. On 15 kHz, 224 lines fill 93 % of 240.
- **Order.** Refresh targets in order. Within a target: the exact rungs that are not padded, in ladder order; then the padded exact rungs; then the fitted rung. A handheld on 15 kHz is then centred with borders, and a padded mode on 31 kHz loses to line doubling.
- **Width.** The policy computes the width, so the reason is its own rule: Switchres gets no pixel clock floor and never doubles the width. Native: the game width (or the border width). Dynamic super width: the smallest whole multiple whose pixel clock is at least the GPU floor, the bound the validator checks; the next multiple is estimated from the last pixel clock, at most 16 calls. Fixed super width: the fixed width, with a sharp filter. A game wider than the GPU gets the GPU's widest mode, with a sharp filter.
- **Placement.** The crop is the whole frame. On a CRT the game fills the width at a whole multiple, or the fixed width, and its lines times the factor, centred when bordered; the fitted rung fills the active area. The UI fills the active area (`ui_dest_fill`). On a flat panel the game gets the largest whole vertical factor at which its display aspect fits, centred; the UI gets one whole scale (`ui_dest_uniform`). `ScaleClaim` gives the whole factors that hold exactly; the filter is `Nearest` when both axes are whole, else `SharpBilinear`.
- **Flat panel.** Each listed mode is a proposal. Order: modes with a whole vertical scale first, then the preferred size (the first listed mode), then the rate per game frame nearest the game's, then the list order.

### Readings of the Definition

Where the Definition allows two readings, the policy takes the one below. Each is one named constant or function.

- "Falls through the ones below" for `3x` and `4x`: the smaller multiples first, never the native rung (`walk_ladder`).
- `native` walks the same rungs as `auto`; on 31 kHz its own rung is padded, so 2x wins and the Display panel greys `native` out.
- A medium-resolution board (384 lines) on 15 kHz: rung 3, interlaced 1:1 with borders, where its lines fill the field; the sources table says "interlaced fit". The ladder puts rung 3 above the fitted rung, and the fit stays the fallback.
- "Matches to two decimals": less than 5 mHz (`REFRESH_MATCH_MHZ`).
- `force_60` is 60.000 Hz and `force_50` is 50.000 Hz (`FORCE_60_MHZ`, `FORCE_50_MHZ`).
- A native or fixed width whose pixel clock is below the GPU floor gives no mode. The candidate is dropped with `PixelClockFloor` (width, pixel clock, floor, widest GPU mode), and the next entry is the same rung with the dynamic super width, marked `strategy_replaced`; `notice` reports it. A source wider than the GPU is dropped once: no multiple of its width fits.
- The dynamic super width needs a pixel clock of at least the floor, the bound of the validator; Switchres doubles while the clock is at most the floor.
- A flat panel: a whole vertical scale, the width at the core's aspect (`panel_placement`); no notice for the panel rate.

### What the engine does

1. On launch and on each geometry change, call `decide`, then `choose` with `|m| neptron_safety::validate(m, profile, caps)`. The winner's token is the `ValidatedMode` to set. The policy does not depend on `neptron-safety` (the dependency rules forbid it), so the engine validates every proposal itself; a proposal the validator refuses is skipped, never set.
2. Call `switch_for` with the current mode. `ScaleOnly`: change only crop, destination and filter. `Modeset`: commit after the debounce.
3. Build each `Scene` from the winner's `Placement`. Run the core one frame per `Timing::repeat()` flips. When `Timing::changes_speed()`, the game runs at `field_rate_mhz / repeat`; audio rate control follows the field rate.
4. Show `notice` in the quick menu. Log each `Considered` and each refusal.
5. Apply the EDID pixel clock limit to the GPU caps first (`EdidProfile::limit_caps`, OP-013).
6. Cache the plan by source geometry and settings. A decision takes tens of microseconds in the dev profile, so it can run on the frame thread, but a cache keeps the Switchres lock off the frame for a repeated geometry.

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

- `src/policy/*`: a unit test per rule of each step, at its limits.
- `tests/integration/policy_table.rs`: timing check 3, one row per rule of the decision procedure: the rung and the mode family that win after the validator, and steps 6 to 8, the padding rule, the dynamic super width and the Display panel rows.
- `tests/integration/policy_golden.rs`: the golden list through the policy on `generic_15`, `arcade_15`, `pc_31_120`, the EDID profile and a 1080p panel (`tests/golden/policy.toml`), and timing check 4, each entry of the GPU capability table (`tests/golden/policy_gpus.toml`). The file headers say where the values come from.
- `tests/integration/policy_property.rs`: 4,320 random decisions with a fixed seed over every preset, the EDID profile and the panel: no panic, every winner validates, every rectangle lies inside its mode, every whole-scale claim holds, and two decisions are equal.
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
