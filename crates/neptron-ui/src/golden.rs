//! The golden image format: binary PPM (`P6`), 8 bits per channel, the
//! header `P6\n<width> <height>\n255\n` and then RGB rows with no padding.
//! The unused byte of the XRGB canvas is not stored. Any image viewer and
//! most converters read it, and a test compares it byte for byte.
//!
//! This module works on bytes only; the caller reads and writes the files.

use core::fmt;

use crate::canvas::CpuCanvas;
use crate::geom::{Color, Rect};

/// The golden directory, relative to the directory of this crate.
pub const GOLDEN_DIR: &str = "tests/golden";
/// The file extension of a golden image.
pub const GOLDEN_EXTENSION: &str = "ppm";
/// The most differing pixels that a [`Diff`] lists one by one.
pub const DIFF_SAMPLES: usize = 8;

/// An RGB image, rows first, three bytes per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

/// An error of a PPM file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GoldenError {
    /// The file does not start with `P6`.
    #[error("not a binary PPM file: the first bytes are {found:?}, expected \"P6\"")]
    NotPpm {
        /// The first two bytes, as text.
        found: String,
    },
    /// A header field is missing or not a number.
    #[error("PPM header field {field} is missing or not a number")]
    Header {
        /// The field: `width`, `height` or `maxval`.
        field: &'static str,
    },
    /// The file uses a different bit depth.
    #[error("PPM maxval is {maxval}, expected 255")]
    Depth {
        /// The maxval of the file.
        maxval: u32,
    },
    /// The pixel data has the wrong length.
    #[error("a {width}x{height} PPM needs {expected} data bytes, the file has {actual}")]
    Length {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// The bytes needed.
        expected: u64,
        /// The bytes present.
        actual: usize,
    },
}

/// How two images differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diff {
    /// The sizes differ.
    Size {
        /// Width and height of the golden.
        expected: (u32, u32),
        /// Width and height of the new image.
        actual: (u32, u32),
    },
    /// Some pixels differ.
    Pixels {
        /// The number of differing pixels.
        count: u64,
        /// The smallest rectangle that holds every differing pixel.
        bounds: Rect,
        /// The first differing pixels, in row order, at most
        /// [`DIFF_SAMPLES`].
        samples: Vec<PixelDiff>,
    },
}

/// One differing pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelDiff {
    /// Column.
    pub x: u32,
    /// Row.
    pub y: u32,
    /// The golden colour.
    pub expected: Color,
    /// The new colour.
    pub actual: Color,
}

impl Image {
    /// The pixels of a canvas.
    pub fn from_canvas(canvas: &CpuCanvas) -> Self {
        use crate::canvas::Canvas;
        let (width, height) = (canvas.width(), canvas.height());
        let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
        for px in canvas
            .as_bytes()
            .chunks_exact(crate::canvas::BYTES_PER_PIXEL)
        {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        Self { width, height, rgb }
    }

    /// Reads a binary PPM file.
    pub fn from_ppm(bytes: &[u8]) -> Result<Self, GoldenError> {
        if bytes.get(..2) != Some(b"P6") {
            let found = String::from_utf8_lossy(bytes.get(..2).unwrap_or(bytes)).into_owned();
            return Err(GoldenError::NotPpm { found });
        }
        let mut rest = bytes.get(2..).unwrap_or(&[]);
        let width = header_field(&mut rest, "width")?;
        let height = header_field(&mut rest, "height")?;
        let maxval = header_field(&mut rest, "maxval")?;
        if maxval != 255 {
            return Err(GoldenError::Depth { maxval });
        }
        // Exactly one whitespace byte separates the header from the data.
        let data = rest.get(1..).unwrap_or(&[]);
        let expected = u64::from(width) * u64::from(height) * 3;
        if u64::try_from(data.len()).ok() != Some(expected) {
            return Err(GoldenError::Length {
                width,
                height,
                expected,
                actual: data.len(),
            });
        }
        Ok(Self {
            width,
            height,
            rgb: data.to_vec(),
        })
    }

    /// The image as a binary PPM file.
    pub fn to_ppm(&self) -> Vec<u8> {
        let header = format!("P6\n{} {}\n255\n", self.width, self.height);
        let mut out = Vec::with_capacity(header.len() + self.rgb.len());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&self.rgb);
        out
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The RGB bytes, rows first.
    pub fn rgb(&self) -> &[u8] {
        &self.rgb
    }

    /// The colour of one pixel, or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = (y as usize * self.width as usize + x as usize) * 3;
        let p = self.rgb.get(at..at + 3)?;
        Some(Color::rgb(p[0], p[1], p[2]))
    }

    /// How `actual` differs from this image, or `None` when every byte is
    /// the same.
    pub fn diff(&self, actual: &Self) -> Option<Diff> {
        if (self.width, self.height) != (actual.width, actual.height) {
            return Some(Diff::Size {
                expected: (self.width, self.height),
                actual: (actual.width, actual.height),
            });
        }
        if self.rgb == actual.rgb {
            return None;
        }
        let mut count = 0_u64;
        let mut samples = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..self.height {
            for x in 0..self.width {
                let (Some(expected), Some(got)) = (self.pixel(x, y), actual.pixel(x, y)) else {
                    continue;
                };
                if expected == got {
                    continue;
                }
                count += 1;
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                if samples.len() < DIFF_SAMPLES {
                    samples.push(PixelDiff {
                        x,
                        y,
                        expected,
                        actual: got,
                    });
                }
            }
        }
        let bounds = Rect::new(
            i32::try_from(x0).unwrap_or(i32::MAX),
            i32::try_from(y0).unwrap_or(i32::MAX),
            x1 - x0 + 1,
            y1 - y0 + 1,
        );
        Some(Diff::Pixels {
            count,
            bounds,
            samples,
        })
    }
}

impl fmt::Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size { expected, actual } => write!(
                f,
                "size differs: golden {}x{}, new {}x{}",
                expected.0, expected.1, actual.0, actual.1
            ),
            Self::Pixels {
                count,
                bounds,
                samples,
            } => {
                write!(
                    f,
                    "{count} pixels differ, inside x {}..{} y {}..{}",
                    bounds.x,
                    bounds.right(),
                    bounds.y,
                    bounds.bottom()
                )?;
                for s in samples {
                    write!(
                        f,
                        "\n  ({}, {}): golden #{:06x}, new #{:06x}",
                        s.x,
                        s.y,
                        s.expected.xrgb(),
                        s.actual.xrgb()
                    )?;
                }
                Ok(())
            }
        }
    }
}

fn header_field(rest: &mut &[u8], field: &'static str) -> Result<u32, GoldenError> {
    let start = rest
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .ok_or(GoldenError::Header { field })?;
    let digits = rest
        .get(start..)
        .unwrap_or(&[])
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    let text = rest.get(start..start + digits).unwrap_or(&[]);
    let value = core::str::from_utf8(text)
        .ok()
        .and_then(|t| t.parse().ok())
        .ok_or(GoldenError::Header { field })?;
    *rest = rest.get(start + digits..).unwrap_or(&[]);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas;

    fn canvas() -> CpuCanvas {
        let mut c = CpuCanvas::new();
        c.fill_rect(Rect::new(1, 2, 3, 4), Color::rgb(0x12, 0x34, 0x56));
        c
    }

    #[test]
    fn ppm_round_trip() {
        let image = Image::from_canvas(&canvas());
        let ppm = image.to_ppm();
        assert!(ppm.starts_with(b"P6\n320 240\n255\n"));
        assert_eq!(ppm.len(), 15 + 320 * 240 * 3);
        assert_eq!(Image::from_ppm(&ppm), Ok(image.clone()));
        assert_eq!(image.pixel(1, 2), Some(Color::rgb(0x12, 0x34, 0x56)));
        assert_eq!(image.pixel(320, 0), None);
    }

    #[test]
    fn broken_files_are_rejected() {
        assert_eq!(
            Image::from_ppm(b"P3\n1 1\n255\n"),
            Err(GoldenError::NotPpm { found: "P3".into() })
        );
        assert_eq!(
            Image::from_ppm(b"P6\nx 1\n255\n"),
            Err(GoldenError::Header { field: "width" })
        );
        assert_eq!(
            Image::from_ppm(b"P6\n1 1\n65535\n"),
            Err(GoldenError::Depth { maxval: 65535 })
        );
        assert_eq!(
            Image::from_ppm(b"P6\n1 1\n255\n\x01\x02"),
            Err(GoldenError::Length {
                width: 1,
                height: 1,
                expected: 3,
                actual: 2
            })
        );
        assert_eq!(
            Image::from_ppm(b""),
            Err(GoldenError::NotPpm {
                found: String::new()
            })
        );
    }

    #[test]
    fn equal_images_have_no_diff() {
        let a = Image::from_canvas(&canvas());
        assert_eq!(a.diff(&a.clone()), None);
    }

    #[test]
    fn a_diff_names_the_count_the_bounds_and_samples() {
        let a = Image::from_canvas(&canvas());
        let mut changed = canvas();
        changed.fill_rect(Rect::new(10, 20, 2, 1), Color::WHITE);
        changed.fill_rect(Rect::new(30, 25, 1, 1), Color::WHITE);
        let diff = a.diff(&Image::from_canvas(&changed)).expect("a diff");
        let Diff::Pixels {
            count,
            bounds,
            samples,
        } = &diff
        else {
            panic!("expected a pixel diff, got {diff:?}");
        };
        assert_eq!(*count, 3);
        assert_eq!(*bounds, Rect::new(10, 20, 21, 6));
        assert_eq!(samples.len(), 3);
        let text = diff.to_string();
        assert!(
            text.starts_with("3 pixels differ, inside x 10..31 y 20..26"),
            "{text}"
        );
        assert!(
            text.contains("(10, 20): golden #000000, new #ffffff"),
            "{text}"
        );
    }

    #[test]
    fn a_size_change_is_a_diff() {
        let a = Image::from_canvas(&canvas());
        let small = Image::from_ppm(b"P6\n1 1\n255\n\x00\x00\x00").expect("valid ppm");
        assert_eq!(
            a.diff(&small),
            Some(Diff::Size {
                expected: (320, 240),
                actual: (1, 1)
            })
        );
    }
}
