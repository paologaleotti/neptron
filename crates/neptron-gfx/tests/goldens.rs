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
use neptron_types::{Filter, PixelFormat, Scanlines, Scene};

#[path = "common/scenes.rs"]
mod scenes;

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
    golden("nearest_integer_scale");
}

#[test]
fn nearest_fractional_scale() {
    golden("nearest_fractional_scale");
}

#[test]
fn sharp_bilinear_fractional_scale() {
    golden("sharp_bilinear_fractional_scale");
}

#[test]
fn sharp_bilinear_downscale() {
    golden("sharp_bilinear_downscale");
}

#[test]
fn crop_outside_the_frame() {
    golden("crop_outside_the_frame");
}

#[test]
fn rgb565_crop() {
    golden("rgb565_crop");
}

#[test]
fn xrgb1555_crop() {
    golden("xrgb1555_crop");
}

#[test]
fn ui_over_the_game() {
    golden("ui_over_the_game");
}

#[test]
fn ui_fills_a_super_width_mode() {
    golden("ui_fills_a_super_width_mode");
}

#[test]
fn ui_fractional_scale_clipped() {
    golden("ui_fractional_scale_clipped");
}

#[test]
fn scanlines_2x_strength_100() {
    golden("scanlines_2x_strength_100");
}

#[test]
fn scanlines_2x_strength_50() {
    golden("scanlines_2x_strength_50");
}

#[test]
fn scanlines_2x_strength_0() {
    golden("scanlines_2x_strength_0");
}

#[test]
fn scanlines_3x_rgb565() {
    golden("scanlines_3x_rgb565");
}

#[test]
fn scanlines_4x_xrgb1555() {
    golden("scanlines_4x_xrgb1555");
}

#[test]
fn scanlines_fractional_scale() {
    golden("scanlines_fractional_scale");
}

#[test]
fn scanlines_1x_scale() {
    golden("scanlines_1x_scale");
}

#[test]
fn scanlines_crop_and_odd_offset() {
    golden("scanlines_crop_and_odd_offset");
}

#[test]
fn scanlines_under_the_ui() {
    golden("scanlines_under_the_ui");
}

#[test]
fn scanlines_sharp_bilinear() {
    golden("scanlines_sharp_bilinear");
}

/// Renders the case `name` of the scene list and compares it with its
/// golden.
fn golden(name: &str) {
    let case = scenes::golden_cases()
        .into_iter()
        .find(|c| c.name == name)
        .expect("the scene list has the case");
    check(name, &render(&case.scene(), case.width, case.height));
}

#[test]
fn scanline_cases_with_no_effect_equal_the_plain_scene() {
    for name in [
        "scanlines_2x_strength_0",
        "scanlines_fractional_scale",
        "scanlines_1x_scale",
    ] {
        let case = scenes::golden_cases()
            .into_iter()
            .find(|c| c.name == name)
            .expect("the scene list has the case");
        let plain = Scene {
            scanlines: Scanlines::NONE,
            ..case.scene()
        };
        let (w, h) = (case.width, case.height);
        assert!(
            render(&case.scene(), w, h) == render(&plain, w, h),
            "{name} differs from the scene with no scanlines"
        );
    }
}

/// A 256x224 game picture: colour bars, a grey ramp, a sky gradient and a
/// floor of tiles, with some hard edges. It only serves the sheet below.
fn game_picture() -> scenes::Source {
    scenes::Source::from_fn(256, 224, PixelFormat::Xrgb8888, |x, y| {
        let (x, y) = (u32::from(x), u32::from(y));
        let rgb = |r: u32, g: u32, b: u32| (r << 16) | (g << 8) | b;
        const BARS: [u32; 8] = [
            0xFFFFFF, 0xFFFF00, 0x00FFFF, 0x00FF00, 0xFF00FF, 0xFF0000, 0x0000FF, 0x000000,
        ];
        match y {
            0..32 => BARS[(x / 32) as usize],
            32..48 => rgb(x, x, x),
            48..144 => {
                let sun = (x as i32 - 180).pow(2) + (y as i32 - 90).pow(2) < 400;
                if sun {
                    rgb(255, 220, 64)
                } else {
                    rgb(40 + (y - 48), 80 + (y - 48), 200)
                }
            }
            _ => {
                let tile = ((x / 16) + (y / 16)) % 2 == 0;
                let edge = x % 16 == 0 || y % 16 == 0;
                match (edge, tile) {
                    (true, _) => rgb(20, 20, 20),
                    (false, true) => rgb(200, 120, 60),
                    (false, false) => rgb(150, 80, 40),
                }
            }
        }
    })
}

/// Renders the game picture at 2x with black scanlines at 0, 50 and 100 %,
/// checks each panel with the pixel rule, and writes the three side by side
/// to `scanline-sheet/scanlines_2x_strength_0_50_100.ppm` in the Cargo test
/// temporary directory, to look at.
#[test]
fn scanline_strength_sheet() {
    const GAP_PX: u32 = 16;
    let source = game_picture();
    let (w, h) = (512, 448);
    let panel = |strength_percent| {
        let case = scenes::Case::game(
            "sheet",
            game_picture(),
            scenes::rect(0, 0, 256, 224),
            scenes::rect(0, 0, w, h),
            Filter::Nearest,
            (w, h),
        )
        .with_black_lines(strength_percent);
        render(&case.scene(), w, h)
    };
    let panels = [panel(0), panel(50), panel(100)];
    let plain = scenes::Case::game(
        "plain",
        source,
        scenes::rect(0, 0, 256, 224),
        scenes::rect(0, 0, w, h),
        Filter::Nearest,
        (w, h),
    );
    assert!(panels[0] == render(&plain.scene(), w, h));
    let row = |image: &Rgb, y: u32| {
        let start = (y * w * 3) as usize;
        image.bytes[start..start + (w * 3) as usize].to_vec()
    };
    for y in (0..h).step_by(2) {
        let bright = row(&panels[0], y);
        assert_eq!(row(&panels[2], y), bright, "line {y} at 100 %");
        assert_eq!(row(&panels[1], y), bright, "line {y} at 50 %");
        assert!(
            row(&panels[2], y + 1).iter().all(|&c| c == 0),
            "line {} at 100 %",
            y + 1
        );
        let half: Vec<u8> = bright
            .iter()
            .map(|&c| ((u32::from(c) * 50 + 50) / 100) as u8)
            .collect();
        assert_eq!(row(&panels[1], y + 1), half, "line {} at 50 %", y + 1);
    }

    let sheet_w = 3 * w + 2 * GAP_PX;
    let mut bytes = Vec::with_capacity((sheet_w * h * 3) as usize);
    for y in 0..h {
        for (i, image) in panels.iter().enumerate() {
            if i > 0 {
                bytes.extend(std::iter::repeat_n(0x40, (GAP_PX * 3) as usize));
            }
            bytes.extend_from_slice(&row(image, y));
        }
    }
    let sheet = Rgb {
        width: sheet_w,
        height: h,
        bytes,
    };
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("scanline-sheet");
    fs::create_dir_all(&dir).expect("make the sheet directory");
    fs::write(
        dir.join("scanlines_2x_strength_0_50_100.ppm"),
        encode_ppm(&sheet),
    )
    .expect("write the sheet");
}

#[test]
fn every_golden_file_has_a_case() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens");
    let names: Vec<_> = scenes::golden_cases().iter().map(|c| c.name).collect();
    for entry in fs::read_dir(&dir).expect("read the goldens directory") {
        let file = entry.expect("a directory entry").file_name();
        let file = file.to_string_lossy();
        let Some(stem) = file.strip_suffix(".ppm") else {
            continue;
        };
        assert!(
            names.contains(&stem),
            "{file} has no case in the scene list"
        );
    }
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
