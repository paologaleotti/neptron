# neptron-switchres-sys

The Switchres C++ build and the raw FFI calls, and nothing else. `neptron-modeline` is its safe wrapper.

## Public API sketch

- `fn modelines(&[Range], &Settings, &Request) -> Result<Vec<Mode>, CallError>`: one modeline per range, the ones in range only, best first in the order of Switchres.
- `Range`, `Settings`, `Request`: the inputs, in the units of Switchres, with types that hold no value the C++ cannot take (see Safety).
- `Mode`: the `#[repr(C)]` output struct.
- `MAX_RANGES`, the limits `REFRESH_MIN_HZ`, `REFRESH_MAX_HZ`, `MONITOR_ASPECT_MIN`, `MONITOR_ASPECT_MAX`, and the `WEIGHT_` result bits.

## How it is built

`build.rs` compiles two archives with the `cc` crate:

- `switchres`: the calculation sources in `vendor/switchres` (`modeline.cpp`, `monitor.cpp`, `log.cpp`), unchanged. Warnings are off for them.
- `neptron_switchres_shim`: `shim/neptron_switchres.cpp`, with warnings as errors.

Both use `-std=c++11` and `-ffp-contract=off`. The flag stops the compiler from fusing a multiply and an add: GCC fuses by default on aarch64, Clang does not. The sources use no fast-math option and from libm only `floor`, `ceil`, `round`, `roundf` and `fabs`, which are exact. With these settings the doubles are the same on macOS and Linux. Only macOS (arm64, Apple Clang) is checked locally; Linux runs in CI.

Switchres has no build for macOS, and its makefile also compiles the display backends. The build script compiles the files directly instead.

## Why a shim

Switchres selects a mode in `display_manager::get_mode` (`display.cpp`). That file also holds the display backends (X11, DRM/KMS, SDL2, Windows), which NEPTRON must not compile. The shim repeats the steps of `get_mode` for a display that can add a mode, which is the case of the `--calc` option of the standalone program:

1. The source mode gets the size, the refresh (rounded to a `float`, as `get_mode` takes it) and the interlace and rotation flags.
2. For each range, `modeline_create` makes a candidate from an editable mode.
3. The selection loop of `get_mode` with `modeline_compare` picks the best. The shim repeats it over the candidates not yet taken, so the first entry is the mode that `get_mode` returns and the list is a ranking.
4. `modeline_adjust` runs on each entry. The geometry adjustment is off, so it changes nothing.

Before the calculation, each range goes through the same steps as `monitor_fill_range` after it parses a `crt_range` line, and `monitor_evaluate_range` checks it. The shim catches every C++ exception and returns a status, so no unwind crosses into Rust.

## Safety

- `src/ffi.rs` holds the `extern "C"` declaration and the private `#[repr(C)]` structs. Compile-time asserts check the size of each struct and the offset of every field, on both sides: `static_assert` lines in `shim/neptron_switchres.cpp` and `const` asserts at the end of `src/ffi.rs`.
- `modelines` is the only `unsafe` call. It holds one lock across the call, because Switchres keeps its log functions in global variables. The lock is in this crate, so that the safe function is sound for every caller.
- `modelines` is a safe function, so every value of its input types must be safe in the C++. Each field that crosses the boundary:

| Field | How an unsafe value is closed |
| --- | --- |
| `Range` rates and times (`f64`) | The shim refuses a value that is not finite (`InvalidRange`): `monitor_evaluate_range` lets a NaN through. It then bounds every rate and time |
| `Range` sync polarities | `bool` |
| `Range` line limits | `u16`; `monitor_evaluate_range` checks their order and fit |
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

## Must never depend on

- Any NEPTRON crate
- Any crate except the build tools for Switchres (`cc`)

## Rules

- `unsafe` is allowed: the crate holds the raw FFI call. The safe wrapper of the values is in `neptron-modeline`.
- No features.
- `CallError` implements `Display` and `Error` by hand, because the crate takes no dependency except `cc`.

## Licence

The source of this crate (build script, shim, Rust code) is `MIT OR Apache-2.0`. The archive it links contains Switchres, which is GPL-2.0-or-later. See `docs/adr/0002-switchres-link-licence.md`.

## Tests

`tests/integration` compares the modelines with the output of the upstream standalone program `switchres <w> <h> <refresh> --calc`, built from the vendored commit. It also covers each error and each limit at its boundary, the three inputs that overflowed Switchres before the types closed them, the ranking over two ranges, and calls from eight threads at once.
