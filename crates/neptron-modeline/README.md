# neptron-modeline

Safe Switchres wrapper, monitor profiles and the ODREN policy: from source geometry, profile and GPU capabilities to a list of candidate modelines. No I/O.

## Public API sketch

- `fn modelines(&SourceGeometry, &MonitorProfile, &GpuCaps) -> Result<Vec<Candidate>, ModelineError>`: the modelines of Switchres with the default settings.
- `fn modelines_with(&SourceGeometry, &MonitorProfile, &GpuCaps, &Settings) -> Result<Vec<Candidate>, ModelineError>`
- `Candidate`: a `Modeline`, the index of the range that made it, and the score of Switchres (stretched, refresh off, scale, refresh difference) in integers.
- `Settings`: doublescan, refresh tolerance, super width, monitor aspect.
- `policy` module: the decision procedure, each candidate with its reason. Not written yet.

## What the solver does

- It accepts a CRT profile with 1 to 10 ranges. A flat-panel profile is an error: it lists its modes, and Switchres does not calculate for it.
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
