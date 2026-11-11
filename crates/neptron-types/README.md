# neptron-types

Shared plain data: modeline, CRT range, monitor profile, GPU capabilities, source geometry, frame, scene and backend events. Plain data with serde, no I/O.

Every contract value is an integer. Frequencies are in millihertz (`_mhz`), or in hertz where the name says `_hz`. Times are in nanoseconds (`_ns`).

## Public API

- `Modeline`: integer timings as DRM gets them. `line_rate_mhz()` and `field_rate_mhz()` round to the nearest millihertz and return 0 for a zero total.
- `CrtRange`: one Switchres `crt_range` line in integer units.
- `MonitorKind` (`Crt { ranges }` or `FlatPanel { modes }`) and `MonitorProfile { id, kind }`. In TOML the variant is a table key: `[[kind.crt.ranges]]`. The presets in `data/monitors/` use this shape.
- `GpuCaps`: pixel clock range, interlace, flip per field, maximum size.
- `Rotation` (counter-clockwise, as in libretro) and `SourceGeometry`.
- `PixelFormat` with `bytes_per_pixel()`, `Frame`, `Rect`, `Filter`, `Scene`. `Frame` and `Scene` borrow pixel data and have no serde.
- `Scanlines { kind, strength_percent }` in `Scene`, with `ScanlineKind` (`none`, `black`) and `Scanlines::NONE`. The doc comment of `Scanlines` states the pixel rule; ADR 0006 records it. `ScanlineKind::dark_lines(k)` is the table of dark lines per vertical multiple.
- `ScanlineMode` (`auto`, `off`, `on`, the choices of the setting `display.scanlines`), `ScreenClass` (`crt15khz`, `crt31khz`, `flat_panel`, with `ScreenClass::of(kind, mode)`) and `scanlines_for(mode, strength_percent, vertical_multiple, screen, interlaced)`: the one rule that turns the settings into the `Scanlines` of a scene, for the engine and for the preview.
- `ui_dest_fill(mode_width, mode_height)` and `ui_dest_uniform(mode_width, mode_height, canvas_width, canvas_height)`: the UI destination rectangle for a CRT (the full active area) and for a flat panel (the largest whole scale that fits both axes, centred).
- `FlipEvent { time_ns, sequence }`, `InputEvent { device, control, value }` and `Control`, the RetroPad controls.

## Must never depend on

- Any other NEPTRON crate
- Any crate that does I/O or talks to hardware
- Floating point in any contract type

`toml` and `serde_json` are test dependencies only.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
- A type on disk or on the wire changes by addition only.
- Every enum serialises in snake_case (`r90`, `sharp_bilinear`, `left_x`). A test pins each wire name.

## Status

The contract types are in place.
