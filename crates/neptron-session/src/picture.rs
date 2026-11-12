//! The last game frame, kept by the session.

use neptron_types::{Frame, PixelFormat};

/// A copy of the last good frame of the core. The host's frame lives only
/// until the next call into the core, and the session presents the frame
/// again when the core does not run: for the second flip of double refresh
/// and under the quick menu. The buffer is made at launch for the core's
/// largest frame, so a copy does not allocate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Picture {
    bytes: Vec<u8>,
    len: usize,
    width: u16,
    height: u16,
    pitch: usize,
    format: PixelFormat,
}

/// The largest pixel: XRGB8888.
const MAX_BYTES_PER_PIXEL: usize = 4;

impl Picture {
    /// An empty picture with room for a frame of `max_width` x
    /// `max_height` in any format.
    pub(crate) fn new(max_width: u16, max_height: u16) -> Self {
        Self {
            bytes: vec![0; Self::max_bytes(max_width, max_height)],
            len: 0,
            width: 0,
            height: 0,
            pitch: 0,
            format: PixelFormat::Xrgb8888,
        }
    }

    /// Makes room for a larger maximum size, after the core changed it.
    /// This allocates, as the AV change of the host does.
    pub(crate) fn reserve(&mut self, max_width: u16, max_height: u16) {
        let bytes = Self::max_bytes(max_width, max_height);
        if bytes > self.bytes.len() {
            self.bytes.resize(bytes, 0);
        }
    }

    /// Copies `frame`. The host gives packed rows, so the frame's bytes are
    /// its pitch times its height.
    pub(crate) fn store(&mut self, frame: &Frame<'_>) {
        let len = frame.pitch.saturating_mul(usize::from(frame.height));
        let Some(source) = frame.data.get(..len) else {
            return;
        };
        if len > self.bytes.len() {
            self.bytes.resize(len, 0);
        }
        self.bytes[..len].copy_from_slice(source);
        self.len = len;
        self.width = frame.width;
        self.height = frame.height;
        self.pitch = frame.pitch;
        self.format = frame.format;
    }

    /// The frame, or `None` before the first one.
    pub(crate) fn frame(&self) -> Option<Frame<'_>> {
        (self.len > 0).then(|| Frame {
            data: &self.bytes[..self.len],
            width: self.width,
            height: self.height,
            pitch: self.pitch,
            format: self.format,
        })
    }

    /// The size of the frame, or `None` before the first one.
    pub(crate) fn size(&self) -> Option<(u16, u16)> {
        (self.len > 0).then_some((self.width, self.height))
    }

    fn max_bytes(max_width: u16, max_height: u16) -> usize {
        usize::from(max_width) * usize::from(max_height) * MAX_BYTES_PER_PIXEL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_frame_reads_back() {
        let data: Vec<u8> = (0..24).collect();
        let mut picture = Picture::new(4, 3);
        assert_eq!(picture.frame(), None);
        let frame = Frame {
            data: &data,
            width: 3,
            height: 4,
            pitch: 6,
            format: PixelFormat::Rgb565,
        };
        picture.store(&frame);
        assert_eq!(picture.frame(), Some(frame));
        assert_eq!(picture.size(), Some((3, 4)));
    }

    #[test]
    fn a_frame_shorter_than_its_size_is_not_stored() {
        let data = [0_u8; 10];
        let mut picture = Picture::new(4, 4);
        picture.store(&Frame {
            data: &data,
            width: 4,
            height: 4,
            pitch: 8,
            format: PixelFormat::Rgb565,
        });
        assert_eq!(picture.frame(), None);
    }

    #[test]
    fn reserve_grows_and_never_shrinks() {
        let mut picture = Picture::new(2, 2);
        picture.reserve(8, 8);
        assert_eq!(picture.bytes.len(), 256);
        picture.reserve(1, 1);
        assert_eq!(picture.bytes.len(), 256);
    }
}
