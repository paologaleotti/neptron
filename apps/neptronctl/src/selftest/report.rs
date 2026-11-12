//! The self-test report: one JSON file, written with temp file, fsync,
//! rename. The format is append-only: a new field gets a new
//! `report_version`, and no field is renamed or removed.

use std::io::Write as _;
use std::path::Path;

use neptron_types::{GpuCaps, Modeline, MonitorProfile};
use serde::{Deserialize, Serialize};

use super::error::SelftestError;
use super::family::{Outcome, PlannedMode, PolicyFacts};
use super::monitor::Setup;

/// The version of the report format.
pub const REPORT_VERSION: u32 = 2;

/// The whole report of one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// [`REPORT_VERSION`].
    pub report_version: u32,
    /// The program and its version.
    pub tool: Tool,
    /// True when no mode was set.
    pub dry_run: bool,
    /// How far the run came.
    pub state: RunState,
    /// Every mode passed and the display was restored. `None` until the
    /// run ends, and in a dry run.
    pub pass: Option<bool>,
    /// The display time of each mode, in seconds.
    pub seconds_per_mode: u32,
    /// The monitor profile.
    pub profile: ProfileReport,
    /// The GPU values that `validate` got.
    pub caps: GpuCaps,
    /// The GPU as sysfs and the capability table give it.
    pub gpu: GpuReport,
    /// The display the modes went to. Empty in a dry run.
    pub display: DisplayReport,
    /// Each mode family and what the plan gave for it.
    pub families: Vec<FamilyReport>,
    /// Each test mode, in the order it showed.
    pub modes: Vec<ModeReport>,
    /// How the display was left at the end; `None` when no mode was set.
    pub restore: Option<RestoreReport>,
    /// Why the run stopped before the last mode.
    pub stopped: Option<String>,
    /// Errors outside one mode.
    pub errors: Vec<String>,
}

/// The program that wrote the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    /// `neptronctl`.
    pub name: String,
    /// The package version.
    pub version: String,
}

/// How far the run came.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// A dry run: the plan only.
    Planned,
    /// Written before the first mode and after each mode.
    Running,
    /// Every planned mode ran.
    Finished,
    /// The run stopped early: a signal, or an error that needs the output
    /// off at once.
    Stopped,
}

/// The monitor profile of the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileReport {
    /// The profile id.
    pub id: String,
    /// `preset` or `edid`.
    pub source: String,
    /// The highest pixel clock of the EDID, for the EDID profile.
    pub edid_pclock_max_hz: Option<u32>,
    /// The whole profile with its ranges.
    pub profile: MonitorProfile,
}

/// The GPU as sysfs and the capability table give it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuReport {
    /// The DRM device name, `card0`.
    pub card: Option<String>,
    /// The kernel driver that sysfs names.
    pub driver: Option<String>,
    /// The PCI id, `vendor:device` in hex.
    pub pci_id: Option<String>,
    /// The capability table entry; `None` for an unknown GPU.
    pub table_entry: Option<String>,
    /// The name of that entry.
    pub table_name: Option<String>,
    /// The connector that the detection chose.
    pub connector: Option<String>,
    /// Its status in sysfs.
    pub connector_status: Option<String>,
}

/// The display the modes went to, as the KMS backend reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayReport {
    /// The device node.
    pub device: Option<String>,
    /// The driver name the device gives: `i915`, `vkms`.
    pub driver: Option<String>,
    /// The connector the backend chose.
    pub connector: Option<String>,
    /// True when the buffers are linear through the modifier list, false
    /// through the linear usage flag.
    pub linear_by_modifier: Option<bool>,
}

/// What the plan gave for one family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyReport {
    /// The family id.
    pub family: String,
    /// `planned`, `same_as` or `unsupported`.
    pub result: String,
    /// The earlier family with the same mode.
    pub same_as: Option<String>,
    /// Why the family has no mode.
    pub reason: Option<String>,
}

/// One test mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeReport {
    /// The family id.
    pub family: String,
    /// The text on the pattern.
    pub title: String,
    /// Where the policy took the mode from.
    pub policy: PolicyReport,
    /// The mode that `validate` approved and the display got.
    pub request: Modeline,
    /// Its line and field rates.
    pub request_rates: Rates,
    /// The test-only commit. `None` in a dry run.
    pub test_only_commit: Option<Step>,
    /// The real commit. `None` in a dry run.
    pub commit: Option<Step>,
    /// The mode the kernel reports after the commit.
    pub readback: Option<Modeline>,
    /// True when the read back equals the request in every field.
    pub readback_equal: Option<bool>,
    /// The fields that differ.
    pub readback_differences: Vec<FieldDifference>,
    /// True when the read back passes `validate`.
    pub readback_valid: Option<bool>,
    /// Why `validate` refuses the read back.
    pub readback_rejection: Option<String>,
    /// How long the pattern showed, in nanoseconds.
    pub shown_ns: Option<u64>,
    /// The flips and the vertical blanks.
    pub timing: Option<TimingReport>,
    /// Errors of the display during this mode.
    pub errors: Vec<String>,
    /// Every check of the mode passed. `None` in a dry run.
    pub pass: Option<bool>,
    /// The checks that failed.
    pub failures: Vec<String>,
}

/// The policy's reasons for a mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReport {
    /// The game source of the family.
    pub source: String,
    /// The picture style asked for.
    pub style: String,
    /// The refresh policy asked for.
    pub refresh_policy: String,
    /// The width strategy asked for; `None` for the GPU default.
    pub horizontal: Option<String>,
    /// The rung of the winner.
    pub rung: String,
    /// Where the field rate comes from.
    pub refresh_kind: String,
    /// Fields per game frame.
    pub repeat: u8,
    /// How the width is filled.
    pub width_rule: Option<String>,
}

/// The rates of a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rates {
    /// Line rate, in millihertz.
    pub line_rate_mhz: u64,
    /// Field rate, in millihertz.
    pub field_rate_mhz: u64,
}

/// One step of a modeset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// What happened.
    pub result: StepResult,
    /// The error text, when there is one.
    pub detail: Option<String>,
}

/// What happened in one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepResult {
    /// The kernel took it.
    Passed,
    /// The kernel refused the test-only commit; nothing changed.
    Refused,
    /// The kernel refused the real commit; nothing changed.
    Failed,
    /// The step did not run.
    NotSent,
    /// The backend failed and cannot tell whether the step ran.
    Unknown,
}

/// One field of the read back that differs from the request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDifference {
    /// The `Modeline` field.
    pub field: String,
    /// The value asked for; a flag is 0 or 1.
    pub requested: u64,
    /// The value read back.
    pub read_back: u64,
}

/// The flips and the vertical blanks of one mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingReport {
    /// Flips that completed.
    pub flips: u64,
    /// Vertical blanks from the first flip to the last.
    pub vblanks: u64,
    /// Vertical blanks with no flip.
    pub missed_vblanks: u64,
    /// Completed flips per second, in millihertz.
    pub flips_per_second_mhz: u64,
    /// The mean time of one vertical blank, in nanoseconds.
    pub vblank_period_ns: Option<u64>,
    /// The period the mode asks for, in nanoseconds: one field, or one
    /// frame of an interlaced mode when the blanks come once per frame.
    pub expected_period_ns: Option<u64>,
    /// `field` or `frame`: what one vertical blank is.
    pub vblank_unit: Option<String>,
    /// The distance of the measured period from the expected one, in parts
    /// per million.
    pub period_error_ppm: Option<u64>,
    /// The pixel clock that the measured period gives, in hertz.
    pub implied_pclock_hz: Option<u64>,
    /// Version 2: true when an interlaced mode's unit is the nearer of
    /// field and frame, because the capability table does not know the
    /// GPU. A version 1 report reads as false.
    #[serde(default)]
    pub unit_assumed: bool,
}

/// How the display was left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// `blank`: the output is off.
    pub method: String,
    /// The display took it.
    pub ok: bool,
    /// The error, when it failed.
    pub error: Option<String>,
}

impl Report {
    /// The report of a plan: every family, and each planned mode with its
    /// request. The display fields are empty.
    pub fn planned(
        setup: &Setup,
        outcomes: &[Outcome],
        seconds_per_mode: u32,
        dry_run: bool,
    ) -> Self {
        let g = &setup.gpu;
        Self {
            report_version: REPORT_VERSION,
            tool: Tool {
                name: env!("CARGO_PKG_NAME").to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            dry_run,
            state: if dry_run {
                RunState::Planned
            } else {
                RunState::Running
            },
            pass: None,
            seconds_per_mode,
            profile: ProfileReport {
                id: setup.profile.id.clone(),
                source: setup.source.to_owned(),
                edid_pclock_max_hz: setup.edid_pclock_max_hz,
                profile: setup.profile.clone(),
            },
            caps: setup.caps,
            gpu: GpuReport {
                card: g.card.clone(),
                driver: g.driver.clone(),
                pci_id: g.pci_id.clone(),
                table_entry: g.table_entry.clone(),
                table_name: g.table_name.clone(),
                connector: g.connector.clone(),
                connector_status: g.connector_status.clone(),
            },
            display: DisplayReport::default(),
            families: outcomes.iter().map(FamilyReport::of).collect(),
            modes: super::family::planned(outcomes)
                .map(ModeReport::planned)
                .collect(),
            restore: None,
            stopped: None,
            errors: Vec::new(),
        }
    }
}

impl FamilyReport {
    fn of(outcome: &Outcome) -> Self {
        match outcome {
            Outcome::Planned(p) => Self {
                family: p.family.to_owned(),
                result: "planned".to_owned(),
                same_as: None,
                reason: None,
            },
            Outcome::SameAs { family, same_as } => Self {
                family: (*family).to_owned(),
                result: "same_as".to_owned(),
                same_as: Some((*same_as).to_owned()),
                reason: None,
            },
            Outcome::Unsupported { family, reason } => Self {
                family: (*family).to_owned(),
                result: "unsupported".to_owned(),
                same_as: None,
                reason: Some(reason.clone()),
            },
        }
    }
}

impl ModeReport {
    /// A mode as planned: the request, and nothing measured yet.
    pub fn planned(p: &PlannedMode) -> Self {
        let m = *p.token.mode();
        Self {
            family: p.family.to_owned(),
            title: p.title.to_owned(),
            policy: PolicyReport::of(&p.policy),
            request: m,
            request_rates: Rates::of(&m),
            test_only_commit: None,
            commit: None,
            readback: None,
            readback_equal: None,
            readback_differences: Vec::new(),
            readback_valid: None,
            readback_rejection: None,
            shown_ns: None,
            timing: None,
            errors: Vec::new(),
            pass: None,
            failures: Vec::new(),
        }
    }
}

impl PolicyReport {
    fn of(f: &PolicyFacts) -> Self {
        Self {
            source: f.source.clone(),
            style: f.style.to_owned(),
            refresh_policy: f.refresh_policy.to_owned(),
            horizontal: f.horizontal.clone(),
            rung: f.rung.clone(),
            refresh_kind: f.refresh_kind.clone(),
            repeat: f.repeat,
            width_rule: f.width_rule.clone(),
        }
    }
}

impl Rates {
    /// The rates of `m`.
    pub fn of(m: &Modeline) -> Self {
        Self {
            line_rate_mhz: m.line_rate_mhz(),
            field_rate_mhz: m.field_rate_mhz(),
        }
    }
}

/// Writes `report` to `path` as pretty JSON: a temporary file in the same
/// folder, fsync, rename, then fsync of the folder. A reader sees the old
/// report or the new one, never a part.
///
/// # Errors
///
/// [`SelftestError::Report`] with the path and the error of the system.
pub fn write(path: &Path, report: &Report) -> Result<(), SelftestError> {
    let failed = |reason: String| SelftestError::Report {
        path: path.to_owned(),
        reason,
    };
    let mut text = serde_json::to_vec_pretty(report).map_err(|e| failed(e.to_string()))?;
    text.push(b'\n');
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| failed("the path names no file".to_owned()))?;
    let mut temp_name = name.to_owned();
    temp_name.push(format!(".tmp-{}", std::process::id()));
    let temp = dir.join(temp_name);
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&text)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        std::fs::File::open(dir)?.sync_all()
    })();
    if let Err(e) = result {
        // The temporary file is ours; a failed removal leaves only a
        // stale temporary file beside the report.
        let _ = std::fs::remove_file(&temp);
        return Err(failed(e.to_string()));
    }
    Ok(())
}

/// Reads a report back, for the tests and the checks.
///
/// # Errors
///
/// [`SelftestError::Report`] when the file cannot be read or is not a
/// report.
#[cfg(test)]
pub fn read(path: &Path) -> Result<Report, SelftestError> {
    let failed = |reason: String| SelftestError::Report {
        path: path.to_owned(),
        reason,
    };
    let text = std::fs::read(path).map_err(|e| failed(e.to_string()))?;
    serde_json::from_slice(&text).map_err(|e| failed(e.to_string()))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use neptron_platform::UNKNOWN_GPU_CAPS;

    use super::*;
    use crate::selftest::family::plan;
    use crate::selftest::monitor::{GpuFacts, load_preset};

    pub(crate) fn setup(id: &str) -> Setup {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors");
        Setup {
            profile: load_preset(&dir, id).unwrap(),
            source: "preset",
            edid_pclock_max_hz: None,
            caps: UNKNOWN_GPU_CAPS,
            gpu: GpuFacts::default(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neptronctl-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_dry_run_report_lists_every_family_and_each_planned_mode() {
        let setup = setup("pc_31_120");
        let outcomes = plan(&setup.profile, &setup.caps);
        let report = Report::planned(&setup, &outcomes, 10, true);
        assert_eq!(report.report_version, REPORT_VERSION);
        assert_eq!(report.state, RunState::Planned);
        assert_eq!(report.pass, None);
        assert_eq!(report.families.len(), outcomes.len());
        assert_eq!(report.modes.len(), 4);
        let first = &report.modes[0];
        assert_eq!(first.family, "240p_double_refresh");
        assert_eq!(
            first.request_rates.field_rate_mhz,
            first.request.field_rate_mhz()
        );
        assert_eq!(first.policy.repeat, 2);
        assert_eq!(first.commit, None);
        let two = report.families.iter().find(|f| f.family == "2x").unwrap();
        assert_eq!(two.result, "same_as");
        assert_eq!(two.same_as.as_deref(), Some("480p"));
    }

    #[test]
    fn a_written_report_parses_back_equal() {
        let dir = temp_dir("report");
        let path = dir.join("r.json");
        let setup = setup("generic_15");
        let report = Report::planned(&setup, &plan(&setup.profile, &setup.caps), 10, true);
        write(&path, &report).unwrap();
        assert_eq!(read(&path).unwrap(), report);
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["report_version"], 2);
        assert_eq!(value["state"], "planned");
        assert_eq!(value["profile"]["id"], "generic_15");
        assert!(value["modes"][0]["request"]["pclock_hz"].is_u64());
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(left.len(), 1, "no temporary file is left");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_report_in_a_missing_folder_is_an_error_with_the_path() {
        let path = std::env::temp_dir()
            .join(format!("neptronctl-missing-{}", std::process::id()))
            .join("r.json");
        let setup = setup("ntsc");
        let report = Report::planned(&setup, &[], 10, true);
        let err = write(&path, &report).unwrap_err();
        assert!(err.to_string().contains("r.json"), "{err}");
    }
}
