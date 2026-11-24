# 0021: The UI canvas has a size of its own, and a turned screen gets a portrait canvas

- Status: proposed
- Date: 2026-11-24
- Task: N01

## Context

The owner decided on 2026-11-24 that a screen turned on its side gets a portrait layout of the menu, "never squeezed", and that the UI is "somewhat responsive" (`docs/spec/ui-layout.md`, section 1). Orientation 90 and 270 get the portrait menu; 180 keeps the landscape menu, turned. A flat panel mounted in portrait gets the portrait menu at a whole scale.

The parts before this change:

- `Scene::ui` is documented as "the 320x240 UI canvas". `Frame` carries its width and height, and both renderers turn and place a canvas of any size (ADR 0020).
- ADR 0020 turns the 320x240 canvas on a turned screen. `T`, the canvas after its turn, is then 240x320 in scan-out. `ui_dest_turned` places it, and the menu plans its mode from the menu source turned with the screen. On a 15 kHz CRT and a TV that gives an interlaced menu or a squeezed, soft canvas (`docs/spec/tate.md`, section 6).
- The placement of the policy places a canvas of the constant size `UI_CANVAS` (320x240).
- `neptron-ui` draws only a 320x240 canvas, and its layout reads constants (`UI_WIDTH`, `UI_HEIGHT`, `SAFE_AREA`).
- The tree reply of the control protocol gives row bounds in canvas pixels, with no canvas size.

## Decision

1. The UI canvas has a size of its own. `Scene::ui` is the canvas, upright for the player, of the size of its `Frame`. Two sizes are valid: landscape 320x240 and portrait 240x320. `Scene` gets no new field.
2. The form factor is a pure function in `neptron-types`: `Portrait` when the face of the display, as the player sees it, is taller than it is wide, else `Landscape`. The face is the shape of the display in scan-out (a CRT or a TV: 4:3; a flat panel: its native mode) turned by the screen orientation. So a CRT, a TV or a landscape panel at 90 or 270 gets `Portrait`, and at 0 or 180 `Landscape`.
3. The video side decides the form, as it decides `ui_rotation` (ADR 0020). The app gives the form to the UI before the next frame and sends the canvas with its real size.
4. The menu plans its mode from the raster of `T`, the canvas after its turn. For a portrait canvas turned by 90 or 270 degrees, and for a landscape canvas upright or turned by 180, `T` is 320x240: the menu source `MENU_SOURCE` at `R0`. So the menu has the same mode in every orientation of a CRT, a TV and a landscape panel, through the same policy and validator.
5. The placement of the policy takes the size of `T` as a parameter in place of `UI_CANVAS`. The rules of ADR 0005 and of the TV (P03) do not change. On a CRT and a TV `T` is always 320x240. On a flat panel `T` can be 240x320 (a panel with a portrait native mode), and the uniform whole scale takes it.
6. The filter rule of ADR 0020, point 4, applies to every placement of `T`: `Nearest` when each axis scales by 1 or more, `SharpBilinear` when an axis is below 1.
7. `ui_dest_turned` is removed. The menu choice of TATE package 3 (`TurnedMenuMode`, `menu_plan_source`) is removed. `ui_rotation`, `ui_filter`, `Orientation` and the game turn stay.
8. The GLES renderer compares canvas rows for its upload only between canvases of the same size; a canvas of a new size goes whole.
9. The tree reply of the control protocol gets the canvas size as an appended field, in a new protocol version. The screenshot reply already carries its size.

## Consequences

- On a turned 15 kHz CRT and a TV the menu is 240p, 1:1, with no flicker and no soft scale: 240 canvas columns on 240 lines. A change of the screen orientation needs no mode change. The questions T3 and T4 of the TATE spec close.
- On a turned flat panel the menu is 4x on 1080p, against 3x for the turned landscape canvas.
- The quick menu over a vertical game on a turned screen is 1:1 at the game's 240 lines. Over a game of fewer lines than the canvas it is soft (`SharpBilinear`) in every orientation; at a screen at 0 this changes the picture of today, where nearest drops canvas lines (an owner question, `docs/spec/ui-layout.md` Q6).
- `neptron-ui` lays out from a `Metrics` value per form (`docs/spec/ui-layout.md`, section 3). Its 78 landscape goldens stay byte-identical; the portrait form gets its own goldens.
- The golden `policy_tate.toml` records the menu cases again: the menu at 90 and 270 equals the menu at 0 for each display class. This changes what NEPTRON sends in the menu of a turned screen, by design.
- `neptron-types` gets the form rule and a `Form` type; `neptron-modeline` loses `ui_dest_turned` and its `UI_CANVAS` constant becomes a parameter; `neptron-video` loses its menu choice; the GLES upload gets a size check; `neptron-ipc` gets a protocol version.
- Nothing on disk changes. The control protocol grows by one field in a new version (append-only).
- A later form (a high-definition canvas) is a new value of `Form` and a new row of the form rule, with no new change of `Scene`.
- On acceptance, ADR 0005 gets the Status line "Amended by ADR 0021: `ui_dest` places `T` of the size of the canvas", and ADR 0020 gets "Amended by ADR 0021: a turned screen gets a portrait canvas; `ui_dest_turned` is removed".

## Alternatives

- Turn the 320x240 canvas on a turned screen (ADR 0020 as built): rejected by the owner. On a 15 kHz CRT and a TV it needs an interlaced menu or a squeezed, soft canvas.
- A second copy of every screen for portrait: rejected. Two layouts drift apart, and every later screen costs twice. One layout code that reads its numbers from the canvas size covers both.
- An arbitrary canvas size with a constraint solver: rejected. The screens are regular, and a closed set of forms keeps the goldens and the fit check finite.
- A landscape canvas on a turned flat panel, because a panel could show it large: rejected. The portrait canvas is larger on a turned panel (4x against 3x on 1080p), and one rule for every display is simpler.
- A new field `Scene::ui_size`: rejected. `Frame` carries the size already; a second copy can disagree with it.
