/// The part of the scanout buffers that a mode shows, and where.
///
/// The renderer draws a `w` by `h` mode at the GL origin, the bottom left.
/// A window surface stores the last GL row first, so the picture is in the
/// last `h` rows of a buffer of height `H`. The plane reads it there and
/// shows it at the top left of the CRTC, with no scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaneRect {
    /// `SRC_X`, `SRC_Y`, `SRC_W`, `SRC_H`, in 16.16 fixed point.
    pub src: [u64; 4],
    /// `CRTC_X`, `CRTC_Y`, `CRTC_W`, `CRTC_H`, in pixels.
    pub crtc: [u64; 4],
}

/// A mode that does not fit the buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the mode {width}x{height} is larger than the buffers of {buffer_width}x{buffer_height}")]
pub struct TooLarge {
    /// The mode width, in pixels.
    pub width: u16,
    /// The mode height, in lines.
    pub height: u16,
    /// The buffer width, in pixels.
    pub buffer_width: u16,
    /// The buffer height, in lines.
    pub buffer_height: u16,
}

impl PlaneRect {
    /// The rectangles of a `width` by `height` mode on buffers of
    /// `buffer_width` by `buffer_height`.
    ///
    /// # Errors
    ///
    /// [`TooLarge`] when the mode is wider or taller than the buffers.
    pub fn for_mode(
        width: u16,
        height: u16,
        buffer_width: u16,
        buffer_height: u16,
    ) -> Result<PlaneRect, TooLarge> {
        if width > buffer_width || height > buffer_height {
            return Err(TooLarge {
                width,
                height,
                buffer_width,
                buffer_height,
            });
        }
        let (w, h) = (u64::from(width), u64::from(height));
        let top = u64::from(buffer_height - height);
        Ok(PlaneRect {
            src: [0, top << FIXED_SHIFT, w << FIXED_SHIFT, h << FIXED_SHIFT],
            crtc: [0, 0, w, h],
        })
    }
}

/// The fraction bits of a plane source coordinate.
const FIXED_SHIFT: u32 = 16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_reads_the_last_rows_of_the_buffer() {
        let r = PlaneRect::for_mode(320, 240, 1920, 576).unwrap();
        assert_eq!(r.src, [0, 336 << 16, 320 << 16, 240 << 16]);
        assert_eq!(r.crtc, [0, 0, 320, 240]);
    }

    #[test]
    fn a_mode_of_the_buffer_size_reads_all_of_it() {
        let r = PlaneRect::for_mode(1920, 576, 1920, 576).unwrap();
        assert_eq!(r.src, [0, 0, 1920 << 16, 576 << 16]);
    }

    #[test]
    fn a_larger_mode_is_refused() {
        let e = TooLarge {
            width: 1921,
            height: 240,
            buffer_width: 1920,
            buffer_height: 576,
        };
        assert_eq!(PlaneRect::for_mode(1921, 240, 1920, 576), Err(e));
        assert!(PlaneRect::for_mode(640, 577, 1920, 576).is_err());
    }
}
