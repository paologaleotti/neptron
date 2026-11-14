use neptron_types::{Frame, PixelFormat};

use super::GlesError;

/// The GL texture format that holds a source pixel format, so the texture
/// unit decodes the channels and the fragment shader does no per-pixel
/// arithmetic for it. GL converts a normalised channel `v` of `b` bits to
/// `v / (2^b - 1)`, the expansion of the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Texels {
    /// `RGB` with `UNSIGNED_SHORT_5_6_5`: the libretro `RGB565` layout,
    /// red in the high bits of a native-endian `u16`. Uploaded as it is.
    Rgb565,
    /// `BGRA_EXT` bytes (`GL_EXT_texture_format_BGRA8888`): `XRGB8888` as
    /// a little-endian `u32` is the bytes B, G, R, X. Uploaded as it is;
    /// the shader ignores the alpha channel.
    Bgra8888,
    /// `RGBA` with `UNSIGNED_SHORT_5_5_5_1`. `XRGB1555` has no GL layout:
    /// each pixel moves one bit to the left on upload, which puts red,
    /// green and blue where GL reads them, and alpha in bit 0.
    Rgba5551,
}

impl Texels {
    /// The texture format of `format`.
    pub(super) fn of(format: PixelFormat) -> Self {
        match format {
            PixelFormat::Rgb565 => Self::Rgb565,
            PixelFormat::Xrgb8888 => Self::Bgra8888,
            PixelFormat::Xrgb1555 => Self::Rgba5551,
        }
    }

    /// Bytes of one texel.
    pub(super) fn bytes_per_texel(self) -> usize {
        match self {
            Self::Rgb565 | Self::Rgba5551 => 2,
            Self::Bgra8888 => 4,
        }
    }
}

/// The texels of `frame`, as packed rows that GL reads in one call: the
/// frame's own bytes when its rows are packed and need no change, else a
/// copy in `staging`. `staging` grows only when a frame needs more bytes
/// than it holds.
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
    let texels = Texels::of(frame.format);
    let row_bytes = usize::from(frame.width) * texels.bytes_per_texel();
    let rows = usize::from(frame.height);
    let out_of_bounds = || GlesError::UploadOutOfBounds {
        len: frame.data.len(),
        width: row_bytes as u32,
        rows: u32::from(frame.height),
        pitch: frame.pitch,
    };
    let needed = rows.checked_sub(1).map_or(Some(0), |r| {
        r.checked_mul(frame.pitch)?.checked_add(row_bytes)
    });
    if needed.is_none_or(|n| n > frame.data.len()) || frame.pitch < row_bytes {
        return Err(out_of_bounds());
    }
    if texels != Texels::Rgba5551 && frame.pitch == row_bytes {
        return Ok(&frame.data[..row_bytes * rows]);
    }
    let len = row_bytes * rows;
    if staging.len() < len {
        staging.resize(len, 0);
    }
    let out = &mut staging[..len];
    if row_bytes == 0 {
        return Ok(out);
    }
    for (r, to) in out.chunks_exact_mut(row_bytes).enumerate() {
        let start = r * frame.pitch;
        let from = &frame.data[start..start + row_bytes];
        if texels == Texels::Rgba5551 {
            for (t, f) in to.chunks_exact_mut(2).zip(from.chunks_exact(2)) {
                let v = u16::from_ne_bytes([f[0], f[1]]);
                t.copy_from_slice(&(v << 1).to_ne_bytes());
            }
        } else {
            to.copy_from_slice(from);
        }
    }
    Ok(out)
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
    fn packed_rows_go_to_gl_as_they_are() {
        let data: Vec<u8> = (0..16).collect();
        let mut staging = Vec::new();
        let f = frame(&data, 2, 2, 8, PixelFormat::Xrgb8888);
        assert_eq!(packed(&f, &mut staging).unwrap(), &data[..]);
        assert!(staging.is_empty(), "no copy");
        let f = frame(&data, 4, 2, 8, PixelFormat::Rgb565);
        assert_eq!(packed(&f, &mut staging).unwrap(), &data[..]);
        assert!(staging.is_empty(), "no copy");
    }

    #[test]
    fn padded_rows_are_packed() {
        let data: Vec<u8> = (0..12).collect();
        let mut staging = Vec::new();
        let f = frame(&data, 2, 2, 6, PixelFormat::Rgb565);
        assert_eq!(packed(&f, &mut staging).unwrap(), [0, 1, 2, 3, 6, 7, 8, 9]);
    }

    #[test]
    fn xrgb1555_moves_one_bit_to_the_left() {
        // X set, red 31, green 1, blue 2; then X clear, red 1, green 31,
        // blue 0.
        let pixels = [0x8000_u16 | 31 << 10 | 1 << 5 | 2, 1 << 10 | 31 << 5];
        let data: Vec<u8> = pixels.iter().flat_map(|p| p.to_ne_bytes()).collect();
        let mut staging = Vec::new();
        let f = frame(&data, 2, 1, 4, PixelFormat::Xrgb1555);
        let out = packed(&f, &mut staging).unwrap();
        let texels: Vec<u16> = out
            .chunks_exact(2)
            .map(|b| u16::from_ne_bytes([b[0], b[1]]))
            .collect();
        // RGBA5551: red in bits 15-11, green 10-6, blue 5-1, alpha bit 0.
        assert_eq!(texels, [31 << 11 | 1 << 6 | 2 << 1, 1 << 11 | 31 << 6]);
    }

    #[test]
    fn staging_grows_only_for_a_larger_frame() {
        let data = vec![0_u8; 64];
        let mut staging = Vec::new();
        let f = frame(&data, 4, 4, 16, PixelFormat::Xrgb1555);
        packed(&f, &mut staging).unwrap();
        let (len, at) = (staging.len(), staging.as_ptr());
        let f = frame(&data, 2, 2, 16, PixelFormat::Xrgb1555);
        assert_eq!(packed(&f, &mut staging).unwrap().len(), 8);
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
