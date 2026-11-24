# Screen simulator

`cargo xtask screen` shows what a screen of a given profile shows for a game and its display settings. It runs the decision path of the app, draws the frame buffer of the mode, and then models the picture on the glass. It writes both pictures as PNG files with a set of measures. A survey runs a matrix of cases and marks the rows that look wrong.

The tool finds and reproduces bad picture modes: a crop, a small picture, a wrong shape, a fraction where a whole scale is promised. It is not a model of a real tube. It needs no ROM, no core and no hardware.

## Commands

```text
cargo xtask screen --display D --source S [--set KEY=VALUE]... [options]
cargo xtask screen survey [--display D]... [--source S]...
cargo xtask screen golden [--bless]
```

One case writes `target/screen/<case>/`: `frame.png` (the source picture), `wire.png`, `glass.png` and `measures.json`. It prints the measures as one row of a table. The survey writes `target/screen/survey/`: one contact sheet per display (`<display>.png`), `survey.md` and `survey.json`. `golden` compares the reference cases with `xtask/tests/golden/screen/`.

## A case

A case is a display, a source and the display settings.

**Display** (`--display`):

- A preset id of `data/monitors`, for example `generic_15`, `arcade_15`, `pc_31_120`. `preset:ID` forces the preset when the id is also a TV standard.
- A TV standard key of `neptron_types::TvStandard`, for example `ntsc` or `pal`: the TV profile of the composite output, as the app drives a TV.
- A flat panel `WIDTHxHEIGHT@RATE`, for example `1920x1080@60` or `2560x1440@120`. The panel lists one mode at its native size and rate, with CVT reduced blanking v2 timing (80 pixels of horizontal blank, at least 460 us of vertical blank) and a pixel clock in whole kHz.

The GPU caps come from an entry of `data/hardware/matrix.toml` (`--gpu ID`). The default is `intel_gen9` (the bench PC) for a preset and a panel, and `pi_vc4_composite` for a TV. A preset and a TV get the analog caps of the entry. A panel gets the widest digital limits of the entry (DisplayPort on the bench PC).

**Source** (`--source`):

- A name of `REFERENCE_SOURCES` of the policy: `gb`, `snes`, `psx`, `nes_pal`, `amiga_pal`, `psx_480i`, `neogeo`, `mk`, `ddonpach`, `dos_vga`.
- An arcade set of `docs/spec/arcade-table.md`, for example `rtype` (384x256 at 55.000 Hz), `gunbird` or `pacman`. The tool reads the headings of that file. A set that is not in the file is an error that lists the known sets. Every board has the 4:3 aspect of its frame as drawn, as FinalBurn Neo reports it.
- A source by hand: `WIDTHxHEIGHT@RATE` with `--aspect` (`4:3`, `1.333` or the pixel aspect `par:8:7`; default 4:3), `--rotate 0|90|180|270` (the turn the core asks for), `--interlaced` and `--arcade`.

The arcade boards and the reference boards `neogeo`, `mk` and `ddonpach` get the arcade rules of the policy (`PolicySettings::arcade`), as the app does for the systems `arcade` and `neogeo`.

**Settings** (`--set KEY=VALUE`, the key with or without `display.`): `display.style`, `display.refresh_policy`, `display.tv_width`, `display.vertical`, `display.panel_scale`, `display.scanlines`, `display.scanline_strength`. A key that is not given has the default of the settings schema. The values are the values of the settings files: the picture mode that the menu labels Fill is `display.style = fit`.

**Picture**: a test pattern at the source size, as the core draws its frame (before the turn): a 1-pixel red border on the outermost pixels, a green border 8 pixels in, a white centre cross, a grid every 16 pixels, a yellow circle that is round when the shape is right, the number of every 8th line at the left edge (right of the green border), and corner marks in four colours (white top left, cyan top right, magenta bottom left, orange bottom right). The first line number that shows tells how many lines are cut. `--frame FILE.ppm` uses a real frame of the source size in place of the pattern.

## Stage A: the wire

The tool builds the policy settings and the scanline settings with the functions of `neptron-video` that the app uses (`policy_settings`, `with_display_choices`, `scanline_settings`). It runs one core frame through `VideoEngine` on the fake display of `neptron-testkit`: the same decision, validator, scene builder and reference renderer as the app and its tests. `wire.png` is the frame buffer of the mode the display reads back, pixel for pixel (for example 720x240 on NTSC, 1280x224 on a 15 kHz preset on the bench PC).

The tool asks the engine for the offer of the settings too: the menus offer the settings, or decline them with a reason.

## Stage B: the glass

The glass is a simple model. It is stated here in full.

**A tube** (a CRT or a TV) shows a fixed window of the scan: a time span of each line after the end of the horizontal sync, and a time span of each field after the end of the vertical sync. A column of the mode lands on the glass where its time falls in the line window; a row lands where its time falls in the field window. A mode with other porches or another rate therefore moves, grows or shrinks on the glass, as on a real tube with fixed adjustments. Each monitor class has one named model of the window:

- `Tv`: the 4:3 picture of the line system of the standard: 704 samples of the 720 at 13.5 MHz, centred, and 240 lines (525-line system) or 288 lines (625-line system) per field, placed as in the progressive mode of the standard. A consumer TV hides an overscan of 5 % of the picture at each side (`TV_OVERSCAN_PERMILLE`). `--overscan PERCENT` changes it.
- `Fixed15khz`: a CRT range below 20 kHz. The tube is set up once so that a mode with the porches of its range fills the screen at 60 Hz (`TUBE_15KHZ_FIELD_MHZ`, clamped into the field rates of the range). The line window is the line period less the horizontal porches and sync of the range. The field window is the 60 Hz field less the vertical porches and sync of the range, at every field rate: a 50 Hz picture scans longer, so it is taller and loses lines at the top and the bottom.
- `PerMode`: a CRT range at 20 kHz or more (a PC or a multisync monitor). The monitor or the user sets the size of each mode: the field window is the field of the mode less the vertical blank of its range.

The range is the range that holds the mode (its line rate and field rate), or else the range nearest its line rate. No overscan is applied to a CRT unless `--overscan` asks for it.

Each line of the mode is a band of light as high as the beam spot. The spot is one line of the densest raster of the tube over the visible height: for a TV the lines of the interlaced picture (480 or 576); for a CRT the visible lines of its highest line rate, twice that for a range below 20 kHz (the spot of a 15 kHz tube is sized for the interlaced picture). Where two lines are further apart than the spot, a dark gap shows: 240p on a 15 kHz tube has gaps, 480p at 31 kHz has none, an interlaced mode weaves its two fields with no gap, a doubled 240p at 31 kHz shows the scanlines the renderer drew. A doublescan row is two bands. The band profile is a box. There is no bloom, no mask, no curvature, no convergence and no blur along the line. The glass picture is 4:3, 1440x1080 by default (`--size WxH`).

**A flat panel** shows its frame buffer pixel for pixel. The glass picture is the panel scaled down with area sampling to the largest size of the panel's aspect inside the size box (1440x810 for 16:9). `--exact` adds two 1:1 crops of 480x270: at the top-left corner of the picture (`exact-corner.png`) and at the centre (`exact-centre.png`).

**Orientation**: `--turn 0|90|180|270` turns the glass picture clockwise, as a player sees a monitor mounted on its side. It only turns the output. It is not TATE mode.

### What the model does not show

- Geometry faults of a real tube: pincushion, linearity, tilt, convergence, a picture off centre by the adjustments of one set.
- The overscan of one given TV: the 5 % is a typical value.
- Whether a monitor accepts a mode: the validator and the profile decide that, and the glass shows any mode the engine sets.
- Motion, flicker, the decay of the phosphor, interlace twitter and the frame pacing.
- Colour, brightness, gamma, the shadow mask and the composite artefacts.

## Measures

`measures.json` and the table row hold:

| Measure | Meaning |
| --- | --- |
| `mode` | The mode read back: active size, totals, pixel clock, line rate and field rate (millihertz), interlace, doublescan. |
| `rung`, `filter` | The winner's rung, and the filter of the scene. |
| `crop`, `dest` | The part of the frame shown (frame pixels) and where the picture goes (mode pixels). |
| `scale_x_milli`, `scale_y_milli`, `*_whole` | Mode pixels per source pixel on each axis of the upright picture, in thousandths, and whether it is a whole number. |
| `cropped_source_px` | Source lines (top, bottom) and columns (left, right) of the upright picture that are not in the frame buffer: the crop of the policy, and any part of `dest` outside the mode. |
| `borders_mode_px` | Mode pixels with no picture at each edge. Negative where `dest` reaches past the mode. |
| `fill_w_permille`, `fill_h_permille` | The part of the visible width and height of the glass that the picture covers. |
| `aspect_shown_milli`, `aspect_asked_milli`, `aspect_error_permille` | The width to height of the picture on the glass (4:3 glass for a tube, square pixels for a panel), the width to height the source asks for the shown part (its display aspect, the crop and the turn applied), and shown over asked less 1. |
| `glass_hidden_source_px` | Source lines and columns in the frame buffer that the glass does not show: outside the window of the scan or under the bezel. |
| `overscan_hidden_source_px` | On a TV, the part of the above that the overscan alone hides. |
| `speed_ppm`, `speed_alert` | The rate the game runs at (the field rate per game frame) less its own rate, in parts per million of its own rate, and whether the status line shows the speed alert (`Timing::changes_speed`). |
| `scanlines` | The scanlines of the scene: `none` or `black N%`. |
| `notices` | The parts of the notice of the policy, for example `cropped_lines=16` or `soft=FillMode`. |
| `offer` | `offered`, or `declined: REASON` as the menus decline the settings. |
| `no_picture` | Why nothing shows, when nothing does. |

## Survey

The default survey runs the displays `generic_15`, `arcade_15`, `pc_31_120`, `ntsc`, `pal`, `1920x1080@60` and `2560x1440@120`; the sources of `REFERENCE_SOURCES` plus `rtype`, `gunbird` and `pacman`; and the settings: every picture mode with the refresh rate at Auto (`exact`), and Auto with every other refresh rate. Each display gets a contact sheet: one row per source, one column per setting, each tile labelled with the source and settings, the mode and its size, and the flags.

A row gets a flag, with its name, when:

| Flag | Condition |
| --- | --- |
| `no_picture` | The engine has no mode: the picture mode and its fallbacks give no picture. |
| `cropped` | `cropped_source_px` is above 0 at an edge. |
| `small` | The picture fills less than 70 % of the visible width and less than 70 % of the visible height (`SMALL_FILL_MIN_PERMILLE`). |
| `aspect` | The aspect error is above 3 % (`ASPECT_ERROR_MAX_PERMILLE`). |
| `not_whole` | `native`, `2x`, `3x`, `4x` or `sharp` (the modes that promise whole steps) scale the lines by a fraction. |
| `fill_smaller_than_auto` | The area of Fill on the glass is more than 0.5 % below the area of Auto (`FILL_SMALLER_MARGIN_PERMILLE`). |
| `same_as_<mode>` | Two of `native`, `2x`, `3x`, `4x`, `fit` that the menus both offer give the same mode, destination and crop. |
| `off_centre` | The centre of `dest` is more than 1 pixel from the centre of the mode on an axis (`OFF_CENTRE_MAX_PX`). |

The rows with the most flags come first. A flag is a hint for a person, not a test failure.

## Goldens

`xtask/tests/golden/screen/` holds the glass of eight reference cases at 360x270, and a unit test of xtask asserts their measures as numbers. `cargo xtask screen golden` compares them; `--bless` writes them after a person has looked at each picture in `target/screen/golden/`.
