# TATE: a screen turned on its side

This spec defines how NEPTRON drives a display that stands turned: the screen orientation, the game orientation, the turn of every picture, the mode policy, the UI, the input and the settings. It settles OP-107. ADR 0020 records the change to the `Scene` contract for the turn of the canvas. ADR 0021 records the portrait canvas, and `docs/spec/ui-layout.md` designs the layout that follows the canvas size. Task N01 builds it in the work packages of section 12.

The owner accepted this design on 2026-11-24 ("ok yes to all, start the work packages"). A mark **Decision: Owner, 2026-11-24** names a point that was a question of section 13. The turned menu on a 15 kHz CRT or a TV (T3) and the quick menu over a game there (T4) were open; the owner decided them on 2026-11-24 with a portrait layout of the menu (section 6.3). On 2026-11-25 the owner changed the option labels of the screen orientation and removed its keep prompt (section 1). A mark **Decision: Owner, 2026-11-25** names these points.

## 1. The owner's decisions

The owner decided on 2026-11-19 (OP-107) and on 2026-11-24. They bind this spec:

- "One system setting gives the screen orientation: 0 (YOKO), 90, 180 or 270 degrees. The UI follows it."
- "In Auto a vertical game runs at its own lines, full screen, on a turned screen; a horizontal game shows upright and small, with a notice." And: "for now dont rotate horizontal games". A horizontal game is never turned to fill a turned screen.
- "Each game has an orientation override (Auto, 0, 90, 180, 270) in its display settings."
- On a screen at 0 (YOKO), a vertical game shows upright and small, as today.
- "a screen can be rotated how the user wants." All four orientations work the same way, with no preferred side.
- "ofc second layout!! let the ui be somewhat responsive. ofc. that is hard requirement. no sense of squeezing anything. tate mode ui needs to be specific and well done." A screen at 90 or 270 gets a portrait menu; a screen at 180 keeps the landscape menu, turned; a flat panel mounted in portrait gets the portrait menu at a whole scale; the quick menu over a horizontal game on a turned screen is the portrait menu at full size, with the small upright game behind it. `docs/spec/ui-layout.md`, section 1, has the whole decision.

The owner decided on 2026-11-25, in the settings package:

- The options of Screen orientation carry the word and the degrees: "Yoko (0)", "Tate left (90)", "Yoko flipped (180)", "Tate right (270)". The help lines stay (section 9.1).
- A change of the screen orientation has no keep prompt. It applies at once and is stored. Reason: the menu stays readable and usable in every orientation, so a wrong pick cannot lock the user out (section 9.1).
- Game orientation Auto shows every game upright to the viewer on the current screen: a vertical game fills a turned screen, a horizontal game shows across it, small (section 4).
- The menu takes the portrait layout at once after the change, with no restart (`docs/spec/ui-layout.md`, section 6.6).

## 2. Facts that drive the design

- A vertical arcade board drives a monitor that is mounted on its side. Its video signal is a normal horizontal raster: DoDonPachi sends 320x240 at 57.55 Hz at 15 kHz, as a horizontal Cave board does. The picture is upright only because the tube is turned.
- FinalBurn Neo gives the frame as the board draws it, on its side, and asks for the turn with `SET_ROTATION` (`docs/spec/arcade.md`, sections 8.2 and 11). DoDonPachi: a 320x240 frame and `R90`. A flipped board asks for `R180` (horizontal) or `R270` (vertical). NEPTRON pins `fbneo-vertical-mode = "disabled"`, so the core never turns a frame itself. The core reports the aspect of the upright picture (3:4 for DoDonPachi). The session inverts it for `R90` and `R270`, so `SourceGeometry::aspect_milli` is always the aspect of the frame as drawn (ADR 0015, point 8). No trace of a vertical set exists yet; this is the reading of the core source that the arcade spec records. The test core covers `R90`, `R180` and `R270`.
- `Scene::rotation` (ADR 0015) turns the cropped game picture before it is scaled into `dest`. It is the turn relative to the scan-out of the display. Both renderers draw all four values. The UI canvas is never turned.
- The policy (`take_source` in `crates/neptron-modeline/src/policy/source.rs`) swaps the axes of a source turned by 90 or 270 degrees, so the long side becomes lines. Every rule of the policy, the picture modes, the refresh rate, the Fit fallback, the speed alert, the notices and the scanline rule then work on that raster. `convert.rs` gives Switchres its "rotated" flag from the same field.
- The engine (`crates/neptron-video/src/scene.rs`) takes the turn of the scene from the same `SourceGeometry` that the policy planned with (ADR 0015, point 9).
- The menu is the 320x240 canvas. The app plans its mode with the same policy, as the source `MENU_SOURCE` (320x240, 60 Hz, 4:3, `R0`, `apps/neptron/src/shell.rs`). The canvas goes into `Scene::ui_dest`: the full active area on a CRT, whole factors on a TV, one whole scale centred on a flat panel (ADR 0005). The renderers scale it with nearest.
- The font is Spleen 6x12. Every screen of `neptron-ui` is laid out for 320x240 (about 9,300 lines of screens and 78 screen goldens).
- The screensaver, the dialogs, the notices, the keep prompts and the guard's prompts all draw into the canvas. Nothing of the UI draws outside it.
- The kernel shows nothing on screen, and the Pi firmware shows no splash. The first picture of a boot is NEPTRON's canvas (the boot prompt or the menu). The settings are read before the first frame.

## 3. Terms

**Scan-out.** The raster as the display scans it: `x` along a line, `y` from line to line. A mode, `dest`, `ui_dest` and the validator are in scan-out terms.

**Screen orientation** (`S`). How the physical display stands, as the player faces it: 0, 90, 180 or 270 degrees, counter-clockwise. `S = 90` means the display is turned a quarter turn to the left: its top edge is now on the left, and each scan line runs from bottom to top. This is the direction of `Rotation` in libretro and in `Scene`, so one type holds both.

```
Screen at 0 ("Normal")             Screen at 90 ("Turned left")

 +------------------------+         +--------------+
 | line 0   -------->     |         | ^ ^          |
 | line 1   -------->     |         | | |          |
 | ...                    |         | | |   ...    |
 | line 239 -------->     |         | | |          |
 +------------------------+         | | |          |
                                    | | |          |
                                    +--------------+
                                    line 0 is the left column;
                                    each line runs bottom to top
```

A screen at 270 ("Turned right") has line 0 at the right, and each line runs from top to bottom. A screen at 180 ("Upside down") has line 0 at the bottom, and each line runs from right to left.

**Game orientation** (`G`). The turn, counter-clockwise, that the frame as drawn needs to be upright for the player on a screen at 0. It is the value of `SET_ROTATION`: `R0` for a horizontal board, `R90` for DoDonPachi. A game is **vertical** when `G` is `R90` or `R270`, and **horizontal** when it is `R0` or `R180`.

**Override** (`O`). The per-game setting `display.rotation`: Auto, or a fixed `G`. A fixed value replaces the core's `G` (Decision: Owner, 2026-11-24, T1). It does not change the frame, the aspect or the input. Its use is a game whose core reports no turn or a wrong one. It is not a way to turn a horizontal game to fill a turned screen (section 10).

**Turn** (`R`). The turn of the game picture relative to the scan-out, the value of `Scene::rotation`:

> `R = (G' - S) mod 360`, where `G'` is `O` when the override is set, and the core's `G` for Auto.

A frame turned by `G'` is upright on a screen at 0. The display adds its own turn `S` as the player sees it. So the renderer turns by `G' - S`, and the sum is `G'`. ADR 0015 already names this rule: "the core's turn less the screen's turn".

**UI turn** (`U`). The turn of the canvas relative to the scan-out, the value of the new `Scene::ui_rotation`. The canvas is always drawn upright, so `U = (0 - S) mod 360`: `R0`, `R270`, `R180`, `R90` for `S` = 0, 90, 180, 270.

**Across.** A picture with `R` of `R90` or `R270`: its lines are columns of the frame. **Own lines.** A picture with `R` of `R0` or `R180`: the display scans the frame's own lines.

## 4. The turn of every case

`G'` and `R` follow from the rule of section 3. The size rule:

- **Own lines, full screen.** The policy sees the frame as drawn, the same raster as a horizontal game of that size. It gets every rule of a horizontal source: DoDonPachi on a turned 15 kHz tube is 240p at 57.55 Hz, every pixel exact.
- **Across, upright and small.** The policy sees the swapped raster, and the rules of a turned source hold (`docs/spec/arcade.md`, section 11): exact lines with borders, interlaced 1:1 on a 15 kHz CRT and on a TV, the fit only with Picture mode Fit. This is how a vertical game shows on a screen at 0 today.

The last column is how the player sees the picture. It depends only on `G'` against the true orientation of the game: with Auto, or with the override equal to the truth, the picture is always upright. So a horizontal game on a turned screen is always shown across, upright and small, unless the user sets an override that does not match the game.

The table uses a horizontal game with `G = R0` and a vertical game with `G = R90`. A flipped board (`R180`, `R270`) follows the same rule. A unit test holds this table equal to the output of the function (section 11).

<!-- tate-table:begin -->
| Screen `S` | Game | Override | `G'` | Turn `R` | Size | The player sees |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | horizontal | Auto | `R0` | `R0` | own lines, full | upright |
| 0 | horizontal | 0 | `R0` | `R0` | own lines, full | upright |
| 0 | horizontal | 90 | `R90` | `R90` | across, small | top at the left |
| 0 | horizontal | 180 | `R180` | `R180` | own lines, full | upside down |
| 0 | horizontal | 270 | `R270` | `R270` | across, small | top at the right |
| 0 | vertical | Auto | `R90` | `R90` | across, small | upright |
| 0 | vertical | 0 | `R0` | `R0` | own lines, full | top at the right |
| 0 | vertical | 90 | `R90` | `R90` | across, small | upright |
| 0 | vertical | 180 | `R180` | `R180` | own lines, full | top at the left |
| 0 | vertical | 270 | `R270` | `R270` | across, small | upside down |
| 90 | horizontal | Auto | `R0` | `R270` | across, small, notice | upright |
| 90 | horizontal | 0 | `R0` | `R270` | across, small, notice | upright |
| 90 | horizontal | 90 | `R90` | `R0` | own lines, full | top at the left |
| 90 | horizontal | 180 | `R180` | `R90` | across, small | upside down |
| 90 | horizontal | 270 | `R270` | `R180` | own lines, full | top at the right |
| 90 | vertical | Auto | `R90` | `R0` | own lines, full | upright |
| 90 | vertical | 0 | `R0` | `R270` | across, small | top at the right |
| 90 | vertical | 90 | `R90` | `R0` | own lines, full | upright |
| 90 | vertical | 180 | `R180` | `R90` | across, small | top at the left |
| 90 | vertical | 270 | `R270` | `R180` | own lines, full | upside down |
| 180 | horizontal | Auto | `R0` | `R180` | own lines, full | upright |
| 180 | horizontal | 0 | `R0` | `R180` | own lines, full | upright |
| 180 | horizontal | 90 | `R90` | `R270` | across, small | top at the left |
| 180 | horizontal | 180 | `R180` | `R0` | own lines, full | upside down |
| 180 | horizontal | 270 | `R270` | `R90` | across, small | top at the right |
| 180 | vertical | Auto | `R90` | `R270` | across, small | upright |
| 180 | vertical | 0 | `R0` | `R180` | own lines, full | top at the right |
| 180 | vertical | 90 | `R90` | `R270` | across, small | upright |
| 180 | vertical | 180 | `R180` | `R0` | own lines, full | top at the left |
| 180 | vertical | 270 | `R270` | `R90` | across, small | upside down |
| 270 | horizontal | Auto | `R0` | `R90` | across, small, notice | upright |
| 270 | horizontal | 0 | `R0` | `R90` | across, small, notice | upright |
| 270 | horizontal | 90 | `R90` | `R180` | own lines, full | top at the left |
| 270 | horizontal | 180 | `R180` | `R270` | across, small | upside down |
| 270 | horizontal | 270 | `R270` | `R0` | own lines, full | top at the right |
| 270 | vertical | Auto | `R90` | `R180` | own lines, full | upright |
| 270 | vertical | 0 | `R0` | `R90` | across, small | top at the right |
| 270 | vertical | 90 | `R90` | `R180` | own lines, full | upright |
| 270 | vertical | 180 | `R180` | `R270` | across, small | top at the left |
| 270 | vertical | 270 | `R270` | `R0` | own lines, full | upside down |
<!-- tate-table:end -->

"Notice" marks the case of the owner's decision: a horizontal game across a turned screen (section 7.4).

## 5. The raster that the policy sees

The rule: **the engine turns the source before the policy, and the policy does not change.**

- The session gives the engine a `SourceGeometry` as today: the frame as drawn, its aspect as drawn, its rate, its interlace flag, and the core's turn `G` in `rotation`. The session does not change.
- The engine holds an `Orientation` (the screen `S` and the override `O`, section 8). For every source, before the policy, it makes the **display source**: the same `SourceGeometry` with `rotation = R` of section 3. Width, height, rate, interlace and aspect stay as they are: they describe the frame as drawn, and `R` is a turn of that frame.
- The policy (`offer`, `decide`, `choose`), the validator, the scene builder and Switchres see only the display source. So `take_source` swaps the axes exactly when the picture is across the scan-out, and the lines that the policy places are the lines that the display scans.
- The engine applies the same step to the picture answers: the running game, and each of `REFERENCE_SOURCES` for Settings > Display (with the override at Auto). On a turned screen DoDonPachi is then a source of own lines, and the horizontal sources are across. A change of `S` or `O` changes the display sources, so the answers are made again, as for any other input.
- A change of `S` or `O` in a running game is a new source. It goes through the debounce of the engine as a new size does (ADR 0015, point 9).

Examples, all with no change to the policy:

| Source | `S` | Display source | Result on `generic_15` |
| --- | --- | --- | --- |
| DoDonPachi, 320x240, `R90`, 57.55 Hz | 0 | 320x240, `R90`: 240 wide, 320 lines | Interlaced 1:1, 448i, 71 %, as today |
| DoDonPachi | 90 | 320x240, `R0`: 240 lines | 240p at 57.55 Hz, 1:1, full screen, no flicker |
| DoDonPachi | 270 | 320x240, `R180`: 240 lines | The same mode; the picture is turned by 180 |
| Street Fighter II, 384x224, `R0`, 59.63 Hz | 90 | 384x224, `R270`: 224 wide, 384 lines | Across: interlaced 1:1, 384 of 448 lines; Fit gives the 240p fit |
| `MENU_SOURCE`, 320x240, `R0`, 60 Hz | 90 | Not a display source: the menu plans the raster of the turned portrait canvas, 320x240 at `R0` (section 6.3) | 240p at 60 Hz, the menu mode of a screen at 0 |

The validator and the hard rails see the real raster of the display in every case. TATE changes no safety rule.

### 5.1 The expert key `display.vertical`

The key stays: the settings formats are append-only, and the picture mode covers it already (`PICTURE_MODE_COVERS`). Its effect does not change in code: it acts on a source that the policy sees across the lines (`OrientedSource::turned`). With TATE that is "a game across the lines of the screen": a vertical game on a screen at 0 or 180, a horizontal game on a screen at 90 or 270. Only its help text changes:

> How a game shows when it stands across the lines of the screen: a vertical game on a normal screen, or a horizontal game on a turned screen. Exact shows every pixel exact, with black borders, and on a 15 kHz screen or a TV with flicker. Fit fills the height, with a soft picture.

The label "Vertical games" stays. No menu shows it.

### 5.2 Scanlines

ADR 0015 sets "the scanlines follow the lines of T": the dark lines are horizontal in scan-out terms, on the display's own scan lines. This stays, and it is physically right on a turned CRT: the drawn lines lie on the tube's real lines, which the player sees vertical. A vertical game on a turned 31 kHz tube now gets 2x and scanlines between its own lines, as its original cabinet showed them. The rule of `scanlines_for` (a whole vertical multiple of 2 or more, no interlace, the class of the mode) reads the lines of the display source, so nothing changes in code. On a turned flat panel the same holds: the lines are vertical for the player, as on the turned tube that the panel imitates. The UI canvas is never darkened.

## 6. The UI on a turned screen

### 6.1 The options

The canvas is 320x240, upright for the player. On a turned screen the player's horizontal axis is the display's line axis. So the 320 canvas columns need 320 lines of the display, and the 240 canvas rows lie along the scan lines.

- **(a) The renderer turns the canvas.** A contract change: `Scene::ui_rotation`, with the pixel rule of `Scene::rotation`. The layout of every screen stays 320x240. In scan-out terms the turned canvas `T` is 240 wide and 320 lines high.
- **(b) A portrait layout.** A second layout of every screen for a 240x320 canvas, turned by (a). Its 240 columns need only 240 lines.
- **(c) (a) with the smaller size accepted.** The same as (a): the turned 320x240 canvas is smaller on a turned 4:3 face, because a landscape picture fits a portrait face only in its width.

(a) and (c) are one option: (a) is the mechanism, (c) is the result.

### 6.2 The numbers of option (a)

These are the numbers of the turned 320x240 canvas, option (a). Package 3 builds it as an interim step. They show why the owner chose (b) (section 6.3). Sizes on the face are as the player sees it. The 20-inch tube has a face of about 16 x 12 inches (12 inches wide when turned); the 17-inch tube about 12.8 x 9.6 inches (9.6 inches wide when turned). A glyph is 6x12 canvas pixels. The mode of the menu is the answer of the policy for the turned `MENU_SOURCE` (section 5); package 2 records it in a golden, and the modes below are the expected values from the results for DoDonPachi on a screen at 0 (`docs/spec/arcade.md`, section 11), which has the same 320 lines across.

| Display class | Mode of the turned menu | Display pixels per canvas pixel | Width of the menu (player) | Glyph on the face | Today at `S = 0` |
| --- | --- | --- | --- | --- | --- |
| 15 kHz CRT (`generic_15`) | Interlaced 1:1, 448i or 480i at 60 Hz | 1 line across; about 1 sample along, the mode width keeps the shape | 67 % to 71 % | 0.16 x 0.32 in (20-inch), interlaced | 240p, full face; glyph 0.30 x 0.60 in |
| 15 kHz CRT, Picture mode Fit | 240p fit | 0.75 line across, soft | 100 % | 0.23 x 0.45 in, soft, no flicker | as above |
| 31 kHz CRT (`arcade_31`) | 320 lines 1:1 in about 500 lines, progressive | 1 line across | 62 % to 64 % | 0.12 x 0.23 in (17-inch), no flicker | 480p 2x, full; glyph 0.24 x 0.48 in |
| Multi-sync CRT from 40 kHz | 2x, 640 lines | 2 lines across | full | as the 31 kHz menu today | 2x or more |
| Flat panel 1080p, turned | 1920x1080 native | 3x3 (720x960 in scan-out) | 89 % | 18 x 36 panel pixels | 4x, 24 x 48 |
| Flat panel 720p, turned | 1280x720 native | 2x2 | 89 % | 12 x 24 | 3x |
| Flat panel 4K, turned | 3840x2160 native | 6x6 | 89 % | 36 x 72 | 9x |
| TV NTSC, turned | 480i, 320 of 480 lines | 1 line across, 1 sample along (240 samples, 9 % narrow) | 67 % | about 0.16 x 0.32 in on a 20-inch set, interlaced | 240p, 640x240 samples |
| TV PAL, turned | 576i, 320 of 576 lines | as NTSC | 56 % | smaller, interlaced | 288p |

The quick menu and the notices over a running game use the game's mode, not the menu's (section 6.4). On a turned screen a vertical game runs at its own lines, so:

| Mode of the game | The turned canvas over it |
| --- | --- |
| 240p or 288p (15 kHz CRT, TV) | 320 columns on 240 or 288 lines: a scale of 0.75 or 0.9, sharp bilinear, soft |
| 480p (31 kHz, 2x) | 320 lines 1:1, 67 %, nearest |
| A flat panel | the whole scale of the table above |

What the numbers say:

- A flat panel is easy. The turned menu is 3x on 1080p, sharp and large.
- A 31 kHz tube is fine: 1:1, progressive, the glyph at about half of today's size. It reads well at arm's length.
- A 15 kHz tube and a TV are the hard case, and option (a) does not solve it. 320 columns do not fit 240 progressive lines. The choices are 1:1 interlaced (sharp, but each vertical stroke of a glyph is one line of one field, so it flickers at 30 Hz, and the glyph is about half of today's size) or the 240p fit (no flicker, but soft: a stroke of one canvas pixel blends over two lines). Both are readable for the short labels of the menu. Neither is good for long help lines and notices. The quick menu over a game at 240p is always the soft case.
- Option (b) solves the hard case completely: 240 canvas columns on 240 lines, 240p, 1:1, progressive, the full width of the face, the glyph at today's size. Its cost is a second layout of every screen (about 9,300 lines of screen code, 78 goldens twice, the crawler in both layouts) and 40 characters per line in place of 53, so many texts wrap again.

### 6.3 The decision

Decision: Owner, 2026-11-24 (T3, T4): a portrait layout, never squeezed, never interlaced.

- A screen at 90 or 270 gets a portrait canvas, 240 wide and 320 high for the player, laid out by the same code as the landscape canvas (`docs/spec/ui-layout.md`). The renderer turns it by `U` (ADR 0020). After the turn, `T` is 320x240 in scan-out: the raster of the landscape canvas.
- The menu plans its mode from the raster of `T`: `MENU_SOURCE` at `R0` in every orientation. So the menu mode on a turned screen is the menu mode of a screen at 0: 240p on a 15 kHz tube and a TV, 240 canvas columns on 240 lines, 1:1, progressive, with the glyph at today's size. `ui_dest` is the rectangle of today (ADR 0005, the TV rule of P03). `docs/spec/ui-layout.md`, section 6.3, checks this for each display class with numbers.
- A screen at 180 keeps the landscape canvas, turned by `R180`.
- A flat panel mounted in portrait gets the portrait canvas at a whole scale: 4x on 1080p.
- The quick menu over a game keeps the game's mode, and `T` takes the placement of that mode: 1:1 over a vertical game at 240 lines on a turned 15 kHz tube; the full size over a horizontal game shown across, with the small upright game behind it.
- ADR 0021 (accepted, Owner, 2026-11-24) records the contract change.

Option (a) of section 6.1 stays in the code until the layout work replaces it (package 7 of section 12): it is the interim path of package 3, and it never runs in the product, because the orientation stays `Orientation::NORMAL` until package 8 wires the setting.

### 6.4 Where the turned canvas goes (interim)

This section describes the interim placement of the turned 320x240 canvas of package 3. Package 7 of section 12 removes `ui_dest_turned`: with the portrait canvas, `T` is 320x240 on every CRT and TV, and the placement of today places it. The filter rule below stays, for every placement of `T` (ADR 0021).

`ui_dest` holds the canvas after the turn, in scan-out pixels (ADR 0020). For `U` of `R0` and `R180` it is the rectangle of today (ADR 0005, the TV rule of P03): a turn by 180 does not change the shape. For `R90` and `R270` one pure function, `ui_dest_turned` in `crates/neptron-modeline/src/policy/placement.rs`, gives the rectangle and the filter from the monitor kind, the mode, and the size of the turned canvas `Tw x Th` (240 x 320 for the 320x240 canvas):

- **CRT.** Lines: `Th x k` for the largest whole `k >= 1` with `Th x k <= vactive`; with no such `k`, all `vactive` lines. Width: the width at which `T` keeps its shape on the face of the profile (`solver.monitor_aspect`, `fw:fh`): `w = round(Tw x h x hactive x fh / (Th x vactive x fw))`, at most `hactive`, at least 1. Centred.
- **TV.** Lines as on a CRT. Width: the whole multiple of `Tw` nearest the shape width, at least 1 and at most `TV_SAFE_WIDTH_SAMPLES` (704). On NTSC 480i that is 240 samples, 9 % narrower than the shape, with nearest. The TV rule of today also uses whole factors, because a fraction gives uneven columns of text. Centred.
- **Flat panel.** `ui_dest_uniform(hactive, vactive, Tw, Th)`: the largest whole scale, centred, as today with the sides swapped.

The filter (`Scene::ui_filter`, ADR 0020): `Nearest` when the scale of each axis is 1 or more, so no canvas pixel is lost; `SharpBilinear` when an axis is below 1, so a stroke of one canvas pixel blends and is not dropped. Nearest below 1 would drop every fourth canvas column at 0.75, and a vertical stroke of a glyph would vanish. An upright canvas has both scales at 1 or more in the menu, on a TV and on a flat panel. On a CRT in a game mode of fewer than 240 lines or 320 samples (a 224-line game) it does not. Decision: Owner, 2026-11-24: the rule applies at every orientation, so the UI over such a game becomes soft on a normal screen too; `docs/spec/ui-layout.md`, section 6.4.1, says what changes and in which package (package 7).

In the menu, the mode is the policy's answer for the turned menu source, so `ui_dest_turned` finds 320 lines 1:1 on a 15 kHz or 31 kHz tube and on a TV.

### 6.5 The menu mode on a turned CRT

With the portrait canvas the menu needs no mode of its own: the menu at 90 and 270 has the mode of the menu at 0, from the same source through the same policy and validator. A change of the screen orientation in the menu is not a mode change on any display; the tube does not re-sync. The change applies at once, with no keep prompt (section 9.1).

The interim path of package 3 plans the menu source turned with the screen, which gives an interlaced mode on a 15 kHz tube and a TV (240p to 448i on `generic_15`). Package 7 removes it.

## 7. Everything that draws

### 7.1 The canvas and its screens

Every screen draws into the canvas, and the engine turns the canvas by `U`: the game list, the settings, the dialogs, the keep prompts, the notices, the guard's prompts and safe-mode rows, the recovery screens, the grid pattern and the guided mapping. On a screen at 90 or 270 each one lays out on the portrait canvas, with the rules of `docs/spec/ui-layout.md` (section 4 there has a sketch of each screen). A canvas golden stays a canvas golden, with a portrait golden for a chosen set of scenes; the turned output has its own goldens (section 11).

### 7.2 The UI over a game

The game picture and the canvas are two layers with two turns. A vertical game on a screen at 90 has `R = R0`; the canvas has `U = R270`. The renderer draws the game, then the canvas over it, each with its own turn, into its own rectangle. The quick menu, the keep prompt of the quick menu, the dialogs over a game and the notices over a running game all show this way.

Decision: Owner, 2026-11-24 (T4): no mode change when the quick menu opens. The canvas is the portrait canvas, so `T` is 320x240 and takes the placement of the game's mode: 1:1 over a vertical game at 240 lines on a turned 15 kHz tube, nothing soft. Over a horizontal game shown across, the portrait menu has the full size, with the small upright game behind it. `docs/spec/ui-layout.md`, section 6.4, gives every case.

### 7.3 The screensaver

The screensaver draws in the canvas, so it bounces in the turned frame: the walls are the walls of the portrait canvas as the player sees it. On a 15 kHz tube in the turned menu it moves at 240p, one canvas pixel per step. In a paused game the session hides the game picture as today.

### 7.4 Notices

- **A horizontal game on a turned screen** (the owner's notice). When `S` is 90 or 270 and the game is across with the override at Auto (or at the game's own orientation), the app adds its text to the picture notice (`apps/neptron/src/shell/picture.rs`) once at each start of the game, at the first picture: "This game is horizontal. It shows small on a turned screen." The notice shows in the quick menu, not over the game, as every picture notice. A later picture notice of the same start (after a change of the picture mode) does not repeat it. Decision: Owner, 2026-11-24 (T7): in the picture notice at each start, as the other picture notices. Decision: Owner, 2026-11-25: the text as written here, in the quick menu, once at each start of the game. No new `NoticeReason`: the policy does not know the screen orientation, and the app does.
- **The one-time notice of an interlaced picture across the lines** (`VERTICAL_INTERLACED` in `apps/neptron/src/notices.rs`). It fires when a picture across the lines gets the interlaced rung on a 15 kHz CRT or a TV. Its last sentence changes, because TATE now exists: "Settings > Display > Screen orientation turns the menus and the games for a screen on its side." A second id, `horizontal_interlaced`, has its own text for a horizontal game across a turned screen: "This is a horizontal game on a turned screen. It shows interlaced, with every pixel exact. Picture mode Fit, in Settings > Display or in Video in the quick menu, shows the whole picture as large as the screen allows, with no flicker." Both are one-time notices in `state/config/notices.toml`. The ids only grow.

### 7.5 The boot, the guard and the first setup

- The screen orientation is a global setting. The app reads it before the first frame, so the first picture of a boot (the boot prompt of the display guard, or the menu) is already turned.
- At the first boot the setting is 0 ("Yoko (0)"). A user with a turned screen sees the boot prompt and the menu on their side. Two ways set it:
  1. The menu, sideways, once: Settings > Display > Screen orientation. The text is readable with the head turned, and the directions are those of the upright canvas (section 8).
  2. Before the first boot: `neptron.toml` on the boot partition, `[display]` `orientation = "90"`. The boot file imports any global key of the schema (`docs/spec/raspberry-pi.md`, "`neptron.toml` on the boot partition"). The image files get a commented line (`# orientation = "0"   # 0 | 90 | 180 | 270`). The files are in `os/board/*/neptron.toml`, outside the paths of N01: package 8 asks the orchestrator to add them. Decision: Owner, 2026-11-24: the first setup of a turned cabinet is these two ways. A first-boot screen that asks which way is up (four texts, one upright along each edge) is later work (`docs/spec/ui-layout.md`, D7).
- The display guard does not change. It judges the signal, and the orientation does not change what a monitor takes: the menu mode of a turned screen goes through the policy and the validator as every mode does. Safe mode, the blind recovery and a reverted boot prompt keep the orientation (Decision: Owner, 2026-11-24, T5): the safe picture is the safe signal, and a turned cabinet needs its turned menu most of all in safe mode.
- A wrong orientation is no lockout (Decision: Owner, 2026-11-25). The orientation never changes the menu mode (section 6.5), and every mode still goes through the validator. The menu is drawn whole in every orientation: at worst it is sideways or upside down for the player, and Up on the stick moves up in the canvas. The player reads it with the head turned, moves to the same row and picks again. `neptron.toml` on the boot partition sets the value too.

## 8. Input

- **The menus.** The canvas is drawn upright for the player at every orientation, and the controls of a cabinet are mounted for the player. Up on the stick is up in the canvas, which is up on the screen as the player sees it. So the navigator needs **no** turn of directions. `nav.rs` and `neptron-input` do not change. A tier 2 test proves it: at each `S`, Down moves the focus to the next row, as the player reads it.
- **A game.** The core gets the directions unchanged. With Auto, or with an override that matches the game, the picture is upright for the player, so the directions are right. A game shown turned by a wrong override (the last column of section 4 is not "upright") gets its directions unchanged too: the controls of a cabinet do not move, and the user chose that picture. A turn of the game input is not in version 1 (section 10).
- **Hotkeys.** The quick menu hold, the Start hold and the blind recovery do not change. They read buttons, not directions.
- **The first boot.** Until the user sets the orientation, the menu is sideways on a turned screen, and Up moves up in the canvas, which is a side for the player. This is once, and section 7.5 gives the way out.

## 9. Settings

Two new keys, at the end of the schema (the key list is append-only). Their choices are strings, so a later value is appended.

### 9.1 Screen orientation

| Field | Value |
| --- | --- |
| Key | `display.orientation` |
| Label | Screen orientation |
| Kind | choice: `0`, `90`, `180`, `270` (default `0`) |
| Levels | global (`Levels::GLOBAL`) |
| Live | yes |
| Safety | `Validated`: the menu mode it gives goes through the validator |
| Applies to | every display |
| Where the row shows | Settings > Display, after Monitor profile (or TV standard) and before Picture mode |
| Where the player changes it | Settings > Display, from the menu only. The row is disabled during a game (T6). The value applies at once and is stored at the global level, with no keep prompt (Decision: Owner, 2026-11-25) |
| Texts | Decision: Owner, 2026-11-24 (T8): the label, the help text and the help lines as written here. Decision: Owner, 2026-11-25: the option labels as written here |
| Help text | "How the screen stands. Turn the menus and the games for a screen turned on its side, as in a vertical cabinet. Vertical games then fill the screen at their own lines." |

| Value | Label | Help line |
| --- | --- | --- |
| `0` | Yoko (0) | The screen stands as usual. |
| `90` | Tate left (90) | The screen is turned 90 degrees to the left: its top is on the left. |
| `180` | Yoko flipped (180) | The screen is upside down: its top is at the bottom. |
| `270` | Tate right (270) | The screen is turned 90 degrees to the right: its top is on the right. |

**No keep prompt.** Decision: Owner, 2026-11-25, in place of the keep prompt of T5: a change of the screen orientation applies at once and is stored at the global level. The menu takes the form of the new orientation in the next frame, with no restart (`docs/spec/ui-layout.md`, section 6.6). Reason: the menu stays readable and usable in every orientation, so a wrong pick cannot lock the user out (section 7.5). A wrong pick (90 for a screen turned right) shows the menu upside down; the player picks again on the same row.

Decision: Owner, 2026-11-24 (T6): the Screen orientation row is disabled during a game, with the reason "Not during a game", the rule of the rescan row. A turn of the screen in a game would replan the game and the menu at once, and a player turns a cabinet's monitor with the machine in the menus.

### 9.2 Game orientation

| Field | Value |
| --- | --- |
| Key | `display.rotation` (the name of the Definition and of ADR 0015) |
| Label | Game orientation |
| Kind | choice: `auto`, `0`, `90`, `180`, `270` (default `auto`) |
| Levels | game only (`Levels::new(true, false, false)`). Decision: Owner, 2026-11-24 (T2) |
| Live | yes: a change in a running game is a new source (section 5) |
| Safety | `Validated` |
| Applies to | every display |
| Where the row shows | `VIDEO_PAGE` of the quick menu, the first row of the display rows |
| Where the player changes it | The Video page of the quick menu, during the game, and in the game's display settings wherever the per-game picture rows show. A change applies at once to the running game (a new source, section 5) and is stored for that game at the game level. Settings > Display has no row, because the key has no global level |
| Auto | Decision: Owner, 2026-11-25: Auto shows every game upright to the viewer on the current screen, as the table of section 4 says. A vertical game fills a turned screen; a horizontal game shows across it, small |
| Texts | Decision: Owner, 2026-11-24 (T8): the label, the help text and the option texts as written here |
| Help text | "How the game picture turns to stand upright. Auto takes it from the game. Change it only when a game shows on its side or upside down." |

| Value | Label | Help line |
| --- | --- | --- |
| `auto` | Auto | The turn that the game asks for. |
| `0` | No turn | The picture shows as the game draws it. |
| `90` | Turn left | The picture turns 90 degrees to the left. |
| `180` | Turn over | The picture turns upside down. |
| `270` | Turn right | The picture turns 90 degrees to the right. |

The row is not judged by `offer`: every value has a picture, because the policy gives a picture for every source shape (or the Fit fallback). The keep prompt of the quick menu does not offer a move to the system or the global level, because the key has the game level only.

### 9.3 The picture mode pair

The pair storage of Picture mode and Refresh rate (`docs/spec/video-policy-cases.md`, section 8.7) does not change. The pair is judged with the display source, so the answers on a turned screen are the answers for the turned picture. One pair serves every orientation, as one pair serves every display class (P4). This is part of the design that the owner accepted on 2026-11-24. A pair stored for a game at `S = 0` (for example `native` for a vertical game) can give another picture at `S = 90`; the notice of a stored candidate (section 8.9 of that spec) says what shows.

## 10. Not in version 1

- A horizontal game turned to fill a turned screen ("for now dont rotate horizontal games"). The override can show it so, sideways, but nothing does it in Auto, and its input is not turned.
- A turn of the game input to match a picture shown turned by the override.
- A menu mode of its own on a 15 kHz tube. The portrait menu needs none (section 6.5).
- A mode change when the quick menu opens over a game. The portrait canvas needs none (section 7.2).
- Automatic detection of the orientation (a pivot sensor, an EDID hint, a setting per monitor).
- The core's own TATE modes (`fbneo-vertical-mode`), and a second core that turns its frames itself (MAME 2003-Plus gives an upright frame with `R0`; on a turned screen such a vertical game would show across). Version 1 needs the core's turn.
- The field order of an interlaced game (a 480i console game) on a screen at 180: the turn moves the first field to the odd lines. Rare; the picture still shows, with judder on motion.
- An orientation per display class (one setting serves every display, as for the picture mode).

## 11. Tests and goldens

### 11.1 Unit tests of the pure functions

- `Rotation` arithmetic (`neptron-types`): `minus` for all 16 pairs, the quarter-turn round trip, `across` for the four values.
- `Orientation::game_turn` and `Orientation::ui_turn`: every row of section 4 for both flipped and unflipped boards (`R0`, `R90`, `R180`, `R270` as the core's turn), and the four UI turns. The test `the_spec_table_of_turns_is_the_output_of_the_function` reads the table between the markers of section 4 and compares it with the function, as the offer table does.
- `Orientation::display_source`: every field but `rotation` is kept, for an interlaced source and a source with no aspect too.
- `ui_dest_turned` (interim; package 7 removes it with its tests, and keeps the tests of the filter rule): `generic_15` at 448i (320 lines 1:1, nearest), at 240p (240 lines, sharp bilinear), `arcade_31` at about 500 lines, NTSC 480i (240 samples, 320 lines, centred), PAL 576i, 240p on a TV (sharp bilinear), panels 1920x1080 (`(600, 60, 720, 960)`), 1280x720 (`(400, 40, 480, 640)`) and 3840x2160, a portrait panel mode 1080x1920, a super-width CRT mode 1920x240, zero sizes and the 16-bit extremes. Each with the filter.
- Renderers (`neptron-gfx`, reference): the canvas turned by each `ui_rotation` with an asymmetric 3x2 canvas, pixel for pixel against the rule of ADR 0015; `ui_filter` below a scale of 1; a turned canvas clipped at each edge; a turned `ui_dest` above the side limit refused; a game and a canvas with different turns in one scene. Every test of today passes with `R0` and `Nearest`.
- Policy (`neptron-modeline`): a golden `policy_tate.toml` with the reference boards of Appendix B of the arcade spec as display sources at `S = 90` and `S = 270`, on one profile of each class (`generic_15`, `arcade_25`, `arcade_31`, `d9800`, `tv_ntsc`, `tv_pal`, `panel_1080`). A test asserts that a vertical board at `S = 90` gives the same mode as the same raster at `R0`. The golden also records the menu cases. Package 2 records the policy's answer for the turned menu source as data; package 7 records them again as the source of the turned portrait canvas, with a test that the menu at 90 and 270 has the mode of the menu at 0 (section 6.3). The policy code does not change, so every other golden of today stays.

### 11.2 Engine and app

- Engine (`neptron-video`, fake display): at each `S`, a vertical and a horizontal source give the scene `rotation`, `ui_rotation`, `ui_dest` and `ui_filter` of sections 4 and 6.4; a change of `S` or `O` is a new source through the debounce; the answers of Settings > Display on a turned screen use the turned reference sources; `Orientation::NORMAL` gives the scenes of today.
- Settings (`neptron-settings`): the two schema entries, their levels and safety, a round trip through a file, a value the schema refuses, the import of `display.orientation` from `neptron.toml`.
- App, tier 2: a boot with `S = 90` turns the first frame; a change on Settings > Display applies at once, is stored, and the menu takes the form of the new orientation with no restart; every mode that the display receives passes the validator at each `S`; the row is disabled in a game; the Game orientation row of `VIDEO_PAGE` changes the turn of the running game; the notice of a horizontal game on a turned screen; the screensaver under a turned menu; Down moves the focus down at each `S` (no turn of input); the crawler finds both rows.

### 11.3 Goldens of the turned output

The landscape canvas goldens of `neptron-ui` stay as they are; the portrait canvas gets its own goldens and the fit check (`docs/spec/ui-layout.md`, section 9). The turned output gets goldens of the whole picture in scan-out pixels, drawn by the reference renderer from the scenes of the engine: for each display class (`generic_15`, `arcade_31`, `panel_1080`, `tv_ntsc`) and each `S` of 90, 180 and 270, three scenes: the game list, a vertical game with the quick menu over it, and a horizontal game. The game frames are test-core patterns. 36 goldens. `S = 0` is checked equal to the output of today. Each golden is opened and looked at before it is blessed.

### 11.4 Tier 3

`gles-compare` gets scenes with a turned canvas, and in package 7 with a 240x320 canvas: `ui_rotation` `R90`, `R180` and `R270`; with `Nearest` at a whole scale, with `SharpBilinear` at 0.75; over a game with `rotation` `R0` and over one with `R270`; a turned canvas clipped at the edge of the output; a turned canvas larger than the texture limit (an error check). They must equal the reference renderer as today: bit exact for nearest, within 1 per channel for a blended pixel.

### 11.5 Hardware checks

Each one is a line in `docs/hardware-checklist.md`, labelled `needs-hardware`. No result is claimed without it.

- A turned 15 kHz CRT: the portrait menu at 240p (readability of 34 characters on a line), DoDonPachi at 240p at its own rate, a horizontal game across, the quick menu over a game at 240p (1:1).
- A turned 31 kHz CRT: the portrait menu at 2x, a vertical game at 2x with scanlines.
- A flat panel turned on a pivot, at 90 and at 270: the portrait menu at 4x, a vertical game filling the width.
- A composite TV turned (if the owner can turn one): the portrait menu at 240p.
- The render time of a scene with a turned canvas on the Raspberry Pi 3, with the existing bench build.
- That the owner can turn a monitor for the bench is a question of the bench session, not of this spec.

## 12. Work packages

Each package builds, passes its gate, and merges alone, in this order. None leaves `main` in a half state: until package 8, the orientation is always `Orientation::NORMAL`, and every picture of today stays the same. Through packages 4 and 5 the 78 landscape goldens of `neptron-ui` stay byte-identical.

Packages 1 to 3 build option (a) of section 6.1: the turn of the canvas, the pure functions and the engine. Package 3 holds an interim path (`TurnedMenuMode`, the turned menu source, `ui_dest_turned`) that never runs in the product. The owner's decision of section 6.3 adds the layout packages 4 to 7 of `docs/spec/ui-layout.md` (L1 to L4 there); package 7 removes the interim path. Package 8 is the settings package with the wiring of the layout (L5). Packages 4 and 5 touch only `neptron-ui` and can run beside package 3. The owner answered the questions of the layout on 2026-11-24 (`docs/spec/ui-layout.md`, section 14). The 78 landscape goldens of `neptron-ui` stay byte-identical in packages 4 and 5.

| # | Package | Crates and paths | Gate |
| --- | --- | --- | --- |
| 1 | The contract and the renderers. `Scene::ui_rotation` and `Scene::ui_filter` (ADR 0020); the reference and GLES renderers draw a turned and filtered canvas; every builder of a `Scene` sets `R0` and `Nearest`; the scene hash of the fake display includes both fields | `neptron-types`, `neptron-gfx`, `neptron-video`. Outside N01's paths: `crates/neptron-kms/src/kms/mode.rs` (only the two fields in the black scene), `crates/neptron-kms/examples/kms_check.rs`, `crates/neptron-window/examples/smoke.rs`, `crates/neptron-testkit` (`src/display/screen.rs`, `src/display/record.rs`, the scene hash, its integration tests), `apps/neptronctl/src/selftest/pattern.rs` | T1; tier 3 `gles-compare` with the new cases |
| 2 | The pure orientation functions. `Rotation` arithmetic and `Orientation` in `neptron-types` with the table of section 4; `ui_dest_turned` in `neptron-modeline`; the golden `policy_tate.toml` | `neptron-types`, `neptron-modeline` | T1 |
| 3 | The engine. `VideoEngine::set_orientation`; the display source before the policy, the scene and the answers; `ui_rotation`, `ui_dest` and `ui_filter` of the scene from the orientation. Holds the interim menu path that package 7 removes | `neptron-video` | T1 |
| 4 | Layout L1: metrics as values. `Form`, `Metrics::of` with the landscape numbers; `chrome::Layout` from `Metrics`; the size constants replaced at every use; `CpuCanvas::with_size`. No visual change | `neptron-ui` | T1; the 78 landscape goldens identical |
| 5 | Layout L2: the widgets take the metrics; rows of one or two lines; `text::draw_short` and the unit rule of `wrap`; the fit check over every landscape scene. No visual change | `neptron-ui` | T1; the 78 landscape goldens identical |
| 6 | Layout L3: the portrait layout. Stacked rows, two-line bands, the registry and the UI tools of `xtask` with the portrait form, 36 portrait goldens, the fit check in both forms, the approved short texts (D1 to D8 of the layout spec) | `neptron-ui`, `xtask` (the UI tools), `apps/neptron` (the approved texts only) | T1; the 36 portrait goldens looked at before the bless; the fit check on all 78 scenes in both forms; no landscape golden changes except `confirm-3s` and `confirm-10s` (the approved text) |
| 7 | Layout L4: the canvas size in the contract (ADR 0021). The form rule in `neptron-types`; the placement takes the size of `T`; the menu plans the raster of `T`; removal of `TurnedMenuMode`, the turned menu source and `ui_dest_turned`; `policy_tate.toml` menu cases recorded again; the GLES upload on a size change; the tree reply with the canvas size (protocol version 4); the filter rule on every screen in its own commit, which changes the UI over a game of fewer than 240 lines or 320 samples on a normal screen (`docs/spec/ui-layout.md`, section 6.4.1) | `neptron-types`, `neptron-modeline`, `neptron-video`, `neptron-gfx`, `apps/neptron`. Outside N01's paths: `neptron-ipc` (the tree reply, protocol version 4), `apps/neptronctl` (the tree print) | T1; tier 3 `gles-compare` with a 240x320 canvas; only the expectations listed under section 6.4.1 of the layout spec change |
| 8 | The settings and the rows, with the layout wiring (L5). The two schema keys; the app resolves them and sets the orientation of the menu engine at boot and of the game engine after `Session::launch`, before the first `run_frame` (the first mode comes with the first frame); the app gives the form to the UI, and the router lays out again at a change; the rows on Settings > Display and `VIDEO_PAGE`; a change of the orientation applies at once, with no keep prompt (Decision: Owner, 2026-11-25); the line of the boot files; the crawler in both forms | `neptron-settings`, `neptron-ui`, `apps/neptron`, `docs/spec`. Outside N01's paths: `os/board/rpi/neptron.toml` and `os/board/pc/neptron.toml` (the commented `orientation` line); `crates/neptron-session` only if the app cannot set the orientation before the first frame | T2; the crawler finds both rows, in both forms |
| 9 | The notices and the turned output. The notice of a horizontal game on a turned screen, the second one-time notice, the text change of the first; the 36 goldens of section 11.3; the tier 2 tests of input and the screensaver | `apps/neptron`, `neptron-gfx` (golden tests) | T2 |
| 10 | Docs and bench. The arcade spec results for turned screens, the crate READMEs, the hardware checklist lines, the open points | `docs/spec`, crate READMEs. Outside N01's paths: `docs/hardware-checklist.md` (the lines of section 11.5) | T0; the owner's bench session |

The path gaps, for each implementer: N01 owns `crates/neptron-types`, `neptron-gfx`, `neptron-modeline`, `neptron-video`, `neptron-ui`, `neptron-settings`, `neptron-input`, `apps/neptron`, `docs/spec` and `docs/adr`. Package 1 also edits the `Scene` builders in `neptron-kms` (`mode.rs`, the KMS commit path: the two fields only), the kms and window examples, `neptron-testkit` and `apps/neptronctl`. Package 6 edits the UI tools of `xtask`. Package 7 edits `neptron-ipc` and `apps/neptronctl`. Package 8 edits `os/board/*/neptron.toml`, and `neptron-session` only in the case above. Package 10 edits `docs/hardware-checklist.md`. The orchestrator widens the paths of N01 for these packages, or runs package 1 and package 7 as small separate branches for the contract (run rule 4).

## 13. Decisions and open questions

The owner accepted the design on 2026-11-24: "ok yes to all, start the work packages". The owner decided T3 and T4 on the same day with the portrait layout (section 6.3). The owner answered the questions of the layout itself on the same day; `docs/spec/ui-layout.md`, section 14, lists the decisions.

| # | Question | State |
| --- | --- | --- |
| T1 | What a fixed Game orientation means | Decision: Owner, 2026-11-24. It replaces the core's turn (section 3). Auto or a correct value always shows the game upright |
| T2 | The levels of Game orientation | Decision: Owner, 2026-11-24. The game level only |
| T3 | The turned menu on a 15 kHz CRT and a TV | Decision: Owner, 2026-11-24: a portrait layout, never squeezed, never interlaced. The menu is the portrait canvas at the menu mode of a screen at 0: 240p, 1:1 (section 6.3) |
| T4 | The quick menu over a game at 240 or 288 lines on a turned screen | Decision: Owner, 2026-11-24: a portrait layout, never squeezed, never interlaced. No mode change; the portrait canvas is 1:1 over a game at 240 lines, and full size over a horizontal game shown across (section 7.2) |
| T5 | The keep prompt of the screen orientation | Decision: Owner, 2026-11-24: the display guard, safe mode and the blind recovery keep the orientation. Changed by the owner on 2026-11-25: no keep prompt; a change applies at once and is stored (section 9.1) |
| T6 | Screen orientation in a game | Decision: Owner, 2026-11-24. The row is disabled during a game: "Not during a game" |
| T7 | The notice of a horizontal game on a turned screen | Decision: Owner, 2026-11-24. In the picture notice at each start; a second one-time notice for its interlaced case. Changed by the owner on 2026-11-25: the text "This game is horizontal. It shows small on a turned screen.", in the quick menu, once at each start (section 7.4) |
| T8 | The texts | Decision: Owner, 2026-11-24. The labels and help lines of sections 7.4, 9.1 and 9.2 as written. Changed by the owner on 2026-11-25: the option labels of Screen orientation carry the word and the degrees (section 9.1) |

## 14. The design notes of the arcade spec

These notes were in `docs/spec/arcade.md`, section 15, before this spec. Each one with what this spec decides:

| Note | Decision here |
| --- | --- |
| The screen orientation is a global setting, safety `ConfirmOrRevert`: the user must read the keep or revert dialog in the new orientation, or the revert must come with no input | Global, safety `Validated`. No keep prompt (Decision: Owner, 2026-11-25): the orientation never changes the menu mode, and the menu stays usable in every orientation (sections 7.5 and 9.1). `ConfirmOrRevert` is the guard's class for the output and the monitor |
| The game turn is the core's rotation less the screen rotation, modulo 360 degrees | Kept: `R = (G' - S) mod 360`, with the override in place of the core's turn (section 3) |
| A vertical game on a vertical screen: turn 0. The policy gets the frame as drawn (320x240) and finds its native mode | Kept (section 5). On a screen at 270 the turn is 180, with the same mode |
| A horizontal game on a vertical screen: turn 90 or 270, the treatment of a turned game; `display.vertical` then means "a game whose long side is across the tube's lines" | Kept (sections 4 and 5.1). The help text of the key says so |
| The core keeps `fbneo-vertical-mode = "disabled"`; NEPTRON does all turns; the core's `TATE` modes are not used | Kept (section 2) |
| The validator and the hard rails see the real raster of the tube. TATE changes no safety rule | Kept (section 5) |
| `Scene` gets a turn for the UI canvas, or one screen turn for the game and the UI | A second field, `ui_rotation`, with `ui_filter` (ADR 0020). One screen turn would change the meaning of `rotation` for every consumer |
| The UI needs a portrait layout; a turned 320x240 canvas does not fit a turned 15 kHz screen at 1:1, so the portrait layout is the likely path | Kept: the owner decided the portrait layout (section 6.3, `docs/spec/ui-layout.md`). The turned 320x240 canvas of packages 1 to 3 is interim |
| The menu directions follow the viewer; the game's input is not turned | Kept. The turned canvas gives this with no change to the input (section 8) |
| Every screen of the frontend must be turned | Kept: every screen draws in the canvas, and the canvas turns (section 7) |
| Tests: policy goldens for turned screens, scene goldens of each screen in portrait, `gles-compare` with a turned UI, a bench check with a turned monitor | Section 11. Portrait canvas goldens for a chosen set of 36 scenes, and a fit check over every scene in both forms (`docs/spec/ui-layout.md`, section 9) |
