//! The frame buffer: the copy of the last frame of the core.

use std::collections::TryReserveError;

use neptron_libretro_sys::{
    RETRO_PIXEL_FORMAT_0RGB1555, RETRO_PIXEL_FORMAT_RGB565, RETRO_PIXEL_FORMAT_XRGB8888,
};
use neptron_types::{Frame, PixelFormat};

use crate::geometry::GameGeometry;
use crate::output::{FrameFault, Video};

/// The largest pitch of a frame, in bytes: two rows of the largest frame at
/// 4 bytes per pixel.
pub const MAX_PITCH_BYTES: usize = 32_768;

/// The largest pixel size of any format.
const MAX_BYTES_PER_PIXEL: usize = 4;

/// The `PixelFormat` for a `retro_pixel_format` value.
pub(crate) fn pixel_format_from_core(value: u32) -> Option<PixelFormat> {
    match value {
        RETRO_PIXEL_FORMAT_0RGB1555 => Some(PixelFormat::Xrgb1555),
        RETRO_PIXEL_FORMAT_XRGB8888 => Some(PixelFormat::Xrgb8888),
        RETRO_PIXEL_FORMAT_RGB565 => Some(PixelFormat::Rgb565),
        _ => None,
    }
}

/// How to copy one frame: the checked sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FramePlan {
    pub width: u16,
    pub height: u16,
    pub format: PixelFormat,
    /// Bytes of one row, without padding.
    pub row_bytes: usize,
    /// Distance between rows in the core's buffer.
    pub pitch: usize,
}

impl FramePlan {
    /// Checks a frame against the maximum size, the pixel format and the
    /// pitch limits. A plan that passes covers at most `MAX_PITCH_BYTES *
    /// MAX_DIMENSION_PX` bytes of the core's buffer, so no length overflows.
    pub(crate) fn check(
        width: u32,
        height: u32,
        pitch: usize,
        format: PixelFormat,
        geometry: Option<&GameGeometry>,
    ) -> Result<Self, FrameFault> {
        let geometry = geometry.ok_or(FrameFault::NoGeometry)?;
        if width == 0 || height == 0 {
            return Err(FrameFault::ZeroSize { width, height });
        }
        let over = FrameFault::OverMaximum {
            width,
            height,
            max_width: geometry.max_width,
            max_height: geometry.max_height,
        };
        let w = u16::try_from(width).map_err(|_| over)?;
        let h = u16::try_from(height).map_err(|_| over)?;
        if w > geometry.max_width || h > geometry.max_height {
            return Err(over);
        }
        let row_bytes = usize::from(w) * format.bytes_per_pixel();
        if pitch < row_bytes {
            return Err(FrameFault::PitchTooSmall { pitch, row_bytes });
        }
        if pitch > MAX_PITCH_BYTES {
            return Err(FrameFault::PitchTooLarge {
                pitch,
                limit: MAX_PITCH_BYTES,
            });
        }
        Ok(Self {
            width: w,
            height: h,
            format,
            row_bytes,
            pitch,
        })
    }

    /// Bytes of the packed copy.
    pub(crate) fn packed_len(&self) -> usize {
        self.row_bytes * usize::from(self.height)
    }
}

/// What the video callback did in the current frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Current {
    Missing,
    New,
    Duplicate,
    Rejected(FrameFault),
}

/// The copy of the last good frame, in a buffer of the largest frame size.
#[derive(Debug)]
pub(crate) struct VideoBuffer {
    bytes: Vec<u8>,
    last: Option<FramePlan>,
    current: Current,
}

impl VideoBuffer {
    pub(crate) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            last: None,
            current: Current::Missing,
        }
    }

    /// Makes the buffer large enough for frames of the maximum size of
    /// `geometry`. It never shrinks.
    pub(crate) fn reserve_for(&mut self, geometry: &GameGeometry) -> Result<(), usize> {
        let needed = usize::from(geometry.max_width)
            * usize::from(geometry.max_height)
            * MAX_BYTES_PER_PIXEL;
        grow_zeroed(&mut self.bytes, needed).map_err(|_| needed)
    }

    pub(crate) fn begin(&mut self) {
        self.current = Current::Missing;
    }

    pub(crate) fn duplicate(&mut self) {
        self.current = Current::Duplicate;
    }

    pub(crate) fn reject(&mut self, fault: FrameFault) {
        self.current = Current::Rejected(fault);
    }

    /// The bytes that a frame of `plan` is copied into, or `None` when the
    /// buffer is too small, which `reserve_for` prevents.
    pub(crate) fn target(&mut self, plan: &FramePlan) -> Option<&mut [u8]> {
        self.bytes.get_mut(..plan.packed_len())
    }

    /// Marks the frame of `plan` as copied.
    pub(crate) fn finish(&mut self, plan: FramePlan) {
        self.last = Some(plan);
        self.current = Current::New;
    }

    pub(crate) fn video(&self) -> Video<'_> {
        match self.current {
            Current::Missing => Video::Missing,
            Current::New => self.last_frame().map_or(Video::Missing, Video::Frame),
            Current::Duplicate => Video::Duplicate(self.last_frame()),
            Current::Rejected(fault) => Video::Rejected(fault),
        }
    }

    fn last_frame(&self) -> Option<Frame<'_>> {
        let plan = self.last?;
        Some(Frame {
            data: self.bytes.get(..plan.packed_len())?,
            width: plan.width,
            height: plan.height,
            pitch: plan.row_bytes,
            format: plan.format,
        })
    }
}

/// Grows `buf` to `len` zeroed elements with one fallible allocation.
pub(crate) fn grow_zeroed<T: Copy + Default>(
    buf: &mut Vec<T>,
    len: usize,
) -> Result<(), TryReserveError> {
    if len > buf.len() {
        buf.try_reserve_exact(len - buf.len())?;
        buf.resize(len, T::default());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(max_width: u16, max_height: u16) -> GameGeometry {
        GameGeometry {
            base_width: max_width,
            base_height: max_height,
            max_width,
            max_height,
            aspect_milli: 1333,
        }
    }

    #[test]
    fn pixel_formats_map_to_the_contract() {
        assert_eq!(pixel_format_from_core(0), Some(PixelFormat::Xrgb1555));
        assert_eq!(pixel_format_from_core(1), Some(PixelFormat::Xrgb8888));
        assert_eq!(pixel_format_from_core(2), Some(PixelFormat::Rgb565));
        assert_eq!(pixel_format_from_core(3), None);
        assert_eq!(pixel_format_from_core(u32::MAX), None);
    }

    #[test]
    fn a_good_frame_gives_a_plan() {
        let g = geometry(320, 240);
        let plan = FramePlan::check(256, 224, 640, PixelFormat::Rgb565, Some(&g)).unwrap();
        assert_eq!(plan.row_bytes, 512);
        assert_eq!(plan.packed_len(), 512 * 224);
    }

    #[test]
    fn bad_frames_are_refused() {
        let g = geometry(320, 240);
        let check = |w, h, pitch| FramePlan::check(w, h, pitch, PixelFormat::Xrgb8888, Some(&g));
        assert_eq!(
            FramePlan::check(1, 1, 4, PixelFormat::Xrgb8888, None),
            Err(FrameFault::NoGeometry)
        );
        assert_eq!(
            check(0, 240, 1280),
            Err(FrameFault::ZeroSize {
                width: 0,
                height: 240
            })
        );
        let over = |width, height| FrameFault::OverMaximum {
            width,
            height,
            max_width: 320,
            max_height: 240,
        };
        assert_eq!(check(321, 240, 1284), Err(over(321, 240)));
        assert_eq!(check(320, 241, 1280), Err(over(320, 241)));
        assert_eq!(check(u32::MAX, 1, 1280), Err(over(u32::MAX, 1)));
        assert_eq!(
            check(320, 240, 1279),
            Err(FrameFault::PitchTooSmall {
                pitch: 1279,
                row_bytes: 1280
            })
        );
        assert_eq!(
            check(320, 240, usize::MAX),
            Err(FrameFault::PitchTooLarge {
                pitch: usize::MAX,
                limit: MAX_PITCH_BYTES
            })
        );
    }

    #[test]
    fn the_buffer_keeps_the_last_good_frame() {
        let g = geometry(4, 2);
        let mut v = VideoBuffer::new();
        v.reserve_for(&g).unwrap();
        v.begin();
        assert_eq!(v.video(), Video::Missing);
        v.duplicate();
        assert_eq!(v.video(), Video::Duplicate(None));

        let plan = FramePlan::check(2, 2, 16, PixelFormat::Xrgb8888, Some(&g)).unwrap();
        v.target(&plan).unwrap().copy_from_slice(&[7; 16]);
        v.finish(plan);
        let frame = Frame {
            data: &[7; 16],
            width: 2,
            height: 2,
            pitch: 8,
            format: PixelFormat::Xrgb8888,
        };
        assert_eq!(v.video(), Video::Frame(frame));

        v.begin();
        v.reject(FrameFault::NoGeometry);
        assert_eq!(v.video(), Video::Rejected(FrameFault::NoGeometry));
        v.begin();
        v.duplicate();
        assert_eq!(v.video(), Video::Duplicate(Some(frame)));
    }

    #[test]
    fn the_buffer_grows_and_never_shrinks() {
        let mut v = VideoBuffer::new();
        v.reserve_for(&geometry(320, 240)).unwrap();
        assert_eq!(v.bytes.len(), 320 * 240 * 4);
        v.reserve_for(&geometry(16, 16)).unwrap();
        assert_eq!(v.bytes.len(), 320 * 240 * 4);
        v.reserve_for(&geometry(640, 480)).unwrap();
        assert_eq!(v.bytes.len(), 640 * 480 * 4);
    }
}
