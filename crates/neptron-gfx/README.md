# neptron-gfx

Renderers for a `Scene`. `ReferenceRenderer` (CPU, pure Rust) defines the correct pixels. `GlesRenderer` (EGL, OpenGL ES 2) must match it.

## Public API sketch

- `struct ReferenceRenderer` with `render(&mut self, &Scene, &mut OutputBuffer) -> Result<(), RenderError>`
- `struct OutputBuffer { data: &mut [u8], width: u32, height: u32, pitch: usize }`, owned by the caller
- `enum RenderError`, with `Image`, `RectKind` and `MAX_RECT_SIDE_PX`. `ScanlineStrengthTooHigh` refuses a scanline strength above 100 %.
- `gles::GlesRenderer` (feature `gles`) with `headless() -> Result<Self, GlesError>`, `on_gbm(Rc<dyn GbmTarget>) -> Result<Self, GlesError>`, `render(&mut self, &Scene, &mut OutputBuffer) -> Result<(), GlesError>`, `draw_to_window(&mut self, &Scene, width, height) -> Result<(), GlesError>`, `gl_strings()`, `max_texture_side()` and `max_output_side()`
- `gles::GlesError`, `gles::MAX_AXIS_PRODUCT`, `gles::GbmTarget` (an unsafe trait: the GBM device and surface pointers)

## Reference renderer

The doc comment of `ReferenceRenderer` states every pixel rule. In short:

- Output: XRGB8888 as DRM defines it, little-endian, X byte `0xFF`. Every pixel of the output area is written. Row padding is not written. A pixel that shows nothing is black.
- Sources: XRGB8888, RGB565 and XRGB1555, native-endian. A 5-bit or 6-bit channel `v` becomes `round(v * 255 / max)`. X bits are ignored.
- Mapping: destination pixel `i` samples the source at `crop.x + (i + 1/2) * crop.w / dest.w`, an exact fraction. Each axis scales on its own. A sample outside the frame is black. A zero-size crop or destination shows no game.
- Nearest: the source pixel under the sample point. A point on an edge takes the right or the lower pixel.
- Sharp bilinear: the libretro `sharp-bilinear` formula with an integer prescale per axis. Taps clamp to the crop inside the frame. Each channel is the exact blend, rounded half up. At an integer scale it equals nearest.
- Scanlines: `scene.scanlines` darkens lines of the game picture, with the rule in the doc comment of `neptron_types::Scanlines` (ADR 0006). Only at a whole vertical scale `k = dest.h / crop.h` of at least 2. Destination line `j`, counted from `dest.y`, is dark when `j mod k >= k - d`, with `d = ScanlineKind::dark_lines(k)`: for black lines the last `floor(k / 2)` lines of each group (2x: 1 of 2, 3x: 1 of 3, 4x: 2 of 4). A dark channel is the exact value times `(100 - strength) / 100`, rounded half up, on the gamma-encoded 8-bit values: `(c * (100 - strength) + 50) / 100` for nearest, and the exact blend scaled before its one rounding for sharp bilinear. Borders, samples outside the frame and the UI canvas do not change. A fractional scale or a scale of 1 shows no scanlines, and `Scanlines::NONE` gives the output of a renderer with no scanline support.
- UI canvas: opaque, over the game. The whole canvas scales to `scene.ui_dest` with nearest scaling, each axis on its own, with the sample rule of the game (pixel centres, `floor`). A part outside the output is clipped. A zero-size `ui_dest` shows no UI. The renderer does not choose the rectangle: the video side sets it with `ui_dest_fill` (CRT) or `ui_dest_uniform` (flat panel) from `neptron-types`.
- Rectangles: a crop, destination or UI destination side above `MAX_RECT_SIDE_PX` (65 536) is refused with `RenderError::RectTooLarge`, so no position or size can make the renderer panic.
- Integer maths only, so every host gives the same bytes.
- No `unsafe`: the module carries `#[forbid(unsafe_code)]`.
- Allocation: two small tables of sample positions grow on the first call and on a larger output. Other calls allocate nothing.

## GLES renderer

`GlesRenderer` (feature `gles`) draws a `Scene` with EGL 1.5 and OpenGL ES 2. It must give the pixels of the reference renderer. The doc comment of `GlesRenderer` states the rules. In short:

- Two quads: the game frame into `dest`, then the UI canvas into `ui_dest`, over black. The UI quad has the whole canvas as the crop and nearest scaling.
- One fragment source, built once per filter at setup (two programs). `SHARP` is a preprocessor constant, not a uniform: the `vc4` compiler of the Raspberry Pi 3 turns a branch on a uniform into a select, so a single program ran the sharp bilinear path for every pixel.
- Each source image is a texture in the GL format of its pixel format, one texel per pixel, so the texture unit decodes the pixels and the shader has no decoder: `RGB565` as `RGB` with `UNSIGNED_SHORT_5_6_5`, `XRGB8888` as `BGRA_EXT` bytes (`GL_EXT_texture_format_BGRA8888`, which Mesa has on every driver; `headless` and `on_gbm` give `GlesError::MissingExtension` without it), and `XRGB1555` as `RGBA` with `UNSIGNED_SHORT_5_5_5_1`, each pixel moved one bit to the left on upload because GL has no layout with the X bit on top. GL converts a channel `v` of `b` bits to `v / (2^b - 1)`, which is the expansion of the reference. Rows that are not packed (a pitch above the row) and `XRGB1555` go through a buffer of the renderer, so one `glTexSubImage2D` call uploads each image. Native byte order must be little-endian; a big-endian build stops with a compile error.
- The fragment shader computes the source pixel of each output pixel from `gl_FragCoord` with the integer rules of `AxisMap::tap`: the sample position as an exact quotient and remainder, the prescale, the weight, and the far tap clamped to the crop inside the frame. It needs `highp` floats. Every value is an integer below 2^24, so a 32-bit float holds it exactly, and each division is corrected to the exact quotient.
- `MAX_AXIS_PRODUCT` (2^23): on each axis the source side times the destination side must not be above it, or `render` returns `GlesError::ScaleTooLarge`. 320 to 1920 is 614 400; 2048 to 4096 is the limit. The reference has no such limit.
- Scanlines: the renderer takes the pattern of dark lines (period, first dark line, kept percent) from the code of the reference and passes it to the shader as the uniform `u_scan`, with a period of 0 for none. The UI quad never has a pattern. The shader finds the line in its period from the destination line, with the exact `divmod`. A nearest pixel on a dark line becomes `(c * keep + 50) / 100` with an exact quotient, so it equals the reference. A blended pixel scales the float blend before its one rounding, as the reference does in exact maths, so it can differ by 1.
- Nearest output equals the reference byte for byte. A sharp bilinear blend is rounded half up in the shader, in floats; a blend whose exact value is within float error of a half can round the other way, so a channel can differ by 1.
- `headless()` checks with `glGetShaderPrecisionFormat` that a highp float of the fragment shader has at least 23 bits of precision, as an IEEE 32-bit float has. GLSL ES 1.00 only promises 16 bits. Less gives `GlesError::LowPrecision`. It also checks `GL_VERSION` before the GL loader reads it, because `glow` panics on a missing or unknown version string (`GlesError::GlVersion`).
- `render` clears old GL error flags first, so a check reports only the call it follows.
- `render` checks the scene with the checks of the reference first, so it refuses the same scenes with `GlesError::Scene`, and leaves the output unchanged after an error.
- Target: `headless()` makes a context on the Mesa surfaceless platform (`EGL_PLATFORM_SURFACELESS_MESA`) with no surface, draws into a framebuffer object and reads it back with `glReadPixels`. `on_gbm(target)` makes a context on the GBM platform (`EGL_PLATFORM_GBM_KHR`) with a window surface on the GBM surface that `neptron-kms` owns; it gets the two pointers through the unsafe trait `GbmTarget`, so no GBM type enters this crate, and it holds the `Rc` until it drops. `draw_to_window(scene, width, height)` draws a `width` by `height` output at the GL origin of the window surface and swaps. `render` and `draw_to_window` go through one private `draw` function (checks, scanline pattern, size limits, the two quads); only the target binding and the finish step differ (read back, or swap). A window surface stores the last GL row first, so the picture is in the last `height` rows of the GBM buffer; the KMS backend shows those rows. The draw code does not depend on the target: GL rows count from the bottom, and the shader maps output rows from the top. `GlesError::NoWindow` is the error of a window draw on a renderer with no window.
- EGL is loaded at run time from `libEGL.so.1` (`khronos-egl` with `dynamic`), and every GL function through `eglGetProcAddress` (`glow`). No build links EGL or GL, so `cargo xtask clippy -p neptron-gfx --features gles` and the unit tests of the module run on macOS too.
- Threads: the renderer is not `Send`. Each call makes its context current first, which replaces any other context that is current on the thread. Dropping the renderer releases the current context of the thread.
- All surfaceless renderers of a process share one EGL display. With `EGL_KHR_display_reference` each renderer holds a counted reference, and dropping one leaves the others working. Without it a renderer never calls `eglTerminate`, so dropping one renderer cannot break another, and the display stays initialized until the process ends. Mesa 26.1 does not have the extension, so the guest takes the second path.
- Allocation: the textures, the buffer that packs source rows, the framebuffer and the read-back buffer are made on the first call and again when a size or a pixel format changes. Other calls allocate nothing.
- `unsafe` is only in `src/gles/ffi.rs`. Every other module of `gles` carries `#[forbid(unsafe_code)]`.

The check against the reference is `cargo xtask gles-compare` (tier T3). It builds `examples/gles_compare.rs` for the guest and runs it in QEMU, where Mesa softpipe gives software GL. It renders every golden scene and more scenes (every value of each pixel format, both filters on every format, crops, a 1920x240 super-width mode with and without the UI, a flat panel with the UI, 1080p, the axis product limit, a UI larger than the mode, an empty scene, frames and canvases with a zero side, crops wholly outside the frame, a 1x1 source, 1x1 destinations, destinations wholly off the mode, and black scanlines: 240p line-doubled to 480p, 224 lines in 480p with sharp bilinear, 3x, 4x, every RGB565 value, a destination above the mode, a fractional scale, the UI over the game) with both renderers. One more check runs every scanline strength from 0 to 100 % on every 8-bit channel value. It also checks the error paths (the reference checks, a scanline strength above 100 %, the axis limit, the texture and output limits read from GL at run time), and that a second renderer can come and go while the first keeps drawing. A nearest or UI pixel must be equal, a sharp bilinear game pixel within 1 per channel, and no row padding byte may change. The exit code is the number of failed checks.

## Goldens

`tests/goldens.rs` renders the scenes of `tests/common/scenes.rs` and compares them with `tests/goldens/*.ppm` byte for byte. The format is binary PPM: the header `P6\n<width> <height>\n255\n`, then 3 bytes (R, G, B) per pixel, rows top to bottom. Most image viewers open it. The output X byte is not in the file; the test checks it separately.

The `scanlines_*` goldens cover 2x at 0, 50 and 100 %, 3x, 4x, a fractional scale, a scale of 1, a crop with a destination on an odd line, the UI over the game, and sharp bilinear, with each pixel format once. The test `scanline_strength_sheet` also writes `target/tmp/scanline-sheet/scanlines_2x_strength_0_50_100.ppm`: one 256x224 picture at 2x with black scanlines at 0, 50 and 100 %, side by side, to look at. It checks each panel with the pixel rule.

When a golden does not match or is missing, the test writes the rendered image to `target/tmp/goldens-actual/<name>.ppm` and prints the difference. To accept a new image, look at it first, then copy it to `tests/goldens/`. Never copy an image to get a green run.

## Must never depend on

- `neptron-kms`, `neptron-video`, `neptron-ui`
- EGL or GL crates outside the `gles` feature

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it, for the GLES code only. Every `unsafe` block has a `// SAFETY:` comment.
- Features: `gles`. No feature is on by default.

## Status

The reference renderer is complete. `GlesRenderer` matches it under Mesa softpipe in the QEMU guest; it is not checked on a real GPU yet. The KMS target (EGL on GBM) is checked on `vkms` by `cargo xtask kms-check`: its writeback picture equals the reference renderer, and equals `render` of the same scene with scanlines on.
