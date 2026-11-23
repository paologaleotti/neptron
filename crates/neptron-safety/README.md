# neptron-safety

The CRT safety validator. `validate` is the only way to make a `ValidatedMode`, and every backend that programs a display takes a `ValidatedMode` only. Range checks and hard rails, integer maths only.

## Must never depend on

- Any crate except `neptron-types` (normal dependencies). `xtask rules` checks this. The build script uses `neptron-types` and `toml` to read the presets, and the tests use `toml` too.
- Floating point maths
- Macros or generics that hide a limit

## Public API

- `struct ValidatedMode`: private fields, no `Default`, no public constructor, no `Deserialize`. `mode()` gives the `Modeline` that passed. `tv_standard()` gives the `TvStandard` of a TV profile, else `None`; only `validate` sets it, so the standard that reaches a backend is the one validated with the mode.
- `fn validate(&Modeline, &MonitorProfile, &GpuCaps) -> Result<ValidatedMode, Rejection>`: judges the modeline and changes nothing.
- `enum Rejection`: one variant per check, each with the value found and the limit: `Malformed`, `LineRate`, `FieldRate`, `LineCount`, `SyncWidth`, `Porch`, `PixelClock`, `Interlace`, `HardRail`, `NotAPanelMode`, `NotATvMode`. It implements `Display` and `std::error::Error` by hand, because the crate cannot depend on `thiserror`.
- `enum Axis`, `enum PorchEdge`, `enum InterlaceLimit`: the details of some rejections.

## What `validate` checks

The checks run in this order. The first one that fails gives the rejection.

1. Structure (`Malformed`): the pixel clock, `hactive` and `vactive` are not 0, and on each axis `active <= sync_start <= sync_end <= total`.
2. GPU (`PixelClock`, `Interlace`): the pixel clock is inside the GPU range, and an interlaced mode needs a GPU that sends interlace.
3. Flat panel (`NotAPanelMode`): the mode is one of the listed modes, field for field. No other check applies to a panel, and the rails do not hold a custom flat-panel profile.
4. TV (`NotATvMode`): the mode is one of the two modes of the profile's standard (`TvStandard::modes`), field for field. It must then also pass the hard rails of its class (step 6), as a second opinion against a mistake in the table. The four table modes are all in the 15 kHz class (ADR 0012).
5. CRT ranges: one range of the profile must accept the whole mode. A range checks, in order: line rate, field rate, active lines, horizontal sync, vertical sync, the four porches. When no range accepts the mode, the rejection comes from the range that failed at the latest stage (low and high of one value are one stage), and among those the one whose broken limit is nearest to the mode's value.
6. Hard rails (`HardRail`): the mode must also pass the rails of the class of its line rate. A custom CRT profile cannot widen them. Above 40 kHz no rail applies and the profile is the only limit.

### Units and rounding

- Line and field rates come from `Modeline::line_rate_mhz` and `field_rate_mhz`, rounded to the nearest millihertz by `neptron-types` (up to 0.5 mHz from the exact value). The rounding rule for times below does not apply to rates.
- The field rate and the highest line rate of a range have no tolerance.
- The lowest line rate of a range is the lower of `h_min` and `h_max` less the field rate of the mode (`line_rate_min_mhz` in `src/tolerance.rs`). Switchres adds lines per field while the line rate is below `h_min` and one more line keeps it below `h_max`, and it does not check `h_min` again. So a fixed range (`ntsc`, `pal`, the 24960 Hz range of `arcade_25`) accepts up to one field rate below its line rate, and a range wider than the field rate keeps its minimum. Such a mode still passes the rails of the class of its own line rate; below a class boundary, the rails of the lower class judge it.
- Active lines count both fields of an interlaced mode and twice `vactive` of a doublescan mode, as Switchres counts them.
- Porch and sync times are exact fractions of a nanosecond. A time is at least a limit when its value rounded down is, and at most a limit when its value rounded up is. A rejection reports the rounded value.
- One vertical unit is half a scan line in an interlaced mode and two scan lines in a doublescan mode. One line, for the vertical tolerance, is one scan line, or two for doublescan.

### Tolerances on porches and sync pulses

A range gives one nominal time per porch and per sync pulse.

| Value | Shortest | Longest |
| --- | --- | --- |
| Horizontal porch | 90 % of nominal | no limit |
| Horizontal sync | 90 % of nominal | 150 % of nominal |
| Vertical porch | 90 % of nominal, or nominal less one line if that is shorter | no limit |
| Vertical sync | 90 % of nominal, or nominal less one line if that is shorter | 150 % of nominal, or nominal plus one line if that is longer |

No porch or pulse can be empty: the shortest value is at least 1 ns. 90 % is the share that Switchres accepts when it fits the horizontal values to whole characters. A vertical value changes only in whole lines, and Switchres rounds it to whole lines after it centres the blanking. The 61 Switchres goldens reach at most 1.26 times the nominal horizontal sync and 1.20 times the nominal vertical sync.

### Not checked

- Sync polarity: a wrong polarity does not stress a monitor.
- `GpuCaps::max_width` and `max_height`: the test-only commit of the KMS backend refuses a mode the output cannot send.

`Rejection` has no variant for either.

## Hard rails

`build.rs` reads every file in `data/monitors` and writes the rails into `OUT_DIR/rails.rs`. A class is a band of line rates (`src/class.rs`):

| Class | Line rates |
| --- | --- |
| 15 kHz | below 19 kHz |
| 25 kHz | 19 kHz to below 28 kHz |
| 31 kHz | 28 kHz to below 40 kHz |

From 40 kHz up no rail applies: the profile built from the monitor's EDID is the limit.

A preset range belongs to every class whose line rates it touches, both ends included. The rails of a class are the widest values among its ranges: the lowest minimum and the highest maximum of the line rate, the field rate and the line counts, the shortest and the longest nominal sync time on each axis, and the shortest nominal porch. The line rates are clipped to the class. A class with no interlaced range allows no interlace. The rail checks use the same tolerances as the range checks, so a mode that a shipped preset accepts never hits a rail.

| Rail | 15 kHz | 25 kHz | 31 kHz |
| --- | --- | --- | --- |
| Line rate, mHz | 15 250 000 to 18 999 999 | 19 000 000 to 27 999 999 | 28 000 000 to 39 999 999 |
| Field rate, mHz | 40 000 to 80 000 | 40 000 to 80 000 | 40 000 to 130 000 |
| Progressive lines | 192 to 320 | 288 to 400 | 192 to 1024 |
| Interlaced lines | 448 to 576 | 768 to 800 | none |
| Horizontal sync nominal, ns | 400 to 4 750 | 400 to 4 688 | 275 to 3 813 |
| Vertical sync nominal, ns | 40 000 to 192 000 | 40 000 to 200 000 | 32 000 to 164 000 |
| Shortest porch nominal (h front, h back, v front, v back), ns | 200, 4 700, 40 000, 640 000 | 200, 3 200, 40 000, 640 000 | 170, 1 750, 20 000, 436 000 |

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`. No features.
- Every limit is a named constant with its unit in the name.
- The crate stays near 500 lines, tests excluded.
- A change to a preset that moves a rail fails `rails_match_the_presets` in `src/rails.rs`, which pins each value with the preset it comes from. The new value needs a review against the presets.

## Tests

Run `cargo xtask test -p neptron-safety`.

- Unit tests next to each module: every bound at its limit, one unit inside and one unit outside, and the rejection it gives. `every_preset_lies_inside_its_rails` proves that each shipped range lies inside the rails of each class it touches.
- `tests/integration/presets.rs`: a mode in the middle of each of the 54 shipped ranges validates.
- `tests/integration/switchres.rs`: the 61 modelines that upstream Switchres generates for the golden list all validate, 12 of them below the line rate of a fixed range.
- `tests/integration/property.rs`: random modes for every preset and three GPU entries validate or are rejected with a reason, and nothing panics.
- `tests/integration/synth.rs`: a test helper that places the nominal porches and sync pulses of a range around an active area.
