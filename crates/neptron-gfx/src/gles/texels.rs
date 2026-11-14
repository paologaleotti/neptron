use neptron_types::{Frame, PixelFormat};

use super::GlesError;
use crate::reference::{Rgb, from_rgb565, from_xrgb1555};

/// Bytes of one texel of a source texture: `BGRA_EXT` bytes
/// (`GL_EXT_texture_format_BGRA8888`).
pub(super) const BYTES_PER_TEXEL: usize = 4;

/// The texels of `frame` as packed rows of `BGRA_EXT` bytes, which GL reads
/// in one call, with the alpha byte ignored by the shader.
///
/// An `XRGB8888` pixel as a little-endian `u32` is the bytes B, G, R, X, so
/// a frame with packed rows goes to GL as it is. Other frames are written
/// into `staging`: `XRGB8888` rows with a pitch above the row are packed,
/// and `RGB565` and `XRGB1555` pixels are expanded to 8 bits per channel
/// with the code of the reference. The texture unit of `vc4` expands a
/// 5-bit or 6-bit channel by bit replication, which differs by 1 from the
/// `round(v * 255 / 31)` that GL defines for some values; the expansion on
/// the CPU is the same on every GPU. `staging` grows only when a frame
/// needs more bytes than it holds.
///
/// # Errors
///
/// [`GlesError::UploadOutOfBounds`] when the frame's bytes are shorter than
/// its size and pitch need. The renderer checks every frame first, so this
/// means a fault in the renderer.
pub(super) fn packed<'a>(
    frame: &Frame<'a>,
    staging: &'a mut Vec<u8>,
) -> Result<&'a [u8], GlesError> {
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
    let row_bytes = width * BYTES_PER_TEXEL;
    let len = row_bytes * rows;
    if frame.format == PixelFormat::Xrgb8888 && frame.pitch == row_bytes {
        return Ok(&frame.data[..len]);
    }
    if staging.len() < len {
        staging.resize(len, 0);
    }
    let out = &mut staging[..len];
    if row_bytes == 0 {
        return Ok(out);
    }
    for (r, to) in out.chunks_exact_mut(row_bytes).enumerate() {
        let start = r * frame.pitch;
        let from = &frame.data[start..start + source_row];
        match frame.format {
            PixelFormat::Xrgb8888 => to.copy_from_slice(from),
            PixelFormat::Rgb565 => expand(to, from, from_rgb565),
            PixelFormat::Xrgb1555 => expand(to, from, from_xrgb1555),
        }
    }
    Ok(out)
}

/// Writes each native-endian 16-bit pixel of `from` into `to` as `BGRA_EXT`
/// bytes.
fn expand(to: &mut [u8], from: &[u8], colour: fn(u16) -> Rgb) {
    for (t, f) in to
        .chunks_exact_mut(BYTES_PER_TEXEL)
        .zip(from.chunks_exact(2))
    {
        let c = colour(u16::from_ne_bytes([f[0], f[1]]));
        t.copy_from_slice(&[c.b, c.g, c.r, 0xFF]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(data: &[u8], width: u16, height: u16, pitch: usize, format: PixelFormat) -> Frame<'_> {
        Frame {
            data,
            width,
            height,
            pitch,
            format,
        }
    }

    #[test]
    fn packed_xrgb8888_goes_to_gl_as_it_is() {
        let data: Vec<u8> = (0..16).collect();
        let mut staging = Vec::new();
        let f = frame(&data, 2, 2, 8, PixelFormat::Xrgb8888);
        assert_eq!(packed(&f, &mut staging).unwrap(), &data[..]);
        assert!(staging.is_empty(), "no copy");
    }

    #[test]
    fn padded_rows_are_packed() {
        let data: Vec<u8> = (0..24).collect();
        let mut staging = Vec::new();
        let f = frame(&data, 2, 2, 12, PixelFormat::Xrgb8888);
        let expected: Vec<u8> = (0..8).chain(12..20).collect();
        assert_eq!(packed(&f, &mut staging).unwrap(), &expected[..]);
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
            let out = packed(&f, &mut staging).unwrap();
            for (v, texel) in (0..=u16::MAX).zip(out.chunks_exact(4)) {
                let c = colour(v);
                assert_eq!(texel, [c.b, c.g, c.r, 0xFF], "{format:?} {v:#06x}");
            }
        }
    }

    #[test]
    fn a_channel_expands_with_rounding_not_bit_replication() {
        // Red 3 of RGB565: round(3 * 255 / 31) = 25; bit replication gives 24.
        let data = (3_u16 << 11).to_ne_bytes();
        let mut staging = Vec::new();
        let f = frame(&data, 1, 1, 2, PixelFormat::Rgb565);
        assert_eq!(packed(&f, &mut staging).unwrap(), [0, 0, 25, 0xFF]);
    }

    #[test]
    fn staging_grows_only_for_a_larger_frame() {
        let data = vec![0_u8; 64];
        let mut staging = Vec::new();
        let f = frame(&data, 4, 4, 16, PixelFormat::Xrgb1555);
        packed(&f, &mut staging).unwrap();
        let (len, at) = (staging.len(), staging.as_ptr());
        let f = frame(&data, 2, 2, 16, PixelFormat::Rgb565);
        assert_eq!(packed(&f, &mut staging).unwrap().len(), 16);
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
        assert!(packed(&f, &mut staging).is_err(), "pitch below the row");
    }

    #[test]
    fn an_empty_frame_is_empty() {
        let mut staging = Vec::new();
        let f = frame(&[], 0, 0, 0, PixelFormat::Xrgb1555);
        assert_eq!(packed(&f, &mut staging).unwrap(), &[] as &[u8]);
    }
}
