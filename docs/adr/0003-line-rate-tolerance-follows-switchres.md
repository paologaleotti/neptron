# 0003: The lowest line rate of a range follows Switchres

- Status: accepted
- Date: 2026-11-11
- Task: T04

## Context

The safety validator checks a mode against the ranges of a CRT profile. At first it checked the line rate exactly: from `h_min` to `h_max` of a range, both ends included.

The 61 modelines of the Switchres golden list were recorded from upstream Switchres at commit 09272ac24640daee45f5be15b1b856ecd220dda2. With the exact check, 49 of them pass. All 12 failures are line rates just below `h_min` of a range with one fixed line rate: `pal` 5 times at 15,600,000 mHz against 15,625,000, `ntsc` 4 times from 15,704,278 to 15,734,250 against 15,734,260, and `system24` 3 times at 24,907,890 against 24,960,000.

Switchres picks the lines per field in `total_lines_for_yres` (`modeline.cpp`). It adds one line while `vfreq * lines < hfreq_min` and `vfreq * (lines + 1) < hfreq_max`. The line rate is the field rate times the lines. The loop therefore ends with a line rate of at least `hfreq_min`, or of at least `hfreq_max - vfreq` when the next line would reach `hfreq_max`. Switchres does not check the result against `hfreq_min` again. A whole number of lines per field cannot meet a fixed line rate and a fixed field rate together, so the mode for a fixed range is often one line short.

The project owner decided that NEPTRON trusts Switchres: the validator accepts every mode that Switchres generates as in range for a monitor range, and nothing wider.

## Decision

The lowest line rate of a range, for a mode with the field rate `field`, is the lower of `h_min` and `h_max - field`. One function, `line_rate_min_mhz` in `crates/neptron-safety/src/tolerance.rs`, computes it. Its doc comment gives the Switchres reason.

What stays strict:

- The highest line rate of a range: no tolerance above `h_max`.
- The field rate: no tolerance.
- The hard rails: they are not widened. A mode must still pass the rails of the class of its own line rate. For every shipped range the lowest accepted line rate stays inside the class of `h_min` and inside its rail, and a test proves this.

The same change tightens the longest sync pulse from 200 % to 150 % of the nominal time. A vertical pulse may still be one line longer than nominal. The Switchres goldens reach 1.26 times the nominal horizontal sync and 1.20 times the nominal vertical sync.

## Consequences

- All 61 Switchres goldens validate, 12 of them below `h_min` of a fixed range. A test in `crates/neptron-safety/tests/integration/switchres.rs` keeps them.
- A fixed range accepts a line rate up to one field rate below its value: 50 Hz below 15,625 Hz for `pal`, 59.94 Hz below 15,734.26 Hz for `ntsc`. That is 0.4 % or less. A range wider than the field rate keeps its exact minimum.
- A custom range with `h_min` close to a class boundary can accept a mode in the class below. The rails of that lower class then judge the mode.
- The pixel clock of the wrapper is rounded to whole hertz. A mode that sits exactly at the new bound can fall below it by about 1 mHz after that rounding. No golden is near the bound.

## Alternatives

- A fixed percentage below `h_min`: rejected, because it accepts more than Switchres generates on wide ranges and has no source.
- `h_min - field` for every range: rejected, because on a range wider than the field rate Switchres never goes below `h_min`.
- A tolerance on the field rate or above `h_max`: rejected, because Switchres output does not need one.
- Keep the exact check: rejected by the project owner, because it refuses Switchres modes for `pal`, `ntsc` and the 24960 Hz arcade ranges.
