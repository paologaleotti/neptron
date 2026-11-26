use crate::Frame;

/// The game picture of a [`Scene`](crate::Scene): pixels in memory, or an
/// image that a display holds on the GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamePicture<'a> {
    /// A frame in memory, as a software core draws it.
    Memory(Frame<'a>),
    /// An image on the GPU, as a hardware core draws it.
    Gpu(GpuFrame),
}

/// A game picture that a display holds on the GPU.
///
/// The picture is the `width` by `height` part of the image that starts at
/// texel `(0, 0)` of a storage of `storage_width` by `storage_height`
/// texels. `origin` says how picture lines map to texel rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuFrame {
    /// The image, valid only for the display that made it.
    pub image: GpuImageId,
    /// Width of the picture, in pixels.
    pub width: u16,
    /// Height of the picture, in lines.
    pub height: u16,
    /// Width of the storage of the image, in texels.
    pub storage_width: u16,
    /// Height of the storage of the image, in texel rows.
    pub storage_height: u16,
    /// Where picture line 0 is in the storage.
    pub origin: FrameOrigin,
}

/// A handle of a GPU image. Only the display that made it can resolve it,
/// and only until the image is destroyed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GpuImageId(pub u32);

/// How the lines of a GPU picture map to the texel rows of its storage.
/// Texel rows count from GL `y = 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FrameOrigin {
    /// Picture line `y` is texel row `y`: line 0, the top of the picture,
    /// is at GL `y = 0`. The libretro default.
    #[default]
    TopLeft,
    /// Picture line `y` is texel row `height - 1 - y`: line 0 is at GL
    /// `y = height - 1`, as GL draws by default.
    BottomLeft,
}

impl GamePicture<'_> {
    /// Width of the picture, in pixels.
    pub fn width(&self) -> u16 {
        match self {
            GamePicture::Memory(frame) => frame.width,
            GamePicture::Gpu(gpu) => gpu.width,
        }
    }

    /// Height of the picture, in lines.
    pub fn height(&self) -> u16 {
        match self {
            GamePicture::Memory(frame) => frame.height,
            GamePicture::Gpu(gpu) => gpu.height,
        }
    }
}

impl FrameOrigin {
    /// The texel row of picture line `line` in a picture of `height`
    /// lines. `None` for a line outside the picture.
    pub fn storage_row(self, line: u16, height: u16) -> Option<u16> {
        if line >= height {
            return None;
        }
        Some(match self {
            FrameOrigin::TopLeft => line,
            FrameOrigin::BottomLeft => height - 1 - line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    #[test]
    fn the_size_comes_from_either_kind() {
        let data = [0_u8; 8];
        let memory = GamePicture::Memory(Frame {
            data: &data,
            width: 2,
            height: 1,
            pitch: 8,
            format: PixelFormat::Xrgb8888,
        });
        assert_eq!((memory.width(), memory.height()), (2, 1));
        let gpu = GamePicture::Gpu(GpuFrame {
            image: GpuImageId(7),
            width: 320,
            height: 240,
            storage_width: 1024,
            storage_height: 512,
            origin: FrameOrigin::BottomLeft,
        });
        assert_eq!((gpu.width(), gpu.height()), (320, 240));
    }

    #[test]
    fn the_origin_maps_lines_to_rows() {
        assert_eq!(FrameOrigin::TopLeft.storage_row(0, 240), Some(0));
        assert_eq!(FrameOrigin::TopLeft.storage_row(239, 240), Some(239));
        assert_eq!(FrameOrigin::BottomLeft.storage_row(0, 240), Some(239));
        assert_eq!(FrameOrigin::BottomLeft.storage_row(239, 240), Some(0));
        assert_eq!(FrameOrigin::TopLeft.storage_row(240, 240), None);
        assert_eq!(FrameOrigin::BottomLeft.storage_row(0, 0), None);
        assert_eq!(FrameOrigin::default(), FrameOrigin::TopLeft);
    }
}
