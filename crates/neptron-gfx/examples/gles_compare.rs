//! The tier T3 check of `GlesRenderer`: it renders every scene with the
//! GLES renderer and with the reference renderer, and compares the bytes.
//!
//! It needs EGL with the Mesa surfaceless platform, so it runs in the QEMU
//! guest: `cargo xtask gles-compare`. A game pixel drawn with nearest
//! scaling and every UI pixel must be equal. A game pixel drawn with sharp
//! bilinear may differ by 1 per channel. The exit code is the number of
//! failed checks.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use neptron_gfx::gles::{GlesError, GlesRenderer};
use neptron_gfx::{Image, OutputBuffer, RectKind, ReferenceRenderer, RenderError};
use neptron_types::{Filter, PixelFormat, Rect, Scanlines, Scene, ui_dest_fill, ui_dest_uniform};

#[path = "../tests/common/scenes.rs"]
mod scenes;

use scenes::{Case, Source, rect};

/// Extra bytes at the end of each output row. The renderers must not write
/// them.
const OUTPUT_PADDING_BYTES: usize = 12;
const PADDING: u8 = 0xAA;
/// The largest channel difference of a sharp bilinear game pixel.
const SHARP_TOLERANCE: u8 = 1;

fn main() -> ExitCode {
    let mut gles = match GlesRenderer::headless() {
        Ok(r) => r,
        Err(e) => {
            println!("FAIL cannot make the GLES renderer: {e}");
            return ExitCode::FAILURE;
        }
    };
    match gles.gl_strings() {
        Ok((renderer, version)) => println!("GL renderer: {renderer}; version: {version}"),
        Err(e) => println!("GL strings: {e}"),
    }
    let mut reference = ReferenceRenderer::new();
    let mut failed = 0_u8;
    let mut count = 0;
    let cases: Vec<Case> = scenes::golden_cases()
        .into_iter()
        .chain(extra_cases())
        .collect();
    for case in &cases {
        count += 1;
        if !compare(&mut gles, &mut reference, case) {
            failed = failed.saturating_add(1);
        }
    }
    // The first scene again: the renderer keeps no state from earlier sizes.
    count += 1;
    if !compare(&mut gles, &mut reference, &cases[0]) {
        failed = failed.saturating_add(1);
    }
    count += 1;
    if !second_renderer(&mut gles, &mut reference, &cases[..2]) {
        failed = failed.saturating_add(1);
    }
    for (name, ok) in error_checks(&mut gles) {
        count += 1;
        println!("{} {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            failed = failed.saturating_add(1);
        }
    }
    println!("gles-compare: {count} checks, {failed} failed");
    ExitCode::from(failed)
}

/// A second renderer shares the EGL display of the first. It draws, goes
/// away, and the first one must still draw. True when all of it holds.
fn second_renderer(
    gles: &mut GlesRenderer,
    reference: &mut ReferenceRenderer,
    cases: &[Case],
) -> bool {
    let mut second = match GlesRenderer::headless() {
        Ok(r) => r,
        Err(e) => {
            println!("FAIL a second renderer: {e}");
            return false;
        }
    };
    let ok = cases
        .first()
        .is_some_and(|c| compare(&mut second, reference, c));
    drop(second);
    let still = cases.get(1).is_some_and(|c| compare(gles, reference, c));
    println!(
        "{} the first renderer still draws after a second one is dropped",
        if ok && still { "ok  " } else { "FAIL" }
    );
    ok && still
}

/// Renders `case` with both renderers and prints one line. True when the
/// output is inside the tolerance.
fn compare(gles: &mut GlesRenderer, reference: &mut ReferenceRenderer, case: &Case) -> bool {
    let scene = case.scene();
    let expected = match render(case, |out| {
        reference.render(&scene, out).map_err(|e| e.to_string())
    }) {
        Ok(bytes) => bytes,
        Err(e) => {
            println!("FAIL {}: the reference refuses the scene: {e}", case.name);
            return false;
        }
    };
    let actual = match render(case, |out| {
        gles.render(&scene, out).map_err(|e| e.to_string())
    }) {
        Ok(bytes) => bytes,
        Err(e) => {
            println!("FAIL {}: {e}", case.name);
            return false;
        }
    };
    let report = Diff::of(case, &expected, &actual);
    let ok = report.within_tolerance();
    println!(
        "{} {}: {report}",
        if ok { "ok  " } else { "FAIL" },
        case.name
    );
    ok
}

fn render(
    case: &Case,
    draw: impl FnOnce(&mut OutputBuffer<'_>) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let pitch = case.width as usize * 4 + OUTPUT_PADDING_BYTES;
    let mut data = vec![PADDING; pitch * case.height as usize];
    let mut out = OutputBuffer {
        data: &mut data,
        width: case.width,
        height: case.height,
        pitch,
    };
    draw(&mut out)?;
    Ok(data)
}

/// The differences between two outputs of a case.
#[derive(Default)]
struct Diff {
    /// Pixels that differ.
    pixels: usize,
    /// Largest channel difference of a pixel that must be equal.
    exact_max: u8,
    /// Largest channel difference of a sharp bilinear game pixel.
    sharp_max: u8,
    /// Bytes outside the pixels (row padding, X bytes) that are wrong.
    bad_bytes: usize,
    first: Option<(u32, u32, [u8; 4], [u8; 4])>,
}

impl Diff {
    fn of(case: &Case, expected: &[u8], actual: &[u8]) -> Self {
        let pitch = case.width as usize * 4 + OUTPUT_PADDING_BYTES;
        let mut d = Diff::default();
        for y in 0..case.height {
            for x in 0..case.width {
                let at = y as usize * pitch + x as usize * 4;
                let (e, a) = (&expected[at..at + 4], &actual[at..at + 4]);
                if a[3] != 0xFF {
                    d.bad_bytes += 1;
                }
                let diff = (0..3).map(|c| e[c].abs_diff(a[c])).max().unwrap_or(0);
                if diff == 0 {
                    continue;
                }
                d.pixels += 1;
                if d.first.is_none() {
                    let px = |s: &[u8]| [s[0], s[1], s[2], s[3]];
                    d.first = Some((x, y, px(e), px(a)));
                }
                if is_sharp_game_pixel(case, x, y) {
                    d.sharp_max = d.sharp_max.max(diff);
                } else {
                    d.exact_max = d.exact_max.max(diff);
                }
            }
            let row_end = y as usize * pitch + case.width as usize * 4;
            let padding = &actual[row_end..row_end + OUTPUT_PADDING_BYTES];
            d.bad_bytes += padding.iter().filter(|&&b| b != PADDING).count();
        }
        d
    }

    fn within_tolerance(&self) -> bool {
        self.exact_max == 0 && self.sharp_max <= SHARP_TOLERANCE && self.bad_bytes == 0
    }
}

impl std::fmt::Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.pixels == 0 {
            write!(f, "equal")?;
        } else {
            write!(
                f,
                "{} pixels differ; max channel difference {} (exact pixels), {} (sharp bilinear)",
                self.pixels, self.exact_max, self.sharp_max
            )?;
        }
        if let Some((x, y, e, a)) = self.first {
            write!(
                f,
                "; first at ({x}, {y}): reference BGRX {e:02X?}, GLES {a:02X?}"
            )?;
        }
        if self.bad_bytes > 0 {
            write!(f, "; {} padding or X bytes are wrong", self.bad_bytes)?;
        }
        Ok(())
    }
}

/// True when output pixel `(x, y)` shows the game with sharp bilinear: the
/// filter is sharp bilinear, and the pixel is inside `dest` and not under
/// the UI.
fn is_sharp_game_pixel(case: &Case, x: u32, y: u32) -> bool {
    let inside = |r: Rect| {
        let (x, y) = (i64::from(x), i64::from(y));
        let (rx, ry) = (i64::from(r.x), i64::from(r.y));
        x >= rx && y >= ry && x < rx + i64::from(r.w) && y < ry + i64::from(r.h)
    };
    case.filter == Filter::SharpBilinear
        && case.game.is_some()
        && inside(case.dest)
        && !(case.ui.is_some() && inside(case.ui_dest))
}

/// Scenes beyond the goldens: every value of each pixel format, both
/// filters on every format, the crop, and full-size modes.
fn extra_cases() -> Vec<Case> {
    use Filter::{Nearest, SharpBilinear};
    use PixelFormat::{Rgb565, Xrgb1555, Xrgb8888};
    let pattern = Source::pattern;
    let every_value =
        |format| Source::from_fn(256, 256, format, |x, y| u32::from(y) * 256 + u32::from(x));
    let full = rect(0, 0, 256, 256);
    vec![
        Case::game(
            "xrgb8888_every_byte",
            Source::from_fn(256, 256, Xrgb8888, |x, y| {
                let (x, y) = (u32::from(x), u32::from(y));
                ((x + y) & 0xFF) << 24 | x << 16 | y << 8 | (x ^ y)
            }),
            full,
            full,
            Nearest,
            (256, 256),
        ),
        Case::game(
            "rgb565_every_value",
            every_value(Rgb565),
            full,
            full,
            Nearest,
            (256, 256),
        ),
        Case::game(
            "xrgb1555_every_value",
            every_value(Xrgb1555),
            full,
            full,
            Nearest,
            (256, 256),
        ),
        Case::game(
            "rgb565_sharp_bilinear_fractional_scale",
            pattern(10, 7, Rgb565),
            rect(0, 0, 10, 7),
            rect(1, 2, 29, 19),
            SharpBilinear,
            (32, 24),
        ),
        Case::game(
            "xrgb1555_sharp_bilinear_fractional_scale",
            pattern(10, 7, Xrgb1555),
            rect(0, 0, 10, 7),
            rect(1, 2, 29, 19),
            SharpBilinear,
            (32, 24),
        ),
        Case::game(
            "xrgb1555_nearest_downscale",
            pattern(16, 12, Xrgb1555),
            rect(0, 0, 16, 12),
            rect(3, 2, 11, 9),
            Nearest,
            (16, 12),
        ),
        Case::game(
            "sharp_bilinear_taps_clamp_to_the_crop",
            pattern(16, 12, Xrgb8888),
            rect(3, 2, 9, 7),
            rect(1, 1, 22, 17),
            SharpBilinear,
            (24, 20),
        ),
        Case::game(
            "rgb565_240p_on_a_super_width_mode",
            pattern(320, 240, Rgb565),
            rect(0, 0, 320, 240),
            rect(0, 0, 1920, 240),
            SharpBilinear,
            (1920, 240),
        ),
        Case::game(
            "xrgb8888_crop_on_a_super_width_mode",
            pattern(320, 240, Xrgb8888),
            rect(8, 8, 304, 224),
            rect(0, 0, 1920, 240),
            SharpBilinear,
            (1920, 240),
        ),
        Case::game(
            "ui_fills_a_1920x240_mode",
            pattern(320, 240, Rgb565),
            rect(0, 0, 320, 240),
            rect(0, 0, 1920, 240),
            Nearest,
            (1920, 240),
        )
        .with_ui(pattern(320, 240, Xrgb8888), ui_dest_fill(1920, 240)),
        Case::game(
            "xrgb1555_on_a_flat_panel_with_ui",
            pattern(256, 224, Xrgb1555),
            rect(0, 0, 256, 224),
            rect(23, 0, 653, 500),
            SharpBilinear,
            (700, 500),
        )
        .with_ui(
            pattern(320, 240, Rgb565),
            ui_dest_uniform(700, 500, 320, 240),
        ),
        Case::game(
            "rgb565_1080p_sharp_bilinear",
            pattern(320, 240, Rgb565),
            rect(0, 0, 320, 240),
            rect(240, 0, 1440, 1080),
            SharpBilinear,
            (1920, 1080),
        ),
        Case::game(
            "sharp_bilinear_downscale_of_240p",
            pattern(320, 240, Xrgb8888),
            rect(0, 0, 320, 240),
            rect(1, 1, 213, 161),
            SharpBilinear,
            (215, 163),
        ),
        Case::game(
            "nearest_at_the_axis_product_limit",
            pattern(2047, 4, Xrgb8888),
            rect(0, 0, 2047, 4),
            rect(0, 0, 4096, 4),
            Nearest,
            (4096, 4),
        ),
        Case::game(
            "sharp_bilinear_at_the_axis_product_limit",
            pattern(2047, 4, Xrgb8888),
            rect(0, 0, 2047, 4),
            rect(0, 0, 4096, 4),
            SharpBilinear,
            (4096, 4),
        ),
        Case {
            game: None,
            ..Case::game(
                "ui_larger_than_the_mode",
                pattern(1, 1, Xrgb8888),
                rect(0, 0, 0, 0),
                rect(0, 0, 0, 0),
                Nearest,
                (300, 200),
            )
        }
        .with_ui(
            pattern(320, 240, Xrgb1555),
            ui_dest_uniform(300, 200, 320, 240),
        ),
        Case {
            game: None,
            ..Case::game(
                "empty_scene",
                pattern(1, 1, Xrgb8888),
                rect(0, 0, 0, 0),
                rect(0, 0, 0, 0),
                Nearest,
                (8, 8),
            )
        },
        Case::game(
            "game_frame_of_zero_width",
            empty(0, 240, 640, Rgb565),
            rect(0, 0, 320, 240),
            rect(0, 0, 32, 24),
            Nearest,
            (32, 24),
        ),
        Case::game(
            "game_frame_of_zero_height",
            empty(320, 0, 0, Xrgb8888),
            rect(0, 0, 320, 240),
            rect(2, 2, 28, 20),
            SharpBilinear,
            (32, 24),
        ),
        Case::game(
            "ui_canvas_of_zero_width",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(0, 0, 32, 24),
            Nearest,
            (32, 24),
        )
        .with_ui(empty(0, 240, 640, Xrgb1555), rect(0, 0, 32, 24)),
        Case::game(
            "ui_canvas_of_zero_height",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(0, 0, 32, 24),
            Nearest,
            (32, 24),
        )
        .with_ui(empty(320, 0, 0, Rgb565), rect(0, 0, 32, 24)),
        Case::game(
            "nearest_crop_wholly_outside_the_frame",
            pattern(8, 6, Xrgb8888),
            rect(20, 10, 8, 6),
            rect(2, 2, 24, 18),
            Nearest,
            (32, 24),
        ),
        Case::game(
            "sharp_bilinear_crop_wholly_outside_the_frame",
            pattern(8, 6, Rgb565),
            rect(-30, -20, 8, 6),
            rect(2, 2, 25, 17),
            SharpBilinear,
            (32, 24),
        ),
        Case::game(
            "one_pixel_source",
            pattern(1, 1, Rgb565),
            rect(0, 0, 1, 1),
            rect(3, 2, 7, 5),
            SharpBilinear,
            (16, 12),
        ),
        Case::game(
            "nearest_one_pixel_dest",
            pattern(16, 12, Xrgb8888),
            rect(0, 0, 16, 12),
            rect(5, 5, 1, 1),
            Nearest,
            (16, 12),
        ),
        Case::game(
            "sharp_bilinear_one_pixel_dest",
            pattern(16, 12, Xrgb1555),
            rect(0, 0, 15, 11),
            rect(5, 5, 1, 1),
            SharpBilinear,
            (16, 12),
        ),
        Case::game(
            "dest_wholly_right_of_the_mode",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(40, 0, 16, 12),
            SharpBilinear,
            (32, 24),
        ),
        Case::game(
            "dest_wholly_above_the_mode",
            pattern(8, 6, Xrgb8888),
            rect(0, 0, 8, 6),
            rect(-20, -20, 16, 12),
            Nearest,
            (32, 24),
        ),
    ]
}

/// An image with no pixel bytes and the given size and pitch.
fn empty(width: u16, height: u16, pitch: usize, format: PixelFormat) -> Source {
    Source {
        data: Vec::new(),
        width,
        height,
        pitch,
        format,
    }
}

/// Scenes that the GLES renderer must refuse, with the error it must give.
fn error_checks(gles: &mut GlesRenderer) -> Vec<(&'static str, bool)> {
    let game = Source::pattern(8, 6, PixelFormat::Rgb565);
    let mut frame = game.frame();
    frame.pitch = 4;
    let short_pitch = Scene {
        game: Some(frame),
        crop: rect(0, 0, 8, 6),
        dest: rect(0, 0, 8, 6),
        filter: Filter::Nearest,
        scanlines: Scanlines::NONE,
        ui: None,
        ui_dest: rect(0, 0, 0, 0),
    };
    let too_large = Scene {
        game: Some(game.frame()),
        crop: rect(0, 0, 4097, 6),
        dest: rect(0, 0, 2048, 6),
        ..short_pitch
    };
    let mut data = vec![PADDING; 8 * 6 * 4];
    let mut out = OutputBuffer {
        data: &mut data,
        width: 8,
        height: 6,
        pitch: 32,
    };
    let refused = gles.render(&short_pitch, &mut out);
    let pitch_ok = refused
        == Err(GlesError::Scene(RenderError::PitchTooSmall {
            image: Image::Game,
            width: 8,
            pitch: 4,
            min: 16,
        }));
    let scale = gles.render(&too_large, &mut out);
    let scale_ok = scale
        == Err(GlesError::ScaleTooLarge {
            rect: RectKind::Dest,
            source_len: 4097,
            dest_len: 2048,
            limit: neptron_gfx::gles::MAX_AXIS_PRODUCT,
        });
    let mut untouched = data.iter().all(|&b| b == PADDING);
    let texture_ok = texture_limit(gles, &mut untouched);
    let output_ok = output_limit(gles, &mut untouched);
    vec![
        (
            "a pitch below one row is refused as the reference refuses it",
            pitch_ok,
        ),
        ("an axis product above the limit is refused", scale_ok),
        (
            "a row above GL_MAX_TEXTURE_SIZE bytes is refused",
            texture_ok,
        ),
        ("an output above the GL limits is refused", output_ok),
        ("a refused scene leaves the output unchanged", untouched),
    ]
}

/// A game frame one pixel wider than the texture limit allows, shown as
/// one pixel. True when the renderer refuses it with `TextureTooLarge`.
/// Clears `untouched` when the output changes.
fn texture_limit(gles: &mut GlesRenderer, untouched: &mut bool) -> bool {
    let limit = gles.max_texture_side();
    let Ok(width) = u16::try_from(limit / 4 + 1) else {
        println!("FAIL GL_MAX_TEXTURE_SIZE {limit} is too large for a frame wider than it");
        return false;
    };
    let game = Source::from_fn(width, 1, PixelFormat::Xrgb8888, |_, _| 0);
    let scene = Scene {
        game: Some(game.frame()),
        crop: rect(0, 0, 1, 1),
        dest: rect(0, 0, 1, 1),
        filter: Filter::Nearest,
        scanlines: Scanlines::NONE,
        ui: None,
        ui_dest: rect(0, 0, 0, 0),
    };
    let mut data = vec![PADDING; 8 * 6 * 4];
    let mut out = OutputBuffer {
        data: &mut data,
        width: 8,
        height: 6,
        pitch: 32,
    };
    let result = gles.render(&scene, &mut out);
    *untouched &= data.iter().all(|&b| b == PADDING);
    result
        == Err(GlesError::TextureTooLarge {
            image: Image::Game,
            width: u64::from(width) * 4,
            height: 1,
            limit,
        })
}

/// An output one pixel wider than the GL limit. True when the renderer
/// refuses it with `OutputTooLarge`. Clears `untouched` when the output
/// changes.
fn output_limit(gles: &mut GlesRenderer, untouched: &mut bool) -> bool {
    let limit = gles.max_output_side();
    let width = limit + 1;
    let scene = Scene {
        game: None,
        crop: rect(0, 0, 0, 0),
        dest: rect(0, 0, 0, 0),
        filter: Filter::Nearest,
        scanlines: Scanlines::NONE,
        ui: None,
        ui_dest: rect(0, 0, 0, 0),
    };
    let mut data = vec![PADDING; width as usize * 4];
    let mut out = OutputBuffer {
        data: &mut data,
        width,
        height: 1,
        pitch: width as usize * 4,
    };
    let result = gles.render(&scene, &mut out);
    *untouched &= data.iter().all(|&b| b == PADDING);
    result
        == Err(GlesError::OutputTooLarge {
            width,
            height: 1,
            limit,
        })
}
