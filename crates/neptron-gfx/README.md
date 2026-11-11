# neptron-gfx

Renderers for a `Scene`. `ReferenceRenderer` (CPU, pure Rust) defines the correct pixels. `GlesRenderer` (EGL, OpenGL ES 2) must match it.

## Public API sketch

- `struct ReferenceRenderer` with `render(&mut self, &Scene, &mut OutputBuffer) -> Result<(), RenderError>`
- `struct OutputBuffer { data: &mut [u8], width: u32, height: u32, pitch: usize }`, owned by the caller
- `enum RenderError`, with `Image`, `RectKind` and `MAX_RECT_SIDE_PX`
- `gles::GlesRenderer` (feature `gles`)

## Reference renderer

The doc comment of `ReferenceRenderer` states every pixel rule. In short:

- Output: XRGB8888 as DRM defines it, little-endian, X byte `0xFF`. Every pixel of the output area is written. Row padding is not written. A pixel that shows nothing is black.
- Sources: XRGB8888, RGB565 and XRGB1555, native-endian. A 5-bit or 6-bit channel `v` becomes `round(v * 255 / max)`. X bits are ignored.
- Mapping: destination pixel `i` samples the source at `crop.x + (i + 1/2) * crop.w / dest.w`, an exact fraction. Each axis scales on its own. A sample outside the frame is black. A zero-size crop or destination shows no game.
- Nearest: the source pixel under the sample point. A point on an edge takes the right or the lower pixel.
- Sharp bilinear: the libretro `sharp-bilinear` formula with an integer prescale per axis. Taps clamp to the crop inside the frame. Each channel is the exact blend, rounded half up. At an integer scale it equals nearest.
- UI canvas: opaque, over the game, at the largest integer scale that fits, centred.
- Integer maths only, so every host gives the same bytes.
- No `unsafe`: the module carries `#[forbid(unsafe_code)]`.
- Allocation: two small tables of sample positions grow on the first call and on a larger output. Other calls allocate nothing.

## Goldens

`tests/goldens.rs` renders small scenes and compares them with `tests/goldens/*.ppm` byte for byte. The format is binary PPM: the header `P6\n<width> <height>\n255\n`, then 3 bytes (R, G, B) per pixel, rows top to bottom. Most image viewers open it. The output X byte is not in the file; the test checks it separately.

When a golden does not match or is missing, the test writes the rendered image to `target/tmp/goldens-actual/<name>.ppm` and prints the difference. To accept a new image, look at it first, then copy it to `tests/goldens/`. Never copy an image to get a green run.

## Must never depend on

- `neptron-kms`, `neptron-video`, `neptron-ui`
- EGL or GL crates outside the `gles` feature

## Rules

- `unsafe` is allowed in this crate, in one module with a safe wrapper around it, for the GLES code only. Every `unsafe` block has a `// SAFETY:` comment.
- Features: `gles`. No feature is on by default.

## Status

The reference renderer is complete. `GlesRenderer` is not written yet.
