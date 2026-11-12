//! Shows each planned mode on a display: the modeset, the read back, the
//! pattern for its time with the flips measured, and the restore at the
//! end. It works on any `Display` and `Clock`, so the tests run it on the
//! fake display in simulated time.

use std::sync::atomic::{AtomicBool, Ordering};

use neptron_backend::{Clock, Display, DisplayError};
use neptron_safety::validate;
use neptron_types::{FlipEvent, GpuCaps, Modeline, MonitorProfile};

use super::family::PlannedMode;
use super::pattern::{Pattern, label_lines};
use super::report::{FieldDifference, ModeReport, RestoreReport, RunState, Step, StepResult};
use super::timing::{BlankUnit, stats};
use super::verdict::judge_mode;

/// The operation name of a refused real commit in `neptron-kms`. The
/// commit is all or nothing, so the mode before it stays. Any other device
/// error of `set_mode` leaves the state unknown, and the run turns the
/// output off.
pub const KMS_COMMIT_OPERATION: &str = "modeset commit";
/// More waits for a flip after a failed one. Each wait is bounded by the
/// backend (500 ms on KMS).
pub const FLIP_WAIT_RETRIES: u32 = 2;
/// The most flips one mode records: 10 s at the 130 Hz of the fastest
/// shipped range, with room.
pub const FLIPS_PER_MODE_MAX: usize = 2_000;

/// What the run gives back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// Each mode that ran, in order.
    pub modes: Vec<ModeReport>,
    /// `Finished` or `Stopped`.
    pub state: RunState,
    /// Why the run stopped early.
    pub stopped: Option<String>,
    /// The restore at the end; `None` when no mode reached the display.
    pub restore: Option<RestoreReport>,
}

/// What the loop needs besides the display.
pub struct Run<'a, C: Clock> {
    /// The time source of the display time.
    pub clock: &'a C,
    /// The profile that the read back is validated against.
    pub profile: &'a MonitorProfile,
    /// The GPU values of the validation.
    pub caps: &'a GpuCaps,
    /// The display time of each mode.
    pub seconds_per_mode: u32,
    /// What one vertical blank of an interlaced mode is; `None` when the
    /// capability table does not say.
    pub blank_unit: Option<BlankUnit>,
    /// Set by a stop signal. The loop reads it before each flip.
    pub stop: &'a AtomicBool,
}

impl<C: Clock> Run<'_, C> {
    /// Shows each of `modes` and restores the display at the end, on a
    /// stop and on an error that leaves the display state unknown: the
    /// output goes off with `blank`, because the state before the run
    /// cannot be read back and validated. `after_mode` gets the reports so
    /// far after each mode, for the report file.
    pub fn show<D: Display>(
        &self,
        display: &mut D,
        modes: &[PlannedMode],
        after_mode: &mut dyn FnMut(&[ModeReport]),
    ) -> Shown {
        let mut reports = Vec::with_capacity(modes.len());
        let mut touched = false;
        let mut stopped = None;
        for planned in modes {
            if self.stop.load(Ordering::Relaxed) {
                stopped = Some(STOP_SIGNAL.to_owned());
                break;
            }
            let mut report = ModeReport::planned(planned);
            let end = self.one_mode(display, planned, &mut report, &mut touched);
            judge_mode(&mut report, self.seconds_per_mode);
            reports.push(report);
            after_mode(&reports);
            if let Err(why) = end {
                stopped = Some(why);
                break;
            }
        }
        let restore = touched.then(|| restore(display));
        Shown {
            modes: reports,
            state: if stopped.is_some() {
                RunState::Stopped
            } else {
                RunState::Finished
            },
            stopped,
            restore,
        }
    }

    /// Sets and shows one mode. `Err` stops the run, with the reason.
    fn one_mode<D: Display>(
        &self,
        display: &mut D,
        planned: &PlannedMode,
        report: &mut ModeReport,
        touched: &mut bool,
    ) -> Result<(), String> {
        let request = *planned.token.mode();
        let readback = match display.set_mode(&planned.token) {
            Ok(readback) => {
                *touched = true;
                report.test_only_commit = Some(step(StepResult::Passed, None));
                report.commit = Some(step(StepResult::Passed, None));
                readback
            }
            Err(DisplayError::ModeRefused { reason, .. }) => {
                report.test_only_commit = Some(step(StepResult::Refused, Some(reason)));
                report.commit = Some(step(StepResult::NotSent, None));
                return Ok(());
            }
            Err(DisplayError::Device { operation, reason })
                if operation == KMS_COMMIT_OPERATION =>
            {
                report.test_only_commit = Some(step(StepResult::Passed, None));
                report.commit = Some(step(StepResult::Failed, Some(reason)));
                return Ok(());
            }
            Err(e) => {
                *touched = true;
                report.test_only_commit = Some(step(StepResult::Unknown, None));
                report.commit = Some(step(StepResult::Unknown, Some(e.to_string())));
                report.errors.push(e.to_string());
                return Err(format!(
                    "set_mode failed and the display state is unknown: {e}"
                ));
            }
        };
        self.read_back(report, &request, &readback);
        if report.readback_valid == Some(false) {
            return Err("the read back fails validate".to_owned());
        }
        if report.readback_equal == Some(false) {
            // The mode on the display is valid but not the one asked for;
            // the pattern of the request would not fit it.
            return Ok(());
        }
        self.show_pattern(display, planned, &request, report)
    }

    fn read_back(&self, report: &mut ModeReport, request: &Modeline, readback: &Modeline) {
        report.readback = Some(*readback);
        report.readback_differences = differences(request, readback);
        report.readback_equal = Some(report.readback_differences.is_empty());
        match validate(readback, self.profile, self.caps) {
            Ok(_) => report.readback_valid = Some(true),
            Err(rejection) => {
                report.readback_valid = Some(false);
                report.readback_rejection = Some(rejection.to_string());
            }
        }
    }

    /// Presents the pattern and waits for each flip until the display time
    /// is over. `Err` stops the run.
    fn show_pattern<D: Display>(
        &self,
        display: &mut D,
        planned: &PlannedMode,
        mode: &Modeline,
        report: &mut ModeReport,
    ) -> Result<(), String> {
        let pattern = Pattern::draw(mode, &label_lines(planned.title, mode));
        let scene = pattern.scene();
        let mut flips: Vec<FlipEvent> = Vec::with_capacity(FLIPS_PER_MODE_MAX);
        let start = self.clock.now_ns();
        let end = start + u64::from(self.seconds_per_mode) * NS_PER_S;
        let mut result = Ok(());
        while self.clock.now_ns() < end && flips.len() < FLIPS_PER_MODE_MAX {
            if self.stop.load(Ordering::Relaxed) {
                result = Err(STOP_SIGNAL.to_owned());
                break;
            }
            if let Err(e) = display.present(&scene) {
                report.errors.push(format!("present: {e}"));
                break;
            }
            match wait(display, &mut report.errors) {
                Some(flip) => flips.push(flip),
                None => {
                    result = Err("a flip did not complete; it stays queued".to_owned());
                    break;
                }
            }
        }
        report.shown_ns = Some(self.clock.now_ns().saturating_sub(start));
        report.timing = Some(stats(&flips, mode, self.blank_unit));
        result
    }
}

/// The reason of a stop by a signal.
pub const STOP_SIGNAL: &str = "a stop signal came";

const NS_PER_S: u64 = 1_000_000_000;

/// Waits for the queued flip, with [`FLIP_WAIT_RETRIES`] more tries after a
/// failure. Each failure goes to `errors`.
fn wait<D: Display>(display: &mut D, errors: &mut Vec<String>) -> Option<FlipEvent> {
    for _ in 0..=FLIP_WAIT_RETRIES {
        match display.wait_flip() {
            Ok(flip) => return Some(flip),
            Err(e) => errors.push(format!("wait_flip: {e}")),
        }
    }
    None
}

/// Turns the output off. With a flip still queued the display refuses it
/// as busy, and the report says so.
fn restore<D: Display>(display: &mut D) -> RestoreReport {
    let result = display.blank();
    RestoreReport {
        method: "blank".to_owned(),
        ok: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
    }
}

fn step(result: StepResult, detail: Option<String>) -> Step {
    Step { result, detail }
}

/// The fields of `b` that differ from `a`, flags as 0 or 1.
pub fn differences(a: &Modeline, b: &Modeline) -> Vec<FieldDifference> {
    let fields: [(&str, u64, u64); 13] = [
        ("pclock_hz", a.pclock_hz.into(), b.pclock_hz.into()),
        ("hactive", a.hactive.into(), b.hactive.into()),
        ("hsync_start", a.hsync_start.into(), b.hsync_start.into()),
        ("hsync_end", a.hsync_end.into(), b.hsync_end.into()),
        ("htotal", a.htotal.into(), b.htotal.into()),
        ("vactive", a.vactive.into(), b.vactive.into()),
        ("vsync_start", a.vsync_start.into(), b.vsync_start.into()),
        ("vsync_end", a.vsync_end.into(), b.vsync_end.into()),
        ("vtotal", a.vtotal.into(), b.vtotal.into()),
        ("interlace", a.interlace.into(), b.interlace.into()),
        ("doublescan", a.doublescan.into(), b.doublescan.into()),
        (
            "hsync_positive",
            a.hsync_positive.into(),
            b.hsync_positive.into(),
        ),
        (
            "vsync_positive",
            a.vsync_positive.into(),
            b.vsync_positive.into(),
        ),
    ];
    fields
        .into_iter()
        .filter(|(_, x, y)| x != y)
        .map(|(field, requested, read_back)| FieldDifference {
            field: field.to_owned(),
            requested,
            read_back,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use neptron_platform::UNKNOWN_GPU_CAPS;
    use neptron_testkit::{DisplayOptions, FakeDisplay, FlipFault, ModeFault, Render, SimClock};

    use super::*;
    use crate::selftest::family::{plan, planned};
    use crate::selftest::report::tests::setup;

    struct Bench {
        clock: SimClock,
        display: FakeDisplay,
        profile: MonitorProfile,
        caps: GpuCaps,
        modes: Vec<PlannedMode>,
        blank_unit: Option<BlankUnit>,
        stop: AtomicBool,
    }

    fn bench(id: &str, caps: GpuCaps) -> Bench {
        let clock = SimClock::new();
        let display = FakeDisplay::with_options(
            clock.clone(),
            caps,
            DisplayOptions {
                render: Render::Off,
                ..DisplayOptions::default()
            },
        );
        let profile = setup(id).profile;
        let modes = planned(&plan(&profile, &caps)).cloned().collect();
        Bench {
            clock,
            display,
            profile,
            caps,
            modes,
            blank_unit: None,
            stop: AtomicBool::new(false),
        }
    }

    impl Bench {
        fn run(&self) -> (Shown, usize) {
            self.run_with(&mut |_| {})
        }

        fn run_with(&self, after: &mut dyn FnMut(&[ModeReport])) -> (Shown, usize) {
            let run = Run {
                clock: &self.clock,
                profile: &self.profile,
                caps: &self.caps,
                seconds_per_mode: 10,
                blank_unit: self.blank_unit,
                stop: &self.stop,
            };
            let mut calls = 0;
            let shown = run.show(&mut self.display.clone(), &self.modes, &mut |r| {
                calls += 1;
                after(r);
            });
            (shown, calls)
        }
    }

    #[test]
    fn every_mode_shows_ten_seconds_with_its_read_back_and_flips() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        assert_eq!(b.modes.len(), 4);
        let (shown, calls) = b.run();
        assert_eq!(shown.state, RunState::Finished);
        assert_eq!(shown.stopped, None);
        assert_eq!(calls, 4, "the report is written after each mode");
        for (m, p) in shown.modes.iter().zip(&b.modes) {
            assert_eq!(m.pass, Some(true), "{}: {:?}", m.family, m.failures);
            assert_eq!(m.readback, Some(*p.token.mode()));
            assert_eq!(m.readback_equal, Some(true));
            let shown_ns = m.shown_ns.unwrap();
            assert!(
                (10 * NS_PER_S..11 * NS_PER_S).contains(&shown_ns),
                "{shown_ns}"
            );
            let t = m.timing.as_ref().unwrap();
            let rate_hz = p.token.mode().field_rate_mhz() / 1_000;
            assert!(t.flips >= 10 * rate_hz - 2, "{}: {}", m.family, t.flips);
            assert_eq!(t.missed_vblanks, 0);
            assert!(t.period_error_ppm.unwrap() < 10, "{t:?}");
            let pclock = u64::from(p.token.mode().pclock_hz);
            assert!(t.implied_pclock_hz.unwrap().abs_diff(pclock) < pclock / 10_000);
        }
        assert!(b.clock.now_ns() >= 40 * NS_PER_S);
        assert_eq!(
            shown.restore,
            Some(RestoreReport {
                method: "blank".into(),
                ok: true,
                error: None
            })
        );
        assert_eq!(b.display.blank_calls().len(), 1);
        assert_eq!(b.display.active_mode(), None, "the output is off");
        let requested: Vec<Modeline> = b.display.mode_calls().iter().map(|c| c.requested).collect();
        let planned: Vec<Modeline> = b.modes.iter().map(|p| *p.token.mode()).collect();
        assert_eq!(requested, planned, "only validated modes reach the display");
    }

    #[test]
    fn an_interlaced_mode_measures_its_blanks_per_field() {
        let caps = GpuCaps {
            pclock_min_hz: 0,
            interlace: true,
            ..UNKNOWN_GPU_CAPS
        };
        let b = bench("generic_15", caps);
        let (shown, _) = b.run();
        let i = shown.modes.iter().find(|m| m.family == "480i").unwrap();
        assert_eq!(i.pass, Some(true), "{:?}", i.failures);
        let t = i.timing.as_ref().unwrap();
        assert_eq!(t.vblank_unit.as_deref(), Some("field"));
        assert!(t.unit_assumed, "the GPU is not in the table");
        assert!(
            t.missed_vblanks > 0,
            "a flip lands once per frame without flip_per_field"
        );
    }

    #[test]
    fn a_gpu_of_the_table_that_flips_per_field_is_measured_per_field() {
        let caps = GpuCaps {
            pclock_min_hz: 0,
            interlace: true,
            flip_per_field: true,
            ..UNKNOWN_GPU_CAPS
        };
        let mut b = bench("generic_15", caps);
        b.blank_unit = BlankUnit::of_caps(caps.flip_per_field, true);
        let (shown, _) = b.run();
        let i = shown.modes.iter().find(|m| m.family == "480i").unwrap();
        assert_eq!(i.pass, Some(true), "{:?}", i.failures);
        let t = i.timing.as_ref().unwrap();
        assert_eq!(t.vblank_unit.as_deref(), Some("field"));
        assert!(!t.unit_assumed);
        assert_eq!(t.missed_vblanks, 0);
    }

    #[test]
    fn a_read_back_that_differs_fails_that_mode_and_the_run_goes_on() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        let other = *b.modes[0].token.mode();
        b.display.fault_mode(1, ModeFault::ReadBack(other));
        let (shown, _) = b.run();
        assert_eq!(shown.state, RunState::Finished);
        let m = &shown.modes[1];
        assert_eq!(m.readback_equal, Some(false));
        assert_eq!(m.readback_valid, Some(true));
        assert!(m.readback_differences.iter().any(|d| d.field == "vactive"));
        assert_eq!(m.pass, Some(false));
        assert!(
            m.failures.iter().any(|f| f.contains("differs")),
            "{:?}",
            m.failures
        );
        assert_eq!(m.shown_ns, None, "the pattern of the request does not show");
        assert!(shown.modes[2..].iter().all(|m| m.pass == Some(true)));
        assert!(shown.restore.unwrap().ok);
    }

    #[test]
    fn a_read_back_that_fails_validate_stops_and_turns_the_output_off() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        let mut wild = *b.modes[0].token.mode();
        wild.pclock_hz /= 2;
        b.display.fault_mode(0, ModeFault::ReadBack(wild));
        let (shown, _) = b.run();
        assert_eq!(shown.state, RunState::Stopped);
        assert!(shown.stopped.as_deref().unwrap().contains("fails validate"));
        assert_eq!(shown.modes.len(), 1);
        assert_eq!(shown.modes[0].readback_valid, Some(false));
        assert!(shown.modes[0].readback_rejection.is_some());
        assert_eq!(b.display.mode_calls().len(), 1);
        assert!(shown.restore.unwrap().ok);
        assert_eq!(b.display.active_mode(), None);
    }

    #[test]
    fn a_refused_commit_fails_that_mode_and_the_run_goes_on() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        b.display.fault_mode(
            0,
            ModeFault::Refuse {
                reason: "EINVAL".into(),
            },
        );
        let (shown, _) = b.run();
        assert_eq!(shown.state, RunState::Finished);
        let m = &shown.modes[0];
        assert_eq!(
            m.test_only_commit,
            Some(step(StepResult::Refused, Some("EINVAL".into())))
        );
        assert_eq!(m.commit, Some(step(StepResult::NotSent, None)));
        assert_eq!(m.pass, Some(false));
        assert!(shown.modes[1..].iter().all(|m| m.pass == Some(true)));
        assert!(shown.restore.unwrap().ok);
    }

    #[test]
    fn a_device_error_in_set_mode_stops_and_turns_the_output_off() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        b.display.fault_mode(
            1,
            ModeFault::Device {
                reason: "gone".into(),
            },
        );
        let (shown, _) = b.run();
        assert_eq!(shown.state, RunState::Stopped);
        assert_eq!(shown.modes.len(), 2);
        assert_eq!(
            shown.modes[1].commit.as_ref().unwrap().result,
            StepResult::Unknown
        );
        assert!(shown.restore.unwrap().ok);
        assert_eq!(b.display.blank_calls().len(), 1);
    }

    #[test]
    fn a_failed_restore_is_reported() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        b.display.fault_blank(0, "no".into());
        let (shown, _) = b.run();
        let restore = shown.restore.unwrap();
        assert!(!restore.ok);
        assert!(restore.error.unwrap().contains("no"));
    }

    #[test]
    fn a_lost_flip_stops_the_run_and_the_busy_restore_is_reported() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        b.display.fault_flip(3, FlipFault::Lost);
        let (shown, _) = b.run();
        assert_eq!(shown.state, RunState::Stopped);
        assert!(shown.stopped.unwrap().contains("did not complete"));
        let m = &shown.modes[0];
        assert_eq!(m.errors.len(), 1 + FLIP_WAIT_RETRIES as usize);
        assert_eq!(m.pass, Some(false));
        assert!(!shown.restore.unwrap().ok, "a queued flip makes blank busy");
    }

    #[test]
    fn a_stop_signal_ends_the_run_after_the_mode_on_screen() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        let (shown, calls) = b.run_with(&mut |_| b.stop.store(true, Ordering::Relaxed));
        assert_eq!(shown.state, RunState::Stopped);
        assert_eq!(shown.stopped.as_deref(), Some(STOP_SIGNAL));
        assert_eq!((shown.modes.len(), calls), (1, 1));
        assert!(shown.restore.unwrap().ok);
    }

    #[test]
    fn a_stop_before_the_first_mode_sets_nothing() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        b.stop.store(true, Ordering::Relaxed);
        let (shown, _) = b.run();
        assert!(shown.modes.is_empty());
        assert_eq!(shown.restore, None);
        assert!(b.display.mode_calls().is_empty());
        assert!(b.display.blank_calls().is_empty());
    }

    #[test]
    fn differences_name_each_field() {
        let b = bench("pc_31_120", UNKNOWN_GPU_CAPS);
        let a = *b.modes[0].token.mode();
        assert!(differences(&a, &a).is_empty());
        let mut c = a;
        c.pclock_hz += 1;
        c.interlace = true;
        let d = differences(&a, &c);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].field, "pclock_hz");
        assert_eq!(
            (d[1].field.as_str(), d[1].requested, d[1].read_back),
            ("interlace", 0, 1)
        );
    }
}
