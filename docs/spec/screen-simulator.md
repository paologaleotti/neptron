# Screen simulator

`cargo xtask screen` shows what a screen of a given profile shows for a game and its display settings. It runs the decision path of the app, draws the frame buffer of the mode, and then models the picture on the glass. It writes both pictures as PNG files with a set of measures. A survey runs a matrix of cases and marks the rows that look wrong.

The tool finds and reproduces bad picture modes: a crop, a small picture, a wrong shape, a fraction where a whole scale is promised. It is not a model of a real tube. It needs no ROM, no core and no hardware.

## Commands

```text
cargo xtask screen --display D --source S [--set KEY=VALUE]... [options]
cargo xtask screen survey [--display D]... [--source S]...
cargo xtask screen golden [--bless]
cargo xtask screen gate
```

The simulator is `neptron_testkit::screen`, run by the example `screen` of `neptron-testkit`. It writes PPM and JSON files; `cargo xtask screen` starts it with the same arguments and turns the PPM files into PNG files. One case writes `target/screen/<case>/`: `frame.png` (the source picture), `wire.png`, `glass.png` and `measures.json`. It prints the measures as one row of a table. The survey writes `target/screen/survey/`: one contact sheet per display (`<display>.png`), `survey.md`, `survey.json`, and the glass of each row to look at in `look/<display>/`. `golden` compares the reference cases with `crates/neptron-testkit/tests/golden/screen/`.

## A case

A case is a display, a source and the display settings.

**Display** (`--display`):

- A preset id of `data/monitors`, for example `generic_15`, `arcade_15`, `pc_31_120`. `preset:ID` forces the preset when the id is also a TV standard.
- A TV standard key of `neptron_types::TvStandard`, for example `ntsc` or `pal`: the TV profile of the composite output, as the app drives a TV.
- A flat panel `WIDTHxHEIGHT@RATE`, for example `1920x1080@60` or `2560x1440@120`. The panel lists one mode at its native size and rate, with CVT reduced blanking v2 timing (80 pixels of horizontal blank, at least 460 us of vertical blank) and a pixel clock in whole kHz.

The GPU caps come from an entry of `data/hardware/matrix.toml` (`--gpu ID`). The default is `intel_gen9` (the bench PC) for a preset and a panel, and `pi_vc4_composite` for a TV. A preset and a TV get the analog caps of the entry. A panel gets the widest digital limits of the entry (DisplayPort on the bench PC).

**Source** (`--source`):

- A name of `REFERENCE_SOURCES` of the policy: `gb`, `snes`, `psx`, `nes_pal`, `amiga_pal`, `psx_480i`, `neogeo`, `mk`, `ddonpach`, `dos_vga`.
- A source of a system with the widths of its system data (`SYSTEM_SOURCES`), as the app runs a game of that system: `snes_256` and `snes_512` (224 lines at 60.099 Hz) and `md_320` and `md_256` (224 lines at 59.922 Hz) with the widths of `data/systems/snes.toml` and `data/systems/megadrive.toml`; `ps1_320`, `ps1_256`, `ps1_368`, `ps1_512` and `ps1_640` (the PlayStation in 240p at 59.826 Hz) and `ps1_480i` (640x480 interlaced at 59.940 Hz) with the PlayStation widths, a fixture until its system file exists; `vga_480p`, a 640x480 progressive frame at 60 Hz with no widths. The policy takes the wide mode of the widths (`neptron_video::wide_strategy`). The reference names (`snes`, `psx`, `psx_480i`) are shapes with no widths.
- An arcade set of `docs/spec/arcade-table.md`, for example `rtype` (384x256 at 55.000 Hz), `gunbird` or `pacman`. The tool reads the headings of that file. A set that is not in the file is an error that lists the known sets. Every board has the 4:3 aspect of its frame as drawn, as FinalBurn Neo reports it.
- A source by hand: `WIDTHxHEIGHT@RATE` with `--aspect` (`4:3`, `1.333` or the pixel aspect `par:8:7`; default 4:3), `--rotate 0|90|180|270` (the turn the core asks for), `--interlaced` and `--arcade`.

The arcade boards and the reference boards `neogeo`, `mk` and `ddonpach` get the arcade rules of the policy (`PolicySettings::arcade`), as the app does for the systems `arcade` and `neogeo`.

**Settings** (`--set KEY=VALUE`, the key with or without `display.`): `display.style`, `display.refresh_policy`, `display.tv_width`, `display.vertical`, `display.panel_scale`, `display.scanlines`, `display.scanline_strength`, `display.orientation` and `display.rotation`. A key that is not given has the default of the settings schema. The values are the values of the settings files: the picture mode that the menu labels Fit is `display.style = fit`. The last two keys set TATE mode (see Orientation).

**Picture**: a test pattern at the source size, as the core draws its frame (before the turn): a 1-pixel red border on the outermost pixels, a green border 8 pixels in, a white centre cross, a grid every 16 pixels, a yellow circle that is round when the shape is right, the number of every 8th line at the left edge (right of the green border), and corner marks in four colours (white top left, cyan top right, magenta bottom left, orange bottom right). The first line number that shows tells how many lines are cut. `--frame FILE.ppm` uses a real frame of the source size in place of the pattern.

## Stage A: the wire

The tool builds the policy settings and the scanline settings with the functions of `neptron-video` that the app uses (`policy_settings`, `with_display_choices`, `scanline_settings`). It runs one core frame through `VideoEngine` on the fake display of `neptron-testkit`: the same decision, validator, scene builder and reference renderer as the app and its tests. `wire.png` is the frame buffer of the mode the display reads back, pixel for pixel (for example 720x240 on NTSC, 1280x224 on a 15 kHz preset on the bench PC).

The tool asks the engine for the offer of the settings too: the menus offer the settings, or decline them with a reason.

The engine gets the orientation of the settings with `VideoEngine::set_orientation` before the frame, as the app does. The engine then plans with the display source of the orientation and turns the picture by its game turn. The tool gives the engine the source as the core sends it.

## Stage B: the glass

The glass is a simple model. It is stated here in full.

**A tube** (a CRT or a TV) shows a fixed window of the scan: a time span of each line after the end of the horizontal sync, and a time span of each field after the end of the vertical sync. A column of the mode lands on the glass where its time falls in the line window; a row lands where its time falls in the field window. A mode with other porches or another rate therefore moves, grows or shrinks on the glass, as on a real tube with fixed adjustments. Each monitor class has one named model of the window:

- `Tv`: the 4:3 picture of the line system of the standard: 704 samples of the 720 at 13.5 MHz, centred, and 240 lines (525-line system) or 288 lines (625-line system) per field, placed as in the progressive mode of the standard. A consumer TV hides an overscan of 5 % of the picture at each side (`TV_OVERSCAN_PERMILLE`). `--overscan PERCENT` changes it.
- `Fixed15khz`: a CRT range below 20 kHz. The tube is set up once so that a mode with the porches of its range fills the screen at 60 Hz (`TUBE_15KHZ_FIELD_MHZ`, clamped into the field rates of the range). The line window is the line period less the horizontal porches and sync of the range. The field window is the 60 Hz field less the vertical porches and sync of the range, at every field rate: a 50 Hz picture scans longer, so it is taller and loses lines at the top and the bottom.
- `PerMode`: a CRT range at 20 kHz or more (a PC or a multisync monitor). The monitor or the user sets the size of each mode, so the window of a mode comes from that mode's own blank: the line window is the active part of its line and the field window the active part of its field. Every mode fills the screen, and a standard mode such as 640x480 at 60 Hz reads its own shape.

**Assumptions, not verified against a real monitor.** Two values of the model decide verdicts of the gate; each has a check in `docs/hardware-checklist.md` (section 4 and the TV checks):

- `TUBE_15KHZ_FIELD_MHZ`: a 15 kHz tube is set up once for 240 lines at 60 Hz with the porches of its range, as an arcade or consumer monitor is adjusted for the games it runs most. A 224-line game then fills the width and 224 of the 240 lines, and a board of 248 or 256 lines at 57.55 Hz scans taller than the window. A person who sets the size for each game would see another shape. The gate cases whose shape depends on it carry the reason "tube model, to measure on the bench".
- `TV_OVERSCAN_PERMILLE`: a consumer TV hides 5 % of the 4:3 picture at each side, a typical value. It changes the lines and columns hidden and the fill on a TV, not the shape.

The range is the range that holds the mode (its line rate and field rate), or else the range nearest its line rate. No overscan is applied to a CRT unless `--overscan` asks for it.

Each line of the mode is a band of light as high as the beam spot. The spot is one line of the densest raster of the tube over the visible height: for a TV the lines of the interlaced picture (480 or 576); for a CRT the visible lines of its highest line rate, twice that for a range below 20 kHz (the spot of a 15 kHz tube is sized for the interlaced picture). Where two lines are further apart than the spot, a dark gap shows: 240p on a 15 kHz tube has gaps, 480p at 31 kHz has none, an interlaced mode weaves its two fields with no gap, a doubled 240p at 31 kHz shows the scanlines the renderer drew. A doublescan row is two bands. The band profile is a box. There is no bloom, no mask, no curvature, no convergence and no blur along the line. The glass picture is 4:3, 1440x1080 by default (`--size WxH`).

**A flat panel** shows its frame buffer pixel for pixel. The glass picture is the panel scaled down with area sampling to the largest size of the panel's aspect inside the size box (1440x810 for 16:9). `--exact` adds two 1:1 crops of 480x270: at the top-left corner of the picture (`exact-corner.png`) and at the centre (`exact-centre.png`).

**Orientation (TATE mode)**: `--set display.orientation=0|90|180|270` is the screen orientation of the product: the display stands turned by that many degrees counter-clockwise, as in a vertical cabinet. `--set display.rotation=auto|0|90|180|270` is the game orientation: `auto` takes the turn that the core asks for, a value replaces it. The tool makes an `Orientation` from the two values, as `Config::orientation` of the app does, and the engine runs with it (Stage A). The glass picture is turned as the player sees it: by the screen orientation, counter-clockwise. At 90 a 4:3 tube gives a portrait glass picture.

`--turn 0|90|180|270` is not TATE mode. It turns the glass picture once more, clockwise, after the turn of the screen orientation. It changes no decision and no measure: use it only to look at a picture from another side.

### What the model does not show

- Geometry faults of a real tube: pincushion, linearity, tilt, convergence, a picture off centre by the adjustments of one set.
- The overscan of one given TV: the 5 % is a typical value.
- Whether a monitor accepts a mode: the validator and the profile decide that, and the glass shows any mode the engine sets.
- Motion, the decay of the phosphor and the frame pacing. The two fields of an interlaced mode and the flicker of thin lines are measured (see Interlace), not animated.
- Tube effects: bloom, convergence, phosphor glow (owner, 2026-11-27).
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
| `policy_aspect_milli`, `aspect_asked_milli`, `policy_aspect_error_permille` | What the policy intends, not what the glass shows: the shape that the policy gives the picture in the mode: the width to height of `dest` when a reference area of the mode fills the screen, the width to height the source asks for the shown part (its display aspect, the crop and the turn applied), and intended over asked less 1. The reference area is the active area of a CRT mode on a 4:3 screen (lines that the policy cuts count as if they showed), the 4:3 picture of a TV (704 samples by 240 or 288 lines per field), and the panel itself with square pixels. A console picture on a TV (not an arcade board, a 4:3 frame, 224 to 240 lines per field: the rule of the policy's `aspect_samples`) is measured against the console width of the policy (644 samples on 525 lines, 650 on 625), and `console_picture` is true. This measure reads the rule of the policy, so it cannot prove the shape; `glass_shape` does. |
| `model_aspect_error_permille` | The error of the shape from the geometry of the glass model: `dest` mapped through the window of the scan. The gate checks that it agrees with `glass_shape` (see The gate). |
| `glass_shape` | The shape of the test card on the glass, measured from its light, against two references. See The shape on the glass. |
| `glass_centre_permille` | The centre of the lit picture on the glass, across and down, in per mille of the glass (measured at 1440x1080). |
| `border_on_wire`, `border_on_glass` | The sides of the outer red border of the card that show in the frame buffer and on the glass (measured at 1440x1080). |
| `fields` | The two fields of an interlaced mode. See Interlace. |
| `mode_speed_ppm` | The speed from the mode read back: the field rate over the whole number of fields nearest one game frame, less the rate of the game, in parts per million of it. |
| `uneven_lines` | The scale down is not a whole number: source lines do not map to whole mode lines, so some lines show taller than others. |
| `dropped_lines` | Source lines of the crop that no mode line samples, by the tap rule of the reference renderer (the line under the centre of each mode line, and with sharp bilinear below a scale of 1 the neighbour it blends). 0 with sharp bilinear from a scale of 0.5 up. |
| `notice_cropped_lines` | The source lines that the notice of the policy says it crops. |
| `placed_area_permille` | The part of the reference area of the mode that the picture covers: its size as the policy places it, with no glass model. |
| `glass_hidden_source_px` | Source lines and columns in the frame buffer that the glass does not show: outside the window of the scan or under the bezel. |
| `overscan_hidden_source_px` | On a TV, the part of the above that the overscan alone hides. |
| `speed_ppm`, `speed_alert` | The rate the game runs at (the field rate per game frame) less its own rate, in parts per million of its own rate, and whether the status line shows the speed alert (`Timing::changes_speed`). |
| `scanlines` | The scanlines of the scene: `none` or `black N%`. |
| `notices` | The parts of the notice of the policy, for example `cropped_lines=16` or `soft=FillMode`. |
| `offer` | `offered`, or `declined: REASON` as the menus decline the settings. |
| `no_picture` | Why nothing shows, when nothing does. |
| `player` | The game as the player sees it, in front of the screen at its orientation. These values read the source as the core sends it and the turn that the core asks for, not the display source of the engine, so a wrong turn or a wrong aspect convention shows here. Every other measure is in the scan-out frame of the mode and reads the display source. |
| `player.screen_orientation` | The screen orientation, in degrees counter-clockwise. |
| `player.aspect_milli`, `player.aspect_asked_milli`, `player.aspect_error_permille` | The shape of the picture for the player: `aspect_shown_milli`, inverted for a screen at 90 or 270. The shape of the upright game: its display aspect, the crop and the turn of the core applied (3:4 for DoDonPachi). Shown over asked, less 1. |
| `player.fill_w_permille`, `player.fill_h_permille` | `fill_w_permille` and `fill_h_permille` across and down for the player: swapped for a screen at 90 or 270. |
| `player.upright` | The test pattern stands upright for the player: the corner mark at the top-left corner of the picture, as the player sees it, is the mark of the top-left corner of the upright game. `false` for a game override that turns the picture the wrong way. `null` when no corner mark shows or the frame comes from `--frame`. |

## The shape on the glass

The glass is the truth for the shape (owner, 2026-11-27). `measure_card` renders the glass of the case at 1440x1080 in the scan-out frame of the mode (not turned for the player) and reads the test card from the light:

- **The circle.** A pixel is the circle when its red and green less blue are at least 24 and green over red lies in 0.75 to 0.95: yellow passes, the orange corner mark and blends of orange or red with the green border do not. In the rows within 15 % of the radius around the centre, the width is the distance from the weighted centre of the first run of circle light to that of the last run; the largest over those rows counts. The height is the same down the columns. The centre of the line is read, so the thickness of the line does not count.
- **The border box.** A side of the red outer border is a group of rows (columns) whose red covers at least 30 % of the red span across them. The box counts only when all four sides show and none is on the edge of the picture.
- **The centre.** `glass_centre_permille` is the centre of the box of lit glass pixels (a channel above 16), across and down, in per mille of the glass.

A glass pixel is `4 / width` by `3 / height` of a tube and the same for the aspect of a panel. The same reading on the frame of the core, in frame pixels, gives the card as drawn, so the rounding of the circle in the frame cancels out.

`glass_shape` holds:

| Field | Meaning |
| --- | --- |
| `shown_milli` | Width over height of the circle on the glass. |
| `core_milli`, `core_error_permille` | Reference (a): the circle of the frame with exactly the display aspect of the core: width times the pixel aspect `aspect x height / width`, over height (inverted when the scene turns the frame by 90 or 270). Shown over it, less 1. |
| `console` | Reference (b), for a source whose console is in the table: `name`, `dot_clock_hz`, `milli` and `error_permille`. |
| `verdict_error_permille` | The error that the gate judges: against (b) when the table has the console, else against (a). |
| `border_error_permille` | The border box against reference (a), when the box shows. |

### The two references

A libretro core says "this frame is 4:3". A real console does not fill the 4:3 line of the TV standard: its dot clock decides the width of its picture. The simulator states both and does not choose:

- **(a) The core.** The frame has exactly the display aspect that the core gives.
- **(b) The console.** Each pixel is one dot of the real console. On a screen with a fixed window, a pixel is `(1 / dot clock) / visible line time x 4` wide and `console line period / visible field time x 3` high (half that for a line of an interlaced frame). A TV and a 15 kHz tube use their own window; a PC monitor and a panel have no fixed window, so they use the window of an NTSC TV, the TV the console was made for. On a 625-line TV the clock of the PAL console counts. The line period is that of the standard (15.734 kHz, 15.625 kHz), except for the Neo Geo (15.625 kHz, 24 MHz / 1536).

**The reference that judges** (owner, 2026-11-27): the truth for the shape is the width of the real console where the table has its clock. The gate judges a case whose source has a console in the table against (b), and every other case (an arcade board, a source by hand) against (a). The table prints both.

**The card of the reference.** The card is drawn for the reference that judges the case, so the eye and the verdict agree: a case judged against (b) draws its circle with the pixel shape of the console on that screen (`test_pattern_for`, the shape of a pixel of the dot clock, as a real game of that system draws), and a case judged against (a) draws it with the display aspect of the core. **Round** then means: under (b), the circle is round on the glass when each pixel has the width that the dot clock of the console gives it; under (a), when the frame has exactly the aspect of the core. The measure does not become circular: it reads the width of the circle on the glass against its height, and compares that with the circle of the same card read in the frame, times the pixel shape of the reference. A picture squeezed by 5 % fails under either card. Under each picture the sheets print `ref: console` or `ref: core 4:3` and the verdict, and each sheet starts with a legend of the border, the circle and the corner mark. The golden pictures of `screen golden` keep the card of the core.

The table is in the simulator (`screen/consoles.rs`), not in `data/systems`: no part of the product reads it yet. The owner decided that the console is the truth; whether and how the policy follows it is open in OP-122.

| System | Width | NTSC dot clock | PAL dot clock | Source |
| --- | --- | --- | --- | --- |
| NES | 256 | 5.369318 MHz (21.477272 / 4) | 5.320342 MHz (26.601712 / 5) | nesdev.org, Cycle reference chart |
| SNES | 256, 512 | 5.369318, 10.738636 MHz | 5.320343, 10.640685 MHz (21.28137 / 4, / 2) | snes.nesdev.org Timing; pineight.com Dot clock rates |
| Mega Drive | 320, 256 | 6.711647 MHz (53.693175 / 8), 5.369318 MHz (/ 10) | 6.650428, 5.320342 MHz (53.203424 / 8, / 10) | pineight.com Dot clock rates; the PAL master clock is from forum measures, not verified |
| PlayStation | 256, 320, 368, 512, 640 | 53.693175 MHz / 10, 8, 7, 5, 4 | 53.203425 MHz / the same | psx-spx (nocash), GPU |
| Neo Geo | 320 | 6 MHz (24 / 4) | 6 MHz, not verified | wiki.neogeodev.org, Display timing |

On an NTSC TV these clocks give the known pixel aspects: 8:7 at 5.37 MHz, 32:35 at 6.71 MHz, 16:35 at 13.42 MHz (a unit test holds this).

## Interlace

An interlaced mode is drawn and measured as two fields. Row `r` of the frame buffer is scanned in field `r % 2 + 1`.

- **Pictures.** The gate writes, for each interlaced case, `fields/<case>-field1.png` and `-field2.png` (the bands of one field lit, the other dark), `-eye.png` (both fields woven, as the eye sees them) and `-eye-filtered.png` (the same with the flicker filter model).
- **Line pairing.** `field_offset_milli`: the distance from the centre of the band of row 0 to that of row 1, over the distance from row 0 to row 2 (one field line), in thousandths. 500 when field 2 lies half a line below field 1. `own_lines`: the bands are in order and do not overlap, so each line of the frame buffer has its own glass line.
- **Source lines of each field, read from the light.** The simulator draws a line card (`line_card`: each pixel has the colour of the number of the line of the upright picture it lands on) with the scene of the case through the reference renderer, and reads the colour of each line of the game area. `source_lines.own` holds when each line shows one source line, no source line shows twice, field 1 carries the even source lines and field 2 the odd ones. `source_lines.shared` counts the source lines that both fields show: a 240-line frame doubled into a 480i mode shows all 240 in both, and the gate fails it.
- **Flicker.** `flicker_permille`: over the pixels of the game area (`dest`), the mean of the difference of light (BT.601 luma) between the line of field 1 and the line of field 2 of each line pair, in per mille of full white. A line pair is one place on the glass that the two fields light in turn, so a difference there blinks at the frame rate: a white line one pixel high on black gives 500.
- **The flicker filter model.** `filtered_flicker_permille`: the same after the blend of `docs/spec/interlaced-systems.md` section 5, applied in the simulator only: each line of the game area becomes `(in(y - 1) + 2 x in(y) + in(y + 1) + 2) / 4` per channel, with the edge lines of the area repeated. The product has no such filter yet (task T102).

## The gate

`cargo xtask screen gate` runs every stored case: the golden cases (`GOLDEN_CASES`, `TATE_CASES`, `INTERLACED_CASES`, `WIDTH_CASES`) and `GATE_CASES` (a SNES, a PlayStation at 240p and 480i, and R-Type on the 25 kHz preset `arcade_25`). It measures the card on the glass of each and prints one table: case, display, mode, the glass shape against reference (a) and (b), fill, the lines cut (top, bottom, left, right), the dropped lines, the flicker with and without the filter model, and the verdict. It exits with a failure when a case fails. It writes under `target/screen/gate/`: `gate.md` (the table), one contact sheet per screen type (`crt_15khz.png`, `crt_25khz.png`, `crt_31khz.png`, `ntsc.png`, `pal.png`, `panel.png`, by the line rate of the mode for a CRT: below 20 kHz, below 28 kHz, from 28 kHz), each picture 360x270 with the verdict and the case name, the mode, both shapes, the lines cut and dropped and the flicker under it; the field pictures; the TV option sheets; and the sequence strips. `screen golden` stays as it is: it compares the glass pictures with the goldens, and the gate does not.

A case **fails** when:

- it shows no picture, or no test card is read on the glass;
- the glass shape against the reference that judges it is off by more than 1 % (`SHAPE_MAX_PERMILLE`), unless the case has an accepted value in `ACCEPTED` (`screen/gate.rs`) with its reason, and the shape is within 0.3 % of that value (`ACCEPTED_MATCH_PERMILLE`). A shape that moves from its accepted value fails, also when it gets better: the value is then stale;
- it differs from its stored values in `PINNED` (`screen/gate.rs`): the source lines and columns cut at each side (exactly), the fill across and down (within 0.3 % of the glass, `FILL_MATCH_PERMILLE`), the centre of the lit picture on the glass (within 0.3 % of the glass, `CENTRE_MATCH_PERMILLE`), and the speed of the game on the mode read back (within 0.1 %, `SPEED_MATCH_PPM`) with the speed alert of the status line. A case with no stored values fails, and its message gives the measured values as the text of a `PINNED` entry, to store after a person has looked at the picture;
- a line is dropped, or the picture is not upright;
- the fields of an interlaced mode do not pair: `own_lines` is false, `field_offset_milli` is not 500 within 10, or the fields do not carry their own source lines;
- two measures of the simulator disagree: a side of the border shows in the frame buffer while the crop says it is cut, or the reverse; a side shows on the glass while the glass model hides 2 lines or more of it, or does not show while it hides none; the speed of the mode and the speed of the timing differ by more than 500 ppm; the shape from the light and `model_aspect_error_permille` differ by more than 0.3 %, plus one mode line over the height of the circle where the lines scale by a fraction.

An accepted value names what the product or the model gives today and does not say that it is right. The reasons:

| Reason | Open point | Cases |
| --- | --- | --- |
| Whole samples for each game pixel on the fixed 13.5 MHz clock of a composite output | none | 5 PAL cases at -1.5 % against the console |
| Tube model, to measure on the bench (`TUBE_15KHZ_FIELD_MHZ`) | none | 17 cases on `generic_15` and `arcade_15` |
| On a CRT of 25 kHz or more and on a panel the policy gives the 4:3 of the core, not the width of the real console | OP-122 | 9 cases, +2.1 % to +9.5 % |
| NTSC TV, Auto: a 384-pixel board is clamped to 704 samples and 16 of its 256 lines are cut | OP-122 | `rtype-ntsc-auto`, -6.3 % |

### Known limit

A 480i frame that waits in a mode of fewer than 240 lines is pressed in below a scale of 0.5: in a 224-line mode, 480 lines at 0.467, and the two taps of sharp bilinear skip 64 of its 480 lines while it waits (the sequence `snes_224p_then_480i`, frames 31 and 32). Below a scale of 0.5 the two-tap filter skips lines; the fix needs a contract change (task N03). The sequence test records the 64.

### Proof of the measures

A measure is proven when the gate verdict fails on a placement made wrong on purpose. The tests of `screen/gate.rs` give the stored case `ps1_320-ntsc` (judged against the console, on the card of the console) a wrong placement through `finish`, with a frame buffer that the reference renderer draws for the wrong scene, and assert a FAIL of `verdict`:

| Measure | Wrong placement | Proven |
| --- | --- | --- |
| Glass shape | the picture squeezed by 5 % across | yes |
| Fill | the picture shrunk by 5 % on both axes | yes |
| Lines cut | a new crop of 8 lines at the top and the bottom, which the border in the frame buffer agrees with | yes |
| Lines cut, centre | the picture moved 8 lines down (on NTSC it leaves the mode; on PAL, `ps1_320-pal`, the centre moves) | yes |
| Border on the wire | the crop drawn while the scene says nothing is cut | yes |
| Upright | the picture turned by 180 degrees | yes |
| Dropped lines | 240 lines in 224 with nearest | yes |
| Speed | a mode read back at 5/6 of its clock | yes |
| Source lines of the fields | a 240-line frame doubled into 480i (`ps1_480i-ntsc`) | yes |
| Field offset | field 2 on top of field 1 | unit test of `pairing` only, not a gate verdict |
| Hidden by the glass | | not proven alone: it moves with the centre and the fill |
| Scanlines | | not proven: the measure reads the scene, not the light |

## The TV options

For the owner's decision in OP-122, the gate writes `tv_options_ntsc.png` and `tv_options_pal.png`: for a 320x240 game at 60 Hz (`ps1_320`), a 224-line game (`snes_256`), R-Type (384x256 at 55 Hz) and a 640x480i game (`ps1_480i`), one row each, with the candidates side by side: **today** (the product); **full 4:3 width** (the lines of today at 704 samples, centred); and on PAL **60 Hz, 525 lines** (the NTSC mode of today on a PAL TV that takes 60 Hz: the geometry of the 525-line system) and the same at full 4:3 width. A candidate is drawn with a placement given by hand through the reference renderer (`screen/redraw.rs`); the product has no code path for it. Each cell draws the card of its reference and names the option, the mode and `dest`, the shape against the reference that judges it (`ref: console` or `ref: core 4:3`, and whether it is within 1 %) with the other reference in brackets, the lines cut and hidden by the overscan, and the speed change.

## The sequence

The cards of a sequence are drawn for the aspect of the core; a sequence has no shape verdict, and its strip says so. `SEQUENCES` drive the engine of the app frame by frame on the fake display, with its debounce of 3 frames and its rule of one modeset in a second, and the gate writes a strip of pictures for each (`sequence_<name>.png`): the first frame, the first frame of each new source, the frame before each modeset and the modeset, and the last frame, each labelled with the frame number, the source, the mode and what the engine did (`MODESET`, `held`, `rescale`) and the lines it drops.

- `playstation_240p_480i` on `generic_15` with `pi5_rp1_dpi`: 240p for 30 frames, 480i for 40, 240p for 40, 480i for 70, 240p for 10. Modesets at frames 1, 33, 93 and 153. The 480i frames 111 to 152 are held in the 240p mode and show all 480 lines pressed in.
- `snes_width_wide_mode` on `generic_15`: 256, 512, 256 wide. One modeset at frame 1; the changes are scale-only.
- `snes_224p_then_480i` on `generic_15` with `pi5_rp1_dpi`: a SNES game, then a 480i frame. The two frames that wait for the debounce show in the 224-line mode with 64 lines dropped (see Known limit).
- `megadrive_width_no_wide_mode` on `pc_31_120`, the Mega Drive with no widths: 320 for 30 frames, 256 for 30, 320 for 60. Each width has its own mode (640x448p and 768x448p), so each change is a modeset: at frame 33 after the debounce (the first change after the launch is not held), and at frame 93 after the hold of one second.
- In the hold of the first sequence, the 240p frames held in the 480i mode (frames 71 to 92) show at 1x, 640x240 in the middle of the 1280x480 mode: the rule of a frame of another size keeps the step of the plan of the 480i frame, which is 1.

## Survey

The default survey runs the displays `generic_15`, `arcade_15`, `pc_31_120`, `ntsc`, `pal`, `1920x1080@60` and `2560x1440@120`; the sources of `REFERENCE_SOURCES` plus `rtype`, `gunbird` and `pacman`; and the settings: every picture mode with the refresh rate at Auto (`exact`), and Auto with every other refresh rate. Each display gets a contact sheet: one row per source, one column per setting, each tile labelled with the source and settings, the mode and its size, and the flags.

A row gets a flag, with its name and a severity. `look` is a likely fault: a person should look at the picture. `note` is a choice the policy makes on purpose and states. The rows to look at come first, and their glass is also in `look/<display>/<source>-<style>-<refresh>.png`. On the contact sheet a tile to look at has an orange frame and orange flags; notes are blue.

| Flag | Severity | Condition |
| --- | --- | --- |
| `no_picture` | look | The engine has no mode: the picture mode and its fallbacks give no picture. |
| `cropped` | look, or note when the notice names it | `cropped_source_px` is above 0 at an edge. It is a note when the notice of the policy names the same lines (`notice_cropped_lines`, top and bottom only). |
| `uneven_lines` | look, or note for Fit | `uneven_lines` on a progressive mode of a CRT or a TV. A panel scales with sharp-bilinear by design, and an interlaced mode weaves, so neither is flagged. It is a note for Fit with sharp bilinear and no dropped line: Fit is the one mode that may scale lines by a fraction (`docs/spec/video-policy-cases.md`, 8.12). |
| `dropped_lines` | look | `dropped_lines` is above 0. |
| `not_whole` | look, or note when the menus decline the mode | `native`, `2x`, `3x`, `4x` or `sharp` (the modes that promise whole steps) scale the lines by a fraction. When the menus decline the mode, they say why and grey it, so it is a note. |
| `aspect` | look | The intended shape (`policy_aspect_error_permille`) is off by more than 3 % (`ASPECT_ERROR_MAX_PERMILLE`). |
| `fill_not_larger` | look | Fit gives a picture no larger than Native, where another picture mode at the game's own rate (refresh rate Auto) gives a picture more than 0.5 % larger (`LARGER_MARGIN_PERMILLE`). The size is `placed_area_permille`: the glass window of a PC CRT changes with the mode, so the glass fill does not compare two modes. |
| `off_centre` | look | The centre of `dest` is more than 1 pixel from the centre of the mode on an axis (`OFF_CENTRE_MAX_PX`). |
| `small` | look, or note for Native or when the notice names it | The picture fills less than 70 % of the visible width and less than 70 % of the visible height (`SMALL_FILL_MIN_PERMILLE`). |
| `same_as_<mode>` | note | Two of `native`, `2x`, `3x`, `4x`, `fit` that the menus both offer give the same mode, destination and crop. |
| `speed_alert` | note | The game runs at another speed and the status line says so. |

`survey.md` lists the pictures to look at first: the rows of one display and source with the same mode, destination, crop and flags are one picture, with every setting that gives it. Then it lists every row. A flag is a hint for a person, not a test failure.

## Findings of the first survey

The first survey found these results of the policy. The Fit rule (`docs/spec/video-policy-cases.md`, 8.12) changes the picture mode Fit for each of them; Auto does not change.

- **Fill on a TV never filled.** On `ntsc` and `pal`, the mode then named Fill gave the same picture as Native for every source of the survey. Fit now fills the progressive mode: the SNES on `ntsc` is 690x240 (was 640x224, as Native).
- **The 704-sample clamp makes a 256-line game too narrow on a TV.** Auto keeps it: R-Type on `ntsc` is 704x240 with 8 lines cut at each edge, 6.2 % narrow. Fit shows all 256 lines in 240 at 704 samples, the shape within 1 %. On `pal` Fit fills 704x288 (Auto: 626x256 in 288 lines).
- **A fractional squeeze of the lines.** The fitted TV rung and the fallback of `build_scene` scaled lines by a fraction in every picture mode. Fit scales by a fraction with sharp bilinear and drops no line: a 384x272 board on `ntsc` shows 272 lines in 240 at 0.882, `dropped_lines` 0. `build_scene` keeps whole steps for a frame of another size where the plan is whole steps.
- **DoDonPachi on `pc_31_120`.** Native gives the 1x picture, 240x320 in a 683x512 mode, 35 % of the width and 63 % of the height. Fit fills the 512 lines at 1.6, the shape within 1 %.

## Goldens

`crates/neptron-testkit/tests/golden/screen/` holds the glass of ten reference cases at 240x180: six in Auto and four in Fit (SNES on `ntsc`, DoDonPachi on `pc_31_120`, R-Type on `ntsc` and on `pal`). A unit test of `neptron-testkit` asserts their measures as numbers, and for the Fit cases that every line shows, at its shape within 1 %, with no dropped line. `cargo xtask screen golden` compares them; `--bless` writes them after a person has looked at each picture in `target/screen/golden/`.

The same folder holds the glass of 16 TATE cases (`TATE_CASES`), turned as the player sees them: DoDonPachi (`ddonpach`, vertical) and Power Instinct 2 (`pwrinst2`, a horizontal Cave board of the same size and rate) on `arcade_15`, `pc_31_120`, `ntsc` and `1920x1080@60`, with `display.orientation` at 0 and at 90, in Auto. The file name is `<source>-<display>-o<degrees>`, with `panel_1080` for the panel. A unit test asserts for each: the mode, `dest`, the player fill, the player shape within 1 % (`PLAYER_SHAPE_MAX_PERMILLE`), `player.upright`, and that a game at the orientation of the screen (a vertical game at 90, a horizontal game at 0) fills one side of the screen to at least 99 %. The same folder holds the glass of 14 cases of 480i and 480p systems (`INTERLACED_CASES`, `docs/spec/interlaced-systems.md`): the PlayStation at 240p and 480i on `generic_15` with the bench PC and with the Raspberry Pi 5 (`pi5_rp1_dpi`, which sends interlace), on `arcade_15`, `pc_31_120`, `ntsc`, `pal` and a 1080p panel, and a VGA frame on `pc_31_120`. A unit test asserts for each: the mode and its scan, the field rate, `dest`, the scale, the fill, the shape error, the scanlines, the notices, every line shown and none dropped. And it holds the glass of 16 cases of a width change in play (`WIDTH_CASES`): the SNES at 256 and 512 and the Mega Drive at 320 and 256 on `generic_15`, `arcade_15`, `ntsc` and `pal`, with the mode, `dest`, the scale, the fill and the shape asserted, and a whole factor on a CRT. A unit test holds that the widths of the PlayStation, the SNES and the Mega Drive share one mode and one `dest` on `generic_15`, `arcade_15` and `pc_31_120`.

A second test runs both boards on `generic_15`, `arcade_15`, `arcade_31`, `pc_31_120`, `ntsc`, `pal` and `1920x1080@60` at 0, 90, 180 and 270, and asserts the player shape within 1 % and an upright picture.
