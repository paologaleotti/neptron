use neptron_types::{Frame, PixelFormat};

/// One colour with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rgb {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

impl Rgb {
    pub(crate) const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

    /// The output pixel: XRGB8888 as DRM defines it, a little-endian `u32`
    /// with the X byte set to `0xFF`.
    pub(crate) fn to_xrgb8888_le(self) -> [u8; 4] {
        [self.b, self.g, self.r, 0xFF]
    }
}

/// Reads the pixel at column `x` and line `y` of `frame` and expands it to
/// 8 bits per channel, rounded to the nearest value. The X bits of `Xrgb8888` and `Xrgb1555` are ignored.
///
/// The caller checks that `frame` holds the pixel. A pixel outside the data
/// reads as black, so that a wrong caller cannot cause a panic.
pub(crate) fn read(frame: &Frame<'_>, x: u32, y: u32) -> Rgb {
    let bpp = frame.format.bytes_per_pixel();
    let start = (y as usize)
        .saturating_mul(frame.pitch)
        .saturating_add((x as usize).saturating_mul(bpp));
    let Some(bytes) = frame.data.get(start..start.saturating_add(bpp)) else {
        return Rgb::BLACK;
    };
    match (frame.format, bytes) {
        (PixelFormat::Xrgb8888, &[b0, b1, b2, b3]) => {
            from_xrgb8888(u32::from_ne_bytes([b0, b1, b2, b3]))
        }
        (PixelFormat::Rgb565, &[b0, b1]) => from_rgb565(u16::from_ne_bytes([b0, b1])),
        (PixelFormat::Xrgb1555, &[b0, b1]) => from_xrgb1555(u16::from_ne_bytes([b0, b1])),
        _ => Rgb::BLACK,
    }
}

fn from_xrgb8888(v: u32) -> Rgb {
    Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

/// The colour of an `RGB565` pixel.
pub(crate) fn from_rgb565(v: u16) -> Rgb {
    Rgb {
        r: expand5(v >> 11),
        g: expand6(v >> 5),
        b: expand5(v),
    }
}

/// The colour of an `XRGB1555` pixel.
pub(crate) fn from_xrgb1555(v: u16) -> Rgb {
    Rgb {
        r: expand5(v >> 10),
        g: expand5(v >> 5),
        b: expand5(v),
    }
}

/// `round(v * 255 / 31)`. This is what GL defines for a normalised 5-bit
/// channel. Bit replication, which the `vc4` texture unit does, differs
/// from it by 1 for some values. `v * 255 / 31`
/// never ends in exactly one half, because 31 is odd.
fn expand5(v: u16) -> u8 {
    ((u32::from(v & 0x1F) * 255 + 15) / 31) as u8
}

/// `round(v * 255 / 63)`, for the same reason as [`expand5`].
fn expand6(v: u16) -> u8 {
    ((u32::from(v & 0x3F) * 255 + 31) / 63) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(data: &[u8], format: PixelFormat) -> Frame<'_> {
        Frame {
            data,
            width: 1,
            height: 1,
            pitch: format.bytes_per_pixel(),
            format,
        }
    }

    /// The nearest integer to `v * 255 / max`, from the exact fraction.
    fn rounded(v: u16, max: u16) -> u8 {
        let num = u32::from(v) * 255;
        let max = u32::from(max);
        let (whole, rem) = (num / max, num % max);
        (if 2 * rem >= max { whole + 1 } else { whole }) as u8
    }

    #[test]
    fn five_bit_expansion_is_the_rounded_scale() {
        for v in 0..32 {
            assert_eq!(expand5(v), rounded(v, 31), "value {v}");
        }
        assert_eq!((expand5(0), expand5(1), expand5(31)), (0, 8, 255));
    }

    #[test]
    fn six_bit_expansion_is_the_rounded_scale() {
        for v in 0..64 {
            assert_eq!(expand6(v), rounded(v, 63), "value {v}");
        }
        assert_eq!((expand6(0), expand6(11), expand6(63)), (0, 45, 255));
    }

    #[test]
    fn expansion_ignores_higher_bits() {
        assert_eq!(expand5(0xFFE0), 0);
        assert_eq!(expand6(0xFFC0), 0);
    }

    #[test]
    fn xrgb8888_ignores_the_x_byte() {
        let data = 0xA512_3456_u32.to_ne_bytes();
        let px = read(&frame(&data, PixelFormat::Xrgb8888), 0, 0);
        assert_eq!(
            px,
            Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56
            }
        );
    }

    #[test]
    fn rgb565_channels() {
        let data = ((16_u16 << 11) | (32 << 5) | 1).to_ne_bytes();
        let px = read(&frame(&data, PixelFormat::Rgb565), 0, 0);
        assert_eq!(
            px,
            Rgb {
                r: 0x84,
                g: 0x82,
                b: 0x08
            }
        );
        let white = 0xFFFF_u16.to_ne_bytes();
        let px = read(&frame(&white, PixelFormat::Rgb565), 0, 0);
        assert_eq!(
            px,
            Rgb {
                r: 0xFF,
                g: 0xFF,
                b: 0xFF
            }
        );
    }

    #[test]
    fn xrgb1555_ignores_the_x_bit() {
        let with_x = (0x8000_u16 | (16 << 10) | (1 << 5) | 31).to_ne_bytes();
        let without_x = ((16_u16 << 10) | (1 << 5) | 31).to_ne_bytes();
        let expected = Rgb {
            r: 0x84,
            g: 0x08,
            b: 0xFF,
        };
        assert_eq!(read(&frame(&with_x, PixelFormat::Xrgb1555), 0, 0), expected);
        assert_eq!(
            read(&frame(&without_x, PixelFormat::Xrgb1555), 0, 0),
            expected
        );
    }

    #[test]
    fn pitch_selects_the_row() {
        let mut data = vec![0_u8; 12];
        data[8..10].copy_from_slice(&0xF800_u16.to_ne_bytes());
        let f = Frame {
            data: &data,
            width: 2,
            height: 2,
            pitch: 8,
            format: PixelFormat::Rgb565,
        };
        assert_eq!(
            read(&f, 0, 1),
            Rgb {
                r: 0xFF,
                g: 0,
                b: 0
            }
        );
        assert_eq!(read(&f, 1, 1), Rgb::BLACK);
    }

    #[test]
    fn a_pixel_outside_the_data_reads_black() {
        let data = [0xFF_u8; 2];
        assert_eq!(read(&frame(&data, PixelFormat::Xrgb8888), 0, 0), Rgb::BLACK);
        assert_eq!(read(&frame(&data, PixelFormat::Rgb565), 5, 0), Rgb::BLACK);
    }

    #[test]
    fn output_pixel_is_little_endian_with_an_opaque_x_byte() {
        let px = Rgb { r: 1, g: 2, b: 3 }.to_xrgb8888_le();
        assert_eq!(u32::from_le_bytes(px), 0xFF01_0203);
    }
}
