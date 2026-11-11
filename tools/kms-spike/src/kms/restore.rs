//! The state before the run, and putting it back.
//!
//! The original mode goes back to the kernel only through a
//! `ValidatedMode`, like every test mode. When the original mode does not
//! convert or does not pass `validate` for the chosen preset, the restore
//! switches the output off instead: no signal is safe for every display.

use drm::control::{AtomicCommitFlags, Device as ControlDevice, atomic::AtomicModeReq, property};
use neptron_safety::{ValidatedMode, validate};
use neptron_types::{GpuCaps, MonitorProfile};

use super::KmsError;
use super::card::{Card, Objects};
use crate::kernel_mode::{KernelMode, RawMode};
use crate::plan::modeline_text;
use crate::report::Original;
use crate::supervisor::warn;

/// The saved state and what the restore puts back.
pub(super) struct Saved {
    connector_crtc: u64,
    crtc_active: u64,
    plane: Vec<u64>,
    target: Target,
    pub original: Original,
}

enum Target {
    /// The original state, with its validated mode.
    Previous(ValidatedMode),
    /// The output off.
    Off,
}

/// Puts the saved state back when it is dropped, also during a panic.
pub(super) struct Guard<'a> {
    card: &'a Card,
    objects: &'a Objects,
    saved: &'a Saved,
    armed: bool,
}

/// Reads the state of the chosen objects before the first modeset.
pub(super) fn save(
    card: &Card,
    objects: &Objects,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Result<Saved, KmsError> {
    let value = |all: &[(String, property::Handle, u64)], id: property::Handle| {
        all.iter().find(|(_, h, _)| *h == id).map(|(_, _, v)| *v)
    };
    let conn = card.properties(objects.connector)?;
    let crtc = card.properties(objects.crtc)?;
    let plane = card.properties(objects.plane)?;
    let connector_crtc = value(&conn, objects.props.connector_crtc_id).unwrap_or(0);
    let crtc_active = value(&crtc, objects.props.crtc_active).unwrap_or(0);
    let plane_values = objects
        .props
        .plane
        .iter()
        .map(|&id| value(&plane, id).unwrap_or(0))
        .collect();
    let mode = card
        .get_crtc(objects.crtc)
        .map_err(KmsError::io("read the CRTC"))?
        .mode();
    let (target, original) = match mode {
        None => (
            Target::Off,
            Original {
                active: crtc_active != 0,
                mode: None,
                problem: None,
                restore_plan: format!(
                    "the output off, because the CRTC had no mode. {CONSOLE_NOTE}"
                ),
            },
        ),
        Some(m) => judge(&raw_of(&m), crtc_active != 0, profile, caps),
    };
    Ok(Saved {
        connector_crtc,
        crtc_active,
        plane: plane_values,
        target,
        original,
    })
}

/// The fields of a kernel mode as the spike reads them.
pub(super) fn raw_of(m: &drm::control::Mode) -> RawMode {
    let (hd, vd) = m.size();
    let (hs, he, ht) = m.hsync();
    let (vs, ve, vt) = m.vsync();
    RawMode {
        clock_khz: m.clock(),
        h: [hd, hs, he, ht],
        v: [vd, vs, ve, vt],
        hskew: m.hskew(),
        vscan: m.vscan(),
        flags: m.flags().bits(),
    }
}

fn judge(
    raw: &RawMode,
    active: bool,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> (Target, Original) {
    let modeline = match raw.to_modeline() {
        Ok(m) => m,
        Err(e) => {
            return (
                Target::Off,
                Original {
                    active,
                    mode: None,
                    problem: Some(e.to_string()),
                    restore_plan: format!(
                        "the output off, because the original mode cannot be judged. \
                         {CONSOLE_NOTE}"
                    ),
                },
            );
        }
    };
    match validate(&modeline, profile, caps) {
        Ok(v) => (
            Target::Previous(v),
            Original {
                active,
                mode: Some(modeline),
                problem: None,
                restore_plan: format!(
                    "the original mode {} and the original framebuffer",
                    modeline_text(&modeline)
                ),
            },
        ),
        Err(r) => (
            Target::Off,
            Original {
                active,
                mode: Some(modeline),
                problem: Some(format!("validate rejects the original mode: {r}")),
                restore_plan: format!(
                    "the output off, because the original mode fails validation for this \
                     preset. {CONSOLE_NOTE}"
                ),
            },
        ),
    }
}

impl<'a> Guard<'a> {
    /// Arms the restore for `saved`.
    pub(super) fn new(card: &'a Card, objects: &'a Objects, saved: &'a Saved) -> Guard<'a> {
        Guard {
            card,
            objects,
            saved,
            armed: true,
        }
    }

    /// Restores now and disarms the guard.
    pub(super) fn finish(mut self) -> Result<String, String> {
        self.armed = false;
        restore(self.card, self.objects, self.saved)
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        match restore(self.card, self.objects, self.saved) {
            Ok(text) => warn(&format!(
                "kms-spike: display restored on the way out: {text}"
            )),
            Err(text) => loud(&text),
        }
    }
}

/// Prints a restore failure so that it cannot be missed.
pub(super) fn loud(text: &str) {
    warn(&format!(
        "\n!!!!!!!!!!!!!!!! kms-spike: RESTORE FAILED !!!!!!!!!!!!!!!!\n{text}\n\
         The display may still show a test mode. Switch the monitor off, then \
         switch to another virtual terminal or reboot.\n\
         !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!\n"
    ));
}

/// What the owner sees after an "output off" restore.
const CONSOLE_NOTE: &str = "With fbdev emulation (a normal desktop kernel) the kernel console \
     then takes the display back in its own mode when the tool closes the device: this is \
     expected. Without it the screen stays dark until the next program sets a mode.";

fn restore(card: &Card, objects: &Objects, saved: &Saved) -> Result<String, String> {
    let first = match &saved.target {
        Target::Previous(v) => previous_request(card, objects, saved, v)
            .and_then(|(req, blob)| {
                let r = commit(card, req);
                destroy_blob(card, blob);
                r
            })
            .map(|()| saved.original.restore_plan.clone()),
        Target::Off => {
            commit(card, off_request(objects)).map(|()| saved.original.restore_plan.clone())
        }
    };
    first.map_err(|e| match commit(card, off_request(objects)) {
        Ok(()) => format!("the restore failed ({e}); the output is OFF instead"),
        Err(off) => {
            format!("the restore failed ({e}), and switching the output off failed too ({off})")
        }
    })
}

fn previous_request(
    card: &Card,
    objects: &Objects,
    saved: &Saved,
    mode: &ValidatedMode,
) -> Result<(AtomicModeReq, u64), String> {
    let record = KernelMode::from_validated(mode, "restore").map_err(|e| e.to_string())?;
    let blob = card
        .create_property_blob(&record)
        .map_err(|e| format!("cannot create the mode blob: {e}"))?;
    let blob_id = match blob {
        property::Value::Blob(id) => id,
        _ => 0,
    };
    let mut req = AtomicModeReq::new();
    req.add_raw_property(
        objects.connector.into(),
        objects.props.connector_crtc_id,
        saved.connector_crtc,
    );
    req.add_property(objects.crtc, objects.props.crtc_mode_id, blob);
    req.add_raw_property(
        objects.crtc.into(),
        objects.props.crtc_active,
        saved.crtc_active,
    );
    for (&id, &value) in objects.props.plane.iter().zip(&saved.plane) {
        req.add_raw_property(objects.plane.into(), id, value);
    }
    Ok((req, blob_id))
}

/// The output off: no mode, no plane, no connector on the CRTC.
pub(super) fn off_request(objects: &Objects) -> AtomicModeReq {
    let mut req = AtomicModeReq::new();
    req.add_raw_property(objects.connector.into(), objects.props.connector_crtc_id, 0);
    req.add_raw_property(objects.crtc.into(), objects.props.crtc_mode_id, 0);
    req.add_raw_property(objects.crtc.into(), objects.props.crtc_active, 0);
    req.add_raw_property(objects.plane.into(), objects.props.plane[0], 0);
    req.add_raw_property(objects.plane.into(), objects.props.plane[1], 0);
    req
}

/// A test-only commit, then the real one.
fn commit(card: &Card, req: AtomicModeReq) -> Result<(), String> {
    card.atomic_commit(
        AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
        req.clone(),
    )
    .map_err(|e| format!("the test-only commit failed: {e}"))?;
    card.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, req)
        .map_err(|e| format!("the commit failed: {e}"))
}

fn destroy_blob(card: &Card, blob: u64) {
    if blob != 0 && card.destroy_property_blob(blob).is_err() {
        warn(&format!(
            "kms-spike: cannot free mode blob {blob}; the kernel frees it at exit"
        ));
    }
}
