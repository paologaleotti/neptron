//! The defined state at drop: the last validated mode with no picture, or
//! no signal.

use std::os::fd::AsFd as _;
use std::time::Duration;

use super::device::{PLANE_CRTC_ID, PLANE_FB_ID};
use super::display::{FLIP_WAIT_LIMIT_MS, KmsDisplay};
use super::events;
use super::request::{self, ALLOW_MODESET, Request};

impl Drop for KmsDisplay {
    fn drop(&mut self) {
        if self.active.is_some() {
            if self.state.is_pending() {
                let limit = Duration::from_millis(FLIP_WAIT_LIMIT_MS);
                // A pending flip makes the commits below fail with EBUSY;
                // when it does not come, the fallback below still runs.
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
        let mut off = Request::new();
        off.add(o.connector, o.props.connector_crtc_id, 0);
        if let Some(wb) = o.writeback {
            off.add(wb.connector, wb.crtc_id, 0);
        }
        off.add(o.crtc, o.props.crtc_mode_id, 0);
        off.add(o.crtc, o.props.crtc_active, 0);
        off.add(o.plane, o.props.plane[PLANE_FB_ID], 0);
        off.add(o.plane, o.props.plane[PLANE_CRTC_ID], 0);
        request::test_then_commit(&off, &self.card, ALLOW_MODESET).map_err(|second| {
            format!(
                "the plane-off commit failed ({first:?}) and the output-off commit failed \
                     too ({second:?})"
            )
        })
    }
}
