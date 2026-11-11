//! Pixel goldens of the reference renderer.
//!
//! Each golden is a binary PPM file (`P6`, maxval 255) in `tests/goldens`.
//! A test renders a scene and compares the RGB bytes exactly. On a
//! difference or a missing golden, the test writes the rendered image to
//! `goldens-actual/` in the Cargo test temporary directory and prints the
//! path.

#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use neptron_gfx::{OutputBuffer, ReferenceRenderer};
use neptron_types::{Filter, Frame, PixelFormat, Rect, Scene};

/// Extra bytes at the end of each source row, so that the goldens also
/// check the pitch.
const ROW_PADDING_BYTES: usize = 6;

/// A source image that owns its bytes.
struct Source {
    data: Vec<u8>,
    width: u16,
    height: u16,
    pitch: usize,
    format: PixelFormat,
}

impl Source {
    /// A test pattern: red grows to the right, blue grows down, and green
    /// alternates as a checkerboard. Every X bit that a format has is set
    /// on some pixels, so the goldens show that it is ignored.
    fn pattern(width: u16, height: u16, format: PixelFormat) -> Self {
        let bpp = format.bytes_per_pixel();
        let pitch = usize::from(width) * bpp + ROW_PADDING_BYTES;
        let mut data = vec![0x5A_u8; pitch * usize::from(height)];
        for y in 0..height {
            for x in 0..width {
                let r = ramp(x, width);
                let g = if (x + y) % 2 == 0 { 0xFF } else { 0x40 };
                let b = ramp(y, height);
                let x_bits = (x + 2 * y) % 3 == 0;
                let start = usize::from(y) * pitch + usize::from(x) * bpp;
                let px = &mut data[start..start + bpp];
                match format {
                    PixelFormat::Xrgb8888 => {
                        let x_byte = if x_bits { 0xA5 } else { 0 };
                        let v = u32::from_be_bytes([x_byte, r, g, b]);
                        px.copy_from_slice(&v.to_ne_bytes());
                    }
                    PixelFormat::Rgb565 => {
                        let v = (u16::from(r >> 3) << 11)
                            | (u16::from(g >> 2) << 5)
                            | u16::from(b >> 3);
                        px.copy_from_slice(&v.to_ne_bytes());
                    }
                    PixelFormat::Xrgb1555 => {
                        let x_bit = if x_bits { 0x8000 } else { 0 };
                        let v = x_bit
                            | (u16::from(r >> 3) << 10)
                            | (u16::from(g >> 3) << 5)
                            | u16::from(b >> 3);
                        px.copy_from_slice(&v.to_ne_bytes());
                    }
                }
            }
        }
        Self {
            data,
            width,
            height,
            pitch,
            format,
        }
    }

    fn frame(&self) -> Frame<'_> {
        Frame {
            data: &self.data,
            width: self.width,
            height: self.height,
            pitch: self.pitch,
            format: self.format,
        }
    }
}

fn ramp(i: u16, len: u16) -> u8 {
    (u32::from(i) * 255 / u32::from(len.max(2) - 1)) as u8
}

fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

fn game_scene(game: &Source, crop: Rect, dest: Rect, filter: Filter) -> Scene<'_> {
    Scene {
        game: Some(game.frame()),
        crop,
        dest,
        filter,
        ui: None,
    }
}

/// An RGB image, 3 bytes per pixel.
#[derive(PartialEq, Eq)]
struct Rgb {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

/// Renders `scene` into a `width` by `height` output with padded rows and
/// checks that the X byte is `0xFF` on every pixel.
fn render(scene: &Scene<'_>, width: u32, height: u32) -> Rgb {
    let pitch = width as usize * 4 + 8;
    let mut data = vec![0_u8; pitch * height as usize];
    let mut out = OutputBuffer {
        data: &mut data,
        width,
        height,
        pitch,
    };
    ReferenceRenderer::new()
        .render(scene, &mut out)
        .expect("the golden scene is valid");
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 3);
    for row in data.chunks_exact(pitch) {
        for px in row[..width as usize * 4].chunks_exact(4) {
            assert_eq!(px[3], 0xFF, "X byte of an output pixel");
            bytes.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    Rgb {
        width,
        height,
        bytes,
    }
}

fn encode_ppm(image: &Rgb) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
    out.extend_from_slice(&image.bytes);
    out
}

/// Parses the PPM files that [`encode_ppm`] writes: no comments, single
/// whitespace between the header fields.
fn decode_ppm(file: &[u8]) -> Result<Rgb, String> {
    let mut fields = Vec::new();
    let mut start = 0;
    for (i, &byte) in file.iter().enumerate() {
        if byte.is_ascii_whitespace() {
            fields.push(std::str::from_utf8(&file[start..i]).map_err(|e| e.to_string())?);
            start = i + 1;
            if fields.len() == 4 {
                break;
            }
        }
    }
    let [magic, width, height, maxval] = fields[..] else {
        return Err("the PPM header is incomplete".into());
    };
    if magic != "P6" || maxval != "255" {
        return Err(format!("not an 8-bit P6 PPM: {magic} with maxval {maxval}"));
    }
    let width: u32 = width.parse().map_err(|_| format!("bad width {width:?}"))?;
    let height: u32 = height
        .parse()
        .map_err(|_| format!("bad height {height:?}"))?;
    let bytes = file[start..].to_vec();
    if bytes.len() != width as usize * height as usize * 3 {
        return Err(format!("{} pixel bytes for {width}x{height}", bytes.len()));
    }
    Ok(Rgb {
        width,
        height,
        bytes,
    })
}

/// A report of the pixels that differ, or `None` when the images are equal.
fn difference(expected: &Rgb, actual: &Rgb) -> Option<String> {
    if (expected.width, expected.height) != (actual.width, actual.height) {
        return Some(format!(
            "size {}x{} differs from the golden size {}x{}",
            actual.width, actual.height, expected.width, expected.height
        ));
    }
    let mut report = String::new();
    let mut count = 0;
    let mut max_channel = 0;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
    let pixels = expected
        .bytes
        .chunks_exact(3)
        .zip(actual.bytes.chunks_exact(3));
    for (i, (e, a)) in pixels.enumerate() {
        if e == a {
            continue;
        }
        let (x, y) = (i as u32 % actual.width, i as u32 / actual.width);
        count += 1;
        (min_x, min_y, max_x, max_y) = (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y));
        for c in 0..3 {
            max_channel = max_channel.max(e[c].abs_diff(a[c]));
        }
        if count <= 10 {
            // Writing to a String cannot fail.
            let _ = writeln!(
                report,
                "  ({x}, {y}): golden #{:02X}{:02X}{:02X}, rendered #{:02X}{:02X}{:02X}",
                e[0], e[1], e[2], a[0], a[1], a[2]
            );
        }
    }
    (count > 0).then(|| {
        format!(
            "{count} pixels differ, in x {min_x} to {max_x} and y {min_y} to {max_y}; \
             largest channel difference {max_channel}\n{report}"
        )
    })
}

fn check(name: &str, image: &Rgb) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden_path = manifest.join("tests/goldens").join(format!("{name}.ppm"));
    let actual_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("goldens-actual");
    let actual_path = actual_dir.join(format!("{name}.ppm"));
    let fault = match fs::read(&golden_path) {
        Err(e) => Some(format!("cannot read {}: {e}", golden_path.display())),
        Ok(file) => match decode_ppm(&file) {
            Err(e) => Some(format!("{}: {e}", golden_path.display())),
            Ok(golden) => difference(&golden, image),
        },
    };
    if let Some(fault) = fault {
        fs::create_dir_all(&actual_dir).expect("make the directory of the rendered images");
        fs::write(&actual_path, encode_ppm(image)).expect("write the rendered image");
        panic!(
            "golden {name} does not match.\n{fault}\nThe rendered image is in {}. \
             Look at it before you copy it to {}.",
            actual_path.display(),
            golden_path.display()
        );
    }
}

#[test]
fn nearest_integer_scale() {
    let game = Source::pattern(8, 6, PixelFormat::Xrgb8888);
    let s = game_scene(&game, rect(0, 0, 8, 6), rect(4, 3, 24, 18), Filter::Nearest);
    check("nearest_integer_scale", &render(&s, 32, 24));
}

#[test]
fn nearest_fractional_scale() {
    let game = Source::pattern(10, 7, PixelFormat::Xrgb8888);
    let s = game_scene(
        &game,
        rect(0, 0, 10, 7),
        rect(1, 2, 29, 19),
        Filter::Nearest,
    );
    check("nearest_fractional_scale", &render(&s, 32, 24));
}

#[test]
fn sharp_bilinear_fractional_scale() {
    let game = Source::pattern(10, 7, PixelFormat::Xrgb8888);
    let s = game_scene(
        &game,
        rect(0, 0, 10, 7),
        rect(1, 2, 29, 19),
        Filter::SharpBilinear,
    );
    check("sharp_bilinear_fractional_scale", &render(&s, 32, 24));
}

#[test]
fn sharp_bilinear_downscale() {
    let game = Source::pattern(16, 12, PixelFormat::Xrgb8888);
    let s = game_scene(
        &game,
        rect(0, 0, 16, 12),
        rect(3, 2, 11, 9),
        Filter::SharpBilinear,
    );
    check("sharp_bilinear_downscale", &render(&s, 16, 12));
}

#[test]
fn crop_outside_the_frame() {
    let game = Source::pattern(8, 8, PixelFormat::Rgb565);
    let s = game_scene(
        &game,
        rect(-2, -1, 12, 10),
        rect(-5, 1, 40, 23),
        Filter::SharpBilinear,
    );
    check("crop_outside_the_frame", &render(&s, 32, 24));
}

#[test]
fn rgb565_crop() {
    let game = Source::pattern(8, 6, PixelFormat::Rgb565);
    let s = game_scene(&game, rect(1, 1, 6, 4), rect(2, 2, 12, 8), Filter::Nearest);
    check("rgb565_crop", &render(&s, 16, 12));
}

#[test]
fn xrgb1555_crop() {
    let game = Source::pattern(8, 6, PixelFormat::Xrgb1555);
    let s = game_scene(&game, rect(1, 1, 6, 4), rect(2, 2, 12, 8), Filter::Nearest);
    check("xrgb1555_crop", &render(&s, 16, 12));
}

#[test]
fn ui_over_the_game() {
    let game = Source::pattern(8, 6, PixelFormat::Xrgb8888);
    let ui = Source::pattern(16, 12, PixelFormat::Xrgb1555);
    let s = Scene {
        ui: Some(ui.frame()),
        ..game_scene(&game, rect(0, 0, 8, 6), rect(0, 0, 40, 30), Filter::Nearest)
    };
    check("ui_over_the_game", &render(&s, 40, 30));
}

#[test]
fn ppm_round_trip() {
    let image = Rgb {
        width: 2,
        height: 1,
        bytes: vec![1, 2, 3, 4, 5, 6],
    };
    let decoded = decode_ppm(&encode_ppm(&image)).expect("valid PPM");
    assert!(decoded == image);
    assert!(decode_ppm(b"P5\n1 1\n255\n\0").is_err());
    assert!(decode_ppm(b"P6\n1 1\n255\n\0").is_err());
}

#[test]
fn difference_reports_the_pixels() {
    let golden = Rgb {
        width: 2,
        height: 2,
        bytes: vec![0; 12],
    };
    let mut rendered = Rgb {
        bytes: vec![0; 12],
        ..golden
    };
    assert_eq!(difference(&golden, &rendered), None);
    rendered.bytes[9] = 3;
    let report = difference(&golden, &rendered).expect("a difference");
    assert!(
        report
            .starts_with("1 pixels differ, in x 1 to 1 and y 1 to 1; largest channel difference 3")
    );
    assert!(report.contains("(1, 1): golden #000000, rendered #030000"));
}
