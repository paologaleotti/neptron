//! The run on a display: the report before the first modeset and after
//! each mode, the modes, the restore, the verdict and the text for the
//! person. It works on any `Display` and `Clock`; only the device and the
//! signals are the KMS build's own.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use neptron_backend::{Clock, Display};

use super::error::SelftestError;
use super::family::{PlannedMode, describe};
use super::monitor::Setup;
use super::report::{self, DisplayReport, ModeReport, Report, RunState};
use super::show::{Run, Shown};
use super::verdict::judge_run;

/// Everything the run needs besides the display.
pub struct Inputs<'a, C: Clock> {
    /// The monitor and the GPU values.
    pub setup: &'a Setup,
    /// The report of the plan, with `dry_run` false.
    pub plan: Report,
    /// The modes, in order.
    pub modes: &'a [PlannedMode],
    /// What the backend says about the display.
    pub display: DisplayReport,
    /// The report file.
    pub report_path: &'a Path,
    /// The time source.
    pub clock: &'a C,
    /// Set by a stop signal.
    pub stop: &'a AtomicBool,
}

/// Shows the modes on `display`, writes the report after each mode and at
/// the end, and prints one line per mode.
///
/// # Errors
///
/// [`SelftestError::Failed`] when the run does not pass,
/// [`SelftestError::Report`] when the last report cannot be written, and
/// [`SelftestError::Output`]. The display is restored in every case.
pub fn run<D: Display, C: Clock>(
    display: &mut D,
    inputs: Inputs<'_, C>,
    out: &mut dyn Write,
) -> Result<(), SelftestError> {
    let mut report = inputs.plan;
    report.display = inputs.display;
    report.modes.clear();
    let seconds = report.seconds_per_mode;
    print(
        out,
        &format!(
            "Showing {} modes on {} ({}), {seconds} s each. Ctrl-C stops the run and turns the \
             output off.\n",
            inputs.modes.len(),
            report.display.connector.as_deref().unwrap_or("the display"),
            report.display.driver.as_deref().unwrap_or("unknown driver"),
        ),
    )?;
    let run = Run {
        clock: inputs.clock,
        profile: &inputs.setup.profile,
        caps: &inputs.setup.caps,
        seconds_per_mode: seconds,
        stop: inputs.stop,
    };
    let mut progress = report.clone();
    let shown = run.show(display, inputs.modes, &mut |modes| {
        progress.modes = modes.to_vec();
        if let Some(last) = modes.last() {
            // The display loop must reach its restore, so an output or a
            // report error here only loses progress; the end reports it.
            let _ = print(out, &mode_line(last));
        }
        let _ = report::write(inputs.report_path, &progress);
    });
    finish(&mut report, shown);
    report::write(inputs.report_path, &report)?;
    print(out, &end_text(&report, inputs.report_path))?;
    if report.pass == Some(true) {
        Ok(())
    } else {
        Err(SelftestError::Failed {
            report: inputs.report_path.to_owned(),
        })
    }
}

/// The size of the scanout buffers: the widest and the tallest planned
/// mode, at least 1 by 1 and at most the GPU limits.
pub fn buffer_size(modes: &[PlannedMode], caps: &neptron_types::GpuCaps) -> (u16, u16) {
    let width = modes
        .iter()
        .map(|p| p.token.mode().hactive)
        .max()
        .unwrap_or(1);
    let height = modes
        .iter()
        .map(|p| p.token.mode().vactive)
        .max()
        .unwrap_or(1);
    (
        width.clamp(1, caps.max_width.max(1)),
        height.clamp(1, caps.max_height.max(1)),
    )
}

/// The report of a run that stopped after the first report was written
/// and before the first modeset: the plan with `state` `stopped`, the
/// reason, and no pass. A refusal then reads differently from a crash,
/// which leaves `state` `running`.
pub fn stopped_before_display(plan: &Report, reason: &SelftestError) -> Report {
    let mut report = plan.clone();
    report.state = RunState::Stopped;
    report.stopped = Some(reason.to_string());
    report.pass = Some(false);
    report
}

/// Puts the result of the display run into the report and judges it.
pub fn finish(report: &mut Report, shown: Shown) {
    report.modes = shown.modes;
    report.state = shown.state;
    report.stopped = shown.stopped;
    report.restore = shown.restore;
    judge_run(report);
}

/// One line per mode: the verdict, the family, the mode and the reasons.
pub fn mode_line(m: &ModeReport) -> String {
    let verdict = if m.pass == Some(true) { "PASS" } else { "FAIL" };
    let mut line = format!("  {verdict} {:<22} {}", m.family, describe(&m.request));
    if let Some(t) = &m.timing {
        line.push_str(&format!(
            ", {} flips, {} missed blanks",
            t.flips, t.missed_vblanks
        ));
    }
    if !m.failures.is_empty() {
        line.push_str(": ");
        line.push_str(&m.failures.join("; "));
    }
    line.push('\n');
    line
}

/// The end of the run: why it stopped, the restore, the verdict and the
/// report file.
pub fn end_text(report: &Report, path: &Path) -> String {
    let mut s = String::new();
    if let Some(why) = &report.stopped {
        s.push_str(&format!("Stopped: {why}\n"));
    }
    match &report.restore {
        Some(r) if r.ok => s.push_str("Restore: the output is off\n"),
        Some(r) => s.push_str(&format!(
            "RESTORE FAILED: {}. The display backend turns the plane off or the output off \
             when the program ends; if the monitor still shows a picture, switch it off.\n",
            r.error.as_deref().unwrap_or("no reason")
        )),
        None => s.push_str("Restore: no mode was set\n"),
    }
    let verdict = match (report.state, report.pass) {
        (_, Some(true)) => "PASS",
        (RunState::Stopped, _) => "FAIL (stopped)",
        _ => "FAIL",
    };
    s.push_str(&format!("Result: {verdict}\nReport: {}\n", path.display()));
    s
}

fn print(out: &mut dyn Write, text: &str) -> Result<(), SelftestError> {
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|e| SelftestError::Output {
            reason: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use neptron_platform::UNKNOWN_GPU_CAPS;
    use neptron_testkit::{DisplayOptions, FakeDisplay, ModeFault, Render, SimClock};

    use super::*;
    use crate::selftest::family::{plan, planned};
    use crate::selftest::report::tests::setup;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neptronctl-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run_on_fake(
        name: &str,
        fault: Option<ModeFault>,
        stop: bool,
    ) -> (Result<(), SelftestError>, Report, String) {
        let dir = temp_dir(name);
        let path = dir.join("report.json");
        let setup = setup("pc_31_120");
        let outcomes = plan(&setup.profile, &UNKNOWN_GPU_CAPS);
        let modes: Vec<PlannedMode> = planned(&outcomes).cloned().collect();
        let clock = SimClock::new();
        let mut display = FakeDisplay::with_options(
            clock.clone(),
            UNKNOWN_GPU_CAPS,
            DisplayOptions {
                render: Render::Off,
                ..DisplayOptions::default()
            },
        );
        if let Some(fault) = fault {
            display.fault_mode(2, fault);
        }
        let flag = AtomicBool::new(stop);
        let mut out = Vec::new();
        let result = run(
            &mut display,
            Inputs {
                setup: &setup,
                plan: Report::planned(&setup, &outcomes, 2, false),
                modes: &modes,
                display: DisplayReport {
                    device: Some("/dev/dri/card0".into()),
                    driver: Some("fake".into()),
                    connector: Some("Virtual-1".into()),
                    linear_by_modifier: Some(false),
                },
                report_path: &path,
                clock: &clock,
                stop: &flag,
            },
            &mut out,
        );
        let report = report::read(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        (result, report, String::from_utf8(out).unwrap())
    }

    #[test]
    fn a_good_run_writes_a_passing_report_that_parses() {
        let (result, report, text) = run_on_fake("good", None, false);
        assert!(result.is_ok(), "{result:?}\n{text}");
        assert_eq!(report.state, RunState::Finished);
        assert_eq!(report.pass, Some(true));
        assert!(!report.dry_run);
        assert_eq!(report.display.driver.as_deref(), Some("fake"));
        assert_eq!(report.modes.len(), 4);
        for m in &report.modes {
            assert_eq!(m.readback_equal, Some(true));
            assert_eq!(m.pass, Some(true));
            assert!(m.timing.as_ref().unwrap().implied_pclock_hz.is_some());
        }
        assert!(report.restore.as_ref().unwrap().ok);
        assert_eq!(text.matches("  PASS ").count(), 4, "{text}");
        assert!(text.contains("Result: PASS"), "{text}");
    }

    #[test]
    fn a_failed_mode_fails_the_run_with_the_report_path() {
        let fault = ModeFault::Refuse {
            reason: "EINVAL".into(),
        };
        let (result, report, text) = run_on_fake("refused", Some(fault), false);
        assert!(
            matches!(result, Err(SelftestError::Failed { .. })),
            "{result:?}"
        );
        assert_eq!(report.pass, Some(false));
        assert_eq!(report.modes[2].pass, Some(false));
        assert!(text.contains("  FAIL "), "{text}");
        assert!(text.contains("Result: FAIL"), "{text}");
    }

    #[test]
    fn a_stop_before_the_first_mode_reports_no_restore() {
        let (result, report, text) = run_on_fake("stopped", None, true);
        assert!(result.is_err());
        assert_eq!(report.state, RunState::Stopped);
        assert!(report.modes.is_empty());
        assert_eq!(report.restore, None);
        assert!(text.contains("FAIL (stopped)"), "{text}");
        assert!(text.contains("no mode was set"), "{text}");
    }

    #[test]
    fn a_run_refused_before_the_first_modeset_is_written_as_stopped() {
        let dir = temp_dir("refused-late");
        let path = dir.join("report.json");
        let setup = setup("pc_31_120");
        let plan = Report::planned(&setup, &[], 10, false);
        assert_eq!(plan.state, RunState::Running);
        let reason = SelftestError::Device {
            path: PathBuf::from("/dev/dri/card9"),
            reason: "No such file".into(),
        };
        report::write(&path, &stopped_before_display(&plan, &reason)).unwrap();
        let back = report::read(&path).unwrap();
        assert_eq!(back.state, RunState::Stopped);
        assert_eq!(back.pass, Some(false));
        assert!(back.stopped.unwrap().contains("/dev/dri/card9"));
        assert_eq!(back.restore, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_buffers_hold_the_largest_mode() {
        let setup = setup("pc_31_120");
        let outcomes = plan(&setup.profile, &UNKNOWN_GPU_CAPS);
        let modes: Vec<PlannedMode> = planned(&outcomes).cloned().collect();
        assert_eq!(buffer_size(&modes, &UNKNOWN_GPU_CAPS), (1920, 480));
        assert_eq!(buffer_size(&[], &UNKNOWN_GPU_CAPS), (1, 1));
    }

    #[test]
    fn a_failed_restore_is_loud() {
        let mut report = Report::planned(&setup("ntsc"), &[], 10, false);
        report.restore = Some(report::RestoreReport {
            method: "blank".into(),
            ok: false,
            error: Some("busy".into()),
        });
        let text = end_text(&report, Path::new("r.json"));
        assert!(text.contains("RESTORE FAILED: busy"), "{text}");
    }
}
