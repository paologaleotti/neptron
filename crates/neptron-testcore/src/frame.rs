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
    let row_bytes = visible_row_bytes(size, format)?;
    if row_bytes > pitch {
        return None;
    }
    for y in 0..size.height {
        let start = usize::try_from(y).ok()?.checked_mul(pitch)?;
        let row = buf.get_mut(start..start.checked_add(row_bytes)?)?;
        for (x, px) in (0..size.width).zip(row.chunks_exact_mut(format.bytes_per_pixel())) {
            let value = pixel(format, frame, buttons, x, y);
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
}
