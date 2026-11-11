/// An RGB image with 8 bits per channel, 3 bytes per pixel, rows packed
/// with no padding. Goldens and screenshots use it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RgbImage {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

/// Why an [`RgbImage`] cannot be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ImageError {
    /// The byte count does not match the size.
    #[error("{len} bytes for a {width}x{height} image, which needs {need} bytes")]
    WrongLength {
        /// Width, in pixels.
        width: u32,
        /// Height, in lines.
        height: u32,
        /// Bytes that were given.
        len: usize,
        /// Bytes the size needs.
        need: usize,
    },
    /// One row needs more bytes than the pitch.
    #[error("a pitch of {pitch} bytes is below the {need} bytes of one row of {width} pixels")]
    PitchTooSmall {
        /// Width, in pixels.
        width: u32,
        /// The pitch, in bytes.
        pitch: usize,
        /// Bytes of one row.
        need: usize,
    },
    /// The buffer holds fewer bytes than the size and the pitch need.
    #[error("the buffer holds {len} bytes, below the {need} bytes of {height} rows")]
    BufferTooSmall {
        /// Height, in lines.
        height: u32,
        /// Bytes in the buffer.
        len: usize,
        /// Bytes the rows need.
        need: usize,
    },
    /// The byte count of the image does not fit in a `usize`.
    #[error("a {width}x{height} image is too large")]
    TooLarge {
        /// Width, in pixels.
        width: u32,
        /// Height, in lines.
        height: u32,
    },
}

impl RgbImage {
    /// An image from packed RGB bytes, 3 per pixel, row after row.
    ///
    /// # Errors
    ///
    /// [`ImageError::WrongLength`] when `bytes` does not hold exactly
    /// `width * height * 3` bytes, [`ImageError::TooLarge`] when that count
    /// overflows.
    pub fn from_rgb(width: u32, height: u32, bytes: Vec<u8>) -> Result<Self, ImageError> {
        let need = rgb_len(width, height)?;
        if bytes.len() != need {
            return Err(ImageError::WrongLength {
                width,
                height,
                len: bytes.len(),
                need,
            });
        }
        Ok(Self {
            width,
            height,
            bytes,
        })
    }

    /// An image from XRGB8888 little-endian pixels, the output format of
    /// the reference renderer. The X byte and the bytes after each row are
    /// ignored.
    ///
    /// # Errors
    ///
    /// [`ImageError::PitchTooSmall`] when one row of `width` pixels does not
    /// fit in `pitch`, [`ImageError::BufferTooSmall`] when `data` holds
    /// fewer than `height` rows, [`ImageError::TooLarge`] when a byte count
    /// overflows.
    pub fn from_xrgb8888(
        data: &[u8],
        width: u32,
        height: u32,
        pitch: usize,
    ) -> Result<Self, ImageError> {
        let too_large = ImageError::TooLarge { width, height };
        let row = usize::try_from(width)
            .ok()
            .and_then(|w| w.checked_mul(4))
            .ok_or(too_large)?;
        if pitch < row {
            return Err(ImageError::PitchTooSmall {
                width,
                pitch,
                need: row,
            });
        }
        let need = match usize::try_from(height).map_err(|_| too_large)? {
            0 => 0,
            h => (h - 1)
                .checked_mul(pitch)
                .and_then(|n| n.checked_add(row))
                .ok_or(too_large)?,
        };
        if data.len() < need {
            return Err(ImageError::BufferTooSmall {
                height,
                len: data.len(),
                need,
            });
        }
        let mut bytes = Vec::with_capacity(rgb_len(width, height)?);
        if row > 0 {
            for line in data[..need].chunks(pitch) {
                for px in line[..row].chunks_exact(4) {
                    bytes.extend_from_slice(&[px[2], px[1], px[0]]);
                }
            }
        }
        Ok(Self {
            width,
            height,
            bytes,
        })
    }

    /// Width, in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height, in lines.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The packed RGB bytes, row after row.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The `[r, g, b]` value of the pixel at `(x, y)`, or `None` outside
    /// the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let start = (y as usize * self.width as usize + x as usize) * 3;
        let px = self.bytes.get(start..start + 3)?;
        Some([px[0], px[1], px[2]])
    }
}

fn rgb_len(width: u32, height: u32) -> Result<usize, ImageError> {
    usize::try_from(u64::from(width) * u64::from(height))
        .ok()
        .and_then(|n| n.checked_mul(3))
        .ok_or(ImageError::TooLarge { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_xrgb8888_skips_the_x_byte_and_the_padding() {
        // Two pixels per row, pitch 10: 2 bytes of padding per row.
        let data = [
            0x03, 0x02, 0x01, 0xFF, 0x06, 0x05, 0x04, 0x00, 0xEE, 0xEE, //
            0x09, 0x08, 0x07, 0xAA, 0x0C, 0x0B, 0x0A, 0x55,
        ];
        let image = RgbImage::from_xrgb8888(&data, 2, 2, 10).unwrap();
        assert_eq!(image.bytes(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12][..]);
        assert_eq!(image.pixel(1, 1), Some([10, 11, 12]));
        assert_eq!(image.pixel(2, 0), None);
        assert_eq!(image.pixel(0, 2), None);
        assert_eq!((image.width(), image.height()), (2, 2));
    }

    #[test]
    fn from_xrgb8888_checks_the_buffer() {
        assert_eq!(
            RgbImage::from_xrgb8888(&[0; 16], 2, 2, 7),
            Err(ImageError::PitchTooSmall {
                width: 2,
                pitch: 7,
                need: 8
            })
        );
        assert_eq!(
            RgbImage::from_xrgb8888(&[0; 17], 2, 2, 10),
            Err(ImageError::BufferTooSmall {
                height: 2,
                len: 17,
                need: 18
            })
        );
        let empty = RgbImage::from_xrgb8888(&[], 0, 0, 0).unwrap();
        assert!(empty.bytes().is_empty());
        let no_rows = RgbImage::from_xrgb8888(&[], 4, 0, 16).unwrap();
        assert!(no_rows.bytes().is_empty());
    }

    #[test]
    fn from_rgb_checks_the_length() {
        assert!(RgbImage::from_rgb(1, 1, vec![1, 2, 3]).is_ok());
        assert_eq!(
            RgbImage::from_rgb(1, 1, vec![1, 2]),
            Err(ImageError::WrongLength {
                width: 1,
                height: 1,
                len: 2,
                need: 3
            })
        );
        assert_eq!(
            ImageError::WrongLength {
                width: 1,
                height: 1,
                len: 2,
                need: 3
            }
            .to_string(),
            "2 bytes for a 1x1 image, which needs 3 bytes"
        );
    }
}
