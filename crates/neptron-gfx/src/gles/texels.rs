use neptron_types::{Frame, PixelFormat};

use super::GlesError;

/// Bytes of one texel of a source texture: `BGRA_EXT` bytes
/// (`GL_EXT_texture_format_BGRA8888`).
pub(super) const BYTES_PER_TEXEL: usize = 4;

/// The texels of a source image, for one upload call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Texels<'a> {
    /// `BGRA_EXT` bytes.
    Bytes(&'a [u8]),
    /// One native-endian `u32` per texel, `0xAARRGGBB`: on a little-endian
    /// machine its bytes are B, G, R, A.
    Words(&'a [u32]),
}

/// The texels of `frame` as packed rows, which GL reads in one call, with
/// the alpha channel ignored by the shader.
///
/// An `XRGB8888` pixel as a little-endian `u32` is the bytes B, G, R, X, so
/// a frame with packed rows goes to GL as it is. Other frames are written
/// into `staging`: `XRGB8888` rows with a pitch above the row are packed,
/// and `RGB565` and `XRGB1555` pixels are expanded to 8 bits per channel.
/// `staging` grows only when a frame needs more texels than it holds.
///
/// The texture unit of `vc4` expands a 5-bit or 6-bit channel of a
/// `UNSIGNED_SHORT_5_6_5` texture by bit replication, which differs by 1
/// from the `round(v * 255 / 31)` that GL and the reference define for
/// some values, so the CPU expands, with integer steps equal to the
/// expansion of the reference for every value.
///
/// The loops index the slices and use no iterator adapter: the bench build
/// of `cargo xtask os push` (incremental, 16 codegen units, no LTO) does not
/// inline the adapters of `core`, so each pixel would make calls.
///
/// # Errors
///
/// [`GlesError::UploadOutOfBounds`] when the frame's bytes are shorter than
/// its size and pitch need. The renderer checks every frame first, so this
/// means a fault in the renderer.
pub(super) fn packed<'a>(
    frame: &Frame<'a>,
    staging: &'a mut Vec<u32>,
) -> Result<Texels<'a>, GlesError> {
    let width = usize::from(frame.width);
    let rows = usize::from(frame.height);
    let source_row = width * frame.format.bytes_per_pixel();
    let needed = rows.checked_sub(1).map_or(Some(0), |r| {
        r.checked_mul(frame.pitch)?.checked_add(source_row)
    });
    if needed.is_none_or(|n| n > frame.data.len()) || frame.pitch < source_row {
        return Err(GlesError::UploadOutOfBounds {
            len: frame.data.len(),
            width: source_row as u32,
            rows: u32::from(frame.height),
            pitch: frame.pitch,
        });
    }
    if frame.format == PixelFormat::Xrgb8888 && frame.pitch == width * BYTES_PER_TEXEL {
        return Ok(Texels::Bytes(&frame.data[..source_row * rows]));
    }
    let len = width * rows;
    if staging.len() < len {
        staging.resize(len, 0);
    }
    let out = &mut staging[..len];
    for r in 0..rows {
        let from = &frame.data[r * frame.pitch..r * frame.pitch + source_row];
        let to = &mut out[r * width..(r + 1) * width];
        match frame.format {
            PixelFormat::Xrgb8888 => pack_row(to, from),
            PixelFormat::Rgb565 => rgb565_row(to, from),
            PixelFormat::Xrgb1555 => xrgb1555_row(to, from),
        }
    }
    Ok(Texels::Words(out))
}

/// `round(v * 255 / 31)` of a 5-bit `v`, as the reference expands it.
fn expand5(v: u32) -> u32 {
    (v * 527 + 23) >> 6
}

/// `round(v * 255 / 63)` of a 6-bit `v`, as the reference expands it.
fn expand6(v: u32) -> u32 {
    (v * 259 + 33) >> 6
}

/// Writes the little-endian `RGB565` pixels of `from` into `to`.
fn rgb565_row(to: &mut [u32], from: &[u8]) {
    let pixels = to.len().min(from.len() / 2);
    for i in 0..pixels {
        let v = from[2 * i] as u32 | (from[2 * i + 1] as u32) << 8;
        let r = expand5(v >> 11);
        let g = expand6((v >> 5) & 0x3F);
        let b = expand5(v & 0x1F);
        to[i] = 0xFF00_0000 | r << 16 | g << 8 | b;
    }
}

/// Writes the little-endian `XRGB1555` pixels of `from` into `to`.
fn xrgb1555_row(to: &mut [u32], from: &[u8]) {
    let pixels = to.len().min(from.len() / 2);
    for i in 0..pixels {
        let v = from[2 * i] as u32 | (from[2 * i + 1] as u32) << 8;
        let r = expand5((v >> 10) & 0x1F);
        let g = expand5((v >> 5) & 0x1F);
        let b = expand5(v & 0x1F);
        to[i] = 0xFF00_0000 | r << 16 | g << 8 | b;
    }
}

/// Writes the little-endian `XRGB8888` pixels of `from` into `to`.
#[expect(
    clippy::needless_range_loop,
    reason = "an iterator does not inline in the bench build"
)]
fn pack_row(to: &mut [u32], from: &[u8]) {
    let pixels = to.len().min(from.len() / 4);
    for i in 0..pixels {
        let at = 4 * i;
        to[i] = from[at] as u32
            | (from[at + 1] as u32) << 8
            | (from[at + 2] as u32) << 16
            | (from[at + 3] as u32) << 24;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{Rgb, from_rgb565, from_xrgb1555};

    fn frame(data: &[u8], width: u16, height: u16, pitch: usize, format: PixelFormat) -> Frame<'_> {
        Frame {
            data,
            width,
            height,
            pitch,
            format,
        }
    }

    fn words(texels: Texels<'_>) -> Vec<u32> {
        match texels {
            Texels::Words(w) => w.to_vec(),
            Texels::Bytes(b) => panic!("{} bytes, not words", b.len()),
        }
    }

    #[test]
    fn packed_xrgb8888_goes_to_gl_as_it_is() {
        let data: Vec<u8> = (0..16).collect();
        let mut staging = Vec::new();
        let f = frame(&data, 2, 2, 8, PixelFormat::Xrgb8888);
        let t = packed(&f, &mut staging).unwrap();
        assert_eq!(t, Texels::Bytes(&data));
        assert!(staging.is_empty(), "no copy");
    }

    #[test]
    fn padded_xrgb8888_rows_are_packed() {
        let data: Vec<u8> = (0..24).collect();
        let mut staging = Vec::new();
        let f = frame(&data, 2, 2, 12, PixelFormat::Xrgb8888);
        let t = packed(&f, &mut staging).unwrap();
        let expected = [0x0302_0100, 0x0706_0504, 0x0F0E_0D0C, 0x1312_1110];
        assert_eq!(words(t), expected);
    }

    /// Every 16-bit value expands as the reference renderer reads it.
    #[test]
    fn sixteen_bit_pixels_expand_as_the_reference() {
        let data: Vec<u8> = (0..=u16::MAX).flat_map(u16::to_ne_bytes).collect();
        let mut staging = Vec::new();
        for (format, colour) in [
            (PixelFormat::Rgb565, from_rgb565 as fn(u16) -> Rgb),
            (PixelFormat::Xrgb1555, from_xrgb1555),
        ] {
            let f = frame(&data, 256, 256, 512, format);
            let out = words(packed(&f, &mut staging).unwrap());
            for (v, texel) in (0..=u16::MAX).zip(out) {
                let c = colour(v);
                let bytes = texel.to_le_bytes();
                assert_eq!(bytes, [c.b, c.g, c.r, 0xFF], "{format:?} {v:#06x}");
            }
        }
    }

    #[test]
    fn a_channel_expands_with_rounding_not_bit_replication() {
        // Red 3 of RGB565: round(3 * 255 / 31) = 25; bit replication gives 24.
        let data = (3_u16 << 11).to_le_bytes();
        let mut staging = Vec::new();
        let f = frame(&data, 1, 1, 2, PixelFormat::Rgb565);
        let t = packed(&f, &mut staging).unwrap();
        assert_eq!(words(t), [0xFF19_0000]);
    }

    #[test]
    fn staging_grows_only_for_a_larger_frame() {
        let data = vec![0_u8; 64];
        let mut staging = Vec::new();
        let f = frame(&data, 4, 4, 16, PixelFormat::Xrgb1555);
        packed(&f, &mut staging).unwrap();
        let (len, at) = (staging.len(), staging.as_ptr());
        let f = frame(&data, 2, 2, 16, PixelFormat::Rgb565);
        assert_eq!(words(packed(&f, &mut staging).unwrap()).len(), 4);
        assert_eq!((staging.len(), staging.as_ptr()), (len, at));
    }

    #[test]
    fn a_short_frame_is_out_of_bounds() {
        let data = [0_u8; 7];
        let mut staging = Vec::new();
        let f = frame(&data, 2, 2, 4, PixelFormat::Rgb565);
        assert!(matches!(
            packed(&f, &mut staging),
            Err(GlesError::UploadOutOfBounds { len: 7, .. })
        ));
        let f = frame(&data, 2, 1, 2, PixelFormat::Rgb565);
        let short = packed(&f, &mut staging);
        assert!(short.is_err(), "pitch below the row");
    }

    #[test]
    fn an_empty_frame_is_empty() {
        let mut staging = Vec::new();
        let f = frame(&[], 0, 0, 0, PixelFormat::Xrgb1555);
        let t = packed(&f, &mut staging).unwrap();
        assert_eq!(words(t), []);
    }
}
