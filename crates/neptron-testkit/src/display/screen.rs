//! The pixels of the fake display: a back buffer for the queued frame and a
//! front buffer for the frame on screen.

use std::collections::{BTreeMap, BTreeSet};

use neptron_gfx::{OutputBuffer, ReferenceRenderer, RenderError};
use neptron_types::{Modeline, Scene};

use crate::{RgbImage, frame_hash};

use super::Render;

#[derive(Debug, Default)]
pub(crate) struct Screen {
    renderer: ReferenceRenderer,
    width: u32,
    height: u32,
    back: Vec<u8>,
    front: Vec<u8>,
    front_shown: bool,
    capture_requests: BTreeSet<u64>,
    captures: BTreeMap<u64, RgbImage>,
}

impl Screen {
    /// Sizes the buffers for `mode`. The screen is black until the first
    /// flip in the mode. This is the only place that allocates pixels.
    pub(crate) fn set_mode(&mut self, mode: &Modeline, render: Render) {
        let (width, height) = match render {
            Render::Pixels => (u32::from(mode.hactive), u32::from(mode.vactive)),
            Render::Off => (0, 0),
        };
        let len = width as usize * height as usize * 4;
        self.width = width;
        self.height = height;
        self.back.clear();
        self.back.resize(len, 0);
        self.front.clear();
        self.front.resize(len, 0);
        self.front_shown = false;
    }

    /// Renders `scene` into the back buffer and returns the pixel hash, or
    /// `None` when rendering is off. A scene is checked in both cases.
    pub(crate) fn render(
        &mut self,
        index: u64,
        scene: &Scene<'_>,
        render: Render,
    ) -> Result<Option<u64>, RenderError> {
        let mut out = OutputBuffer {
            data: &mut self.back,
            width: self.width,
            height: self.height,
            pitch: out_pitch(self.width),
        };
        self.renderer.render(scene, &mut out)?;
        if render == Render::Off {
            return Ok(None);
        }
        if self.capture_requests.remove(&index) {
            // The back buffer holds exactly `height` rows at this pitch, so
            // the conversion cannot fail.
            if let Ok(image) =
                RgbImage::from_xrgb8888(&self.back, self.width, self.height, out_pitch(self.width))
            {
                self.captures.insert(index, image);
            }
        }
        Ok(Some(frame_hash(&self.back)))
    }

    /// The queued frame is now on screen.
    pub(crate) fn flip(&mut self) {
        std::mem::swap(&mut self.back, &mut self.front);
        self.front_shown = true;
    }

    /// The frame on screen, or `None` before the first flip in the mode or
    /// with rendering off.
    pub(crate) fn shown(&self) -> Option<RgbImage> {
        if !self.front_shown || self.width == 0 {
            return None;
        }
        RgbImage::from_xrgb8888(&self.front, self.width, self.height, out_pitch(self.width)).ok()
    }

    pub(crate) fn request_capture(&mut self, index: u64) {
        self.capture_requests.insert(index);
    }

    pub(crate) fn captured(&self, index: u64) -> Option<RgbImage> {
        self.captures.get(&index).cloned()
    }
}

fn out_pitch(width: u32) -> usize {
    width as usize * 4
}
