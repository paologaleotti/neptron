use std::fmt;

use neptron_types::{Frame, Rect};

use super::OutputBuffer;

/// The largest width or height of the crop and the destination rectangle,
/// in pixels. It keeps every intermediate value of the pixel maths inside
/// a `u64`.
pub const MAX_RECT_SIDE_PX: u32 = 1 << 16;

/// Why [`ReferenceRenderer::render`](super::ReferenceRenderer::render)
/// refused a scene or an output buffer. The output buffer is not changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// One row of the image needs more bytes than its pitch.
    #[error(
        "the {image} pitch is {pitch} bytes, below the {min} bytes of one row of {width} pixels"
    )]
    PitchTooSmall {
        /// The image with the fault.
        image: Image,
        /// Width of the image, in pixels.
        width: u32,
        /// The pitch, in bytes.
        pitch: usize,
        /// The smallest valid pitch, in bytes.
        min: usize,
    },
    /// The image holds fewer bytes than its size and pitch need.
    #[error("the {image} holds {len} bytes, below the {min} bytes that its size and pitch need")]
    BufferTooSmall {
        /// The image with the fault.
        image: Image,
        /// Bytes in the buffer.
        len: usize,
        /// Bytes the image needs.
        min: usize,
    },
    /// The byte count of the image does not fit in a `usize`.
    #[error("the {image} of {width}x{height} pixels with a pitch of {pitch} bytes is too large")]
    SizeOverflow {
        /// The image with the fault.
        image: Image,
        /// Width, in pixels.
        width: u32,
        /// Height, in lines.
        height: u32,
        /// The pitch, in bytes.
        pitch: usize,
    },
    /// A side of the crop or the destination rectangle is above
    /// [`MAX_RECT_SIDE_PX`].
    #[error("the {rect} rectangle is {w}x{h} pixels, above the limit of {limit} pixels per side")]
    RectTooLarge {
        /// The rectangle with the fault.
        rect: RectKind,
        /// Its width, in pixels.
        w: u32,
        /// Its height, in pixels.
        h: u32,
        /// The limit, [`MAX_RECT_SIDE_PX`].
        limit: u32,
    },
}

/// An image that the renderer reads or writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Image {
    /// The game frame of the scene.
    Game,
    /// The UI canvas of the scene.
    Ui,
    /// The output buffer.
    Output,
}

/// A rectangle of the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectKind {
    /// The crop, in game frame pixels.
    Crop,
    /// The destination, in output pixels.
    Dest,
}

impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Image::Game => "game frame",
            Image::Ui => "UI canvas",
            Image::Output => "output buffer",
        })
    }
}

impl fmt::Display for RectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RectKind::Crop => "crop",
            RectKind::Dest => "destination",
        })
    }
}

pub(crate) fn check_frame(frame: &Frame<'_>, image: Image) -> Result<(), RenderError> {
    check_image(
        image,
        u32::from(frame.width),
        u32::from(frame.height),
        frame.pitch,
        frame.format.bytes_per_pixel(),
        frame.data.len(),
    )
}

pub(crate) fn check_output(out: &OutputBuffer<'_>) -> Result<(), RenderError> {
    check_image(
        Image::Output,
        out.width,
        out.height,
        out.pitch,
        4,
        out.data.len(),
    )
}

pub(crate) fn check_rect(rect: &Rect, kind: RectKind) -> Result<(), RenderError> {
    if rect.w > MAX_RECT_SIDE_PX || rect.h > MAX_RECT_SIDE_PX {
        return Err(RenderError::RectTooLarge {
            rect: kind,
            w: rect.w,
            h: rect.h,
            limit: MAX_RECT_SIDE_PX,
        });
    }
    Ok(())
}

/// An image of zero width or zero height needs no bytes, whatever its pitch.
fn check_image(
    image: Image,
    width: u32,
    height: u32,
    pitch: usize,
    bpp: usize,
    len: usize,
) -> Result<(), RenderError> {
    if width == 0 || height == 0 {
        return Ok(());
    }
    let overflow = RenderError::SizeOverflow {
        image,
        width,
        height,
        pitch,
    };
    let row = usize::try_from(width)
        .ok()
        .and_then(|w| w.checked_mul(bpp))
        .ok_or(overflow)?;
    if pitch < row {
        return Err(RenderError::PitchTooSmall {
            image,
            width,
            pitch,
            min: row,
        });
    }
    let min = usize::try_from(height - 1)
        .ok()
        .and_then(|rows| rows.checked_mul(pitch))
        .and_then(|bytes| bytes.checked_add(row))
        .ok_or(overflow)?;
    if len < min {
        return Err(RenderError::BufferTooSmall { image, len, min });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_size_is_accepted() {
        assert_eq!(check_image(Image::Game, 3, 2, 8, 2, 14), Ok(()));
        assert_eq!(check_image(Image::Game, 3, 2, 6, 2, 12), Ok(()));
    }

    #[test]
    fn the_last_row_needs_no_padding() {
        assert_eq!(check_image(Image::Output, 2, 3, 16, 4, 2 * 16 + 8), Ok(()));
    }

    #[test]
    fn one_byte_short_is_refused() {
        assert_eq!(
            check_image(Image::Ui, 3, 2, 8, 2, 13),
            Err(RenderError::BufferTooSmall {
                image: Image::Ui,
                len: 13,
                min: 14
            })
        );
    }

    #[test]
    fn a_small_pitch_is_refused() {
        assert_eq!(
            check_image(Image::Output, 4, 1, 15, 4, 100),
            Err(RenderError::PitchTooSmall {
                image: Image::Output,
                width: 4,
                pitch: 15,
                min: 16
            })
        );
    }

    #[test]
    fn an_empty_image_needs_no_bytes() {
        assert_eq!(check_image(Image::Game, 0, 240, 0, 4, 0), Ok(()));
        assert_eq!(check_image(Image::Game, 320, 0, 0, 4, 0), Ok(()));
    }

    #[test]
    fn a_size_that_overflows_is_refused() {
        assert_eq!(
            check_image(Image::Output, 2, u32::MAX, usize::MAX / 2, 4, 0),
            Err(RenderError::SizeOverflow {
                image: Image::Output,
                width: 2,
                height: u32::MAX,
                pitch: usize::MAX / 2
            })
        );
    }

    #[test]
    fn rect_limit_is_inclusive() {
        let ok = Rect {
            x: -5,
            y: 0,
            w: MAX_RECT_SIDE_PX,
            h: 1,
        };
        assert_eq!(check_rect(&ok, RectKind::Crop), Ok(()));
        let big = Rect {
            h: MAX_RECT_SIDE_PX + 1,
            ..ok
        };
        assert_eq!(
            check_rect(&big, RectKind::Dest),
            Err(RenderError::RectTooLarge {
                rect: RectKind::Dest,
                w: MAX_RECT_SIDE_PX,
                h: MAX_RECT_SIDE_PX + 1,
                limit: MAX_RECT_SIDE_PX
            })
        );
    }

    #[test]
    fn messages_name_the_image_and_the_values() {
        let e = RenderError::PitchTooSmall {
            image: Image::Game,
            width: 4,
            pitch: 15,
            min: 16,
        };
        assert_eq!(
            e.to_string(),
            "the game frame pitch is 15 bytes, below the 16 bytes of one row of 4 pixels"
        );
    }
}
