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
use neptron_types::Scene;

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
