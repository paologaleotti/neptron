//! The pixels of the fake display. With [`Render::Pixels`], a back buffer
//! holds the queued frame and a front buffer the frame on screen. With
//! [`Render::Scene`], a copy of the queued scene and of the scene on screen
//! take their place, and pixels are rendered only when a test asks.

use std::collections::{BTreeMap, BTreeSet};

use neptron_gfx::{OutputBuffer, ReferenceRenderer, RenderError};
use neptron_types::{Filter, Frame, Modeline, PixelFormat, Rect, Scene};

use crate::{RgbImage, frame_hash};

use super::Render;

#[derive(Debug, Default)]
pub(crate) struct Screen {
    renderer: ReferenceRenderer,
    render: Render,
    width: u32,
    height: u32,
    back: Vec<u8>,
    front: Vec<u8>,
    queued: OwnedScene,
    shown: OwnedScene,
    front_shown: bool,
    capture_requests: BTreeSet<u64>,
    captures: BTreeMap<u64, RgbImage>,
}

impl Screen {
    /// Sizes the buffers for `mode`. The screen is black until the first
    /// flip in the mode. Only this call and a first scene of a larger size
    /// allocate.
    pub(crate) fn set_mode(&mut self, mode: &Modeline, render: Render) {
        self.render = render;
        self.width = u32::from(mode.hactive);
        self.height = u32::from(mode.vactive);
        let len = match render {
            Render::Pixels => self.width as usize * self.height as usize * 4,
            Render::Scene | Render::Off => 0,
        };
        for buffer in [&mut self.back, &mut self.front] {
            buffer.clear();
            buffer.resize(len, 0);
        }
        self.front_shown = false;
    }

    /// Checks `scene`, keeps what the render mode needs, and returns the
    /// hash of the mode, or `None` when rendering is off.
    pub(crate) fn present(
        &mut self,
        index: u64,
        scene: &Scene<'_>,
    ) -> Result<Option<u64>, RenderError> {
        match self.render {
            Render::Pixels => {
                let (width, height) = (self.width, self.height);
                draw(&mut self.renderer, scene, &mut self.back, width, height)?;
                if self.capture_requests.remove(&index)
                    && let Some(image) = image(&self.back, width, height)
                {
                    self.captures.insert(index, image);
                }
                Ok(Some(frame_hash(&self.back)))
            }
            Render::Scene => {
                draw(&mut self.renderer, scene, &mut [], 0, 0)?;
                self.queued.copy_from(scene, self.width, self.height);
                if self.capture_requests.remove(&index)
                    && let Some(image) = self.render_owned(Which::Queued)
                {
                    self.captures.insert(index, image);
                }
                Ok(Some(frame_hash(&self.queued.bytes)))
            }
            Render::Off => {
                draw(&mut self.renderer, scene, &mut [], 0, 0)?;
                Ok(None)
            }
        }
    }

    /// The queued frame is now on screen.
    pub(crate) fn flip(&mut self) {
        std::mem::swap(&mut self.back, &mut self.front);
        std::mem::swap(&mut self.queued, &mut self.shown);
        self.front_shown = true;
    }

    /// The frame on screen, or `None` before the first flip in the mode or
    /// with rendering off.
    pub(crate) fn shown(&mut self) -> Option<RgbImage> {
        if !self.front_shown {
            return None;
        }
        match self.render {
            Render::Pixels => image(&self.front, self.width, self.height),
            Render::Scene => self.render_owned(Which::Shown),
            Render::Off => None,
        }
    }

    pub(crate) fn request_capture(&mut self, index: u64) {
        self.capture_requests.insert(index);
    }

    pub(crate) fn captured(&self, index: u64) -> Option<RgbImage> {
        self.captures.get(&index).cloned()
    }

    /// Renders a kept scene into a new buffer of the mode size.
    fn render_owned(&mut self, which: Which) -> Option<RgbImage> {
        let owned = match which {
            Which::Queued => &self.queued,
            Which::Shown => &self.shown,
        };
        let mut pixels = vec![0_u8; self.width as usize * self.height as usize * 4];
        draw(
            &mut self.renderer,
            &owned.scene(),
            &mut pixels,
            self.width,
            self.height,
        )
        .ok()?;
        image(&pixels, self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy)]
enum Which {
    Queued,
    Shown,
}

/// A scene copied into one buffer: a header with the mode size, the crop,
/// the destination and the UI destination, the filter and the size and format of each frame, then the
/// rows of the game frame and of the UI canvas with no padding. All values
/// are little-endian. The hash of [`Render::Scene`] is the hash of these
/// bytes, so it is the same on every host.
#[derive(Debug, Default)]
struct OwnedScene {
    bytes: Vec<u8>,
    crop: Option<Rect>,
    dest: Option<Rect>,
    ui_dest: Option<Rect>,
    filter: Option<Filter>,
    game: Option<Layout>,
    ui: Option<Layout>,
}

/// Where a frame sits in [`OwnedScene::bytes`].
#[derive(Debug, Clone, Copy)]
struct Layout {
    start: usize,
    width: u16,
    height: u16,
    format: PixelFormat,
}

impl OwnedScene {
    /// Copies `scene`, which the renderer checked: each frame holds its
    /// rows.
    fn copy_from(&mut self, scene: &Scene<'_>, width: u32, height: u32) {
        let bytes = &mut self.bytes;
        bytes.clear();
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        for rect in [scene.crop, scene.dest, scene.ui_dest] {
            bytes.extend_from_slice(&rect.x.to_le_bytes());
            bytes.extend_from_slice(&rect.y.to_le_bytes());
            bytes.extend_from_slice(&rect.w.to_le_bytes());
            bytes.extend_from_slice(&rect.h.to_le_bytes());
        }
        bytes.push(match scene.filter {
            Filter::Nearest => 0,
            Filter::SharpBilinear => 1,
        });
        for frame in [&scene.game, &scene.ui] {
            match frame {
                None => bytes.push(0),
                Some(f) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&f.width.to_le_bytes());
                    bytes.extend_from_slice(&f.height.to_le_bytes());
                    bytes.push(match f.format {
                        PixelFormat::Xrgb8888 => 0,
                        PixelFormat::Rgb565 => 1,
                        PixelFormat::Xrgb1555 => 2,
                    });
                }
            }
        }
        self.game = scene.game.as_ref().map(|f| copy_rows(bytes, f));
        self.ui = scene.ui.as_ref().map(|f| copy_rows(bytes, f));
        self.crop = Some(scene.crop);
        self.dest = Some(scene.dest);
        self.ui_dest = Some(scene.ui_dest);
        self.filter = Some(scene.filter);
    }

    fn scene(&self) -> Scene<'_> {
        let frame = |layout: &Layout| {
            let pitch = usize::from(layout.width) * layout.format.bytes_per_pixel();
            let len = pitch * usize::from(layout.height);
            Frame {
                data: self
                    .bytes
                    .get(layout.start..layout.start + len)
                    .unwrap_or(&[]),
                width: layout.width,
                height: layout.height,
                pitch,
                format: layout.format,
            }
        };
        let empty = Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
        Scene {
            game: self.game.as_ref().map(frame),
            crop: self.crop.unwrap_or(empty),
            dest: self.dest.unwrap_or(empty),
            filter: self.filter.unwrap_or(Filter::Nearest),
            ui: self.ui.as_ref().map(frame),
            ui_dest: self.ui_dest.unwrap_or(empty),
        }
    }
}

/// Appends the rows of `frame` to `bytes` with no padding.
fn copy_rows(bytes: &mut Vec<u8>, frame: &Frame<'_>) -> Layout {
    let start = bytes.len();
    let row = usize::from(frame.width) * frame.format.bytes_per_pixel();
    for y in 0..usize::from(frame.height) {
        let from = y * frame.pitch;
        // The renderer checked that the frame holds every row.
        if let Some(line) = frame.data.get(from..from + row) {
            bytes.extend_from_slice(line);
        }
    }
    Layout {
        start,
        width: frame.width,
        height: frame.height,
        format: frame.format,
    }
}

fn draw(
    renderer: &mut ReferenceRenderer,
    scene: &Scene<'_>,
    data: &mut [u8],
    width: u32,
    height: u32,
) -> Result<(), RenderError> {
    let mut out = OutputBuffer {
        data,
        width,
        height,
        pitch: width as usize * 4,
    };
    renderer.render(scene, &mut out)
}

/// The image of a buffer of `width` by `height` XRGB8888 pixels with no
/// padding. Every caller sizes the buffer so, which makes `None`
/// impossible.
fn image(data: &[u8], width: u32, height: u32) -> Option<RgbImage> {
    if width == 0 || height == 0 {
        return None;
    }
    RgbImage::from_xrgb8888(data, width, height, width as usize * 4).ok()
}
