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

- `tests/integration/safety.rs`: the second opinion. Every golden modeline validates with `neptron_safety::validate` against its preset, and so do 640x480 at 60 Hz, 320x240 at 120 Hz and 256x224 at 60.099 Hz on the profile that `neptron-platform` builds from the EDID of a 30 to 71 kHz, 50 to 160 Hz PC CRT (values copied from its tests). `neptron-safety` is a dev-dependency only.

## The golden list

The expected modelines are recorded from Switchres at the vendored commit. No source independent of Switchres gives them. Each one is the output of the upstream standalone program, built from the same commit and not from this crate. That program reads the presets from its own table and selects the mode in its own `get_mode`, so a match also checks the shim and the conversion. The source geometries and where they come from are in the file.

To check a value against the standalone program:

1. Get the Switchres archive of the vendored commit (`vendor/switchres/README.md` has the URL) and unpack it into a new directory.
2. Build the program there: `c++ -std=c++11 -ffp-contract=off -I. switchres_main.cpp switchres.cpp display.cpp custom_video.cpp monitor.cpp modeline.cpp log.cpp edid.cpp -o switchres`. On Linux, add `display_linux.cpp`.
3. Run it from an empty directory with the command line in the comment of the case. The program reads a `switchres.ini` from the working directory and from `/etc` (or `./ini`) if one exists, and that changes the result.
4. `expect` is the timing part of the "Modeline" line, after the quoted label, with the spaces collapsed.

A changed modeline is a change in what NEPTRON sends to a CRT. Update the file only after the standalone program gives the same value.
