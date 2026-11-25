//! The frame content of the test core and its hash.
//!
//! Each pixel is a fixed function of the frame number, the joypad buttons
//! that the core read for that frame, and the pixel position. A test that
//! knows these three values knows the frame, and so its hash.

#![forbid(unsafe_code)]

use crate::script::{PixelFormat, Size};

/// The colour value of one pixel, in the bits of `format`. The core writes it
/// in native byte order, as a `u32` for `Xrgb8888` and a `u16` for the 16-bit
/// formats.
pub fn pixel(format: PixelFormat, frame: u64, buttons: u16, x: u32, y: u32) -> u32 {
    // The low and high halves of the frame number both reach the value.
    let frame_lo = frame as u32;
    let frame_hi = (frame >> 32) as u32;
    let mut v = x.wrapping_mul(0x9E37_79B1)
        ^ y.wrapping_mul(0x85EB_CA77)
        ^ frame_lo.wrapping_mul(0xC2B2_AE3D)
        ^ frame_hi
        ^ u32::from(buttons).wrapping_mul(0x27D4_EB2F);
    v ^= v >> 15;
    v = v.wrapping_mul(0x2C1B_3C6D);
    v ^= v >> 12;
    v & format.colour_mask()
}

/// Writes the pixels of one frame into `buf`, row by row, `pitch` bytes
/// apart. Bytes outside the visible area stay as they are. Returns `None`
/// when the frame does not fit `buf` or a row does not fit `pitch`.
pub fn render(
    buf: &mut [u8],
    pitch: usize,
    size: Size,
    format: PixelFormat,
    frame: u64,
    buttons: u16,
) -> Option<()> {
    fill(buf, pitch, size, format, |x, y| {
        pixel(format, frame, buttons, x, y)
    })
}

/// The colour value of one pixel of the test card ([`Pattern::Shape`]), in
/// the bits of `format`, for a frame of `size` that the core turns by
/// `rotation` quarter turns (the `SET_ROTATION` value) to show it upright.
/// `aspect_milli` is the aspect that the core reports, the shape of the
/// upright picture, or 0 for square pixels. A rotation above 3 counts as 0.
///
/// The card is drawn on the upright picture: a border, a diagonal from
/// the top-left to the bottom-right corner, a circle that is round on the
/// glass, an arrow that points to the top, and a filled top-left corner.
///
/// [`Pattern::Shape`]: crate::Pattern::Shape
pub fn shape_pixel(
    format: PixelFormat,
    size: Size,
    aspect_milli: u32,
    rotation: u32,
    x: u32,
    y: u32,
) -> u32 {
    let card = Card::new(size, aspect_milli, rotation);
    let (ux, uy) = card.upright(x, y);
    encode(format, card.colour(ux, uy))
}

/// Writes the test card into `buf` as [`render`] writes a frame: the same
/// in every frame, see [`shape_pixel`].
pub fn render_shape(
    buf: &mut [u8],
    pitch: usize,
    size: Size,
    format: PixelFormat,
    aspect_milli: u32,
    rotation: u32,
) -> Option<()> {
    let card = Card::new(size, aspect_milli, rotation);
    fill(buf, pitch, size, format, |x, y| {
        let (ux, uy) = card.upright(x, y);
        encode(format, card.colour(ux, uy))
    })
}

/// The FNV-1a 64-bit hash of the visible bytes of a frame: `width` pixels of
/// each row, rows `pitch` bytes apart. Returns `None` when `data` is too short.
pub fn hash(data: &[u8], pitch: usize, size: Size, format: PixelFormat) -> Option<u64> {
    let row_bytes = visible_row_bytes(size, format)?;
    let mut h = FNV_OFFSET;
    for y in 0..usize::try_from(size.height).ok()? {
        let start = y.checked_mul(pitch)?;
        let row = data.get(start..start.checked_add(row_bytes)?)?;
        for &byte in row {
            h ^= u64::from(byte);
            h = h.wrapping_mul(FNV_PRIME);
        }
    }
    Some(h)
}

/// The hash of the frame that the core sends for this frame number and these
/// buttons. Returns `None` for a size beyond the limits of a script.
pub fn expected_hash(size: Size, format: PixelFormat, frame: u64, buttons: u16) -> Option<u64> {
    let pitch = visible_row_bytes(size, format)?;
    let mut buf = vec![0; pitch.checked_mul(usize::try_from(size.height).ok()?)?];
    render(&mut buf, pitch, size, format, frame, buttons)?;
    hash(&buf, pitch, size, format)
}

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

fn fill(
    buf: &mut [u8],
    pitch: usize,
    size: Size,
    format: PixelFormat,
    value: impl Fn(u32, u32) -> u32,
) -> Option<()> {
    let row_bytes = visible_row_bytes(size, format)?;
    if row_bytes > pitch {
        return None;
    }
    for y in 0..size.height {
        let start = usize::try_from(y).ok()?.checked_mul(pitch)?;
        let row = buf.get_mut(start..start.checked_add(row_bytes)?)?;
        for (x, px) in (0..size.width).zip(row.chunks_exact_mut(format.bytes_per_pixel())) {
            let value = value(x, y);
            match format {
                PixelFormat::Xrgb8888 => px.copy_from_slice(&value.to_ne_bytes()),
                PixelFormat::Rgb565 | PixelFormat::Rgb1555 => {
                    px.copy_from_slice(&u16::try_from(value).ok()?.to_ne_bytes());
                }
            }
        }
    }
    Some(())
}

/// The colours of the test card, `0xRRGGBB`.
const CARD_BACKGROUND: u32 = 0x20_20_20;
const CARD_BORDER: u32 = 0xFF_FF_FF;
const CARD_DIAGONAL: u32 = 0x00_FF_00;
const CARD_CIRCLE: u32 = 0xFF_00_00;
const CARD_ARROW: u32 = 0xFF_FF_00;
const CARD_CORNER: u32 = 0x00_00_FF;

/// `0xRRGGBB` in the bits of `format`.
fn encode(format: PixelFormat, rgb: u32) -> u32 {
    let (r, g, b) = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF);
    match format {
        PixelFormat::Xrgb8888 => rgb,
        PixelFormat::Rgb565 => ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3),
        PixelFormat::Rgb1555 => ((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3),
    }
}

/// The test card of one frame size. Every length is in pixels of the
/// upright picture; `aspect` is its display aspect in thousandths, so a
/// pixel is `aspect * height / (1000 * width)` lines wide on the glass.
struct Card {
    frame: Size,
    rotation: u32,
    width: i128,
    height: i128,
    aspect: i128,
}

impl Card {
    fn new(frame: Size, aspect_milli: u32, rotation: u32) -> Self {
        let rotation = if rotation > 3 { 0 } else { rotation };
        let (w, h) = if rotation % 2 == 1 {
            (frame.height, frame.width)
        } else {
            (frame.width, frame.height)
        };
        let (width, height) = (i128::from(w.max(1)), i128::from(h.max(1)));
        let aspect = match aspect_milli {
            0 => width * 1000 / height,
            a => i128::from(a),
        };
        Card {
            frame,
            rotation,
            width,
            height,
            aspect: aspect.max(1),
        }
    }

    /// The pixel of the upright picture that frame pixel `(x, y)` shows.
    /// The core asks for a counter-clockwise turn: at 1 the right column
    /// of the frame is the top row of the upright picture.
    fn upright(&self, x: u32, y: u32) -> (i128, i128) {
        let (x, y) = (i128::from(x), i128::from(y));
        let (w, h) = (i128::from(self.frame.width), i128::from(self.frame.height));
        match self.rotation {
            1 => (y, w - 1 - x),
            2 => (w - 1 - x, h - 1 - y),
            3 => (h - 1 - y, x),
            _ => (x, y),
        }
    }

    fn colour(&self, x: i128, y: i128) -> u32 {
        let (w, h, a) = (self.width, self.height, self.aspect);
        let line = (h / 60).max(1);
        if x < line || y < line || x >= w - line || y >= h - line {
            return CARD_BORDER;
        }
        // Distances on the glass, in thousandths of a line and doubled, so
        // that the pixel centres are whole numbers: a pixel step is
        // `a * h / w` across and 1000 down.
        let across = (2 * x + 1 - w) * a * h;
        let down = (2 * y + 1 - h) * 1000 * w;
        let arrow_tip = h / 20;
        let arrow_base = h / 8;
        let in_head =
            y >= arrow_tip && y < arrow_base && across.abs() * 5 <= 6_000 * w * (y - arrow_tip);
        let in_stem = y >= arrow_base && y < h / 6 && across.abs() * 40 <= 2_000 * w * h;
        if in_head || in_stem {
            return CARD_ARROW;
        }
        let radius = 3 * h * a.min(1000) / 10;
        let ring = (h * 1000 / 80).max(1000);
        let distance = across * across + down * down;
        let scale = 2 * w;
        let inner = (radius - ring).max(0) * scale;
        let outer = (radius + ring) * scale;
        if distance >= inner * inner && distance <= outer * outer {
            return CARD_CIRCLE;
        }
        if ((2 * y + 1) * w - (2 * x + 1) * h).abs() <= 2 * w.max(h) {
            return CARD_DIAGONAL;
        }
        if x < w / 6 && y < h / 6 {
            return CARD_CORNER;
        }
        CARD_BACKGROUND
    }
}

fn visible_row_bytes(size: Size, format: PixelFormat) -> Option<usize> {
    usize::try_from(size.width)
        .ok()?
        .checked_mul(format.bytes_per_pixel())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Size = Size {
        width: 4,
        height: 3,
    };

    #[test]
    fn pixels_use_only_the_colour_bits() {
        for format in [
            PixelFormat::Xrgb8888,
            PixelFormat::Rgb565,
            PixelFormat::Rgb1555,
        ] {
            for x in 0..64 {
                let v = pixel(format, 1234, 0xFFFF, x, 7);
                assert_eq!(v & !format.colour_mask(), 0, "{format:?}");
            }
        }
    }

    #[test]
    fn every_input_changes_the_frame() {
        let f = PixelFormat::Xrgb8888;
        let base = expected_hash(SIZE, f, 10, 0);
        assert_ne!(base, expected_hash(SIZE, f, 11, 0));
        assert_ne!(base, expected_hash(SIZE, f, 10 + (1 << 32), 0));
        for bit in 0..16 {
            assert_ne!(base, expected_hash(SIZE, f, 10, 1 << bit), "button {bit}");
        }
    }

    #[test]
    #[cfg(target_endian = "little")]
    fn hash_is_stable() {
        // Pinned values for a little-endian host: a change here changes every
        // recorded test core hash.
        assert_eq!(pixel(PixelFormat::Xrgb8888, 0, 0, 0, 0), 0);
        assert_eq!(pixel(PixelFormat::Xrgb8888, 1, 0, 0, 0), 0x00B3_7FD9);
        assert_eq!(
            expected_hash(SIZE, PixelFormat::Xrgb8888, 1, 0),
            Some(0x2FFE_12A9_BFC1_C10C)
        );
        assert_eq!(
            expected_hash(SIZE, PixelFormat::Rgb565, 1, 0x0101),
            Some(0x853D_655D_6F1D_3395)
        );
    }

    #[test]
    fn hash_ignores_the_padding_of_a_row() {
        let format = PixelFormat::Rgb565;
        let pitch = 16;
        let mut a = vec![0xA5; pitch * 3];
        let mut b = vec![0x00; pitch * 3];
        render(&mut a, pitch, SIZE, format, 5, 3).unwrap();
        render(&mut b, pitch, SIZE, format, 5, 3).unwrap();
        assert_ne!(a, b);
        assert_eq!(hash(&a, pitch, SIZE, format), hash(&b, pitch, SIZE, format));
        assert_eq!(
            hash(&a, pitch, SIZE, format),
            expected_hash(SIZE, format, 5, 3)
        );
        assert_eq!(a[8..16], [0xA5; 8], "render leaves the padding alone");
    }

    #[test]
    fn short_buffers_are_rejected() {
        let format = PixelFormat::Xrgb8888;
        let mut buf = vec![0; 47];
        assert_eq!(render(&mut buf, 16, SIZE, format, 0, 0), None);
        assert_eq!(hash(&buf, 16, SIZE, format), None);
        let mut buf = vec![0; 48];
        assert_eq!(
            render(&mut buf, 15, SIZE, format, 0, 0),
            None,
            "pitch too small"
        );
        assert!(render(&mut buf, 16, SIZE, format, 0, 0).is_some());
    }

    const XRGB: PixelFormat = PixelFormat::Xrgb8888;

    fn size(width: u32, height: u32) -> Size {
        Size { width, height }
    }

    /// The frame pixel that shows upright pixel `(ux, uy)` of a frame of
    /// `frame` turned by `rotation`.
    fn frame_of(frame: Size, rotation: u32, ux: u32, uy: u32) -> (u32, u32) {
        let (w, h) = (frame.width, frame.height);
        match rotation {
            1 => (w - 1 - uy, ux),
            2 => (w - 1 - ux, h - 1 - uy),
            3 => (uy, h - 1 - ux),
            _ => (ux, uy),
        }
    }

    #[test]
    fn the_card_is_the_same_upright_picture_at_every_turn() {
        for rotation in 0..4 {
            let frame = if rotation % 2 == 1 {
                size(320, 240)
            } else {
                size(240, 320)
            };
            for uy in 0..320 {
                for ux in 0..240 {
                    let (x, y) = frame_of(frame, rotation, ux, uy);
                    assert_eq!(
                        shape_pixel(XRGB, frame, 750, rotation, x, y),
                        shape_pixel(XRGB, size(240, 320), 750, 0, ux, uy),
                        "turn {rotation} at {ux},{uy}"
                    );
                }
            }
        }
        assert_eq!(
            shape_pixel(XRGB, size(320, 240), 750, 9, 7, 7),
            shape_pixel(XRGB, size(320, 240), 750, 0, 7, 7),
            "a turn above 3 counts as 0"
        );
    }

    #[test]
    fn the_card_marks_the_top_left_corner_and_points_up() {
        let card = size(320, 240);
        let at = |x, y| shape_pixel(XRGB, card, 1333, 0, x, y);
        assert_eq!(at(0, 0), CARD_BORDER);
        assert_eq!(at(20, 10), CARD_CORNER);
        assert_eq!(at(300, 10), CARD_BACKGROUND);
        assert_eq!(at(20, 230), CARD_BACKGROUND);
        assert_eq!(at(160, 25), CARD_ARROW);
        assert_eq!(at(160, 36), CARD_ARROW, "the stem");
        let arrow_rows: Vec<u32> = (0..240)
            .filter(|&y| (0..320).any(|x| at(x, y) == CARD_ARROW))
            .collect();
        assert!(arrow_rows.iter().all(|&y| y < 48), "{arrow_rows:?}");
        let width = |y| (0..320).filter(|&x| at(x, y) == CARD_ARROW).count();
        assert!(width(15) < width(28), "the head widens down from the tip");
        assert_eq!(at(140, 105), CARD_DIAGONAL, "y = x * 240 / 320");
    }

    /// The circle of a card of `frame` and `aspect_milli`: its width on
    /// the glass over its height, in thousandths.
    fn circle_shape(frame: Size, aspect_milli: u32) -> u64 {
        let red = |x, y| shape_pixel(XRGB, frame, aspect_milli, 0, x, y) == CARD_CIRCLE;
        let (cx, cy) = (frame.width / 2, frame.height / 2);
        let across: Vec<u32> = (0..frame.width).filter(|&x| red(x, cy)).collect();
        let down: Vec<u32> = (0..frame.height).filter(|&y| red(cx, y)).collect();
        let span = |v: &[u32]| u64::from(v.last().unwrap() - v.first().unwrap() + 1);
        // A pixel is `aspect * height / width` thousandths of a line wide.
        let wide = span(&across) * u64::from(aspect_milli) * u64::from(frame.height)
            / u64::from(frame.width);
        wide / span(&down)
    }

    #[test]
    fn the_circle_is_round_on_the_glass() {
        for (frame, aspect) in [
            (size(320, 240), 1333),
            (size(320, 240), 1000),
            (size(256, 224), 1333),
            (size(240, 320), 750),
            (size(640, 240), 1333),
        ] {
            let shape = circle_shape(frame, aspect);
            assert!(shape.abs_diff(1000) <= 20, "{frame:?} {aspect}: {shape}");
        }
    }

    #[test]
    fn the_card_takes_the_bits_of_each_format() {
        let card = size(64, 48);
        for format in [PixelFormat::Rgb565, PixelFormat::Rgb1555] {
            assert_eq!(shape_pixel(format, card, 0, 0, 0, 0), format.colour_mask());
        }
        assert_eq!(encode(PixelFormat::Rgb565, CARD_CIRCLE), 0xF800);
        assert_eq!(encode(PixelFormat::Rgb1555, CARD_CORNER), 0x001F);
        let pitch = 64 * 2;
        let mut buf = vec![0; pitch * 48];
        render_shape(&mut buf, pitch, card, PixelFormat::Rgb565, 0, 1).unwrap();
        let (x, y) = (10, 20);
        let at = usize::from(u16::try_from(y * pitch as u32 + x * 2).unwrap());
        let got = u16::from_ne_bytes([buf[at], buf[at + 1]]);
        assert_eq!(
            u32::from(got),
            shape_pixel(PixelFormat::Rgb565, card, 0, 1, x, y)
        );
        assert_eq!(
            render_shape(&mut buf, 8, card, PixelFormat::Rgb565, 0, 1),
            None
        );
    }
}
