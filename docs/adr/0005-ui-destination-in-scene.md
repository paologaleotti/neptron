# 0005: The scene carries the UI destination rectangle

- Status: accepted
- Date: 2026-11-11
- Task: C01

## Context

The UI is drawn on a 320x240 canvas. Before this change, the reference renderer placed the canvas with its own rule: the largest whole scale that fits both axes, at least 1, centred. The renderer knows only the output size in pixels.

A GPU with a pixel clock floor reaches 240p with a super-width mode, for example 1920x240. There, the rule gives scale 1, and the menu is a strip 320 pixels wide in the middle of a 1920-pixel line. On the tube that is one sixth of the screen width.

The renderer cannot choose the right rule from the output size. A CRT in a 1920x240 mode needs a 6x horizontal and a 1x vertical stretch. A flat panel in 1920x1080 needs one scale for both axes. Only the video side knows the monitor kind and the horizontal strategy (native, dynamic super width, fixed super width). OP-011 records the question.

## Decision

1. `Scene` gets the field `ui_dest: Rect`, next to `dest`. It is the rectangle that the whole UI canvas goes to, in pixels of the display mode. A rectangle with a zero side shows no UI. The renderer ignores it when `ui` is `None`.
2. The video side sets `ui_dest`. On a CRT it is the full active area. On a flat panel it is a centred box with one whole scale for both axes. `neptron-types` gives the two rules as pure functions: `ui_dest_fill(mode_width, mode_height)` and `ui_dest_uniform(mode_width, mode_height, canvas_width, canvas_height)`. `ui_dest_uniform` is the rule that the renderer used before, so the output on a flat panel does not change.
3. The renderer does not choose the placement. It draws the canvas into `ui_dest` with nearest scaling, each axis on its own, clipped to the output. It uses the sample rule of the game path: pixel `i` of `ui_dest` shows canvas column `floor((i + 1/2) * ui.width / ui_dest.w)`, and the same for lines. The placement function of the renderer is removed.
4. A side of `ui_dest` above `MAX_RECT_SIDE_PX` is refused with `RenderError::RectTooLarge { rect: RectKind::UiDest, .. }` when the scene has a canvas, as for the game rectangles.
5. The canvas stays opaque. Transparency is not part of this change (OP-010 stays open).

## Consequences

- The UI fills the tube on a super-width CRT mode, and a flat panel keeps a sharp, uniform scale.
- Every code that builds a `Scene` sets `ui_dest`. `Scene` has no `Default`, so a missing rectangle is a build error, not a hidden wrong value.
- The OpenGL ES renderer draws the UI quad into `ui_dest` with nearest sampling, each axis on its own, and must match the reference renderer pixel for pixel. It does not compute a placement.
- The scene hash of the fake display in `neptron-testkit` now includes `ui_dest`, so a change of the rectangle changes the hash. Scene hash values from before this change are not comparable with new ones. No test pins such a value.
- Nothing on disk or on the wire changes. `Scene` borrows its frames and has no serde (ADR 0001). `Rect` keeps its serde shape.
- On a CRT, the 320x240 canvas in a super-width mode has wide pixels by design. A canvas of a different size per mode is a later question.

## Alternatives

- A scale per axis in `Scene` instead of a rectangle: rejected, because the video side also needs to set the position (centring, overscan), and a rectangle says both.
- The renderer picks the rule from the mode size (for example, a stretch when the width is much larger than the height): rejected, because a 1920x1080 flat panel and a 1920x240 CRT mode cannot be told apart by a fixed ratio rule, and the monitor kind is not known to the renderer.
- A `Default` for `Scene` with a zero `ui_dest`: rejected, because a caller that forgets the field then shows no UI with no build error.
