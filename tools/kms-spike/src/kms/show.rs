//! One confirmed run: buffers, then each valid mode with a test-only
//! commit, the real commit, a readback and the timing measurement, then
//! the restore.

use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use drm::buffer::{Buffer as _, DrmFourcc};
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    AtomicCommitFlags, Device as ControlDevice, Event, atomic::AtomicModeReq, framebuffer, property,
};
use drm::{Device, VblankWaitFlags, VblankWaitTarget};
use neptron_safety::{ValidatedMode, validate};
use neptron_types::{GpuCaps, Modeline, MonitorProfile};

use super::KmsError;
use super::card::{self, Card, Objects};
use super::restore::{self, Guard};
use crate::kernel_mode::{KernelMode, differences};
use crate::output;
use crate::pattern::{Canvas, label};
use crate::plan::{Planned, modeline_text};
use crate::report::{Framebuffer, KmsResult, Output, Readback, Report, System};
use crate::supervisor::{Link, warn};
use crate::timing::{self, Sample};

/// The inputs of one confirmed run.
pub struct Inputs<'a> {
    /// The DRM device node.
    pub device: &'a Path,
    /// The connector the user named.
    pub connector: Option<&'a str>,
    /// The preset as a profile, for `validate`.
    pub profile: &'a MonitorProfile,
    /// The GPU values for `validate`.
    pub caps: &'a GpuCaps,
    /// The plan; only its valid modes show.
    pub planned: &'a [Planned],
    /// How long each mode shows.
    pub per_mode: Duration,
    /// Nothing shows after this time.
    pub deadline: Instant,
}

/// Shows every valid planned mode, fills `report`, and restores the
/// original state.
pub fn run<W: Write>(
    inputs: &Inputs<'_>,
    report: &mut Report,
    link: &mut Link<W>,
) -> Result<(), KmsError> {
    let Inputs {
        device,
        connector,
        profile,
        caps,
        planned,
        per_mode,
        deadline,
    } = *inputs;
    let card = Card::open(device)?;
    card.enable_atomic()?;
    report.system = Some(system(&card, device));
    let objects = card::select(&card, connector)?;
    report.output = Some(Output {
        connector: objects.connector_info.name.clone(),
        connector_id: objects.connector.into(),
        connector_state: objects.connector_info.state.name().to_owned(),
        encoder_id: objects.encoder.map(Into::into),
        crtc_id: objects.crtc.into(),
        crtc_index: objects.crtc_index,
        plane_id: objects.plane.into(),
    });
    link.line(&format!(
        "using connector {}, CRTC {}, plane {}",
        objects.connector_info.name,
        u32::from(objects.crtc),
        u32::from(objects.plane)
    ))?;
    if let Some(note) = output::note(&objects.connector_info) {
        link.line(&format!("note: {note}"))?;
    }
    let saved = restore::save(&card, &objects, profile, caps)?;
    report.original = Some(saved.original.clone());
    if let Some(problem) = &saved.original.problem {
        link.line(&format!("original mode: {problem}"))?;
    }
    link.line(&format!(
        "on exit the tool restores {}",
        saved.original.restore_plan
    ))?;

    let (width, height) = largest(planned);
    let mut buffers = Buffers::create(&card, width, height)?;
    report.framebuffer = Some(Framebuffer {
        format: "XR24 (XRGB8888)".to_owned(),
        modifier: "none given: a dumb buffer, which is linear".to_owned(),
        count: 2,
        width: u32::from(width),
        height: u32::from(height),
        pitch: buffers.dumb[0].pitch(),
    });

    let guard = Guard::new(&card, &objects, &saved);
    let ctx = Ctx {
        card: &card,
        objects: &objects,
        profile,
        caps,
    };
    for (index, p) in planned.iter().enumerate() {
        let Some(valid) = p.valid() else { continue };
        if Instant::now() >= deadline {
            report.stopped = Some("the hard time limit of the run is reached".to_owned());
            break;
        }
        let name = p.derived.test.name();
        if let Err(e) = link.line(&format!("{name}: {}", modeline_text(valid.mode()))) {
            report.stopped = Some(e.to_string());
            break;
        }
        let end_by = deadline.min(Instant::now() + per_mode);
        let (result, stop) = ctx.show(&mut buffers, valid, name, end_by, link);
        let gone = result
            .readback
            .as_ref()
            .and_then(|r| link.line(&readback_line(name, r)).err());
        report.modes[index].kms = Some(result);
        if let Some(why) = stop.or_else(|| gone.map(|e| e.to_string())) {
            report.stopped = Some(why);
            break;
        }
    }
    let restored = guard.finish();
    match &restored {
        Ok(text) => {
            if link.line(&format!("restored: {text}")).is_err() {
                warn(&format!("kms-spike: restored: {text}"));
            }
        }
        Err(text) => restore::loud(text),
    }
    report.restore = Some(restored);
    for e in buffers.destroy(&card) {
        report.errors.push(e);
    }
    Ok(())
}

struct Ctx<'a> {
    card: &'a Card,
    objects: &'a Objects,
    profile: &'a MonitorProfile,
    caps: &'a GpuCaps,
}

struct Buffers {
    dumb: Vec<DumbBuffer>,
    fbs: Vec<framebuffer::Handle>,
}

/// Samples kept per mode: ten seconds at 240 events per second. The vectors
/// get this capacity before the mode shows, so the loop does not allocate.
const SAMPLES_MAX: usize = 2_400;
/// Vblank waits that fail in a row before the measurement of a mode stops.
const VBLANK_ERRORS_MAX: u32 = 3;
/// Vblanks to wait for the last queued flip after the display time.
const DRAIN_VBLANKS: u32 = 2;

impl Ctx<'_> {
    /// Shows one mode. Returns what happened, and a reason to stop the run
    /// when the run must not go on.
    fn show<W: Write>(
        &self,
        buffers: &mut Buffers,
        valid: &ValidatedMode,
        name: &str,
        end_by: Instant,
        link: &mut Link<W>,
    ) -> (KmsResult, Option<String>) {
        let mut result = KmsResult::default();
        let m = *valid.mode();
        if let Err(e) = buffers.draw(self.card, &m, name) {
            result.errors.push(e);
            return (result, None);
        }
        let record = match KernelMode::from_validated(valid, name) {
            Ok(r) => r,
            Err(e) => {
                result.test_commit = Some(Err(e.to_string()));
                return (result, None);
            }
        };
        let blob = match self.card.create_property_blob(&record) {
            Ok(b) => b,
            Err(e) => {
                result.test_commit = Some(Err(format!("cannot create the mode blob: {e}")));
                return (result, None);
            }
        };
        let req = self.modeset(blob, buffers.fbs[0], &m);
        let test = self.card.atomic_commit(
            AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
            req.clone(),
        );
        result.test_commit = Some(test.as_ref().map(|_| ()).map_err(ToString::to_string));
        let committed = test.and_then(|()| {
            self.card
                .atomic_commit(AtomicCommitFlags::ALLOW_MODESET, req)
        });
        if result.test_commit == Some(Ok(())) {
            result.commit = Some(committed.as_ref().map(|_| ()).map_err(ToString::to_string));
        }
        // The CRTC state holds its own reference to the blob.
        if let property::Value::Blob(id) = blob
            && self.card.destroy_property_blob(id).is_err()
        {
            result.errors.push(format!("cannot free mode blob {id}"));
        }
        if committed.is_err() {
            return (result, None);
        }
        let shown = Instant::now();
        let readback = self.readback(&m);
        let rejected = readback.rejection.clone();
        result.readback = Some(readback);
        if let Some(r) = rejected {
            return (
                result,
                Some(format!(
                    "the mode read back from {name} fails validation ({r}); the run stops"
                )),
            );
        }
        let stop = self.measure(buffers, &m, end_by, link, &mut result);
        result.shown_ms = u64::try_from(shown.elapsed().as_millis()).unwrap_or(u64::MAX);
        (result, stop)
    }

    fn modeset(
        &self,
        blob: property::Value<'static>,
        fb: framebuffer::Handle,
        m: &Modeline,
    ) -> AtomicModeReq {
        let o = self.objects;
        let mut req = AtomicModeReq::new();
        req.add_property(
            o.connector,
            o.props.connector_crtc_id,
            property::Value::CRTC(Some(o.crtc)),
        );
        req.add_property(o.crtc, o.props.crtc_mode_id, blob);
        req.add_property(o.crtc, o.props.crtc_active, property::Value::Boolean(true));
        let (w, h) = (u64::from(m.hactive), u64::from(m.vactive));
        let values = [
            property::Value::Framebuffer(Some(fb)),
            property::Value::CRTC(Some(o.crtc)),
            property::Value::UnsignedRange(0),
            property::Value::UnsignedRange(0),
            property::Value::UnsignedRange(w << 16),
            property::Value::UnsignedRange(h << 16),
            property::Value::SignedRange(0),
            property::Value::SignedRange(0),
            property::Value::UnsignedRange(w),
            property::Value::UnsignedRange(h),
        ];
        for (&id, value) in o.props.plane.iter().zip(values) {
            req.add_property(o.plane, id, value);
        }
        req
    }

    fn flip(&self, fb: framebuffer::Handle) -> io::Result<()> {
        let mut req = AtomicModeReq::new();
        req.add_property(
            self.objects.plane,
            self.objects.props.plane[0],
            property::Value::Framebuffer(Some(fb)),
        );
        self.card.atomic_commit(
            AtomicCommitFlags::PAGE_FLIP_EVENT | AtomicCommitFlags::NONBLOCK,
            req,
        )
    }

    fn readback(&self, requested: &Modeline) -> Readback {
        let mode = match self.card.get_crtc(self.objects.crtc) {
            Err(e) => Err(format!("cannot read the CRTC: {e}")),
            Ok(info) => match info.mode() {
                None => Err("the CRTC reports no mode".to_owned()),
                Some(m) => restore::raw_of(&m).to_modeline().map_err(|e| e.to_string()),
            },
        };
        match mode {
            Ok(m) => Readback {
                differences: differences(requested, &m),
                rejection: validate(&m, self.profile, self.caps)
                    .err()
                    .map(|r| r.to_string()),
                mode: Ok(m),
            },
            Err(e) => Readback {
                mode: Err(e.clone()),
                differences: Vec::new(),
                rejection: Some(e),
            },
        }
    }

    /// Flips between the two buffers and waits for each vblank until
    /// `end_by`. Every wait is bounded: the vblank wait by the kernel's own
    /// timeout, the event read by the non-blocking device.
    fn measure<W: Write>(
        &self,
        buffers: &Buffers,
        m: &Modeline,
        end_by: Instant,
        link: &mut Link<W>,
        result: &mut KmsResult,
    ) -> Option<String> {
        let mut vblanks = Vec::with_capacity(SAMPLES_MAX);
        let mut flips = Vec::with_capacity(SAMPLES_MAX);
        let mut errors = ErrorLog::default();
        let mut next = 1;
        let mut pending = false;
        let mut vblank_errors = 0;
        let mut stop = None;
        while Instant::now() < end_by {
            if let Err(e) = link.heartbeat(Instant::now()) {
                stop = Some(e.to_string());
                break;
            }
            if !pending {
                match self.flip(buffers.fbs[next]) {
                    Ok(()) => {
                        pending = true;
                        next = 1 - next;
                        result.flips_queued += 1;
                    }
                    Err(e) => errors.add(&format!("page flip: {e}")),
                }
            }
            if self.wait_vblank(&mut vblanks, &mut errors) {
                vblank_errors = 0;
            } else {
                vblank_errors += 1;
                if vblank_errors >= VBLANK_ERRORS_MAX {
                    errors.add("the vblank wait failed three times in a row; measurement stopped");
                    break;
                }
            }
            pending &= !self.drain(&mut flips, &mut errors, result);
        }
        for _ in 0..DRAIN_VBLANKS {
            if !pending {
                break;
            }
            self.wait_vblank(&mut vblanks, &mut errors);
            pending &= !self.drain(&mut flips, &mut errors, result);
        }
        result.vblank = timing::stats(&vblanks);
        result.flips = timing::stats(&flips);
        if let Some(v) = &result.vblank {
            result.implied_pclock_hz = Some(timing::implied_pclock_hz(
                v.rate_mhz,
                m.htotal,
                m.vtotal,
                m.interlace,
            ));
        }
        if let Some(f) = &result.flips {
            result.cadence = Some(timing::cadence(f.event_rate_mhz, m.field_rate_mhz()));
        }
        result.errors.extend(errors.into_lines());
        stop
    }

    /// Waits for the next vblank and keeps its timestamp. False on error.
    fn wait_vblank(&self, samples: &mut Vec<Sample>, errors: &mut ErrorLog) -> bool {
        match self.card.wait_vblank(
            VblankWaitTarget::Relative(1),
            VblankWaitFlags::empty(),
            self.objects.crtc_index,
            0,
        ) {
            Ok(reply) => {
                if let Some(t) = reply.time()
                    && samples.len() < samples.capacity()
                {
                    samples.push(Sample {
                        time_ns: u64::try_from(t.as_nanos()).unwrap_or(u64::MAX),
                        sequence: reply.frame(),
                    });
                }
                true
            }
            Err(e) => {
                errors.add(&format!("vblank wait: {e}"));
                false
            }
        }
    }

    /// Reads every pending event. True when a flip of this CRTC completed.
    fn drain(
        &self,
        flips: &mut Vec<Sample>,
        errors: &mut ErrorLog,
        result: &mut KmsResult,
    ) -> bool {
        let mut completed = false;
        loop {
            match self.card.receive_events() {
                Ok(events) => {
                    for event in events {
                        if let Event::PageFlip(f) = event
                            && f.crtc == self.objects.crtc
                        {
                            completed = true;
                            result.flips_completed += 1;
                            if flips.len() < flips.capacity() {
                                flips.push(Sample {
                                    time_ns: u64::try_from(f.duration.as_nanos())
                                        .unwrap_or(u64::MAX),
                                    sequence: f.frame,
                                });
                            }
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return completed,
                Err(e) => {
                    errors.add(&format!("event read: {e}"));
                    return completed;
                }
            }
        }
    }
}

impl Buffers {
    fn create(card: &Card, width: u16, height: u16) -> Result<Buffers, KmsError> {
        let mut out = Buffers {
            dumb: Vec::with_capacity(2),
            fbs: Vec::with_capacity(2),
        };
        for _ in 0..2 {
            let db = card
                .create_dumb_buffer(
                    (u32::from(width), u32::from(height)),
                    DrmFourcc::Xrgb8888,
                    32,
                )
                .map_err(KmsError::io("create a dumb buffer"))?;
            let fb = card
                .add_framebuffer(&db, 24, 32)
                .map_err(KmsError::io("add a framebuffer"))?;
            out.dumb.push(db);
            out.fbs.push(fb);
        }
        Ok(out)
    }

    fn draw(&mut self, card: &Card, m: &Modeline, name: &str) -> Result<(), String> {
        let text = label(name, m.hactive, m.vactive, m.field_rate_mhz());
        for db in &mut self.dumb {
            let pitch = db.pitch() as usize;
            let mut map = card
                .map_dumb_buffer(db)
                .map_err(|e| format!("cannot map a dumb buffer: {e}"))?;
            let mut canvas = Canvas::over(
                map.as_mut(),
                pitch,
                usize::from(m.hactive),
                usize::from(m.vactive),
            )
            .map_err(|e| format!("buffer too small: {} of {} bytes", e.len, e.needed))?;
            canvas.draw_pattern(&text);
        }
        Ok(())
    }

    /// Frees the buffers after the restore, so that no plane scans out a
    /// removed framebuffer. Returns the errors.
    fn destroy(self, card: &Card) -> Vec<String> {
        let mut errors = Vec::new();
        for fb in self.fbs {
            if let Err(e) = card.destroy_framebuffer(fb) {
                errors.push(format!("cannot remove framebuffer {}: {e}", u32::from(fb)));
            }
        }
        for db in self.dumb {
            if let Err(e) = card.destroy_dumb_buffer(db) {
                errors.push(format!("cannot free a dumb buffer: {e}"));
            }
        }
        errors
    }
}

/// Kernel errors of one mode, each text once with how often it came.
#[derive(Default)]
struct ErrorLog(Vec<(String, u32)>);

impl ErrorLog {
    fn add(&mut self, text: &str) {
        match self.0.iter_mut().find(|(t, _)| t == text) {
            Some((_, n)) => *n += 1,
            None => self.0.push((text.to_owned(), 1)),
        }
    }

    fn into_lines(self) -> Vec<String> {
        self.0
            .into_iter()
            .map(|(t, n)| {
                if n == 1 {
                    t
                } else {
                    format!("{t} ({n} times)")
                }
            })
            .collect()
    }
}

fn largest(planned: &[Planned]) -> (u16, u16) {
    planned
        .iter()
        .filter_map(Planned::valid)
        .map(|v| (v.mode().hactive, v.mode().vactive))
        .fold((1, 1), |(w, h), (a, b)| (w.max(a), h.max(b)))
}

fn system(card: &Card, device: &Path) -> System {
    let (driver, driver_version, driver_description) = match card.get_driver() {
        Ok(d) => (
            d.name().to_string_lossy().into_owned(),
            format!("{}.{}.{}", d.version.0, d.version.1, d.version.2),
            d.description().to_string_lossy().into_owned(),
        ),
        Err(e) => (format!("unknown ({e})"), String::new(), String::new()),
    };
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map_or_else(|e| format!("unknown ({e})"), |s| s.trim().to_owned());
    System {
        device: device.display().to_string(),
        driver,
        driver_version,
        driver_description,
        kernel,
    }
}

fn readback_line(name: &str, r: &Readback) -> String {
    match &r.mode {
        Err(e) => format!("{name}: readback: {e}"),
        Ok(_) if r.differences.is_empty() => format!("{name}: readback equals the request"),
        Ok(_) => format!("{name}: readback differs: {}", r.differences.join(", ")),
    }
}
