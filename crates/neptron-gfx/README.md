# neptron-gfx

Renderers for a `Scene`. `ReferenceRenderer` (CPU, pure Rust) defines the correct pixels. `GlesRenderer` (EGL, OpenGL ES 2) must match it.

## Must never depend on

- `neptron-kms`, `neptron-video`, `neptron-ui`
- EGL or GL crates outside the `gles` feature

`unsafe` is allowed only in `src/gles/ffi.rs`, with a safe wrapper; every other module carries `#[forbid(unsafe_code)]`. Every `unsafe` block has a `// SAFETY:` comment. Features: `gles`, off by default.

## Public API sketch

- `ReferenceRenderer` with `render(&mut self, &Scene, &mut OutputBuffer) -> Result<(), RenderError>`
- `OutputBuffer { data: &mut [u8], width: u32, height: u32, pitch: usize }`, owned by the caller
- `RenderError`, `Image`, `RectKind`, `MAX_RECT_SIDE_PX`. `ScanlineStrengthTooHigh` refuses a strength above 100 %. `NoPixels` refuses a `GamePicture::Gpu` picture: the reference renderer reads only pixels in memory, and the `GlesRenderer` does not draw a GPU picture yet.
- `gles::GlesRenderer` (feature `gles`): `headless()`, `on_gbm(Rc<dyn GbmTarget>)`, `pbuffer(width, height)` and `read_window(&mut OutputBuffer)` for checks of the window path, `render(&Scene, &mut OutputBuffer)`, `draw_to_window(&Scene, width, height)`, `gl_strings()`, `max_texture_side()`, `max_output_side()`, and the counters `ui_rows_uploaded()`, `game_uploads()`, `game_parts_drawn()`. Errors are `GlesError`.
- `gles::MAX_AXIS_PRODUCT`, `gles::GbmTarget` (an unsafe trait: the GBM device and surface pointers)

## Reference renderer

The doc comment of `ReferenceRenderer` states every pixel rule. In short:

- Output: XRGB8888 as DRM defines it, little-endian, X byte `0xFF`. Every pixel of the output area is written; row padding is not. A pixel that shows nothing is black.
- Sources: XRGB8888, RGB565 and XRGB1555, native-endian. A 5-bit or 6-bit channel `v` becomes `round(v * 255 / max)`. X bits are ignored.
- Mapping: destination pixel `i` samples the source at `crop.x + (i + 1/2) * crop.w / dest.w`, an exact fraction. Each axis scales on its own. A sample outside the frame is black. A zero-size crop or destination shows no game.
- Nearest: the source pixel under the sample point. A point on an edge takes the right or the lower pixel.
- Sharp bilinear: the libretro `sharp-bilinear` formula with an integer prescale per axis. Taps clamp to the crop inside the frame. Each channel is the exact blend, rounded half up. At an integer scale it equals nearest.
- Turn: `scene.rotation` turns the cropped picture counter-clockwise before the scale (doc comment of `Scene::rotation`, ADR 0015). Every other rule applies to the turned picture `T`; for `R90` and `R270` its columns are the lines of the crop. An axis that runs against its frame axis is the crop of the mirrored frame axis (`reference/turn.rs`), so the other rules need no change. A turn adds no rounding. The UI canvas has its own turn, `scene.ui_rotation`.
- Scanlines (doc comment of `neptron_types::Scanlines`, ADR 0006): only at a whole vertical scale `k = dest.h / T.h` of at least 2, so the dark lines stay horizontal on the screen. Destination line `j`, counted from `dest.y`, is dark when `j mod k >= k - d`, with `d = ScanlineKind::dark_lines(k)` (black lines: 2x 1 of 2, 3x 1 of 3, 4x 2 of 4). A dark channel is `(c * (100 - strength) + 50) / 100` on the gamma-encoded 8-bit value for nearest; for sharp bilinear the exact blend is scaled before its one rounding. Borders, samples outside the frame and the UI canvas do not change. `Scanlines::NONE` gives the output of a renderer with no scanline support.
- UI canvas (ADR 0005, ADR 0020): opaque, over the game, turned whole by `scene.ui_rotation` with the pixel rule of the game turn (the whole canvas is the crop), then scaled to `scene.ui_dest` with `scene.ui_filter`, each axis on its own, with the sample rules of the game and no scanlines. The game and the canvas turn on their own. The video side sets `SharpBilinear` only when an axis of the turned canvas scales below 1. A part outside the output is clipped; a zero-size `ui_dest` shows no UI. The canvas has the size of its frame: 320x240 (landscape) or 240x320 (portrait), and the rules hold for any size (ADR 0021). The video side sets the rectangle with `ui_placement` of `neptron-modeline`, which uses `ui_dest_fill` (CRT) or `ui_dest_uniform` (flat panel) from `neptron-types`.
- A crop, destination or UI destination side above `MAX_RECT_SIDE_PX` (65 536) gives `RenderError::RectTooLarge`, so no rectangle can make the renderer panic.
- Integer maths only, so every host gives the same bytes.
- Two small tables of sample positions grow on the first call and on a larger output. Other calls allocate nothing.

## GLES renderer

`GlesRenderer` (feature `gles`) draws a `Scene` with EGL 1.5 and OpenGL ES 2 and must give the pixels of the reference. Its doc comment states the rules. These are the shader rules and the reasons for them:

- Two quads over black: the game frame into `dest`, then the UI canvas into `ui_dest` (whole canvas as crop, with `ui_rotation` and `ui_filter`, on the same path as the game quad).
- Four programs from one fragment source, one per pair of axes that blend. An axis blends only with sharp bilinear at a scale that is not whole. `BLEND_X` and `BLEND_Y` are preprocessor constants, not uniforms: the `vc4` compiler of the Raspberry Pi 3 turns a branch on a uniform into a select, so one program read four taps for every pixel.
- Textures are `BGRA_EXT` (`GL_EXT_texture_format_BGRA8888`; without it `headless` and `on_gbm` give `GlesError::MissingExtension`), one texel per pixel, so the shader has no decoder. Packed `XRGB8888` goes to GL as it is. `RGB565` and `XRGB1555` expand to 8 bits on upload with the reference's rule (a unit test checks all 65 536 values). A `UNSIGNED_SHORT_5_6_5` texture is not used: `vc4` expands by bit replication, which differs by 1 for 23 200 of the 65 536 `RGB565` values. The expansion loops index the slices, because the unoptimized bench build of `cargo xtask os push` does not inline the iterator adapters (3 ms against about 0.5 ms for a 256x224 frame on the Pi). `XRGB8888` rows with a larger pitch are packed in the same buffer, so one `glTexSubImage2D` uploads each image. Native byte order must be little-endian; a big-endian build is a compile error.
- The fragment shader computes the taps from the destination index with the integer rules of `AxisMap::tap` (exact quotient and remainder, prescale, weight, far tap clamped to the crop inside the frame), reads each tap with one `NEAREST` read and blends itself: two reads for one blending axis, four for two. It needs `highp` floats. Every value is an integer below 2^24 and each division is corrected to the exact quotient. The destination index comes from a varying, not from `gl_FragCoord`: `vc4` gives `gl_FragCoord` at the pixel corner, and Mesa flips its y for a window-system framebuffer and not for a framebuffer object.
- The texture unit's `LINEAR` filter is not used: on the Pi 3 it missed the tolerance by up to 5 (exact sample point) or 37 (interpolated varying), because `vc4` filters in fixed point.
- Turn: the quad and the uniforms `u_crop`, `u_axis`, `u_bounds` are on the axes of the turned picture, from the same `Turn` as the reference. `u_turn` gives each axis an integer offset and sign, `u_swap` swaps the axes for `R90` and `R270`; the shader applies them to each tap before its read. All values are below 2^16, so the turn is exact. No extra pass; the texture is not turned. The UI quad turns the same way with its own turn and filter.
- The game quad covers only the output pixels whose near source pixel is inside the frame (`AxisMap::inside`), so the clear leaves the others black.
- `MAX_AXIS_PRODUCT` (2^23): on each axis of the turned picture, source side times destination side above it gives `GlesError::ScaleTooLarge`. 320 to 1920 is 614 400; 2048 to 4096 is the limit. The reference has no such limit.
- Scanlines: the renderer takes the pattern (period, first dark line, kept percent) from the reference code and passes it as `u_scan`, period 0 for none. The UI quad has none. A nearest pixel on a dark line is `(c * keep + 50) / 100` exactly. A blended pixel scales the float blend before its one rounding, so it can differ by 1.
- Nearest output equals the reference byte for byte. A sharp bilinear channel can differ by 1, because a blend within float error of a half can round the other way.
- `headless()` refuses a fragment highp float with fewer than 23 bits of precision (`GlesError::LowPrecision`; GLSL ES 1.00 promises 16). It checks `GL_VERSION` before the GL loader reads it, because `glow` panics on a missing or unknown version string (`GlesError::GlVersion`).
- `render` clears old GL error flags first, then runs the scene checks of the reference (`GlesError::Scene`), and leaves the output unchanged after an error.
- Targets: `headless()` uses the Mesa surfaceless platform (`EGL_PLATFORM_SURFACELESS_MESA`) with no surface, draws into a framebuffer object and reads it back with `glReadPixels`. `on_gbm(target)` uses the GBM platform (`EGL_PLATFORM_GBM_KHR`) with a window surface on the GBM surface that `neptron-kms` owns. It gets the pointers through `GbmTarget`, so no GBM type enters this crate, and it holds the `Rc` until it drops. `draw_to_window` draws at the GL origin of the window surface and swaps; a window surface stores the last GL row first, so the picture is in the last `height` rows of the GBM buffer, which the KMS backend shows. `render` and `draw_to_window` share one private `draw`; only the target binding and the finish step differ. `GlesError::NoWindow` is a window draw on a renderer with no window.
- EGL loads at run time from `libEGL.so.1` (`khronos-egl` with `dynamic`), and GL through `eglGetProcAddress` (`glow`). No build links EGL or GL, so `cargo xtask clippy -p neptron-gfx --features gles` runs on macOS. The pure modules of `gles` also build in the default build, so `cargo xtask test -p neptron-gfx` runs their unit tests; only `ffi` and `renderer` need the feature.
- Threads: the renderer is not `Send`. Each call makes its context current first, which replaces any other current context of the thread. Dropping the renderer releases the current context of the thread.
- All surfaceless renderers of a process share one EGL display. With `EGL_KHR_display_reference` each holds a counted reference. Without it (Mesa 26.1) a renderer never calls `eglTerminate`, so dropping one cannot break another.
- Allocation: textures, the texel buffer, the framebuffer and the read-back buffer are made on the first call. A source texture and the texel buffer only grow; a smaller frame goes to the top left, and `u_frame` gives the storage size. So a game that switches sizes allocates once, at the first frame of its largest size (only the session knows the core's maximum geometry). The framebuffer and read-back buffer are made again when the output size changes. Other calls allocate nothing.
- The game under the UI: the game quad is drawn only in the parts outside the UI quad (at most four scissor rectangles), and not at all, with no upload, when the UI covers it. The pixels do not change. On the Pi 3 this took a game under the confirm dialog from 4.9 ms to 2.2 ms of GPU time a frame.
- UI uploads (a menu changes only on input or once a second): the renderer keeps a copy of the canvas rows and sends only the rows from the first to the last that differ; an unchanged canvas sends nothing. The texture holds the canvas upright, so the rows are canvas rows for every turn, and a change of the turn alone sends nothing. The game frame goes whole each call. New storage, a canvas of another size or format, and a failed upload send the whole canvas next time.

### The check against the reference

`cargo xtask gles-compare` (tier T3) builds `examples/gles_compare.rs` for the guest and runs it in QEMU with Mesa softpipe. It renders every golden scene and more (the list is in the example) with both renderers: every value of each pixel format, both filters, crops, super-width, flat panel and 1080p modes, the size limits, empty and zero-side scenes, crops and destinations outside the frame or mode, each turn with each format and filter, the UI canvas turned by each turn with nearest at a whole scale and with sharp bilinear at 0.75, over an upright and a turned game and clipped at the edges, the 240x320 portrait canvas at each `ui_rotation` (upright at 2x, upside down at 0.75, turned across 240 lines 1:1 and across 224 lines with sharp bilinear), black scanlines at each scale, and every scanline strength from 0 to 100 % on every 8-bit value. It also checks:

- the error paths (reference checks, strength above 100 %, the axis limit also on turned axes of the game and of the canvas, the texture and output limits read from GL, also for a turned canvas);
- that a second renderer can come and go while the first keeps drawing;
- that a UI canvas that changes in steps sends only the changed rows and still equals the reference, upright and turned;
- that a canvas of a new size goes whole: the landscape and the portrait canvas in turn send all their rows (`ui_size_change`);
- that a game under the UI is drawn only in the parts the UI leaves (none for a covered game, two for a UI in a corner, four for a UI in the middle rows, two for a turned UI in a corner). Two scenes put the UI over the game asymmetrically, so a scissor that counts rows from the wrong edge fails.

Every scene up to 1920x1920 also goes through the window path: `pbuffer` draws with `draw_to_window` into an EGL pbuffer, and `read_window` reads it back. A nearest pixel must be equal, a sharp bilinear game or UI pixel within 1 per channel, and no row padding byte may change. The exit code is the number of failed checks.

Softpipe treats both paths alike; only the target GPU shows a driver that does not. On the Raspberry Pi 3, copy `target/gles-compare/gles_compare` there and run it by hand; no tier does this. It passes there (`VC4 V3D 2.1`, Mesa 26.1.8) on both paths, except the two axis product cases, whose 4096-pixel output is above the 2048 limit of `vc4`. `cargo xtask kms-check` checks the GBM target on `vkms`: its writeback picture equals the reference, also with scanlines on.

## Goldens

`tests/goldens.rs` renders the scenes of `tests/common/scenes.rs` and compares them with `tests/goldens/*.ppm` byte for byte. The format is binary PPM: `P6\n<width> <height>\n255\n`, then R, G, B per pixel, rows top to bottom. The X byte is not in the file; the test checks it separately.

- `turn_*`: `R90` at 2x, with 2x scanlines and with a crop outside the frame (sharp bilinear), `R180` with a crop (RGB565), `R270` with sharp bilinear on both axes (XRGB1555). The pattern has red to the right and blue down, so each turn differs; `r90_shows_a_column_read_downward_as_a_line_read_left_to_right` pins the direction.
- `scanlines_*`: 2x at 0, 50 and 100 %, 3x, 4x, a fractional scale, a scale of 1, a crop with a destination on an odd line, the UI over the game, and sharp bilinear, each pixel format once. `scanline_strength_sheet` also writes `target/tmp/scanline-sheet/scanlines_2x_strength_0_50_100.ppm` to look at, and checks each panel with the pixel rule.

A golden that does not match or is missing makes the test write `target/tmp/goldens-actual/<name>.ppm` and print the difference. To accept a new image, look at it first, then copy it to `tests/goldens/`. Never copy an image to get a green run.
