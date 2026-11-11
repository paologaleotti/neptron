//! The picture the CRTC composes, through a writeback connector.

use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use drm::buffer::{Buffer as _, DrmFourcc};
use drm::control::Device as _;
use neptron_backend::DisplayError;

use super::display::KmsDisplay;
use super::request::{self, Request};
use super::{events, ffi};

/// One picture from the writeback connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// Width, in pixels.
    pub width: u32,
    /// Height, in lines.
    pub height: u32,
    /// XRGB8888 pixels as DRM defines them (little-endian), rows from the
    /// top, no padding.
    pub pixels: Vec<u8>,
}

/// The longest wait for the writeback fence.
pub const WRITEBACK_WAIT_LIMIT_MS: u64 = 1_000;

impl KmsDisplay {
    /// Captures the picture on the CRTC now. Needs `writeback` in the
    /// configuration. It allocates, and is for tests and diagnostics, not
    /// for the frame loop.
    ///
    /// # Errors
    ///
    /// [`DisplayError::NoMode`] before the first mode,
    /// [`DisplayError::Device`] with no writeback connector, while a flip is
    /// pending, or when a step of the capture fails.
    pub fn capture(&mut self) -> Result<Capture, DisplayError> {
        let failed = |operation: &'static str| {
            move |e: std::io::Error| DisplayError::Device {
                operation,
                reason: e.to_string(),
            }
        };
        let Some(active) = self.active else {
            return Err(DisplayError::NoMode);
        };
        if self.state.is_pending() {
            return Err(DisplayError::Device {
                operation: "capture",
                reason: "a flip is pending; call wait_flip first".to_owned(),
            });
        }
        let Some(wb) = self.objects.writeback else {
            return Err(DisplayError::Device {
                operation: "capture",
                reason: "writeback is off in the configuration".to_owned(),
            });
        };
        let (w, h) = (
            u32::from(active.requested.hactive),
            u32::from(active.requested.vactive),
        );
        let mut buffer = self
            .card
            .create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)
            .map_err(failed("create the writeback buffer"))?;
        let result = self
            .card
            .add_framebuffer(&buffer, 24, 32)
            .map_err(failed("add the writeback framebuffer"))
            .and_then(|fb| {
                let r = self.write_back(wb, u32::from(fb), w, h, &mut buffer);
                // The capture result is the error to report.
                let _ = self.card.destroy_framebuffer(fb);
                r
            });
        // A buffer that cannot be freed here is freed when the descriptor
        // closes.
        let _ = self.card.destroy_dumb_buffer(buffer);
        result
    }

    fn write_back(
        &mut self,
        wb: super::device::Writeback,
        fb: u32,
        w: u32,
        h: u32,
        buffer: &mut drm::control::dumbbuffer::DumbBuffer,
    ) -> Result<Capture, DisplayError> {
        let device =
            |operation: &'static str, reason: String| DisplayError::Device { operation, reason };
        // The kernel writes the fence descriptor here during the commit.
        // The address is exposed to it as an integer, and the slot is
        // atomic, so the read after the commit sees the kernel's write.
        let slot = AtomicI32::new(-1);
        let mut req = Request::new();
        req.add(wb.connector, wb.fb_id, u64::from(fb));
        req.add(wb.connector, wb.out_fence_ptr, slot.as_ptr() as u64);
        request::test_then_commit(&req, &self.card, 0)
            .map_err(|e| device("writeback commit", format!("{e:?}")))?;
        let fence = ffi::take_fence(slot.load(Ordering::SeqCst))
            .ok_or_else(|| device("writeback commit", "the kernel gave no fence".to_owned()))?;
        let limit = Duration::from_millis(WRITEBACK_WAIT_LIMIT_MS);
        match events::wait_readable(&fence, limit) {
            Ok(true) => {}
            Ok(false) => {
                return Err(device(
                    "wait for the writeback",
                    format!("the fence did not signal within {WRITEBACK_WAIT_LIMIT_MS} ms"),
                ));
            }
            Err(e) => return Err(device("wait for the writeback", e.to_string())),
        }
        let pitch = buffer.pitch() as usize;
        let map = self
            .card
            .map_dumb_buffer(buffer)
            .map_err(|e| device("map the writeback buffer", e.to_string()))?;
        let row = w as usize * 4;
        let mut pixels = Vec::with_capacity(row * h as usize);
        for y in 0..h as usize {
            let start = y * pitch;
            let line = map.get(start..start + row).ok_or_else(|| {
                device(
                    "read the writeback buffer",
                    format!("row {y} is outside the {} byte buffer", map.len()),
                )
            })?;
            pixels.extend_from_slice(line);
        }
        Ok(Capture {
            width: w,
            height: h,
            pixels,
        })
    }
}
