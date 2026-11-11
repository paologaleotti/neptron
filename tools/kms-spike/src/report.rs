//! The report of a run: one versioned JSON file and a readable summary.
//!
//! The format is append-only: a new field gets a new version number, and no
//! field is renamed or removed.

use std::io;
use std::path::Path;

use neptron_types::{GpuCaps, Modeline};

use crate::caps::Allowances;
use crate::json::Json;
use crate::plan::{Planned, Verdict, milli_text, modeline_text};
use crate::timing::{Cadence, Stats};

/// The version of the report format.
pub const REPORT_VERSION: u32 = 1;

/// Everything one run found out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The preset name.
    pub preset: String,
    /// The GPU values that `validate` got.
    pub caps: GpuCaps,
    /// What the user allowed beyond the conservative values.
    pub allow: Allowances,
    /// Active pixels per line.
    pub width: u16,
    /// Display time per mode, in seconds.
    pub seconds_per_mode: u32,
    /// True when the user gave the confirmation flag.
    pub confirmed: bool,
    /// The device, driver and kernel.
    pub system: Option<System>,
    /// The connector, encoder, CRTC and plane used.
    pub output: Option<Output>,
    /// The state of the CRTC before the run.
    pub original: Option<Original>,
    /// The scanout buffers.
    pub framebuffer: Option<Framebuffer>,
    /// One entry per test mode, in the order of the run.
    pub modes: Vec<ModeReport>,
    /// The result of putting the original state back.
    pub restore: Option<Result<String, String>>,
    /// Why the run stopped before the last mode, if it did.
    pub stopped: Option<String>,
    /// Errors outside any one mode.
    pub errors: Vec<String>,
}

/// The device, driver and kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System {
    /// The DRM device node.
    pub device: String,
    /// The driver name, for example `i915` or `vkms`.
    pub driver: String,
    /// The driver version, `major.minor.patch`.
    pub driver_version: String,
    /// The driver description.
    pub driver_description: String,
    /// The kernel release.
    pub kernel: String,
}

/// The display objects the run uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The connector name.
    pub connector: String,
    /// The connector object id.
    pub connector_id: u32,
    /// The connection state at the start.
    pub connector_state: String,
    /// The encoder object id, when the connector has one.
    pub encoder_id: Option<u32>,
    /// The CRTC object id.
    pub crtc_id: u32,
    /// The index of the CRTC (the pipe).
    pub crtc_index: u32,
    /// The primary plane object id.
    pub plane_id: u32,
}

/// The CRTC state before the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Original {
    /// True when the CRTC was on.
    pub active: bool,
    /// The mode on the CRTC, when it converts to a modeline.
    pub mode: Option<Modeline>,
    /// Why the mode does not convert or does not validate.
    pub problem: Option<String>,
    /// What the restore puts back.
    pub restore_plan: String,
}

/// The scanout buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    /// The pixel format, as a fourcc.
    pub format: String,
    /// The format modifier.
    pub modifier: String,
    /// How many buffers.
    pub count: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per row.
    pub pitch: u32,
}

/// The result of one test mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeReport {
    /// The test mode name.
    pub name: String,
    /// Where the timings come from.
    pub source: String,
    /// The modeline asked for.
    pub requested: Option<Modeline>,
    /// The result of `validate`: `None` when it passed.
    pub rejection: Option<String>,
    /// Ways the mode is outside the conservative GPU values.
    pub beyond_conservative: Vec<String>,
    /// What the kernel did, when the mode reached it.
    pub kms: Option<KmsResult>,
}

/// What the kernel did with one mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KmsResult {
    /// The test-only commit: `Err` holds the kernel error.
    pub test_commit: Option<Result<(), String>>,
    /// The real commit.
    pub commit: Option<Result<(), String>>,
    /// The mode read back after the commit.
    pub readback: Option<Readback>,
    /// How long the mode showed, in milliseconds.
    pub shown_ms: u64,
    /// Statistics of the vblanks waited for.
    pub vblank: Option<Stats>,
    /// Statistics of the completed page flips.
    pub flips: Option<Stats>,
    /// Page flips queued.
    pub flips_queued: u64,
    /// Page-flip events received.
    pub flips_completed: u64,
    /// Flip cadence against the field rate.
    pub cadence: Option<Cadence>,
    /// The pixel clock that the measured vblank rate implies, in hertz.
    pub implied_pclock_hz: Option<u64>,
    /// Kernel errors during the display time, each once with its count.
    pub errors: Vec<String>,
}

/// The mode read back from the kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readback {
    /// The mode, or why it does not convert.
    pub mode: Result<Modeline, String>,
    /// Fields that differ from the request.
    pub differences: Vec<String>,
    /// `validate` on the read-back mode: `None` when it passed.
    pub rejection: Option<String>,
}

impl Report {
    /// A report with the plan of a run and nothing measured yet.
    pub fn new(
        preset: &str,
        allow: &Allowances,
        width: u16,
        seconds: u32,
        confirmed: bool,
        planned: &[Planned],
    ) -> Report {
        Report {
            preset: preset.to_owned(),
            caps: allow.caps(),
            allow: *allow,
            width,
            seconds_per_mode: seconds,
            confirmed,
            system: None,
            output: None,
            original: None,
            framebuffer: None,
            modes: planned.iter().map(ModeReport::from_plan).collect(),
            restore: None,
            stopped: None,
            errors: Vec::new(),
        }
    }

    /// The report as JSON.
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("report_version", REPORT_VERSION)
            .with("tool", "kms-spike")
            .with("preset", self.preset.as_str())
            .with("dry_run", !self.confirmed)
            .with("width", self.width)
            .with("seconds_per_mode", self.seconds_per_mode)
            .with("gpu_caps", caps_json(&self.caps))
            .with(
                "allowed",
                Json::object()
                    .with("low_pclock_hz", self.allow.pclock_min_hz)
                    .with("interlace", self.allow.interlace),
            )
            .with("system", self.system.as_ref().map(System::to_json))
            .with("output", self.output.as_ref().map(Output::to_json))
            .with("original", self.original.as_ref().map(Original::to_json))
            .with(
                "framebuffer",
                self.framebuffer.as_ref().map(Framebuffer::to_json),
            )
            .with(
                "modes",
                Json::Array(self.modes.iter().map(ModeReport::to_json).collect()),
            )
            .with("restore", self.restore.as_ref().map(result_json))
            .with("stopped", self.stopped.clone())
            .with("errors", self.errors.clone())
    }

    /// The readable summary.
    pub fn summary(&self) -> String {
        let mut out = format!("summary for preset {}:\n", self.preset);
        if let Some(s) = &self.system {
            out.push_str(&format!(
                "  driver {} {} on kernel {}\n",
                s.driver, s.driver_version, s.kernel
            ));
        }
        if let Some(o) = &self.output {
            out.push_str(&format!(
                "  connector {} (id {}), CRTC {} (pipe {}), plane {}\n",
                o.connector, o.connector_id, o.crtc_id, o.crtc_index, o.plane_id
            ));
        }
        if let Some(f) = &self.framebuffer {
            out.push_str(&format!(
                "  buffers: {} x {}x{} {} {}\n",
                f.count, f.width, f.height, f.format, f.modifier
            ));
        }
        for m in &self.modes {
            out.push_str(&m.summary());
        }
        match &self.restore {
            Some(Ok(text)) => out.push_str(&format!("  restore: {text}\n")),
            Some(Err(text)) => out.push_str(&format!("  RESTORE FAILED: {text}\n")),
            None => {}
        }
        if let Some(why) = &self.stopped {
            out.push_str(&format!("  stopped early: {why}\n"));
        }
        for e in &self.errors {
            out.push_str(&format!("  error: {e}\n"));
        }
        out
    }

    /// Writes the JSON report to `path` through a temporary file and a
    /// rename, so that a reader never sees half a report.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        std::fs::write(&tmp, self.to_json().pretty())?;
        std::fs::rename(&tmp, path)
    }
}

impl ModeReport {
    fn from_plan(p: &Planned) -> ModeReport {
        ModeReport {
            name: p.derived.test.name().to_owned(),
            source: p.derived.source.to_string(),
            requested: p.derived.modeline,
            rejection: match &p.verdict {
                Verdict::Valid(_) => None,
                Verdict::Rejected(r) => Some(r.to_string()),
                Verdict::NoModeline => Some("no modeline fits this width".to_owned()),
            },
            beyond_conservative: p.beyond.iter().map(ToString::to_string).collect(),
            kms: None,
        }
    }

    fn to_json(&self) -> Json {
        Json::object()
            .with("name", self.name.as_str())
            .with("source", self.source.as_str())
            .with("requested", self.requested.as_ref().map(modeline_json))
            .with(
                "validation",
                Json::object()
                    .with("passed", self.rejection.is_none())
                    .with("rejection", self.rejection.clone()),
            )
            .with("beyond_conservative", self.beyond_conservative.clone())
            .with("kms", self.kms.as_ref().map(KmsResult::to_json))
    }

    fn summary(&self) -> String {
        let mut out = format!("  {}: ", self.name);
        if let Some(r) = &self.rejection {
            out.push_str(&format!("skipped, validate rejected it: {r}\n"));
            return out;
        }
        let Some(k) = &self.kms else {
            out.push_str("valid, not shown (dry run or the run stopped first)\n");
            return out;
        };
        match (&k.test_commit, &k.commit) {
            (Some(Err(e)), _) => out.push_str(&format!("test-only commit FAILED: {e}\n")),
            (_, Some(Err(e))) => out.push_str(&format!("commit FAILED: {e}\n")),
            (_, Some(Ok(()))) => {
                out.push_str(&format!("shown for {} ms", k.shown_ms));
                if let Some(v) = &k.vblank {
                    out.push_str(&format!(
                        ", vblank {} Hz (jitter {} us)",
                        milli_text(v.rate_mhz),
                        v.jitter_ns / 1_000
                    ));
                }
                out.push_str(&format!(", flips {}/{}", k.flips_completed, k.flips_queued));
                if let Some(c) = k.cadence {
                    out.push_str(&format!(" ({})", c.name()));
                }
                out.push('\n');
                if let Some(rb) = &k.readback {
                    out.push_str(&rb.summary());
                }
            }
            _ => out.push_str("not committed\n"),
        }
        for e in &k.errors {
            out.push_str(&format!("    kernel error: {e}\n"));
        }
        out
    }
}

impl Readback {
    fn summary(&self) -> String {
        match &self.mode {
            Err(e) => format!("    readback: {e}\n"),
            Ok(m) if self.differences.is_empty() => {
                format!("    readback equals the request: {}\n", modeline_text(m))
            }
            Ok(m) => format!(
                "    readback DIFFERS ({}): {}\n",
                self.differences.join(", "),
                modeline_text(m)
            ),
        }
    }

    fn to_json(&self) -> Json {
        let (mode, error) = match &self.mode {
            Ok(m) => (Some(modeline_json(m)), None),
            Err(e) => (None, Some(e.clone())),
        };
        Json::object()
            .with("mode", mode)
            .with("error", error)
            .with(
                "equals_request",
                self.mode.is_ok() && self.differences.is_empty(),
            )
            .with("differences", self.differences.clone())
            .with(
                "validation",
                Json::object()
                    .with("passed", self.mode.is_ok() && self.rejection.is_none())
                    .with("rejection", self.rejection.clone()),
            )
    }
}

impl KmsResult {
    fn to_json(&self) -> Json {
        Json::object()
            .with(
                "test_commit",
                self.test_commit.as_ref().map(unit_result_json),
            )
            .with("commit", self.commit.as_ref().map(unit_result_json))
            .with("readback", self.readback.as_ref().map(Readback::to_json))
            .with("shown_ms", self.shown_ms)
            .with("vblank", self.vblank.as_ref().map(stats_json))
            .with("flips", self.flips.as_ref().map(stats_json))
            .with("flips_queued", self.flips_queued)
            .with("flips_completed", self.flips_completed)
            .with("flip_cadence", self.cadence.map(Cadence::name))
            .with("implied_pclock_hz", self.implied_pclock_hz)
            .with("kernel_errors", self.errors.clone())
    }
}

impl System {
    fn to_json(&self) -> Json {
        Json::object()
            .with("device", self.device.as_str())
            .with("driver", self.driver.as_str())
            .with("driver_version", self.driver_version.as_str())
            .with("driver_description", self.driver_description.as_str())
            .with("kernel", self.kernel.as_str())
    }
}

impl Output {
    fn to_json(&self) -> Json {
        Json::object()
            .with("connector", self.connector.as_str())
            .with("connector_id", self.connector_id)
            .with("connector_state", self.connector_state.as_str())
            .with("encoder_id", self.encoder_id)
            .with("crtc_id", self.crtc_id)
            .with("crtc_index", self.crtc_index)
            .with("plane_id", self.plane_id)
    }
}

impl Original {
    fn to_json(&self) -> Json {
        Json::object()
            .with("active", self.active)
            .with("mode", self.mode.as_ref().map(modeline_json))
            .with("problem", self.problem.clone())
            .with("restore_plan", self.restore_plan.as_str())
    }
}

impl Framebuffer {
    fn to_json(&self) -> Json {
        Json::object()
            .with("format", self.format.as_str())
            .with("modifier", self.modifier.as_str())
            .with("count", self.count)
            .with("width", self.width)
            .with("height", self.height)
            .with("pitch", self.pitch)
    }
}

fn modeline_json(m: &Modeline) -> Json {
    Json::object()
        .with("pclock_hz", m.pclock_hz)
        .with("hactive", m.hactive)
        .with("hsync_start", m.hsync_start)
        .with("hsync_end", m.hsync_end)
        .with("htotal", m.htotal)
        .with("vactive", m.vactive)
        .with("vsync_start", m.vsync_start)
        .with("vsync_end", m.vsync_end)
        .with("vtotal", m.vtotal)
        .with("interlace", m.interlace)
        .with("doublescan", m.doublescan)
        .with("hsync_positive", m.hsync_positive)
        .with("vsync_positive", m.vsync_positive)
        .with("line_rate_mhz", m.line_rate_mhz())
        .with("field_rate_mhz", m.field_rate_mhz())
}

fn caps_json(c: &GpuCaps) -> Json {
    Json::object()
        .with("pclock_min_hz", c.pclock_min_hz)
        .with("pclock_max_hz", c.pclock_max_hz)
        .with("interlace", c.interlace)
        .with("flip_per_field", c.flip_per_field)
        .with("max_width", c.max_width)
        .with("max_height", c.max_height)
}

fn stats_json(s: &Stats) -> Json {
    Json::object()
        .with("count", s.count)
        .with("rate_mhz", s.rate_mhz)
        .with("mean_interval_ns", s.mean_interval_ns)
        .with("min_interval_ns", s.min_interval_ns)
        .with("max_interval_ns", s.max_interval_ns)
        .with("jitter_ns", s.jitter_ns)
        .with("skipped_vblanks", s.skipped_vblanks)
}

fn unit_result_json(r: &Result<(), String>) -> Json {
    Json::object()
        .with("ok", r.is_ok())
        .with("error", r.as_ref().err().cloned())
}

fn result_json(r: &Result<String, String>) -> Json {
    match r {
        Ok(text) => Json::object()
            .with("ok", true)
            .with("detail", text.as_str()),
        Err(text) => Json::object()
            .with("ok", false)
            .with("detail", text.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::TestMode;
    use crate::plan::plan;
    use crate::presets;

    fn dry() -> Report {
        let preset = presets::find("pc_31_120").unwrap();
        let allow = Allowances::default();
        let p = plan(preset, &TestMode::ALL, 640, &allow);
        Report::new(preset.id, &allow, 640, 10, false, &p)
    }

    #[test]
    fn a_dry_run_report_has_the_plan_and_no_kernel_results() {
        let text = dry().to_json().pretty();
        assert!(text.starts_with("{\n  \"report_version\": 1,\n  \"tool\": \"kms-spike\""));
        assert!(text.contains("\"dry_run\": true"));
        assert!(text.contains("\"name\": \"480i\""));
        assert!(text.contains("\"passed\": false"));
        assert!(text.contains("\"kms\": null"));
        assert!(text.contains("\"pclock_min_hz\": 25000000"));
    }

    #[test]
    fn the_summary_says_why_a_mode_is_skipped() {
        let s = dry().summary();
        assert!(s.contains("480i: skipped, validate rejected it"), "{s}");
        assert!(s.contains("480p: valid, not shown"), "{s}");
    }

    #[test]
    fn kernel_results_appear_in_json_and_summary() {
        let mut r = dry();
        let requested = r.modes[2].requested.unwrap();
        r.modes[2].kms = Some(KmsResult {
            test_commit: Some(Ok(())),
            commit: Some(Ok(())),
            readback: Some(Readback {
                mode: Ok(requested),
                differences: Vec::new(),
                rejection: None,
            }),
            shown_ms: 10_000,
            flips_queued: 600,
            flips_completed: 599,
            cadence: Some(Cadence::PerField),
            ..KmsResult::default()
        });
        r.restore = Some(Err("EINVAL".to_owned()));
        let text = r.to_json().pretty();
        assert!(text.contains("\"equals_request\": true"), "{text}");
        assert!(text.contains("\"flip_cadence\": \"per_field\""), "{text}");
        let s = r.summary();
        assert!(
            s.contains("480p: shown for 10000 ms, flips 599/600 (per_field)"),
            "{s}"
        );
        assert!(s.contains("readback equals the request"), "{s}");
        assert!(s.contains("RESTORE FAILED: EINVAL"), "{s}");
    }

    #[test]
    fn a_failed_test_commit_is_reported() {
        let mut r = dry();
        r.modes[2].kms = Some(KmsResult {
            test_commit: Some(Err("Invalid argument (os error 22)".to_owned())),
            ..KmsResult::default()
        });
        assert!(
            r.summary()
                .contains("test-only commit FAILED: Invalid argument")
        );
        assert!(r.to_json().pretty().contains("\"ok\": false"));
    }

    #[test]
    fn a_differing_readback_lists_the_fields() {
        let rb = Readback {
            mode: Ok(dry().modes[2].requested.unwrap()),
            differences: vec!["pclock_hz: 1 -> 2".to_owned()],
            rejection: None,
        };
        assert!(
            rb.summary()
                .contains("readback DIFFERS (pclock_hz: 1 -> 2)")
        );
        assert!(rb.to_json().pretty().contains("\"equals_request\": false"));
    }

    #[test]
    fn the_report_is_written_whole() {
        let dir = std::env::temp_dir().join(format!("kms-spike-report-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.json");
        dry().write(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, dry().to_json().pretty());
        assert!(!dir.join("r.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
