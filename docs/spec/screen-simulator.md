# Screen simulator

`cargo xtask screen` shows what a screen of a given profile shows for a game and its display settings. It runs the decision path of the app, draws the frame buffer of the mode, and then models the picture on the glass. It writes both pictures as PNG files with a set of measures. A survey runs a matrix of cases and marks the rows that look wrong.

The tool finds and reproduces bad picture modes: a crop, a small picture, a wrong shape, a fraction where a whole scale is promised. It is not a model of a real tube. It needs no ROM, no core and no hardware.

## Commands

```text
cargo xtask screen --display D --source S [--set KEY=VALUE]... [options]
cargo xtask screen survey [--display D]... [--source S]...
cargo xtask screen golden [--bless]
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
- `PerMode`: a CRT range at 20 kHz or more (a PC or a multisync monitor). The monitor or the user sets the size of each mode: the field window is the field of the mode less the vertical blank of its range.

The range is the range that holds the mode (its line rate and field rate), or else the range nearest its line rate. No overscan is applied to a CRT unless `--overscan` asks for it.

Each line of the mode is a band of light as high as the beam spot. The spot is one line of the densest raster of the tube over the visible height: for a TV the lines of the interlaced picture (480 or 576); for a CRT the visible lines of its highest line rate, twice that for a range below 20 kHz (the spot of a 15 kHz tube is sized for the interlaced picture). Where two lines are further apart than the spot, a dark gap shows: 240p on a 15 kHz tube has gaps, 480p at 31 kHz has none, an interlaced mode weaves its two fields with no gap, a doubled 240p at 31 kHz shows the scanlines the renderer drew. A doublescan row is two bands. The band profile is a box. There is no bloom, no mask, no curvature, no convergence and no blur along the line. The glass picture is 4:3, 1440x1080 by default (`--size WxH`).

**A flat panel** shows its frame buffer pixel for pixel. The glass picture is the panel scaled down with area sampling to the largest size of the panel's aspect inside the size box (1440x810 for 16:9). `--exact` adds two 1:1 crops of 480x270: at the top-left corner of the picture (`exact-corner.png`) and at the centre (`exact-centre.png`).

**Orientation (TATE mode)**: `--set display.orientation=0|90|180|270` is the screen orientation of the product: the display stands turned by that many degrees counter-clockwise, as in a vertical cabinet. `--set display.rotation=auto|0|90|180|270` is the game orientation: `auto` takes the turn that the core asks for, a value replaces it. The tool makes an `Orientation` from the two values, as `Config::orientation` of the app does, and the engine runs with it (Stage A). The glass picture is turned as the player sees it: by the screen orientation, counter-clockwise. At 90 a 4:3 tube gives a portrait glass picture.

`--turn 0|90|180|270` is not TATE mode. It turns the glass picture once more, clockwise, after the turn of the screen orientation. It changes no decision and no measure: use it only to look at a picture from another side.

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
| `aspect_shown_milli`, `aspect_asked_milli`, `aspect_error_permille` | The shape that the policy gives the picture in the mode: the width to height of `dest` when a reference area of the mode fills the screen, the width to height the source asks for the shown part (its display aspect, the crop and the turn applied), and shown over asked less 1. The reference area is the active area of a CRT mode on a 4:3 screen (a tube has no fixed pixel shape; the usual set-up of the size controls makes the whole frame of the game fill the screen, so lines that the policy cuts count as if they showed), the 4:3 picture of a TV (704 samples by 240 or 288 lines per field), and the panel itself with square pixels. A console picture on a TV (not an arcade board, a 4:3 frame, 224 to 240 lines per field, counting the whole frame for an exact rung and the filled lines for the fitted rung: the rule of the policy's `aspect_samples`) does not fill the 4:3 line by design: its shape is its width against the console width of the policy (644 samples on 525 lines, 650 on 625), and `console_picture` is true. The fitted rung of Fit scales that console width with the lines it fills over the lines of the frame. |
| `glass_aspect_error_permille` | The same error on the glass model, where the window of the class model sets the size. For information only: a monitor adjustment changes it, so no flag reads it. |
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
| `aspect` | look | The placement aspect error is above 3 % (`ASPECT_ERROR_MAX_PERMILLE`). |
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
- **DoDonPachi on `pc_31_120`.** Native gives the 1x picture, 240x320 in a 683x512 mode, 35 % of the width and 61 % of the height. Fit fills the 512 lines at 1.6, the shape within 1 %.

## Goldens

`crates/neptron-testkit/tests/golden/screen/` holds the glass of ten reference cases at 240x180: six in Auto and four in Fit (SNES on `ntsc`, DoDonPachi on `pc_31_120`, R-Type on `ntsc` and on `pal`). A unit test of `neptron-testkit` asserts their measures as numbers, and for the Fit cases that every line shows, at its shape within 1 %, with no dropped line. `cargo xtask screen golden` compares them; `--bless` writes them after a person has looked at each picture in `target/screen/golden/`.

The same folder holds the glass of 16 TATE cases (`TATE_CASES`), turned as the player sees them: DoDonPachi (`ddonpach`, vertical) and Power Instinct 2 (`pwrinst2`, a horizontal Cave board of the same size and rate) on `arcade_15`, `pc_31_120`, `ntsc` and `1920x1080@60`, with `display.orientation` at 0 and at 90, in Auto. The file name is `<source>-<display>-o<degrees>`, with `panel_1080` for the panel. A unit test asserts for each: the mode, `dest`, the player fill, the player shape within 1 % (`PLAYER_SHAPE_MAX_PERMILLE`), `player.upright`, and that a game at the orientation of the screen (a vertical game at 90, a horizontal game at 0) fills one side of the screen to at least 99 %. The same folder holds the glass of 14 cases of 480i and 480p systems (`INTERLACED_CASES`, `docs/spec/interlaced-systems.md`): the PlayStation at 240p and 480i on `generic_15` with the bench PC and with the Raspberry Pi 5 (`pi5_rp1_dpi`, which sends interlace), on `arcade_15`, `pc_31_120`, `ntsc`, `pal` and a 1080p panel, and a VGA frame on `pc_31_120`. A unit test asserts for each: the mode and its scan, the field rate, `dest`, the scale, the fill, the shape error, the scanlines, the notices, every line shown and none dropped. And it holds the glass of 16 cases of a width change in play (`WIDTH_CASES`): the SNES at 256 and 512 and the Mega Drive at 320 and 256 on `generic_15`, `arcade_15`, `ntsc` and `pal`, with the mode, `dest`, the scale, the fill and the shape asserted, and a whole factor on a CRT. A unit test holds that the widths of the PlayStation, the SNES and the Mega Drive share one mode and one `dest` on `generic_15`, `arcade_15` and `pc_31_120`.

A second test runs both boards on `generic_15`, `arcade_15`, `arcade_31`, `pc_31_120`, `ntsc`, `pal` and `1920x1080@60` at 0, 90, 180 and 270, and asserts the player shape within 1 % and an upright picture.
