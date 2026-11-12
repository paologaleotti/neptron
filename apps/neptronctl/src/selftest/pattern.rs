//! The test pattern of one mode, drawn by the CPU into an XRGB8888 frame:
//! a border on the outermost pixels, a centre cross, colour bars, and the
//! mode label and rates as text.

use neptron_types::{Filter, Frame, Modeline, PixelFormat, Rect, Scanlines, Scene};

/// The widest texture side that every supported GPU takes, in texels:
/// `GL_MAX_TEXTURE_SIZE` of vc4 on the Raspberry Pi. The GPUs of the
/// x86 targets take more. `KmsDisplay` does not report the value of the
/// GPU at hand, so the pattern keeps to the smallest one.
pub const TEXTURE_SIDE_MAX_TEXELS: u16 = 2_048;
/// Texels per frame pixel: the GLES renderer uploads each byte of a row as
/// one `LUMINANCE` texel, so an XRGB8888 pixel takes 4.
pub const TEXELS_PER_PIXEL: u16 = 4;
/// The widest frame the pattern draws, in pixels: one texture row. A
/// wider mode gets a frame narrower by a whole factor, which the renderer
/// scales back with nearest sampling. The source side times the
/// destination side then stays far below the GLES axis limit of 2^23
/// (512 times 3840).
pub const PATTERN_WIDTH_MAX_PX: u16 = TEXTURE_SIDE_MAX_TEXELS / TEXELS_PER_PIXEL;

/// Little-endian XRGB8888 white.
pub const WHITE: u32 = 0x00ff_ffff;
/// The eight colour bars, left to right: white, yellow, cyan, green,
/// magenta, red, blue, black, at 75 %.
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

/// The pattern of one mode: a frame and the scene that shows it on the
/// whole active area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pixels: Vec<u8>,
    width: u16,
    height: u16,
    mode_width: u16,
    mode_height: u16,
}

impl Pattern {
    /// Draws the pattern for `mode` with `lines` of text at the top left.
    /// It allocates the frame once; the display loop reuses it.
    pub fn draw(mode: &Modeline, lines: &[String]) -> Self {
        let width = mode.hactive.div_ceil(width_factor(mode.hactive));
        let height = mode.vactive;
        let mut pattern = Self {
            pixels: vec![0; usize::from(width) * usize::from(height) * BYTES_PER_PIXEL],
            width,
            height,
            mode_width: mode.hactive,
            mode_height: mode.vactive,
        };
        pattern.paint(lines);
        pattern
    }

    /// The scene that shows the frame on the whole active area of the mode,
    /// with nearest sampling and no UI.
    pub fn scene(&self) -> Scene<'_> {
        let full = |w: u16, h: u16| Rect {
            x: 0,
            y: 0,
            w: u32::from(w),
            h: u32::from(h),
        };
        Scene {
            game: Some(Frame {
                data: &self.pixels,
                width: self.width,
                height: self.height,
                pitch: usize::from(self.width) * BYTES_PER_PIXEL,
                format: PixelFormat::Xrgb8888,
            }),
            crop: full(self.width, self.height),
            dest: full(self.mode_width, self.mode_height),
            filter: Filter::Nearest,
            scanlines: Scanlines::NONE,
            ui: None,
            ui_dest: full(0, 0),
        }
    }

    /// The pixel at `x`, `y` of the frame, or `None` outside it.
    #[cfg(test)]
    pub fn pixel(&self, x: u16, y: u16) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = (usize::from(y) * usize::from(self.width) + usize::from(x)) * BYTES_PER_PIXEL;
        let b = self.pixels.get(at..at + BYTES_PER_PIXEL)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn paint(&mut self, lines: &[String]) {
        let (w, h) = (usize::from(self.width), usize::from(self.height));
        if w == 0 || h == 0 {
            return;
        }
        let bar_w = w / BARS.len();
        for (i, colour) in BARS.iter().enumerate() {
            let x = i * bar_w;
            let bar = if i + 1 == BARS.len() { w - x } else { bar_w };
            self.fill(x, h / 2 + h / 8, bar, h / 4, *colour);
        }
        let arm_w = w / 8;
        let arm_h = h / 8;
        self.fill(w / 2 - arm_w, h / 2, 2 * arm_w + 1, 1, WHITE);
        self.fill(w / 2, h / 2 - arm_h, 1, 2 * arm_h + 1, WHITE);
        self.fill(0, 0, w, 1, WHITE);
        self.fill(0, h - 1, w, 1, WHITE);
        self.fill(0, 0, 1, h, WHITE);
        self.fill(w - 1, 0, 1, h, WHITE);
        // The text has the size of a 320x240 picture on the mode; a frame
        // narrower than the mode by a factor holds it in fewer pixels.
        let factor = usize::from(self.mode_width).div_ceil(w);
        let sx = (usize::from(self.mode_width) / TEXT_REFERENCE_W_PX)
            .div_ceil(factor)
            .max(1);
        let sy = (h / TEXT_REFERENCE_H_PX).max(1);
        for (row, line) in lines.iter().enumerate() {
            let y = (TEXT_MARGIN_PX + row * (GLYPH_H + 2)) * sy;
            self.text(TEXT_MARGIN_PX * sx, y, sx, sy, line);
        }
    }

    /// Fills a rectangle, clipped to the frame.
    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, colour: u32) {
        let (fw, fh) = (usize::from(self.width), usize::from(self.height));
        let bytes = colour.to_le_bytes();
        for row in y..(y + h).min(fh) {
            for col in x..(x + w).min(fw) {
                let at = (row * fw + col) * BYTES_PER_PIXEL;
                self.pixels[at..at + BYTES_PER_PIXEL].copy_from_slice(&bytes);
            }
        }
    }

    /// Draws `text` with the 5x7 font, each font pixel `sx` by `sy` frame
    /// pixels. A character the font lacks is a filled box.
    fn text(&mut self, x: usize, y: usize, sx: usize, sy: usize, text: &str) {
        for (i, c) in text.chars().enumerate() {
            let left = x + i * (GLYPH_W + 1) * sx;
            for (r, bits) in glyph(c).iter().enumerate() {
                for col in 0..GLYPH_W {
                    if bits & (1 << (GLYPH_W - 1 - col)) != 0 {
                        self.fill(left + col * sx, y + r * sy, sx, sy, WHITE);
                    }
                }
            }
        }
    }
}

/// The whole factor by which the frame is narrower than a mode of `width`
/// pixels: the smallest that divides the width and brings it to at most
/// [`PATTERN_WIDTH_MAX_PX`], so the scale is exact. A width with no such
/// divisor gets the smallest factor that fits; the last frame column then
/// covers a little less than the others.
pub fn width_factor(width: u16) -> u16 {
    let fits = |k: u16| width.div_ceil(k) <= PATTERN_WIDTH_MAX_PX;
    let smallest = (1..=width.max(1)).find(|&k| fits(k)).unwrap_or(1);
    (smallest..=smallest.saturating_mul(2))
        .find(|&k| width.is_multiple_of(k) && fits(k))
        .unwrap_or(smallest)
}

/// The text lines of a mode: the family title, the size and scan, the
/// rates, and the pixel clock. Upper case, for the font.
pub fn label_lines(title: &str, m: &Modeline) -> Vec<String> {
    let line = m.line_rate_mhz();
    let field = m.field_rate_mhz();
    vec![
        title.to_ascii_uppercase(),
        format!(
            "{}X{}{}",
            m.hactive,
            m.vactive,
            if m.interlace { "I" } else { "P" }
        ),
        format!(
            "{}.{:02} KHZ {}.{:03} HZ",
            line / 1_000_000,
            (line / 10_000) % 100,
            field / 1_000,
            field % 1_000
        ),
        format!(
            "{}.{:03} MHZ",
            m.pclock_hz / 1_000_000,
            (m.pclock_hz / 1_000) % 1_000
        ),
    ]
}

const BYTES_PER_PIXEL: usize = 4;
const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;
/// The text scale is chosen for a 320x240 picture.
const TEXT_REFERENCE_W_PX: usize = 320;
const TEXT_REFERENCE_H_PX: usize = 240;
/// The text starts this far from the border, in font pixels.
const TEXT_MARGIN_PX: usize = 8;

/// Rows of a 5x7 glyph, top first, bit 4 on the left.
fn glyph(c: char) -> [u8; GLYPH_H] {
    match c.to_ascii_uppercase() {
        ' ' => [0; 7],
        '.' => [0, 0, 0, 0, 0, 0x0c, 0x0c],
        ':' => [0, 0x0c, 0x0c, 0, 0x0c, 0x0c, 0],
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
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1c, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1c],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0a, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        _ => [0x1f; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selftest::family::FAMILIES;

    const BLACK: u32 = 0x0000_0000;

    fn mode(w: u16, h: u16, interlace: bool) -> Modeline {
        Modeline {
            pclock_hz: 25_175_000,
            hactive: w,
            hsync_start: w + 16,
            hsync_end: w + 112,
            htotal: w + 160,
            vactive: h,
            vsync_start: h + 10,
            vsync_end: h + 12,
            vtotal: h + 45,
            interlace,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn the_border_is_on_the_outermost_pixels_of_all_four_edges() {
        let p = Pattern::draw(&mode(320, 240, false), &[]);
        for x in 0..320 {
            assert_eq!(p.pixel(x, 0), Some(WHITE));
            assert_eq!(p.pixel(x, 239), Some(WHITE));
        }
        for y in 0..240 {
            assert_eq!(p.pixel(0, y), Some(WHITE));
            assert_eq!(p.pixel(319, y), Some(WHITE));
        }
        assert_eq!(p.pixel(1, 1), Some(BLACK));
        assert_eq!(p.pixel(318, 238), Some(BLACK));
    }

    #[test]
    fn the_cross_is_at_the_centre() {
        let p = Pattern::draw(&mode(480, 480, false), &[]);
        assert_eq!(p.pixel(240, 240), Some(WHITE));
        assert_eq!(p.pixel(240 - 60, 240), Some(WHITE));
        assert_eq!(p.pixel(240 + 60, 240), Some(WHITE));
        assert_eq!(p.pixel(240, 240 - 60), Some(WHITE));
        assert_eq!(p.pixel(240, 240 + 60), Some(WHITE));
        assert_eq!(p.pixel(241, 241), Some(BLACK));
    }

    #[test]
    fn the_bars_run_left_to_right_below_the_centre() {
        let p = Pattern::draw(&mode(320, 240, false), &[]);
        let y = 240 / 2 + 240 / 8 + 1;
        for (i, colour) in BARS.iter().enumerate() {
            let x = u16::try_from(i * 40 + 5).unwrap();
            assert_eq!(p.pixel(x, y), Some(*colour), "bar {i}");
        }
    }

    #[test]
    fn the_label_is_drawn_inside_the_border() {
        let plain = Pattern::draw(&mode(320, 240, false), &[]);
        let labelled = Pattern::draw(&mode(320, 240, false), &["1".to_owned()]);
        let differs = (8..8 + 5u16)
            .flat_map(|x| (8..8 + 7u16).map(move |y| (x, y)))
            .any(|(x, y)| plain.pixel(x, y) != labelled.pixel(x, y));
        assert!(differs);
    }

    #[test]
    fn the_scene_fills_the_active_area() {
        let p = Pattern::draw(&mode(1280, 240, false), &[]);
        let scene = p.scene();
        let game = scene.game.unwrap();
        assert_eq!((game.width, game.height), (320, 240), "1280 / 4");
        assert_eq!(game.data.len(), 320 * 240 * 4);
        assert_eq!((scene.dest.w, scene.dest.h), (1280, 240));
        assert_eq!(scene.filter, Filter::Nearest);
        assert!(scene.ui.is_none());
    }

    #[test]
    fn a_wide_mode_gets_a_frame_narrower_by_a_whole_factor() {
        let p = Pattern::draw(&mode(3840, 240, false), &["X".to_owned()]);
        let scene = p.scene();
        assert_eq!(scene.game.unwrap().width, 480);
        assert_eq!(scene.dest.w, 3840);
        assert_eq!(p.pixel(479, 5), Some(WHITE), "the right border");
    }

    #[test]
    fn the_width_factor_keeps_the_frame_inside_one_texture_row() {
        assert_eq!(PATTERN_WIDTH_MAX_PX, 512);
        for (width, factor) in [
            (320, 1),
            (512, 1),
            (640, 2),
            (683, 2),
            (768, 2),
            (1280, 4),
            (1536, 3),
            (1920, 4),
            (3840, 8),
        ] {
            assert_eq!(width_factor(width), factor, "{width}");
        }
        for width in 1..=4096u16 {
            let k = width_factor(width);
            assert!(width.div_ceil(k) <= PATTERN_WIDTH_MAX_PX, "{width}");
            assert!(u32::from(width.div_ceil(k)) * u32::from(width) <= 1 << 23);
        }
    }

    #[test]
    fn an_interlaced_mode_draws_both_fields() {
        let p = Pattern::draw(&mode(480, 480, true), &[]);
        assert_eq!(p.scene().game.unwrap().height, 480);
        assert_eq!(p.pixel(10, 479), Some(WHITE));
    }

    #[test]
    fn every_label_character_has_a_glyph() {
        for family in &FAMILIES {
            for line in label_lines(family.title, &mode(1920, 576, true)) {
                for c in line.chars() {
                    assert_ne!(glyph(c), [0x1f; 7], "{c:?} in {line}");
                }
            }
        }
        assert_eq!(glyph('?'), [0x1f; 7]);
    }

    #[test]
    fn the_label_gives_size_scan_and_rates() {
        let m = mode(640, 480, false);
        assert_eq!(
            label_lines("480p", &m),
            ["480P", "640X480P", "31.46 KHZ 59.940 HZ", "25.175 MHZ"]
        );
    }
}
