//! The last game frame, kept by the session.

use neptron_types::{Frame, PixelFormat};

/// A copy of the last good frame of the core. The host's frame lives only
/// until the next call into the core, and the session presents the frame
/// again when the core does not run: for the second flip of double refresh
/// and under the quick menu. The buffer is made at launch for the core's
/// largest frame, so a copy does not allocate.
///
/// A GL core leaves its picture in the image of the display; the picture
/// then keeps only its size, and the buffer takes a read back of the
/// image on a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Picture {
    bytes: Vec<u8>,
    len: usize,
    width: u16,
    height: u16,
    pitch: usize,
    format: PixelFormat,
    /// The last picture is in the image of a GL core.
    gpu: bool,
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
            gpu: false,
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
        self.gpu = false;
    }

    /// Takes a picture of `width` by `height` that a GL core drew into the
    /// image of the display.
    pub(crate) fn store_gpu(&mut self, width: u16, height: u16) {
        self.len = 0;
        self.width = width;
        self.height = height;
        self.gpu = true;
    }

    /// The size of the last picture when a GL core drew it, else `None`.
    pub(crate) fn gpu_size(&self) -> Option<(u16, u16)> {
        self.gpu.then_some((self.width, self.height))
    }

    /// The buffer for a read back of the GL picture: XRGB8888 rows of
    /// `width * 4` bytes, with no padding. `None` before a GL picture.
    pub(crate) fn read_back_buffer(&mut self) -> Option<&mut [u8]> {
        if !self.gpu {
            return None;
        }
        let len = usize::from(self.width) * usize::from(self.height) * MAX_BYTES_PER_PIXEL;
        if len > self.bytes.len() {
            self.bytes.resize(len, 0);
        }
        self.bytes.get_mut(..len)
    }

    /// The GL picture in the buffer after a read back into
    /// [`Picture::read_back_buffer`].
    pub(crate) fn read_back_frame(&self) -> Option<Frame<'_>> {
        let len = usize::from(self.width) * usize::from(self.height) * MAX_BYTES_PER_PIXEL;
        (self.gpu && len > 0).then(|| Frame {
            data: &self.bytes[..len],
            width: self.width,
            height: self.height,
            pitch: usize::from(self.width) * MAX_BYTES_PER_PIXEL,
            format: PixelFormat::Xrgb8888,
        })
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

    /// The size of the frame, in memory or in the image of a GL core, or
    /// `None` before the first one.
    pub(crate) fn size(&self) -> Option<(u16, u16)> {
        (self.len > 0 || self.gpu).then_some((self.width, self.height))
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
    fn a_gl_picture_keeps_its_size_and_no_memory_frame() {
        let mut picture = Picture::new(4, 2);
        assert_eq!(picture.gpu_size(), None);
        assert!(picture.read_back_buffer().is_none());
        picture.store_gpu(3, 2);
        assert_eq!(picture.frame(), None, "the pixels are on the GPU");
        assert_eq!(picture.size(), Some((3, 2)));
        assert_eq!(picture.gpu_size(), Some((3, 2)));
        let buffer = picture.read_back_buffer().expect("a buffer");
        assert_eq!(buffer.len(), 24);
        buffer.fill(7);
        let frame = picture.read_back_frame().expect("the read back");
        assert_eq!((frame.width, frame.height, frame.pitch), (3, 2, 12));
        assert_eq!(frame.format, PixelFormat::Xrgb8888);
        let data = [1_u8; 8];
        picture.store(&Frame {
            data: &data,
            width: 2,
            height: 2,
            pitch: 4,
            format: PixelFormat::Rgb565,
        });
        assert_eq!(picture.gpu_size(), None, "a memory frame replaces it");
        assert!(picture.read_back_frame().is_none());
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
