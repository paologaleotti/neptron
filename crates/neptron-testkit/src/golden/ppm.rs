use crate::{ImageError, RgbImage};

/// Why a file is not a PPM image that [`decode_ppm`] reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PpmError {
    /// The header ends before its four fields.
    #[error("the PPM header is incomplete")]
    Incomplete,
    /// The file is not an 8-bit binary PPM.
    #[error("not an 8-bit P6 PPM: magic {magic:?}, maxval {maxval:?}")]
    Format {
        /// The first header field.
        magic: String,
        /// The fourth header field.
        maxval: String,
    },
    /// A size field is not a number.
    #[error("the PPM {field} {value:?} is not a number")]
    Size {
        /// `width` or `height`.
        field: &'static str,
        /// The text of the field.
        value: String,
    },
    /// The pixel bytes do not match the size.
    #[error("{len} pixel bytes for a {width}x{height} PPM, which needs {need}")]
    Length {
        /// Width, in pixels.
        width: u32,
        /// Height, in lines.
        height: u32,
        /// Pixel bytes in the file.
        len: usize,
        /// Pixel bytes the size needs.
        need: usize,
    },
    /// The byte count of the size does not fit in a `usize`.
    #[error("a {width}x{height} PPM is too large")]
    TooLarge {
        /// Width, in pixels.
        width: u32,
        /// Height, in lines.
        height: u32,
    },
}

/// The binary PPM file of `image`: `P6`, the width, the height and the
/// maxval 255, each followed by one newline, then the RGB bytes. The
/// reference renderer goldens use the same format.
pub fn encode_ppm(image: &RgbImage) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n255\n", image.width(), image.height()).into_bytes();
    out.extend_from_slice(image.bytes());
    out
}

/// Reads a binary PPM file of the form that [`encode_ppm`] writes: no
/// comments, one whitespace byte after each header field, maxval 255.
///
/// # Errors
///
/// [`PpmError`] when the file has another form or its pixel bytes do not
/// match its size.
pub fn decode_ppm(file: &[u8]) -> Result<RgbImage, PpmError> {
    let mut fields = Vec::with_capacity(4);
    let mut start = 0;
    for (i, byte) in file.iter().enumerate() {
        if byte.is_ascii_whitespace() {
            fields.push(String::from_utf8_lossy(&file[start..i]).into_owned());
            start = i + 1;
            if fields.len() == 4 {
                break;
            }
        }
    }
    let [magic, width, height, maxval] =
        <[String; 4]>::try_from(fields).map_err(|_| PpmError::Incomplete)?;
    if magic != "P6" || maxval != "255" {
        return Err(PpmError::Format { magic, maxval });
    }
    let width = parse_size("width", width)?;
    let height = parse_size("height", height)?;
    RgbImage::from_rgb(width, height, file[start..].to_vec()).map_err(|e| match e {
        ImageError::WrongLength { len, need, .. } => PpmError::Length {
            width,
            height,
            len,
            need,
        },
        _ => PpmError::TooLarge { width, height },
    })
}

fn parse_size(field: &'static str, value: String) -> Result<u32, PpmError> {
    value.parse().map_err(|_| PpmError::Size { field, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let image = RgbImage::from_rgb(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap();
        let file = encode_ppm(&image);
        assert_eq!(&file[..11], b"P6\n2 1\n255\n");
        assert_eq!(decode_ppm(&file), Ok(image));
    }

    #[test]
    fn errors_name_the_fault() {
        assert_eq!(decode_ppm(b"P6\n1 1"), Err(PpmError::Incomplete));
        assert_eq!(
            decode_ppm(b"P5\n1 1\n255\n\0"),
            Err(PpmError::Format {
                magic: "P5".to_owned(),
                maxval: "255".to_owned()
            })
        );
        assert_eq!(
            decode_ppm(b"P6\n1 x\n255\n\0\0\0"),
            Err(PpmError::Size {
                field: "height",
                value: "x".to_owned()
            })
        );
        assert_eq!(
            decode_ppm(b"P6\n1 1\n255\n\0"),
            Err(PpmError::Length {
                width: 1,
                height: 1,
                len: 1,
                need: 3
            })
        );
        assert_eq!(
            PpmError::Length {
                width: 1,
                height: 1,
                len: 1,
                need: 3
            }
            .to_string(),
            "1 pixel bytes for a 1x1 PPM, which needs 3"
        );
    }
}
