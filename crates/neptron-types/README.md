# neptron-types

Shared plain data with serde and no I/O: modeline, CRT range, monitor profile, GPU capabilities, source geometry, frame, scene and backend events.

## Must never depend on

- Any other NEPTRON crate
- Any crate that does I/O or talks to hardware
- Floating point in any contract type

`toml` and `serde_json` are test dependencies only.

## Public API

- `Modeline`: integer timings as DRM gets them. `line_rate_mhz()` and `field_rate_mhz()` round to the nearest millihertz and return 0 for a zero total.
- `CrtRange`: one Switchres `crt_range` line in integer units.
- `MonitorKind` (`Crt { ranges }`, `FlatPanel { modes }` or `Tv { standard }`) and `MonitorProfile { id, kind }`. In TOML the variant is a table key: `[[kind.crt.ranges]]`, `[kind.tv]`. The presets in `data/monitors/` are all CRTs. A TV profile is built in: `TvStandard::profile()` gives the id `tv_<key>`, and `TvStandard::from_profile_id()` reads it back.
- `TvStandard`: the ten broadcast standards of a composite encoder (`TvStandard::ALL`), with `lines()`, `kms_name()` (the value of the connector property `TV mode`), `key()` and `from_key()` (the name in settings and files, also the serde name), and `modes()` (the progressive and the interlaced mode of its line system). The four modes are `TV_525_240P`, `TV_525_480I`, `TV_625_288P` and `TV_625_576I`, all at `TV_PCLOCK_HZ` (13.5 MHz), with the values that the Linux kernel gives the Raspberry Pi composite encoder (ADR 0012).
- `GpuCaps`: pixel clock range, interlace, flip per field, maximum size.
- `OutputKind` (`Analog`, the default, or `Digital`): the kind of link from an output to its display, decided once from the class rule of the detection. It picks the GPU limits that bound a mode: the careful CRT limits on an analog output, the limits of the link and the scanout on a digital link to a flat panel (ADR 0010).
- `Rotation` (counter-clockwise, as in libretro) and `SourceGeometry`. The arithmetic of `Rotation` is modulo a full turn: `plus`, `minus`, `inverse`, `quarter_turns` and `from_quarter_turns`, `degrees` and `from_degrees` (`None` for a value that is not 0, 90, 180 or 270), and `across` (the turn swaps the axes).
- `Orientation { screen, game_override }`: the screen orientation `S` (counter-clockwise, the direction of `Rotation`: `R90` is a display turned to the left) and the game override (`None` for Auto). `Orientation::NORMAL` is a screen at 0 and Auto. `game_turn(core)` is `(G' - S) mod 360`, `ui_turn()` is `(0 - S) mod 360`, and `turns(core)` gives both in `Turns` with `PictureSize` (`OwnLinesFull` or `AcrossSmall`) and the flag of a horizontal game across a turned screen. `display_source(&source)` is the source with `rotation` set to the game turn. A test holds `turns` equal to the table of `docs/spec/tate.md`, section 4.
- `PixelFormat` with `bytes_per_pixel()`, `Frame`, `Rect`, `Filter`, `Scene`. `Frame` and `Scene` borrow pixel data and have no serde.
- `Scanlines { kind, strength_percent }` in `Scene`, with `ScanlineKind` (`none`, `black`) and `Scanlines::NONE`. The doc comment of `Scanlines` states the pixel rule (ADR 0006). `ScanlineKind::dark_lines(k)` gives the dark lines per vertical multiple.
- `ScanlineMode` (`auto`, `off`, `on`: the choices of `display.scanlines`), `ScreenClass` (`crt15khz`, `crt31khz`, `flat_panel`; `ScreenClass::of(kind, mode)`) and `scanlines_for(mode, strength_percent, vertical_multiple, screen, interlaced)`: the one rule that turns the settings into the `Scanlines` of a scene, for the engine and for the preview.
- `ui_dest_fill(mode_width, mode_height)` and `ui_dest_uniform(mode_width, mode_height, canvas_width, canvas_height)`: the UI destination rectangle on a CRT (the full active area) and on a flat panel (the largest whole scale that fits both axes, centred).
- `FlipEvent { time_ns, sequence }`, `InputEvent { device, control, value }` and `Control`, the RetroPad controls.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
- Every contract value is an integer. Frequencies are in millihertz (`_mhz`), or in hertz where the name says `_hz`. Times are in nanoseconds (`_ns`).
- A type on disk or on the wire changes by addition only.
- Every enum serialises in snake_case (`r90`, `sharp_bilinear`, `left_x`). A test pins each wire name.

## How to test

`cargo xtask test -p neptron-types`.
