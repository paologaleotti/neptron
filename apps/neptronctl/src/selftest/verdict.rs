//! The pass and fail rules of a mode and of a run. Pure: they read the
//! report and write the verdict into it.

use super::report::{ModeReport, Report, RunState, StepResult};

/// The fewest flips that measure a vertical blank period.
pub const FLIPS_MIN: u64 = 2;
/// The largest distance of the measured vertical blank period from the
/// period of the mode, in parts per million. The kernel's time stamps
/// follow the clock the hardware makes; 1 % covers the rounding of a
/// driver's clock and finds a wrong rate, as timing check 7 does.
pub const VBLANK_PERIOD_TOLERANCE_PPM: u64 = 10_000;

/// Writes `pass` and `failures` of `mode`, which showed for
/// `seconds_per_mode` seconds.
///
/// Missed vertical blanks are recorded and do not fail a mode: they count
/// how fast the tool draws, not what the display does. The period of the
/// blanks comes from the kernel's counter and time stamps, so missed
/// blanks do not change it.
pub fn judge_mode(mode: &mut ModeReport, seconds_per_mode: u32) {
    let mut failures = Vec::new();
    let step_failed = |step: &Option<super::report::Step>| {
        step.as_ref().is_none_or(|s| s.result != StepResult::Passed)
    };
    if step_failed(&mode.test_only_commit) {
        failures.push("the test-only commit did not pass".to_owned());
    }
    if step_failed(&mode.commit) {
        failures.push("the commit did not pass".to_owned());
    }
    match (mode.readback.is_some(), mode.readback_equal) {
        (false, _) => failures.push("no mode was read back".to_owned()),
        (true, Some(false)) => failures.push(format!(
            "the read back differs in {}",
            mode.readback_differences
                .iter()
                .map(|d| d.field.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        (true, _) => {}
    }
    if mode.readback_valid == Some(false) {
        failures.push(format!(
            "the read back fails validate: {}",
            mode.readback_rejection.as_deref().unwrap_or("no reason")
        ));
    }
    let shown_min_ns = u64::from(seconds_per_mode) * NS_PER_S;
    match mode.shown_ns {
        Some(ns) if ns >= shown_min_ns => {}
        Some(ns) => failures.push(format!(
            "the pattern showed for {} ms of {} ms",
            ns / NS_PER_MS,
            shown_min_ns / NS_PER_MS
        )),
        None => failures.push("the pattern did not show".to_owned()),
    }
    match &mode.timing {
        None => failures.push("no flip was measured".to_owned()),
        Some(t) if t.flips < FLIPS_MIN => failures.push(format!(
            "{} flips completed, at least {FLIPS_MIN} are needed",
            t.flips
        )),
        Some(t) => match t.period_error_ppm {
            None => failures.push("no vertical blank period was measured".to_owned()),
            Some(ppm) if ppm > VBLANK_PERIOD_TOLERANCE_PPM => failures.push(format!(
                "the vertical blank period is {ppm} ppm from the mode, above \
                 {VBLANK_PERIOD_TOLERANCE_PPM} ppm"
            )),
            Some(_) => {}
        },
    }
    if !mode.errors.is_empty() {
        failures.push(format!("{} display errors", mode.errors.len()));
    }
    mode.pass = Some(failures.is_empty());
    mode.failures = failures;
}

/// Writes the `pass` of the run: it finished, it had at least one mode,
/// every mode passed, the restore succeeded, and no other error came.
pub fn judge_run(report: &mut Report) {
    let restored = report.restore.as_ref().is_none_or(|r| r.ok);
    report.pass = Some(
        report.state == RunState::Finished
            && !report.modes.is_empty()
            && report.modes.iter().all(|m| m.pass == Some(true))
            && restored
            && report.errors.is_empty(),
    );
}

const NS_PER_S: u64 = 1_000_000_000;
const NS_PER_MS: u64 = 1_000_000;

#[cfg(test)]
mod tests {
    use neptron_types::Modeline;

    use super::*;
    use crate::selftest::report::{
        FieldDifference, PolicyReport, Rates, RestoreReport, Step, TimingReport,
    };

    fn passed() -> Option<Step> {
        Some(Step {
            result: StepResult::Passed,
            detail: None,
        })
    }

    fn good() -> ModeReport {
        let m = Modeline {
            pclock_hz: 25_175_000,
            hactive: 640,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 525,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        };
        ModeReport {
            family: "480p".into(),
            title: "480P".into(),
            policy: PolicyReport {
                source: String::new(),
                style: String::new(),
                refresh_policy: String::new(),
                horizontal: None,
                rung: String::new(),
                refresh_kind: String::new(),
                repeat: 1,
                width_rule: None,
            },
            request: m,
            request_rates: Rates::of(&m),
            test_only_commit: passed(),
            commit: passed(),
            readback: Some(m),
            readback_equal: Some(true),
            readback_differences: Vec::new(),
            readback_valid: Some(true),
            readback_rejection: None,
            shown_ns: Some(10 * NS_PER_S),
            timing: Some(TimingReport {
                flips: 600,
                vblanks: 599,
                missed_vblanks: 0,
                flips_per_second_mhz: 59_940,
                vblank_period_ns: Some(16_683_350),
                expected_period_ns: Some(16_683_350),
                vblank_unit: Some("field".into()),
                period_error_ppm: Some(0),
                implied_pclock_hz: Some(25_175_000),
            }),
            errors: Vec::new(),
            pass: None,
            failures: Vec::new(),
        }
    }

    fn judged(mut m: ModeReport) -> ModeReport {
        judge_mode(&mut m, 10);
        m
    }

    #[test]
    fn a_good_mode_passes() {
        let m = judged(good());
        assert_eq!(m.pass, Some(true), "{:?}", m.failures);
        assert!(m.failures.is_empty());
    }

    #[test]
    fn missed_blanks_alone_do_not_fail() {
        let mut m = good();
        if let Some(t) = &mut m.timing {
            t.missed_vblanks = 500;
            t.flips = 100;
        }
        assert_eq!(judged(m).pass, Some(true));
    }

    #[test]
    fn each_broken_check_fails_with_its_reason() {
        type Breaks = fn(&mut ModeReport);
        let cases: Vec<(Breaks, &str)> = vec![
            (
                |m| {
                    m.test_only_commit = Some(Step {
                        result: StepResult::Refused,
                        detail: Some("EINVAL".into()),
                    });
                },
                "test-only",
            ),
            (|m| m.commit = None, "the commit"),
            (|m| m.readback = None, "read back"),
            (
                |m| {
                    m.readback_equal = Some(false);
                    m.readback_differences = vec![FieldDifference {
                        field: "pclock_hz".into(),
                        requested: 1,
                        read_back: 2,
                    }];
                },
                "differs in pclock_hz",
            ),
            (
                |m| {
                    m.readback_valid = Some(false);
                    m.readback_rejection = Some("line rate".into());
                },
                "fails validate: line rate",
            ),
            (|m| m.shown_ns = Some(NS_PER_S), "1000 ms of 10000 ms"),
            (|m| m.shown_ns = None, "did not show"),
            (|m| m.timing = None, "no flip"),
            (
                |m| {
                    if let Some(t) = &mut m.timing {
                        t.flips = 1;
                    }
                },
                "at least 2",
            ),
            (
                |m| {
                    if let Some(t) = &mut m.timing {
                        t.period_error_ppm = Some(VBLANK_PERIOD_TOLERANCE_PPM + 1);
                    }
                },
                "10001 ppm",
            ),
            (
                |m| {
                    if let Some(t) = &mut m.timing {
                        t.period_error_ppm = None;
                    }
                },
                "no vertical blank period",
            ),
            (|m| m.errors.push("lost".into()), "1 display errors"),
        ];
        for (breaks, reason) in cases {
            let mut m = good();
            breaks(&mut m);
            let m = judged(m);
            assert_eq!(m.pass, Some(false), "{reason}");
            assert!(
                m.failures.iter().any(|f| f.contains(reason)),
                "{reason}: {:?}",
                m.failures
            );
        }
    }

    #[test]
    fn the_limit_itself_passes() {
        let mut m = good();
        if let Some(t) = &mut m.timing {
            t.period_error_ppm = Some(VBLANK_PERIOD_TOLERANCE_PPM);
        }
        assert_eq!(judged(m).pass, Some(true));
    }

    #[test]
    fn a_run_passes_only_when_finished_restored_and_every_mode_passed() {
        let setup = crate::selftest::report::tests::setup("pc_31_120");
        let mut report = crate::selftest::report::Report::planned(&setup, &[], 10, false);
        report.state = RunState::Finished;
        report.modes = vec![judged(good()), judged(good())];
        report.restore = Some(RestoreReport {
            method: "blank".into(),
            ok: true,
            error: None,
        });
        let verdict = |r: &Report| {
            let mut r = r.clone();
            judge_run(&mut r);
            r.pass
        };
        assert_eq!(verdict(&report), Some(true));
        let mut stopped = report.clone();
        stopped.state = RunState::Stopped;
        assert_eq!(verdict(&stopped), Some(false));
        let mut empty = report.clone();
        empty.modes.clear();
        assert_eq!(verdict(&empty), Some(false));
        let mut failed = report.clone();
        failed.modes[1].pass = Some(false);
        assert_eq!(verdict(&failed), Some(false));
        let mut unrestored = report.clone();
        if let Some(r) = &mut unrestored.restore {
            r.ok = false;
        }
        assert_eq!(verdict(&unrestored), Some(false));
        let mut errors = report.clone();
        errors.errors.push("x".into());
        assert_eq!(verdict(&errors), Some(false));
    }
}
