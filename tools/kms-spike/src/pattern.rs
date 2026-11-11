//! The test pattern, drawn by the CPU into an XRGB8888 buffer: a border on
//! the outermost pixels of the active area, a centre cross, colour bars and
//! the mode label.

/// Little-endian XRGB8888 white.
pub const WHITE: u32 = 0x00ff_ffff;
/// Little-endian XRGB8888 black.
pub const BLACK: u32 = 0x0000_0000;

/// The eight colour bars, left to right: white, yellow, cyan, green,
/// magenta, red, blue, black at 75 %.
pub const BARS: [u32; 8] = [
    0x00bf_bfbf,
    0x00bf_bf00,
    0x0000_bfbf,
    0x0000_bf00,
    0x00bf_00bf,
    0x00bf_0000,
    0x0000_00bf,
    0x0000_0000,
];

/// An XRGB8888 image in a byte buffer with a row pitch.
#[derive(Debug)]
pub struct Canvas<'a> {
    bytes: &'a mut [u8],
    pitch: usize,
    width: usize,
    height: usize,
}

/// The buffer cannot hold an image of the size asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooSmall {
    /// Bytes the image needs.
    pub needed: usize,
    /// Bytes the buffer has.
    pub len: usize,
}

/// The label of a mode: its name, its size and its field rate, for example
/// `240P 640X240 59.94HZ`.
pub fn label(name: &str, width: u16, height: u16, field_rate_mhz: u64) -> String {
    let centi = (field_rate_mhz + 5) / 10;
    format!(
        "{} {}X{} {}.{:02}HZ",
        name.to_ascii_uppercase(),
        width,
        height,
        centi / 100,
        centi % 100
    )
}

impl<'a> Canvas<'a> {
    /// A canvas of `width` by `height` pixels over `bytes`, whose rows are
    /// `pitch` bytes apart.
    pub fn over(
        bytes: &'a mut [u8],
        pitch: usize,
        width: usize,
        height: usize,
    ) -> Result<Canvas<'a>, TooSmall> {
        let needed = if height == 0 {
            0
        } else {
            pitch * (height - 1) + width * BYTES_PER_PIXEL
        };
        if pitch < width * BYTES_PER_PIXEL || bytes.len() < needed {
            return Err(TooSmall {
                needed: needed.max(pitch * height),
                len: bytes.len(),
            });
        }
        Ok(Canvas {
            bytes,
            pitch,
            width,
            height,
        })
    }

    /// The pixel at `x`, `y`, or `None` outside the canvas.
    #[cfg(test)]
    pub fn pixel(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = y * self.pitch + x * BYTES_PER_PIXEL;
        let b = self.bytes.get(at..at + BYTES_PER_PIXEL)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Fills a rectangle, clipped to the canvas.
    pub fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, colour: u32) {
        let bytes = colour.to_le_bytes();
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width) {
                let at = row * self.pitch + col * BYTES_PER_PIXEL;
                self.bytes[at..at + BYTES_PER_PIXEL].copy_from_slice(&bytes);
            }
        }
    }

    /// Draws the whole test pattern with `label` in the top left.
    pub fn draw_pattern(&mut self, label: &str) {
        let (w, h) = (self.width, self.height);
        self.fill(0, 0, w, h, BLACK);
        let bar_w = w / BARS.len();
        for (i, colour) in BARS.iter().enumerate() {
            let x = i * bar_w;
            let width = if i + 1 == BARS.len() { w - x } else { bar_w };
            self.fill(x, h / 3, width, h / 3, *colour);
        }
        let arm_w = w / 8;
        let arm_h = h / 8;
        self.fill(w / 2 - arm_w.min(w / 2), h / 2, 2 * arm_w, 1, WHITE);
        self.fill(w / 2, h / 2 - arm_h.min(h / 2), 1, 2 * arm_h, WHITE);
        self.fill(0, 0, w, 1, WHITE);
        self.fill(0, h.saturating_sub(1), w, 1, WHITE);
        self.fill(0, 0, 1, h, WHITE);
        self.fill(w.saturating_sub(1), 0, 1, h, WHITE);
        let sx = (w / LABEL_REFERENCE_W).max(1);
        let sy = (h / LABEL_REFERENCE_H).max(1);
        self.text(8 * sx, 8 * sy, sx, sy, label);
    }

    /// Draws `text` with the built-in 5x7 font, each font pixel `sx` by `sy`
    /// pixels. A character the font lacks is a filled box.
    pub fn text(&mut self, x: usize, y: usize, sx: usize, sy: usize, text: &str) {
        for (i, c) in text.chars().enumerate() {
            let left = x + i * (GLYPH_W + 1) * sx;
            let rows = glyph(c);
            for (r, bits) in rows.iter().enumerate() {
                for col in 0..GLYPH_W {
                    if bits & (1 << (GLYPH_W - 1 - col)) != 0 {
                        self.fill(left + col * sx, y + r * sy, sx, sy, WHITE);
                    }
                }
            }
        }
    }
}

const BYTES_PER_PIXEL: usize = 4;
const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;
/// The label scale is chosen for a 320x240 reference picture.
const LABEL_REFERENCE_W: usize = 320;
const LABEL_REFERENCE_H: usize = 240;

/// Rows of a 5x7 glyph, top first, bit 4 on the left.
fn glyph(c: char) -> [u8; GLYPH_H] {
    match c.to_ascii_uppercase() {
        ' ' => [0; 7],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        '.' => [0, 0, 0, 0, 0, 0x0c, 0x0c],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        _ => [0x1f; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 64;
    const H: usize = 48;
    const PITCH: usize = W * 4 + 16;

    fn drawn(label: &str) -> Vec<u8> {
        let mut bytes = vec![0xaa; PITCH * H];
        Canvas::over(&mut bytes, PITCH, W, H)
            .unwrap()
            .draw_pattern(label);
        bytes
    }

    fn at(bytes: &mut [u8], x: usize, y: usize) -> u32 {
        Canvas::over(bytes, PITCH, W, H)
            .unwrap()
            .pixel(x, y)
            .unwrap()
    }

    #[test]
    fn the_border_is_on_the_outermost_pixels() {
        let mut b = drawn("");
        for x in 0..W {
            assert_eq!(at(&mut b, x, 0), WHITE);
            assert_eq!(at(&mut b, x, H - 1), WHITE);
        }
        for y in 0..H {
            assert_eq!(at(&mut b, 0, y), WHITE);
            assert_eq!(at(&mut b, W - 1, y), WHITE);
        }
        assert_eq!(at(&mut b, 1, 1), BLACK);
        assert_eq!(at(&mut b, W - 2, H - 2), BLACK);
    }

    #[test]
    fn the_cross_is_at_the_centre() {
        let mut b = drawn("");
        assert_eq!(at(&mut b, W / 2, H / 2), WHITE);
        assert_eq!(at(&mut b, W / 2 - W / 8, H / 2), WHITE);
        assert_eq!(at(&mut b, W / 2, H / 2 - H / 8), WHITE);
    }

    #[test]
    fn the_bars_run_left_to_right() {
        let mut b = drawn("");
        let y = H / 3 + 1;
        for (i, colour) in BARS.iter().enumerate() {
            assert_eq!(at(&mut b, i * (W / 8) + 1, y), *colour, "bar {i}");
        }
    }

    #[test]
    fn padding_past_the_width_is_untouched() {
        let b = drawn("");
        assert!(b[W * 4..PITCH].iter().all(|&x| x == 0xaa));
    }

    #[test]
    fn the_label_is_drawn() {
        let mut plain = drawn("");
        let mut labelled = drawn("1");
        let differs = (8..8 + GLYPH_W)
            .flat_map(|x| (8..8 + GLYPH_H).map(move |y| (x, y)))
            .any(|(x, y)| at(&mut plain, x, y) != at(&mut labelled, x, y));
        assert!(differs);
    }

    #[test]
    fn every_label_character_has_a_glyph() {
        for c in label("480i", 640, 480, 59_940).chars() {
            assert_ne!(glyph(c), [0x1f; 7], "{c}");
        }
        assert_eq!(glyph('?'), [0x1f; 7]);
    }

    #[test]
    fn label_rounds_the_field_rate() {
        assert_eq!(label("240p", 640, 240, 59_940), "240P 640X240 59.94HZ");
        assert_eq!(label("480i", 640, 480, 119_995), "480I 640X480 120.00HZ");
    }

    #[test]
    fn the_last_row_needs_no_padding() {
        let mut bytes = vec![0; PITCH * (H - 1) + W * 4];
        assert!(Canvas::over(&mut bytes, PITCH, W, H).is_ok());
    }

    #[test]
    fn a_short_buffer_is_refused() {
        let mut bytes = vec![0; PITCH * (H - 1) + W * 4 - 1];
        assert!(Canvas::over(&mut bytes, PITCH, W, H).is_err());
        let mut bytes = vec![0; 100];
        assert!(Canvas::over(&mut bytes, 4, 2, 1).is_err());
    }

    #[test]
    fn drawing_past_the_edge_is_clipped() {
        let mut bytes = vec![0; 16];
        let mut c = Canvas::over(&mut bytes, 8, 2, 2).unwrap();
        c.fill(1, 1, 10, 10, WHITE);
        assert_eq!(c.pixel(1, 1), Some(WHITE));
        assert_eq!(c.pixel(0, 0), Some(0));
        assert_eq!(c.pixel(2, 0), None);
    }
}
