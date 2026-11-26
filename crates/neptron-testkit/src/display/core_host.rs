//! The fake display as the host of the context of a GL core, on the
//! reference core GL of this crate (`docs/spec/hw-render.md`, section 7.2).

use std::collections::BTreeSet;

use neptron_backend::{
    ContextOffer, ContextRequest, CoreFrameStatus, CoreRender, CoreRenderError, CoreTarget,
};
use neptron_types::{Frame, FrameOrigin, GamePicture, GpuImageId, PixelFormat, Scene};

use super::FakeDisplay;
use crate::gl;

/// The largest side of the image of a core context, in pixels.
pub const CORE_MAX_SIDE_PX: u16 = 4096;

/// One call of the [`CoreRender`] of the fake display, with its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreCall {
    /// `create`, and the image it made.
    Create {
        /// The request.
        request: ContextRequest,
        /// The width of the image.
        max_width: u16,
        /// The height of the image.
        max_height: u16,
        /// The image, or the error.
        result: Result<GpuImageId, CoreRenderError>,
    },
    /// `resize`.
    Resize {
        /// The new width.
        max_width: u16,
        /// The new height.
        max_height: u16,
        /// What it returned.
        result: Result<(), CoreRenderError>,
    },
    /// `begin`.
    Begin(Result<(), CoreRenderError>),
    /// `end`.
    End(Result<CoreFrameStatus, CoreRenderError>),
    /// `read_back`.
    ReadBack(Result<(), CoreRenderError>),
    /// `destroy`.
    Destroy,
}

/// A [`CoreCall`] and when it came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreRecord {
    /// The `present` calls before it, failed ones included: the order of
    /// the core calls against the frames.
    pub presents_before: u64,
    /// The call.
    pub call: CoreCall,
}

/// The state of the core context of a fake display.
#[derive(Debug, Default)]
pub(crate) struct CoreHost {
    made: Option<Made>,
    begun: bool,
    next_image: u32,
    /// The picture of a `Gpu` scene, resolved to memory. Sized at `create`
    /// and `resize`.
    picture: Vec<u8>,
    create_fault: Option<String>,
    losses: BTreeSet<u64>,
    end_calls: u64,
    records: Vec<CoreRecord>,
}

#[derive(Debug, Clone, Copy)]
struct Made {
    framebuffer: u32,
    image: GpuImageId,
    width: u16,
    height: u16,
}

impl FakeDisplay {
    /// Scripts the next `create` of the core context to fail with
    /// [`CoreRenderError::Driver`] and `reason`.
    pub fn fault_core_create(&self, reason: String) {
        self.lock().core.create_fault = Some(reason);
    }

    /// Scripts the `end` call number `end_call` (from 0) to report a lost
    /// context. The image and the framebuffer are then gone, as after a
    /// GPU reset.
    pub fn fault_core_loss(&self, end_call: u64) {
        self.lock().core.losses.insert(end_call);
    }

    /// Every call of the core context, in order.
    pub fn core_calls(&self) -> Vec<CoreRecord> {
        self.lock().core.records.clone()
    }

    fn record_core(&self, call: CoreCall) {
        let mut state = self.lock();
        let presents_before = state.frames.len() as u64;
        state.core.records.push(CoreRecord {
            presents_before,
            call,
        });
    }
}

impl CoreRender for FakeDisplay {
    fn offer(&mut self) -> ContextOffer {
        self.lock().options.core_offer.unwrap_or(ContextOffer::NONE)
    }

    fn create(
        &mut self,
        request: &ContextRequest,
        max_width: u16,
        max_height: u16,
    ) -> Result<CoreTarget, CoreRenderError> {
        let offer = self.offer();
        let result = {
            let mut state = self.lock();
            let core = &mut state.core;
            core.drop_made();
            core.make(&offer, request, max_width, max_height)
        };
        self.record_core(CoreCall::Create {
            request: *request,
            max_width,
            max_height,
            result: result.clone().map(|t| t.image),
        });
        result
    }

    fn resize(&mut self, max_width: u16, max_height: u16) -> Result<(), CoreRenderError> {
        let result = self.lock().core.resize(max_width, max_height);
        self.record_core(CoreCall::Resize {
            max_width,
            max_height,
            result: result.clone(),
        });
        result
    }

    fn begin(&mut self) -> Result<(), CoreRenderError> {
        let result = self.lock().core.begin();
        self.record_core(CoreCall::Begin(result.clone()));
        result
    }

    fn end(&mut self) -> Result<CoreFrameStatus, CoreRenderError> {
        let result = self.lock().core.end();
        self.record_core(CoreCall::End(result.clone()));
        result
    }

    fn read_back(
        &mut self,
        out: &mut [u8],
        width: u16,
        height: u16,
        origin: FrameOrigin,
    ) -> Result<(), CoreRenderError> {
        let result = self.lock().core.read_back(out, width, height, origin);
        self.record_core(CoreCall::ReadBack(result.clone()));
        result
    }

    fn destroy(&mut self) {
        self.lock().core.drop_made();
        self.record_core(CoreCall::Destroy);
    }
}

impl CoreHost {
    fn make(
        &mut self,
        offer: &ContextOffer,
        request: &ContextRequest,
        width: u16,
        height: u16,
    ) -> Result<CoreTarget, CoreRenderError> {
        if !offer.holds(request) {
            return Err(CoreRenderError::NotOffered { request: *request });
        }
        check_size("create", width, height, CORE_MAX_SIDE_PX, CORE_MAX_SIDE_PX)?;
        if let Some(reason) = self.create_fault.take() {
            return Err(CoreRenderError::Driver {
                operation: "create",
                reason,
            });
        }
        let framebuffer = gl::make_image(width, height);
        self.next_image = self.next_image.wrapping_add(1);
        let image = GpuImageId(self.next_image);
        self.picture.resize(picture_len(width, height), 0);
        self.made = Some(Made {
            framebuffer,
            image,
            width,
            height,
        });
        Ok(CoreTarget {
            framebuffer,
            image,
            lookup: gl::gl_lookup,
        })
    }

    fn resize(&mut self, width: u16, height: u16) -> Result<(), CoreRenderError> {
        let made = self.made.as_mut().ok_or(CoreRenderError::NoContext {
            operation: "resize",
        })?;
        check_size("resize", width, height, CORE_MAX_SIDE_PX, CORE_MAX_SIDE_PX)?;
        gl::resize_image(made.framebuffer, width, height);
        made.width = width;
        made.height = height;
        self.picture.resize(picture_len(width, height), 0);
        Ok(())
    }

    fn begin(&mut self) -> Result<(), CoreRenderError> {
        if self.made.is_none() {
            return Err(CoreRenderError::NoContext { operation: "begin" });
        }
        gl::set_current(true);
        self.begun = true;
        Ok(())
    }

    fn end(&mut self) -> Result<CoreFrameStatus, CoreRenderError> {
        if !self.begun {
            return Err(CoreRenderError::NoContext { operation: "end" });
        }
        gl::set_current(false);
        self.begun = false;
        let call = self.end_calls;
        self.end_calls = self.end_calls.saturating_add(1);
        let gl_errors = gl::take_errors_raised();
        let context_lost = self.losses.remove(&call);
        if context_lost {
            self.drop_made();
        }
        Ok(CoreFrameStatus {
            gl_errors,
            context_lost,
        })
    }

    fn read_back(
        &mut self,
        out: &mut [u8],
        width: u16,
        height: u16,
        origin: FrameOrigin,
    ) -> Result<(), CoreRenderError> {
        let made = self.made.ok_or(CoreRenderError::NoContext {
            operation: "read_back",
        })?;
        check_size("read_back", width, height, made.width, made.height)?;
        let min = picture_len(width, height);
        if out.len() < min {
            return Err(CoreRenderError::BufferTooSmall {
                len: out.len(),
                min,
            });
        }
        if gl::read_picture(made.framebuffer, out, width, height, origin) {
            Ok(())
        } else {
            Err(CoreRenderError::Driver {
                operation: "read_back",
                reason: "the image is gone".to_owned(),
            })
        }
    }

    fn drop_made(&mut self) {
        if let Some(made) = self.made.take() {
            gl::drop_image(made.framebuffer);
        }
        if self.begun {
            gl::set_current(false);
            self.begun = false;
        }
    }

    /// The game picture of `scene` in memory: a `Gpu` picture read from its
    /// image, upright by its origin, with that origin. `None` for a scene
    /// with no `Gpu` picture.
    ///
    /// # Errors
    ///
    /// The reason for a picture of an image that this display did not
    /// make, that is gone, or that is larger than its image.
    pub(crate) fn resolve(
        &mut self,
        scene: &Scene<'_>,
    ) -> Result<Option<(Frame<'_>, FrameOrigin)>, String> {
        let Some(GamePicture::Gpu(gpu)) = scene.game else {
            return Ok(None);
        };
        let made = self.made.filter(|m| m.image == gpu.image).ok_or_else(|| {
            format!(
                "the GPU image {} is not an image of this display",
                gpu.image.0
            )
        })?;
        if gpu.width > made.width
            || gpu.height > made.height
            || gpu.storage_width != made.width
            || gpu.storage_height != made.height
        {
            return Err(format!(
                "the GPU picture {}x{} of a {}x{} storage does not fit the image of {}x{}",
                gpu.width,
                gpu.height,
                gpu.storage_width,
                gpu.storage_height,
                made.width,
                made.height
            ));
        }
        let len = picture_len(gpu.width, gpu.height);
        let data = self
            .picture
            .get_mut(..len)
            .ok_or_else(|| "the picture buffer is too small".to_owned())?;
        if !gl::read_picture(made.framebuffer, data, gpu.width, gpu.height, gpu.origin) {
            return Err("the GPU image is gone".to_owned());
        }
        let frame = Frame {
            data: &*data,
            width: gpu.width,
            height: gpu.height,
            pitch: usize::from(gpu.width) * 4,
            format: PixelFormat::Xrgb8888,
        };
        Ok(Some((frame, gpu.origin)))
    }
}

fn check_size(
    operation: &'static str,
    width: u16,
    height: u16,
    max_width: u16,
    max_height: u16,
) -> Result<(), CoreRenderError> {
    if width == 0 || height == 0 || width > max_width || height > max_height {
        return Err(CoreRenderError::Size {
            operation,
            width,
            height,
            max_width,
            max_height,
        });
    }
    Ok(())
}

fn picture_len(width: u16, height: u16) -> usize {
    usize::from(width) * usize::from(height) * 4
}
