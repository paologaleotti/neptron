//! The built-in test picture, drawn at the size of the source as the core
//! draws its frame (before the turn of a vertical game).

use super::image::Rgb;

/// The background.
pub const BACKGROUND: [u8; 3] = [0x20, 0x20, 0x20];
/// The grid lines, every [`GRID_STEP_PX`] pixels.
pub const GRID: [u8; 3] = [0x48, 0x48, 0x48];
/// The 1-pixel border on the outermost pixels.
pub const OUTER_BORDER: [u8; 3] = [0xFF, 0x20, 0x20];
/// The second border, [`INNER_BORDER_INSET_PX`] pixels in.
pub const INNER_BORDER: [u8; 3] = [0x20, 0xFF, 0x20];
/// The centre cross.
pub const CROSS: [u8; 3] = [0xFF, 0xFF, 0xFF];
/// The circle that is round when the display aspect is right.
pub const CIRCLE: [u8; 3] = [0xFF, 0xE0, 0x20];
/// The line numbers at the left edge.
pub const NUMBERS: [u8; 3] = [0xFF, 0xFF, 0xFF];
/// The corner marks: top left, top right, bottom left, bottom right. Each
/// corner has its own colour, so a turn or a mirror shows.
pub const CORNERS: [[u8; 3]; 4] = [
    [0xFF, 0xFF, 0xFF],
    [0x20, 0xE0, 0xFF],
    [0xFF, 0x40, 0xFF],
    [0xFF, 0xA0, 0x20],
];

/// The distance between two grid lines, in source pixels.
pub const GRID_STEP_PX: u32 = 16;
/// The inset of the second border, in source pixels.
pub const INNER_BORDER_INSET_PX: u32 = 8;
/// A line number is drawn on every line that is a multiple of this.
pub const NUMBER_STEP_LINES: u32 = 8;
/// The left edge of the line numbers, in source pixels: right of the
/// second border, so the outer 8 columns stay free to count a crop.
pub const NUMBER_LEFT_PX: u32 = INNER_BORDER_INSET_PX + 2;
/// The length of each arm of a corner mark, in source pixels.
pub const CORNER_ARM_PX: u32 = 6;
/// The inset of the corner marks, in source pixels.
pub const CORNER_INSET_PX: u32 = 2;
/// The radius of the circle, in thousandths of the frame height.
pub const CIRCLE_RADIUS_PERMILLE: u32 = 400;

/// The test picture for a frame of `width` by `height` pixels whose display
/// aspect is `aspect_milli` (width over height, in thousandths; 0 for
/// square pixels).
pub fn test_pattern(width: u16, height: u16, aspect_milli: u32) -> Rgb {
    let (w, h) = (u32::from(width), u32::from(height));
    let mut img = Rgb::new(w, h, BACKGROUND);
    grid(&mut img);
    circle(&mut img, aspect_milli);
    cross(&mut img);
    rectangle(&mut img, 0, OUTER_BORDER);
    rectangle(&mut img, INNER_BORDER_INSET_PX, INNER_BORDER);
    corners(&mut img);
    numbers(&mut img);
    img
}

fn grid(img: &mut Rgb) {
    for y in 0..img.height {
        for x in 0..img.width {
            if x % GRID_STEP_PX == 0 || y % GRID_STEP_PX == 0 {
                img.put(x, y, GRID);
            }
        }
    }
}

fn cross(img: &mut Rgb) {
    let (cx, cy) = (img.width / 2, img.height / 2);
    for x in 0..img.width {
        img.put(x, cy, CROSS);
    }
    for y in 0..img.height {
        img.put(cx, y, CROSS);
    }
}

/// A 1-pixel rectangle `inset` pixels in from every edge.
fn rectangle(img: &mut Rgb, inset: u32, color: [u8; 3]) {
    let (w, h) = (img.width, img.height);
    if w <= 2 * inset || h <= 2 * inset {
        return;
    }
    let (right, bottom) = (w - 1 - inset, h - 1 - inset);
    for x in inset..=right {
        img.put(x, inset, color);
        img.put(x, bottom, color);
    }
    for y in inset..=bottom {
        img.put(inset, y, color);
        img.put(right, y, color);
    }
}

/// An ellipse that a screen of the right aspect shows as a circle: its
/// radius across is the radius down divided by the pixel aspect.
fn circle(img: &mut Rgb, aspect_milli: u32) {
    let (w, h) = (f64::from(img.width), f64::from(img.height));
    let pixel_aspect = match aspect_milli {
        0 => 1.0,
        a => f64::from(a) / 1000.0 * h / w,
    };
    let ry = h * f64::from(CIRCLE_RADIUS_PERMILLE) / 1000.0;
    let rx = ry / pixel_aspect;
    let (cx, cy) = (w / 2.0, h / 2.0);
    // Enough steps that two neighbours are at most one pixel apart.
    let steps = (8.0 * rx.max(ry)).ceil().max(8.0) as u32;
    for i in 0..steps {
        let a = f64::from(i) * std::f64::consts::TAU / f64::from(steps);
        let x = (cx + rx * a.cos()).floor();
        let y = (cy + ry * a.sin()).floor();
        if x >= 0.0 && y >= 0.0 {
            img.put(x as u32, y as u32, CIRCLE);
        }
    }
}

fn corners(img: &mut Rgb) {
    let (w, h) = (img.width, img.height);
    let need = CORNER_INSET_PX + CORNER_ARM_PX;
    if w < 2 * need || h < 2 * need {
        return;
    }
    let left = CORNER_INSET_PX;
    let right = w - 1 - CORNER_INSET_PX;
    let top = CORNER_INSET_PX;
    let bottom = h - 1 - CORNER_INSET_PX;
    for i in 0..CORNER_ARM_PX {
        img.put(left + i, top, CORNERS[0]);
        img.put(left, top + i, CORNERS[0]);
        img.put(right - i, top, CORNERS[1]);
        img.put(right, top + i, CORNERS[1]);
        img.put(left + i, bottom, CORNERS[2]);
        img.put(left, bottom - i, CORNERS[2]);
        img.put(right - i, bottom, CORNERS[3]);
        img.put(right, bottom - i, CORNERS[3]);
    }
}

/// The number of every 8th line, its top row on that line, so the first
/// number that shows on the screen tells how many lines are cut.
fn numbers(img: &mut Rgb) {
    let mut line = 0;
    while line < img.height {
        draw_number(img, NUMBER_LEFT_PX, line, line);
        line += NUMBER_STEP_LINES;
    }
}

/// The glyphs of the digits, 3 pixels wide and 5 high, one row per entry,
/// bit 2 the left column.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b011, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];
/// The distance from one digit to the next, in pixels.
const DIGIT_ADVANCE_PX: u32 = 4;

fn draw_number(img: &mut Rgb, x0: u32, y0: u32, value: u32) {
    let text = value.to_string();
    for (i, c) in text.bytes().enumerate() {
        let glyph = &DIGITS[usize::from(c - b'0')];
        let gx = x0 + i as u32 * DIGIT_ADVANCE_PX;
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    img.put(gx + col, y0 + row as u32, NUMBERS);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_borders_sit_on_the_edge_and_8_pixels_in() {
        let img = test_pattern(64, 48, 1_333);
        assert_eq!(img.get(0, 20), OUTER_BORDER);
        assert_eq!(img.get(63, 20), OUTER_BORDER);
        assert_eq!(img.get(20, 47), OUTER_BORDER);
        assert_eq!(img.get(8, 20), INNER_BORDER);
        assert_eq!(img.get(55, 20), INNER_BORDER);
        assert_eq!(img.get(20, 39), INNER_BORDER);
    }

    #[test]
    fn the_corners_have_their_own_colours() {
        let img = test_pattern(64, 48, 1_333);
        assert_eq!(img.get(3, 2), CORNERS[0]);
        assert_eq!(img.get(60, 2), CORNERS[1]);
        assert_eq!(img.get(3, 45), CORNERS[2]);
        assert_eq!(img.get(60, 45), CORNERS[3]);
    }

    #[test]
    fn every_8th_line_has_its_number() {
        let img = test_pattern(64, 48, 1_333);
        // "8" at line 8: its top row is 3 pixels wide.
        for x in NUMBER_LEFT_PX..NUMBER_LEFT_PX + 3 {
            assert_eq!(img.get(x, 8), NUMBERS, "x {x}");
        }
        // "16" at line 16: the "1" has its stem in the middle column.
        assert_eq!(img.get(NUMBER_LEFT_PX + 1, 17), NUMBERS);
        assert_eq!(img.get(NUMBER_LEFT_PX, 17), NUMBERS, "the flag of the 1");
    }

    #[test]
    fn the_circle_is_wider_in_pixels_when_the_pixels_are_narrow() {
        // 256x224 at 4:3: a pixel is 1.1667 wide, so the circle is 0.857
        // times as wide in pixels as it is high.
        let img = test_pattern(256, 224, 1_333);
        let row: Vec<u32> = (0..256)
            .filter(|&x| img.get(x, 112 + 1) == CIRCLE)
            .collect();
        let span = row.last().unwrap() - row.first().unwrap();
        let ry = 224.0 * 0.4;
        let expected = 2.0 * ry / (1.333 * 224.0 / 256.0);
        assert!(
            (f64::from(span) - expected).abs() <= 2.0,
            "{span} {expected}"
        );
    }

    #[test]
    fn a_tiny_frame_draws_what_fits() {
        let img = test_pattern(4, 4, 0);
        assert_eq!(img.get(0, 0), OUTER_BORDER);
        assert_eq!(img.rgb.len(), 4 * 4 * 3);
    }
}
