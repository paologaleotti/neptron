# 0015: The scene carries the rotation of the game picture

- Status: accepted
- Date: 2026-11-16
- Task: M06 (decision); M08a builds it

## Context

A vertical arcade game (DoDonPachi, 240x320 upright) reaches the frontend on its side. FinalBurn Neo draws the frame as the board draws it, 320x240, and asks for a turn of 90 degrees counter-clockwise with `SET_ROTATION(1)`. It cannot send an upright frame to a horizontal screen. `docs/spec/arcade.md`, section 11, gives the details.

The parts of NEPTRON before the renderer already know rotation:

- `neptron_types::Rotation` (`R0`, `R90`, `R180`, `R270`, counter-clockwise, as `libretro.h` defines it) and `SourceGeometry::rotation`.
- The policy (`take_source` in `neptron-modeline`) swaps the axes of a source turned by 90 or 270 degrees, inverts its aspect, and places the upright picture. Its crop is in frame pixels: an overscan crop of a turned frame leaves out columns.

`Scene` has no rotation, and neither renderer can turn a picture. OP-047 says that the engine must therefore not show a turned game. The renderer cannot choose the turn: only the video side knows the source, the settings and later the screen orientation (as for `ui_dest`, ADR 0005, and `scanlines`, ADR 0006).

## Decision

1. `Scene` gets the field `rotation: Rotation`: the turn, counter-clockwise, of the cropped game picture before it is scaled into `dest`.
2. `crop` stays in pixels of the frame as the core draws it. `dest` stays in pixels of the display mode, and holds the picture after the turn.
3. The pixel rule, stated in the doc comment of the field. Let `crop` be `w` by `h`. The turned picture `T` is `w` by `h` for `R0` and `R180`, and `h` by `w` for `R90` and `R270`. Pixel `(tx, ty)` of `T` is the frame pixel `(crop.x + fx, crop.y + fy)` with:

   | Rotation | `fx` | `fy` |
   | --- | --- | --- |
   | `R0` | `tx` | `ty` |
   | `R90` | `w - 1 - ty` | `tx` |
   | `R180` | `w - 1 - tx` | `h - 1 - ty` |
   | `R270` | `ty` | `h - 1 - tx` |

   Every rule of the renderers then applies to `T` in place of the crop: the axis maps, nearest and sharp bilinear per axis, the test that a source pixel is inside the frame, and the black clear outside it. The turn is a permutation of pixels, so it adds no rounding and no tolerance.
4. Scanlines (ADR 0006) follow the lines of `T`: the whole vertical scale is `k = dest.h / T.h`, so `dest.h / crop.w` for `R90` and `R270`. The dark lines stay horizontal on the screen.
5. The UI canvas is never turned by this field. A turned UI (TATE) is a later decision.
6. The GLES renderer turns the picture with the texture coordinates of the game quad and swaps the axes of its per-axis maths for `R90` and `R270`. It draws no extra pass. `gles-compare` gets scenes for each rotation (`R90`, `R180` and `R270`: FBNeo asks for `R180` for a flipped horizontal board and `R270` for a flipped vertical one), with nearest, with sharp bilinear, with scanlines, and with a crop, and must equal the reference renderer as today (bit exact for nearest, within 1 per channel for a blended pixel).
7. The scene hash of the fake display in `neptron-testkit` includes the rotation.
8. `SourceGeometry::aspect_milli` is the aspect of the frame as the core draws it, as `take_source` (`crates/neptron-modeline/src/policy/source.rs`) already reads it: it swaps the axes of a source turned by 90 or 270 degrees and inverts the aspect. Its doc comment says so. FBNeo reports the aspect of the upright picture: its `retro_get_system_av_info` takes the driver's aspect from `BurnDrvGetAspect` with no swap when `fbneo-vertical-mode` is `disabled` (3:4 for DoDonPachi, whose frame is 320x240). So the session inverts the aspect for `R90` and `R270`, but only an aspect that the core reported. When the core gives 0 or less, the host's fallback (`crates/neptron-libretro/src/geometry.rs`) is the base width over the base height, which is already the shape of the frame as drawn, and the session keeps it. The host tells the two apart with `GameGeometry::aspect_from_core`, which M07a adds (`docs/spec/arcade.md`, sections 8.6 and 11).
9. The engine takes the rotation of the scene from the same `SourceGeometry` that the policy planned the mode with, so the placement and the turn always agree. A new rotation from the core is a new source, and goes through the engine's debounce like a new size.

## Consequences

- A vertical game shows upright on every display: on a flat panel at a whole factor, on a 31 kHz CRT at its own lines, on a 15 kHz CRT and on a TV at rung 3 (interlaced 1:1) or fitted, as `display.vertical` says.
- Every code that builds a `Scene` sets `rotation`. `Scene` has no `Default`, so a missing value is a build error. All callers in the workspace set `Rotation::R0` in the first commit, and every existing golden stays unchanged. The callers that build a `neptron_types::Scene` today:
  - `crates/neptron-video/src/scene.rs`, the engine's scene.
  - `crates/neptron-gfx`: the reference tests, the golden tests and the scenes of `examples/gles_compare.rs`.
  - `crates/neptron-testkit/src/display/screen.rs` and its integration tests.
  - `crates/neptron-kms/src/kms/mode.rs`: the black scene of a mode change. This is the KMS commit path, which is safety-critical; the change there is only the field `rotation: Rotation::R0`, and nothing else.
  - `crates/neptron-kms/examples/kms_check.rs` and `crates/neptron-window/examples/smoke.rs`.
  - `apps/neptronctl/src/selftest/pattern.rs`.

  `neptron-ui` builds its own `UiScene`, not a `neptron_types::Scene`, and does not change.
- `Rotation` is exhaustive. A renderer matches all four values with no wildcard arm.
- OP-047 loses its rotation limit when M08a is merged.
- The GPU cost of a turn is expected to be zero, because the quad and the shader stay the same. M08a checks the render time of a turned scene on the Pi with the existing bench build, and records it.
- Open for later, with no second change of this field: a `rotation` setting per game (auto, 0, 90, 180, 270) that overrides the core's turn, and TATE, where the turn is the core's turn less the screen's turn. TATE also needs a turn for the UI canvas, by a new ADR.

## Alternatives

- Turn the frame on the CPU before the renderer: rejected. A copy of every frame on the Pi 3 costs CPU time in the frame loop that the core needs, and it allocates or keeps a second buffer.
- Turn in the core: FinalBurn Neo cannot. MAME 2003-Plus can, at a CPU cost on the Pi, and it is not in the MVP.
- A rotation in `Frame` instead of `Scene`: rejected. `Frame` is a borrowed image that the UI canvas uses too, and the turn is a decision of the video side about the game picture, as the crop and the destination are.
- A general affine transform: rejected. Only quarter turns keep pixels exact, and two renderers must give the same bytes.
