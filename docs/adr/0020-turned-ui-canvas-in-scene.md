# 0020: The scene carries the turn and the filter of the UI canvas

- Status: proposed
- Date: 2026-11-24
- Task: N01

## Context

A screen can stand turned on its side (TATE, OP-107, `docs/spec/tate.md`). The owner decided that one system setting gives the screen orientation, 0, 90, 180 or 270 degrees, and that the UI follows it.

The parts before this change:

- `Scene::rotation` (ADR 0015) turns the game picture relative to the scan-out. With TATE it carries the game turn `(G' - S) mod 360`, where `G'` is the game orientation and `S` the screen orientation. The engine makes that turn before the policy (`docs/spec/tate.md`, section 5), so the field and its rule do not change.
- The UI canvas (320x240) goes into `Scene::ui_dest` (ADR 0005). ADR 0015, point 5, says: "The UI canvas is never turned by this field. A turned UI (TATE) is a later decision." Neither renderer can turn the canvas.
- Both renderers scale the canvas with nearest, each axis on its own. Every canvas of today has a scale of 1 or more on both axes. A turned canvas on a mode of 240 or 288 lines (a turned 15 kHz CRT or a TV in a game) puts 320 canvas columns on 240 or 288 lines: a scale of 0.75 or 0.9. Nearest then drops canvas columns, and a vertical stroke of a glyph one canvas pixel wide can vanish.
- Only the video side knows the screen orientation, the monitor kind and the mode, as for `ui_dest` (ADR 0005) and `scanlines` (ADR 0006). The renderer cannot choose the turn or the filter.

## Decision

1. `Scene` gets the field `ui_rotation: Rotation`: the turn, counter-clockwise, of the whole UI canvas before it is scaled into `ui_dest`. The pixel rule is the rule of `Scene::rotation` (ADR 0015, point 3), with the crop equal to the whole canvas: for a canvas of `w` by `h`, the turned canvas `T` is `w` by `h` for `R0` and `R180`, and `h` by `w` for `R90` and `R270`, and pixel `(tx, ty)` of `T` is the canvas pixel of the table of ADR 0015.
2. `Scene` gets the field `ui_filter: Filter`: how `T` is scaled into `ui_dest`. `Filter::Nearest` is the rule of ADR 0005, point 3. `Filter::SharpBilinear` is the rule of the game picture, per axis, with no scanlines.
3. `ui_dest` holds `T`, the canvas after the turn, in pixels of the display mode. For `R0` its meaning does not change.
4. The video side sets both fields. The UI turn is `(0 - S) mod 360`: `R0`, `R270`, `R180` and `R90` for a screen at 0, 90, 180 and 270. The filter is `Nearest` when the scale of each axis of `T` into `ui_dest` is 1 or more, and `SharpBilinear` when an axis is below 1. `neptron-modeline` gives the destination and the filter of a turned canvas as one pure function, `ui_dest_turned` (`docs/spec/tate.md`, section 6.4).
5. `neptron-types` gets the arithmetic of `Rotation` (`minus`, the quarter turns, `across`) and the type `Orientation` (the screen orientation and the game override), with the pure functions `game_turn`, `ui_turn` and `display_source`. `SourceGeometry` does not change: `display_source` is a copy with `rotation` set to the game turn.
6. The canvas is never darkened (ADR 0006), and it stays opaque (ADR 0005). The game picture and the canvas keep their own turns: a scene can show a game at `R0` under a canvas at `R270`.
7. The GLES renderer turns the canvas with the texture coordinates of the UI quad, as it turns the game quad (ADR 0015, point 6), and uses the per-axis maths of the game path for `SharpBilinear`. It draws no extra pass. `gles-compare` gets scenes for each `ui_rotation` (`R90`, `R180`, `R270`), with `Nearest` at a whole scale, with `SharpBilinear` below 1, over a game with `rotation` `R0` and over one with `R270`, a turned canvas clipped at the edge of the output, and a turned canvas over the texture limit (an error check). They must equal the reference renderer as today: bit exact for nearest, within 1 per channel for a blended pixel.
8. A side of `ui_dest` above `MAX_RECT_SIDE_PX` stays refused with `RenderError::RectTooLarge { rect: RectKind::UiDest, .. }`, for every turn.
9. The scene hash of the fake display in `neptron-testkit` includes `ui_rotation` and `ui_filter`.

## Consequences

- The UI turns with the screen with no change to `neptron-ui`: every screen, dialog, notice, prompt and the screensaver draw into the upright canvas as today. The menu directions need no turn, because the canvas is upright for the player.
- Every code that builds a `Scene` sets the two fields. `Scene` has no `Default`, so a missing value is a build error. All callers set `ui_rotation: Rotation::R0` and `ui_filter: Filter::Nearest` in the first commit, and every golden of today stays unchanged. The callers that build a `neptron_types::Scene` today:
  - `crates/neptron-video/src/scene.rs`, the engine's scene.
  - `crates/neptron-gfx`: the reference tests, the golden tests (`tests/common/scenes.rs`) and the scenes of `examples/gles_compare.rs`.
  - `crates/neptron-testkit/src/display/screen.rs`, `src/display/record.rs` and its integration tests.
  - `crates/neptron-kms/src/kms/mode.rs`: the black scene of a mode change. This is the KMS commit path, which is safety-critical; the change there is only the two fields with their `R0` and `Nearest` values, and nothing else.
  - `crates/neptron-kms/examples/kms_check.rs` and `crates/neptron-window/examples/smoke.rs`.
  - `apps/neptronctl/src/selftest/pattern.rs`.

  `neptron-ui` builds its own `UiScene`, not a `neptron_types::Scene`, and does not change.
- A turned 320x240 canvas is smaller on a turned 4:3 face than the canvas of today, and on a 15 kHz CRT or a TV it needs an interlaced menu mode or a soft scale. `docs/spec/tate.md`, section 6, gives the numbers. A portrait layout (a 240x320 canvas) is a later version; it needs no second change of these fields, because the canvas frame carries its own size and `ui_dest_turned` is written for a canvas of any size.
- `ui_filter` is the first filter of the canvas. Nothing outside a turned canvas sets `SharpBilinear` for it in this change.
- `Rotation` is exhaustive. A renderer matches all four values of `ui_rotation` with no wildcard arm.
- When the owner accepts this ADR, ADR 0015 gets a line in its Status: "Amended by ADR 0020: the UI canvas turns by `Scene::ui_rotation`." ADR 0005 gets the line: "Amended by ADR 0020: `ui_dest` holds the canvas after its turn, and `ui_filter` names its scaling."
- The GPU cost of a turned canvas is expected to be zero, as for the game picture. The work package checks the render time of a turned scene on the Pi with the existing bench build, and records it.
- Nothing on disk or on the wire changes. `Scene` borrows its frames and has no serde (ADR 0001).

## Alternatives

- One screen turn in `Scene` for the game and the canvas: rejected. `rotation` would then be the turn relative to the player, and every consumer of ADR 0015 (the policy, the scene builder, the scanline rule, Switchres' rotated flag) would change its reading. The game turn and the canvas turn are two values that the video side already knows, so two fields keep each consumer as it is.
- Turn the canvas on the CPU in `neptron-ui` (draw into a 240x320 buffer turned): rejected. It moves a decision of the video side into the UI, it needs a second buffer or a turned drawing path for every primitive, and the destination rule must still know the turn.
- A portrait layout first, with no turn in the renderer: rejected for version 1. It is a second layout of every screen, and on a flat panel and a 31 kHz CRT the turned canvas is already good. It stays the later answer for the 15 kHz CRT and the TV, and it uses `ui_rotation` too.
- Nearest at every scale: rejected. Below a scale of 1 it drops canvas columns, and text of one-pixel strokes loses strokes.
- A filter chosen by the renderer from the scale: rejected. The renderer does not decide (ADR 0005); the video side sets the field, as it sets `filter` for the game.
