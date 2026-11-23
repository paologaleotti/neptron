# neptron-switchres-sys

The Switchres C++ build and the raw FFI calls, and nothing else. `neptron-modeline` is its safe wrapper.

## Must never depend on

- Any NEPTRON crate
- Any crate except the build tools for Switchres (`cc`). So `CallError` implements `Display` and `Error` by hand.

## Public API sketch

- `fn modelines(&[Range], &Settings, &Request) -> Result<Vec<Mode>, CallError>`: one modeline per range, the ones in range only, best first in the order of Switchres.
- `Range`, `Settings`, `Request`: the inputs, in the units of Switchres, with types that hold no value the C++ cannot take (see Safety).
- `Mode`: the `#[repr(C)]` output struct.
- `MAX_RANGES`; the input limits `REFRESH_MIN_HZ`, `REFRESH_MAX_HZ`, `MONITOR_ASPECT_MIN`, `MONITOR_ASPECT_MAX`; the blanking limits `HBLANK_MAX_FRACTION`, `VBLANK_MAX_FRACTION`, `VBLANK_MIN_LINES` (`CallError::Blanking`, `BlankingFault`); the `WEIGHT_` result bits.

## How it is built

`build.rs` compiles two archives with the `cc` crate. It does not use the Switchres makefile: that has no macOS build and also compiles the display backends.

- `switchres`: `modeline.cpp`, `monitor.cpp` and `log.cpp` from `vendor/switchres`, unchanged, with warnings off.
- `neptron_switchres_shim`: `shim/neptron_switchres.cpp`, with warnings as errors.

Both use `-std=c++11` and `-ffp-contract=off`. The flag stops the compiler from fusing a multiply and an add (GCC fuses by default on aarch64, Clang does not). The sources use no fast-math option and from libm only `floor`, `ceil`, `round`, `roundf` and `fabs`, which are exact. So the doubles are the same on macOS and Linux. Only macOS (arm64, Apple Clang) is checked locally; Linux runs in CI.

## Why a shim

Switchres selects a mode in `display_manager::get_mode` (`display.cpp`), next to the display backends (X11, DRM/KMS, SDL2, Windows) that NEPTRON must not compile. The shim repeats the steps of `get_mode` for a display that can add a mode, as the `--calc` option of the standalone program does:

1. The source mode gets the size, the refresh (rounded to a `float`, as `get_mode` takes it) and the interlace and rotation flags.
2. For each range, `modeline_create` makes a candidate from an editable mode.
3. The selection loop of `get_mode` with `modeline_compare` picks the best. The shim repeats it over the candidates not yet taken, so the first entry is the mode that `get_mode` returns and the list is a ranking.
4. `modeline_adjust` runs on each entry. The geometry adjustment is off, so it changes nothing.

Before the calculation, each range goes through the steps of `monitor_fill_range` after a parsed `crt_range` line, and `monitor_evaluate_range` checks it. The shim catches every C++ exception and returns a status, so no unwind crosses into Rust.

## Safety

- `src/ffi.rs` holds the `extern "C"` declaration and the private `#[repr(C)]` structs. Compile-time asserts check the size of each struct and the offset of every field on both sides: `static_assert` lines in the shim and `const` asserts at the end of `src/ffi.rs`.
- `modelines` is the only `unsafe` call. It holds one lock across the call, because Switchres keeps its log functions in global variables. The lock is in this crate, so the safe function is sound for every caller.
- `modelines` is safe, so every value of its input types must be safe in the C++:

| Field | How an unsafe value is closed |
| --- | --- |
| `Range` rates and times (`f64`) | The shim refuses a value that is not finite (`InvalidRange`): `monitor_evaluate_range` lets a NaN through. It then bounds every rate and time |
| `Range` sync polarities | `bool` |
| `Range` line limits | `u16`; `monitor_evaluate_range` checks their order and fit |
| `Range` blanking | The shim refuses a blanking outside the limits below (`Blanking`) |
| `Settings.pclock_min_hz` | `u32`: a larger floor overflowed the width doubling in `modeline_create` |
| `Settings.monitor_aspect` (`f64`) | The shim refuses a value outside 0.25 to 4 (`InvalidSettings`): the width that Switchres scales by it can overflow `int` |
| `Settings.refresh_tolerance_hz` (`f64`) | Every value is safe: Switchres only compares with it |
| `Settings.super_width` | `u16`; Switchres only compares with it |
| `Settings` flags | `bool` |
| Vertical shift correction | Not in the type; the shim keeps it off. With it on, a large interlaced range gave a negative border and a signed overflow |
| `Request.width`, `Request.height` | `u16`; the shim refuses 0 (`InvalidRequest`), on which Switchres loops forever. A width of 2,000,000,000 overflowed `get_line_params` |
| `Request.refresh_hz` (`f64`) | The shim refuses a value outside 0.001 to 10,000,000 Hz (`InvalidRequest`): below, the float that `get_mode` makes can reach 0 and Switchres loops forever; above, the float conversion can overflow |
| `Request` flags | `bool` |
| Pointers, range count, capacity | Set by `modelines` from slices and a local array |
| `Mode` (output) | Plain data; every value is safe in Rust |

`monitor_evaluate_range` accepts ranges whose blanking Switchres cannot handle. Then Switchres loops forever (and overflows `int`) in `get_line_params` when the horizontal blanking fills the line, or divides by zero in `total_lines_for_yres` and `max_vfreq_for_yres`. A hang would also hold the lock for every later call. So the shim refuses such a range with `CallError::Blanking`, the value and the limit:

| Limit | Value | Why |
| --- | --- | --- |
| Horizontal blanking, as a fraction of `1 / hfreq_max` | below 0.75 | A mode line can be up to about 1.2 % shorter than `1 / hfreq_max`, so the blanking fills at most 0.759 of a line, and `get_line_params` ends for every fraction below 1. 0.5 would refuse `pc_70_120` (0.501) |
| Vertical blanking, as a fraction of `1 / vfreq_max` | below 0.5 | The divisor `1 - vfreq * vblank` of `total_lines_for_yres` stays above 0.5, because no mode has a field rate above `vfreq_max` and the border stays 0 |
| Vertical blanking, in lines at `hfreq_max` | at least 1 | The divisor of `max_vfreq_for_yres` stays at 1 or more when the range allows zero lines |

Every shipped preset passes. The largest fractions are 0.501 horizontal (`pc_70_120`) and 0.141 vertical (`d9200`).

Every other loop in the calculation counts toward a bound that the checked values keep finite. A fuzz run of 6,000,000 random calls over the whole domain of each Rust type, built with `-fsanitize=undefined,float-cast-overflow,float-divide-by-zero` outside the repository, found no hang (5 s limit per call), no exception and no undefined behaviour; the line rate of a mode was at most 1.0113 times `hfreq_max`, and the blanking filled at most 0.756 of a line. Nothing can stop a thread inside C++, so the guard cannot stop a call that is only slow.

## Rules

- `unsafe` is allowed: the crate holds the raw FFI call. The safe wrapper of the values is in `neptron-modeline`.
- No features.

## Licence

The source of this crate (build script, shim, Rust code) is `MIT OR Apache-2.0`. The archive it links contains Switchres, which is GPL-2.0-or-later. See `docs/adr/0002-switchres-link-licence.md`.

## Tests

Run `cargo xtask test -p neptron-switchres-sys`.

- `tests/integration` compares the modelines with the output of the upstream standalone program `switchres <w> <h> <refresh> --calc`, built from the vendored commit. It also covers each error and each limit at its boundary, the three inputs that overflowed Switchres before the types closed them, the ranking over two ranges, and calls from eight threads at once.
- `tests/integration/fuzz.rs` draws as the fuzz run does, with a fixed seed: 50,000 calls in tier 1, and 2,000,000 on demand (`--run-ignored only`, about a minute).
- The guard and fuzz tests rely on the slow-test timeout in `.config/nextest.toml`: a hung test fails after three minutes instead of blocking the run.
