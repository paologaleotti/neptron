//! RGB images for people to look at: enlarged shots, the contact sheet and
//! diff images.

use neptron_ui::font;
use neptron_ui::golden::Image;

const SHEET_COLUMNS: usize = 3;
const SHEET_GUTTER_PX: u32 = 8;
const LABEL_HEIGHT_PX: u32 = 14;
const SHEET_BACKGROUND: [u8; 3] = [0x30, 0x30, 0x30];
const LABEL_COLOR: [u8; 3] = [0xFF, 0xFF, 0xFF];
const DIFF_COLOR: [u8; 3] = [0xFF, 0x00, 0x00];

/// An RGB image, rows first, three bytes per pixel.
pub struct Rgb {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The pixels.
    pub rgb: Vec<u8>,
}

impl Rgb {
    fn new(width: u32, height: u32, fill: [u8; 3]) -> Self {
        let rgb = fill.repeat(width as usize * height as usize);
        Self { width, height, rgb }
    }

    fn put(&mut self, x: u32, y: u32, color: [u8; 3]) {
        if x < self.width && y < self.height {
            let at = (y as usize * self.width as usize + x as usize) * 3;
            self.rgb[at..at + 3].copy_from_slice(&color);
        }
    }

    fn paste(&mut self, image: &Image, x0: u32, y0: u32) {
        for y in 0..image.height() {
            for x in 0..image.width() {
                if let Some(c) = image.pixel(x, y) {
                    self.put(x0 + x, y0 + y, [c.r(), c.g(), c.b()]);
                }
            }
        }
    }

    fn text(&mut self, x0: u32, y0: u32, text: &str) {
        for (i, c) in text.chars().enumerate() {
            let glyph = font::glyph(c);
            let gx = x0 + i as u32 * font::ADVANCE;
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..font::GLYPH_WIDTH {
                    if bits & (0x80 >> col) != 0 {
                        self.put(gx + col, y0 + row as u32, LABEL_COLOR);
                    }
                }
            }
        }
    }
}

/// A copy of a golden image.
pub fn from_golden(image: &Image) -> Rgb {
    Rgb {
        width: image.width(),
        height: image.height(),
        rgb: image.rgb().to_vec(),
    }
}

/// The image enlarged `factor` times with nearest-neighbour sampling.
pub fn scale(image: &Image, factor: u32) -> Rgb {
    let factor = factor.max(1);
    let mut out = Rgb::new(image.width() * factor, image.height() * factor, [0; 3]);
    for y in 0..out.height {
        for x in 0..out.width {
            if let Some(c) = image.pixel(x / factor, y / factor) {
                out.put(x, y, [c.r(), c.g(), c.b()]);
            }
        }
    }
    out
}

/// Every tile under its name, in rows of three.
pub fn contact_sheet(tiles: &[(&str, Image)]) -> Rgb {
    let tile_w = tiles.iter().map(|(_, i)| i.width()).max().unwrap_or(0);
    let tile_h = tiles.iter().map(|(_, i)| i.height()).max().unwrap_or(0) + LABEL_HEIGHT_PX;
    let columns = tiles.len().clamp(1, SHEET_COLUMNS) as u32;
    let rows = tiles.len().div_ceil(SHEET_COLUMNS).max(1) as u32;
    let width = columns * tile_w + (columns + 1) * SHEET_GUTTER_PX;
    let height = rows * tile_h + (rows + 1) * SHEET_GUTTER_PX;
    let mut sheet = Rgb::new(width, height, SHEET_BACKGROUND);
    for (i, (name, image)) in tiles.iter().enumerate() {
        let col = (i % SHEET_COLUMNS) as u32;
        let row = (i / SHEET_COLUMNS) as u32;
        let x = SHEET_GUTTER_PX + col * (tile_w + SHEET_GUTTER_PX);
        let y = SHEET_GUTTER_PX + row * (tile_h + SHEET_GUTTER_PX);
        sheet.text(x, y + 1, name);
        sheet.paste(image, x, y + LABEL_HEIGHT_PX);
    }
    sheet
}

/// The golden at a quarter of its brightness, with every pixel that differs
/// in the new image marked in full red. Images of a different size show
/// the golden only.
pub fn diff(expected: &Image, actual: &Image) -> Rgb {
    let mut out = Rgb::new(expected.width(), expected.height(), [0; 3]);
    for y in 0..expected.height() {
        for x in 0..expected.width() {
            let Some(e) = expected.pixel(x, y) else {
                continue;
            };
            let color = if actual.pixel(x, y).is_some_and(|a| a != e) {
                DIFF_COLOR
            } else {
                [e.r() / 4, e.g() / 4, e.b() / 4]
            };
            out.put(x, y, color);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny(rgb: &[u8]) -> Image {
        let mut ppm = b"P6\n2 1\n255\n".to_vec();
        ppm.extend_from_slice(rgb);
        Image::from_ppm(&ppm).expect("valid ppm")
    }

    #[test]
    fn scaling_repeats_pixels() {
        let big = scale(&tiny(&[1, 2, 3, 4, 5, 6]), 2);
        assert_eq!((big.width, big.height), (4, 2));
        assert_eq!(&big.rgb[..12], &[1, 2, 3, 1, 2, 3, 4, 5, 6, 4, 5, 6]);
        assert_eq!(&big.rgb[..12], &big.rgb[12..]);
    }

    #[test]
    fn the_diff_marks_changed_pixels() {
        let d = diff(&tiny(&[40, 40, 40, 8, 8, 8]), &tiny(&[40, 40, 40, 9, 8, 8]));
        assert_eq!(d.rgb, [10, 10, 10, 0xFF, 0, 0]);
    }

    #[test]
    fn the_sheet_has_room_for_every_tile() {
        let tiles: Vec<_> = (0..4).map(|_| ("t", tiny(&[0; 6]))).collect();
        let sheet = contact_sheet(&tiles);
        assert_eq!(sheet.width, 3 * 2 + 4 * SHEET_GUTTER_PX);
        assert_eq!(
            sheet.height,
            2 * (1 + LABEL_HEIGHT_PX) + 3 * SHEET_GUTTER_PX
        );
        assert_eq!(sheet.rgb.len(), (sheet.width * sheet.height * 3) as usize);
    }
}
