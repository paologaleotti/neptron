//! `set_mode` and `blank`: the only paths that change the mode on the
//! wire. `MODE_ID` gets a blob of a `ModeRecord::from_validated`, or 0.

use drm::control::{Device as _, connector, crtc};
use neptron_backend::DisplayError;
use neptron_safety::ValidatedMode;
use neptron_types::{Filter, Modeline, Rect, Scanlines, Scene, TvStandard};

use super::device::{PLANE_CRTC_ID, PLANE_FB_ID};
use super::display::{KmsDisplay, render_error};
use super::request::{self, ALLOW_MODESET, Refusal, Request};
use crate::request::{ModesetIds, OutputIds, modeset, output_off};
use crate::tv::{self, TvWrite};
use crate::{ModeRecord, NextStep, PlaneRect};

impl KmsDisplay {
    /// Commits `mode`, test-only first, and returns the mode read back.
    pub(super) fn commit_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        self.state.check_set_mode()?;
        let m = *mode.mode();
        let tv = mode.tv_standard();
        match self.wire.next_step(&m, tv) {
            NextStep::Reuse(readback) => return Ok(readback),
            NextStep::ReadAgain => {
                let readback = self.read_back(tv)?;
                self.wire.read_back(readback);
                return Ok(readback);
            }
            NextStep::Commit => {}
        }
        let refused = |reason: String| DisplayError::ModeRefused { mode: m, reason };
        let record = ModeRecord::from_validated(mode).map_err(|e| refused(e.to_string()))?;
        let tv_write =
            tv::plan(tv, self.objects.tv_mode.as_ref()).map_err(|e| refused(e.to_string()))?;
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
        let req = self.modeset_request(tv_write, blob, fb, &rect);
        let result = request::test_then_commit(&req, &self.card, ALLOW_MODESET);
        // The CRTC state holds its own reference to the blob. A blob that
        // cannot be freed here is freed when the descriptor closes.
        let _ = self.card.destroy_property_blob(blob);
        if let Err(refusal) = result {
            self.gbm.release(bo);
            return Err(match refusal {
                Refusal::Test(e) => refused(format!("the test-only commit failed: {e}")),
                other => other.device("modeset commit"),
            });
        }
        if let Some(old) = self.front.replace(bo) {
            self.gbm.release(old);
        }
        // The mode is on the wire now, whatever the readback gives: a later
        // request for another mode must commit, and drop must restore.
        self.wire.committed(m, tv);
        self.state.mode_committed();
        let readback = self.read_back(tv)?;
        self.wire.read_back(readback);
        Ok(readback)
    }

    /// Turns the output off in one commit, test-only first. A refusal
    /// changes nothing, because a commit is all or nothing.
    pub(super) fn commit_blank(&mut self) -> Result<(), DisplayError> {
        self.state.check_blank()?;
        request::test_then_commit(&self.off_request(), &self.card, ALLOW_MODESET)
            .map_err(|refusal| refusal.device("blank commit"))?;
        self.wire.blanked();
        self.state.blanked();
        // The blocking commit returned, so the plane no longer reads the
        // buffer.
        if let Some(old) = self.front.take() {
            self.gbm.release(old);
        }
        Ok(())
    }

    /// The request that turns this output off.
    pub(super) fn off_request(&self) -> Request {
        let o = &self.objects;
        output_off(&OutputIds {
            connector: o.connector,
            connector_crtc_id: o.props.connector_crtc_id,
            writeback: o.writeback.map(|wb| (wb.connector, wb.crtc_id)),
            crtc: o.crtc,
            crtc_mode_id: o.props.crtc_mode_id,
            crtc_active: o.props.crtc_active,
            plane: o.plane,
            plane_fb_id: o.props.plane[PLANE_FB_ID],
            plane_crtc_id: o.props.plane[PLANE_CRTC_ID],
        })
    }

    fn modeset_request(
        &self,
        tv: Option<TvWrite>,
        blob: u64,
        fb: u32,
        rect: &PlaneRect,
    ) -> Request {
        let o = &self.objects;
        let ids = ModesetIds {
            connector: o.connector,
            connector_crtc_id: o.props.connector_crtc_id,
            margins: o.props.margins,
            writeback: o.writeback.map(|wb| (wb.connector, wb.crtc_id)),
            crtc: o.crtc,
            crtc_mode_id: o.props.crtc_mode_id,
            crtc_active: o.props.crtc_active,
            plane: o.plane,
            plane_props: o.props.plane,
        };
        modeset(&ids, tv, blob, fb, rect)
    }

    /// The mode the kernel stores for the CRTC, after a check that the
    /// connector's `TV mode` value is the standard `tv` on that mode.
    fn read_back(&self, tv: Option<TvStandard>) -> Result<Modeline, DisplayError> {
        let readback = self.read_mode()?;
        if let Some(standard) = tv {
            let failed = |reason: String| DisplayError::Device {
                operation: "read the TV standard back",
                reason,
            };
            let property = self
                .objects
                .tv_mode
                .as_ref()
                .ok_or_else(|| failed("the connector has no TV mode property".to_owned()))?;
            let handle: connector::Handle = drm::control::from_u32(self.objects.connector)
                .ok_or_else(|| failed("connector id 0".to_owned()))?;
            let set = self
                .card
                .get_properties(handle)
                .map_err(|e| failed(e.to_string()))?;
            let (ids, values) = set.as_props_and_values();
            let value = ids
                .iter()
                .zip(values)
                .find(|(id, _)| u32::from(**id) == property.prop)
                .map(|(_, &value)| value)
                .ok_or_else(|| failed("the connector reports no TV mode value".to_owned()))?;
            tv::check_readback(standard, value, property, &readback)
                .map_err(|e| failed(e.to_string()))?;
        }
        Ok(readback)
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
