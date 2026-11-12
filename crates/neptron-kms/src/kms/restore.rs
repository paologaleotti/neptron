//! The defined state at drop: the last validated mode with no picture, or
//! no signal. After a blank no mode is on the wire, and drop commits
//! nothing.

use std::os::fd::AsFd as _;
use std::time::Duration;

use super::device::{PLANE_CRTC_ID, PLANE_FB_ID};
use super::display::{FLIP_WAIT_LIMIT_MS, KmsDisplay};
use super::events;
use super::request::{self, ALLOW_MODESET, Request};

impl Drop for KmsDisplay {
    fn drop(&mut self) {
        if self.wire.on_wire().is_some() {
            if self.state.is_pending() {
                let limit = Duration::from_millis(FLIP_WAIT_LIMIT_MS);
                // The restore commits below are blocking. With a flip
                // pending, the kernel holds a blocking commit until the
                // flip is done, bounded only by the kernel's own commit
                // timeout (10 s in the DRM atomic helpers). Reading the
                // event first, within FLIP_WAIT_LIMIT_MS, keeps that wait
                // short and gives the buffer back. When the event does not
                // come, the restore still runs and the kernel bounds it.
                if let Ok(Some(_)) = events::wait_flip(&self.card, self.objects.crtc, limit) {
                    self.flip_done();
                }
            }
            if let Err(text) = self.restore() {
                eprintln!("neptron-kms: restore at drop failed: {text}");
            }
        }
        for (_, fb) in self.framebuffers.iter_mut().filter_map(Option::take) {
            // The kernel removes a framebuffer it still has when the
            // descriptor closes.
            let _ = drm_ffi::mode::rm_fb(self.card.as_fd(), fb);
        }
        for bo in [self.front.take(), self.queued.take()]
            .into_iter()
            .flatten()
        {
            self.gbm.release(bo);
        }
    }
}

impl KmsDisplay {
    /// The plane off and the mode kept; when the driver refuses that, the
    /// output off.
    fn restore(&self) -> Result<(), String> {
        let o = &self.objects;
        let mut plane_off = Request::new();
        plane_off.add(o.plane, o.props.plane[PLANE_FB_ID], 0);
        plane_off.add(o.plane, o.props.plane[PLANE_CRTC_ID], 0);
        let Err(first) = request::test_then_commit(&plane_off, &self.card, 0) else {
            return Ok(());
        };
        request::test_then_commit(&self.off_request(), &self.card, ALLOW_MODESET).map_err(
            |second| {
                format!(
                    "the plane-off commit failed ({first:?}) and the output-off commit failed \
                     too ({second:?})"
                )
            },
        )
    }
}
