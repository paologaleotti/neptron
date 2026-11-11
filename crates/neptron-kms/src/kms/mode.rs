//! `set_mode`: the one path that changes the mode on the wire.

use drm::control::{Device as _, crtc};
use neptron_backend::DisplayError;
use neptron_safety::ValidatedMode;
use neptron_types::{Filter, Modeline, Rect, Scanlines, Scene};

use super::display::{Active, KmsDisplay, render_error};
use super::request::{self, ALLOW_MODESET, Refusal, Request};
use crate::{ModeRecord, PlaneRect};

impl KmsDisplay {
    /// Commits `mode`, test-only first, and returns the mode read back.
    pub(super) fn commit_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        self.state.check_set_mode()?;
        let m = *mode.mode();
        if let Some(active) = self.active
            && active.requested == m
        {
            return Ok(active.readback);
        }
        let refused = |reason: String| DisplayError::ModeRefused { mode: m, reason };
        let record = ModeRecord::from_validated(mode).map_err(|e| refused(e.to_string()))?;
        let rect = PlaneRect::for_mode(
            m.hactive,
            m.vactive,
            self.config.buffer_width,
            self.config.buffer_height,
        )
        .map_err(|e| refused(e.to_string()))?;
        self.renderer
            .draw_to_window(&black(), u32::from(m.hactive), u32::from(m.vactive))
            .map_err(render_error)?;
        let (bo, fb) = self.lock_swapped()?;
        let blob = match self.card.create_property_blob(&kernel_record(&record)) {
            Ok(drm::control::property::Value::Blob(id)) => id,
            Ok(_) | Err(_) => {
                self.gbm.release(bo);
                return Err(DisplayError::Device {
                    operation: "create the mode blob",
                    reason: "the kernel refused the blob".to_owned(),
                });
            }
        };
        let req = self.modeset_request(blob, fb, &rect);
        let result = request::test_then_commit(&req, &self.card, ALLOW_MODESET);
        // The CRTC state holds its own reference to the blob. A blob that
        // cannot be freed here is freed when the descriptor closes.
        let _ = self.card.destroy_property_blob(blob);
        match result {
            Ok(()) => {}
            Err(Refusal::Test(e)) => {
                self.gbm.release(bo);
                return Err(refused(format!("the test-only commit failed: {e}")));
            }
            Err(Refusal::Commit(e)) => {
                self.gbm.release(bo);
                return Err(DisplayError::Device {
                    operation: "modeset commit",
                    reason: e.to_string(),
                });
            }
        }
        if let Some(old) = self.front.replace(bo) {
            self.gbm.release(old);
        }
        self.state.mode_committed();
        let readback = self.read_mode()?;
        self.active = Some(Active {
            requested: m,
            readback,
        });
        Ok(readback)
    }

    fn modeset_request(&self, blob: u64, fb: u32, rect: &PlaneRect) -> Request {
        let o = &self.objects;
        let crtc = u64::from(o.crtc);
        let mut req = Request::new();
        req.add(o.connector, o.props.connector_crtc_id, crtc);
        if let Some(wb) = o.writeback {
            req.add(wb.connector, wb.crtc_id, crtc);
        }
        req.add(o.crtc, o.props.crtc_mode_id, blob);
        req.add(o.crtc, o.props.crtc_active, 1);
        let values = [
            u64::from(fb),
            crtc,
            rect.src[0],
            rect.src[1],
            rect.src[2],
            rect.src[3],
            rect.crtc[0],
            rect.crtc[1],
            rect.crtc[2],
            rect.crtc[3],
        ];
        for (&prop, value) in o.props.plane.iter().zip(values) {
            req.add(o.plane, prop, value);
        }
        req
    }

    /// The mode the kernel stores for the CRTC.
    pub(super) fn read_mode(&self) -> Result<Modeline, DisplayError> {
        let failed = |reason: String| DisplayError::Device {
            operation: "read the mode back",
            reason,
        };
        let handle: crtc::Handle = drm::control::from_u32(self.objects.crtc)
            .ok_or_else(|| failed("CRTC id 0".to_owned()))?;
        let mode = self
            .card
            .get_crtc(handle)
            .map_err(|e| failed(e.to_string()))?
            .mode()
            .ok_or_else(|| failed("the CRTC reports no mode".to_owned()))?;
        record_of(mode)
            .to_modeline()
            .map_err(|e| failed(e.to_string()))
    }
}

/// The record of a mode the kernel reports.
pub(super) fn record_of(mode: drm::control::Mode) -> ModeRecord {
    let raw: drm_ffi::drm_mode_modeinfo = mode.into();
    let mut name = [0; crate::mode_record::NAME_LEN];
    for (dst, &src) in name.iter_mut().zip(raw.name.iter()) {
        *dst = src as u8;
    }
    ModeRecord {
        clock_khz: raw.clock,
        h: [raw.hdisplay, raw.hsync_start, raw.hsync_end, raw.htotal],
        v: [raw.vdisplay, raw.vsync_start, raw.vsync_end, raw.vtotal],
        hskew: raw.hskew,
        vscan: raw.vscan,
        vrefresh_hz: raw.vrefresh,
        flags: raw.flags,
        kind: raw.type_,
        name,
    }
}

fn kernel_record(r: &ModeRecord) -> drm_ffi::drm_mode_modeinfo {
    let mut name = [0; crate::mode_record::NAME_LEN];
    for (dst, &src) in name.iter_mut().zip(r.name.iter()) {
        *dst = src as std::ffi::c_char;
    }
    drm_ffi::drm_mode_modeinfo {
        clock: r.clock_khz,
        hdisplay: r.h[0],
        hsync_start: r.h[1],
        hsync_end: r.h[2],
        htotal: r.h[3],
        hskew: r.hskew,
        vdisplay: r.v[0],
        vsync_start: r.v[1],
        vsync_end: r.v[2],
        vtotal: r.v[3],
        vscan: r.vscan,
        vrefresh: r.vrefresh_hz,
        flags: r.flags,
        type_: r.kind,
        name,
    }
}

/// A scene that shows nothing: black at every pixel.
fn black() -> Scene<'static> {
    let none = Rect {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    };
    Scene {
        game: None,
        crop: none,
        dest: none,
        filter: Filter::Nearest,
        ui: None,
        ui_dest: none,
        scanlines: Scanlines::NONE,
    }
}
