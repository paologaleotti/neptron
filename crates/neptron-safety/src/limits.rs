use neptron_types::CrtRange;

use crate::Axis;
use crate::check::Check;
use crate::measure::{Measure, Span};
use crate::tolerance::{line_rate_min_mhz, min_ns, sync_max_ns};

/// The bounds of one CRT range or of one hard-rail class. A range has one
/// nominal sync time per axis; a class has the shortest and the longest of
/// its presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    pub(crate) line_rate_min_mhz: u64,
    pub(crate) line_rate_max_mhz: u64,
    pub(crate) field_rate_min_mhz: u64,
    pub(crate) field_rate_max_mhz: u64,
    pub(crate) progressive_lines_min: u16,
    pub(crate) progressive_lines_max: u16,
    /// 0 when no interlaced mode is allowed.
    pub(crate) interlaced_lines_min: u16,
    /// 0 when no interlaced mode is allowed.
    pub(crate) interlaced_lines_max: u16,
    pub(crate) h_sync_nominal_min_ns: u64,
    pub(crate) h_sync_nominal_max_ns: u64,
    pub(crate) v_sync_nominal_min_ns: u64,
    pub(crate) v_sync_nominal_max_ns: u64,
    pub(crate) h_front_porch_nominal_ns: u64,
    pub(crate) h_back_porch_nominal_ns: u64,
    pub(crate) v_front_porch_nominal_ns: u64,
    pub(crate) v_back_porch_nominal_ns: u64,
}

/// The first bound a mode breaks, with the value of the mode and the
/// bounds. `max` is `u64::MAX` for a check with no upper bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fault {
    pub(crate) check: Check,
    pub(crate) value: u64,
    pub(crate) min: u64,
    pub(crate) max: u64,
}

impl Limits {
    /// The limits of a range for a mode with the field rate
    /// `field_rate_mhz`, which sets the lowest line rate.
    pub(crate) fn of_range(range: &CrtRange, field_rate_mhz: u64) -> Limits {
        Limits {
            line_rate_min_mhz: line_rate_min_mhz(range.h_min_mhz, range.h_max_mhz, field_rate_mhz),
            line_rate_max_mhz: range.h_max_mhz,
            field_rate_min_mhz: range.v_min_mhz,
            field_rate_max_mhz: range.v_max_mhz,
            progressive_lines_min: range.progressive_lines_min,
            progressive_lines_max: range.progressive_lines_max,
            interlaced_lines_min: range.interlaced_lines_min,
            interlaced_lines_max: range.interlaced_lines_max,
            h_sync_nominal_min_ns: u64::from(range.h_sync_ns),
            h_sync_nominal_max_ns: u64::from(range.h_sync_ns),
            v_sync_nominal_min_ns: u64::from(range.v_sync_ns),
            v_sync_nominal_max_ns: u64::from(range.v_sync_ns),
            h_front_porch_nominal_ns: u64::from(range.h_front_porch_ns),
            h_back_porch_nominal_ns: u64::from(range.h_back_porch_ns),
            v_front_porch_nominal_ns: u64::from(range.v_front_porch_ns),
            v_back_porch_nominal_ns: u64::from(range.v_back_porch_ns),
        }
    }

    /// Checks every bound in the order of `Check`, exactly: no value is
    /// rounded in favour of the mode.
    pub(crate) fn check(&self, m: &Measure) -> Result<(), Fault> {
        within(
            (Check::LineRateMin, Check::LineRateMax),
            m.line_rate_mhz,
            self.line_rate_min_mhz,
            self.line_rate_max_mhz,
        )?;
        within(
            (Check::FieldRateMin, Check::FieldRateMax),
            m.field_rate_mhz,
            self.field_rate_min_mhz,
            self.field_rate_max_mhz,
        )?;
        let lines = u64::from(m.lines);
        if m.interlace {
            within(
                (Check::InterlacedLinesMin, Check::InterlacedLinesMax),
                lines,
                u64::from(self.interlaced_lines_min),
                u64::from(self.interlaced_lines_max),
            )?;
        } else {
            within(
                (Check::ProgressiveLinesMin, Check::ProgressiveLinesMax),
                lines,
                u64::from(self.progressive_lines_min),
                u64::from(self.progressive_lines_max),
            )?;
        }
        let line_ns = m.line.floor_ns();
        let h = Axis::Horizontal;
        let v = Axis::Vertical;
        span_within(
            (Check::HorizontalSyncMin, Check::HorizontalSyncMax),
            m.h_sync,
            min_ns(h, self.h_sync_nominal_min_ns, line_ns),
            sync_max_ns(h, self.h_sync_nominal_max_ns, line_ns),
        )?;
        span_within(
            (Check::VerticalSyncMin, Check::VerticalSyncMax),
            m.v_sync,
            min_ns(v, self.v_sync_nominal_min_ns, line_ns),
            sync_max_ns(v, self.v_sync_nominal_max_ns, line_ns),
        )?;
        let porches = [
            (
                Check::HorizontalFrontPorchMin,
                m.h_front_porch,
                min_ns(h, self.h_front_porch_nominal_ns, line_ns),
            ),
            (
                Check::HorizontalBackPorchMin,
                m.h_back_porch,
                min_ns(h, self.h_back_porch_nominal_ns, line_ns),
            ),
            (
                Check::VerticalFrontPorchMin,
                m.v_front_porch,
                min_ns(v, self.v_front_porch_nominal_ns, line_ns),
            ),
            (
                Check::VerticalBackPorchMin,
                m.v_back_porch,
                min_ns(v, self.v_back_porch_nominal_ns, line_ns),
            ),
        ];
        for (check, span, min) in porches {
            let value = span.floor_ns();
            if value < min {
                return Err(Fault {
                    check,
                    value,
                    min,
                    max: u64::MAX,
                });
            }
        }
        Ok(())
    }
}

/// `checks` names the lower and the upper bound.
fn within(checks: (Check, Check), value: u64, min: u64, max: u64) -> Result<(), Fault> {
    let check = if value < min {
        checks.0
    } else if value > max {
        checks.1
    } else {
        return Ok(());
    };
    Err(Fault {
        check,
        value,
        min,
        max,
    })
}

/// A span below `min` reports its value rounded down, a span above `max`
/// its value rounded up, so the reported value is beyond the limit.
fn span_within(checks: (Check, Check), span: Span, min: u64, max: u64) -> Result<(), Fault> {
    let low = span.floor_ns();
    if low < min {
        return within(checks, low, min, max);
    }
    within(checks, span.ceil_ns(), min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Line 64 us. The sync and porch bounds that follow from it:
    /// horizontal sync 3600 to 6000 ns, vertical sync 136000 to 300000 ns,
    /// porches at least 1800, 7200, 36000 and 900000 ns.
    const LIMITS: Limits = Limits {
        line_rate_min_mhz: 15_000_000,
        line_rate_max_mhz: 16_000_000,
        field_rate_min_mhz: 50_000,
        field_rate_max_mhz: 60_000,
        progressive_lines_min: 200,
        progressive_lines_max: 300,
        interlaced_lines_min: 400,
        interlaced_lines_max: 600,
        h_sync_nominal_min_ns: 4_000,
        h_sync_nominal_max_ns: 4_000,
        v_sync_nominal_min_ns: 200_000,
        v_sync_nominal_max_ns: 200_000,
        h_front_porch_nominal_ns: 2_000,
        h_back_porch_nominal_ns: 8_000,
        v_front_porch_nominal_ns: 100_000,
        v_back_porch_nominal_ns: 1_000_000,
    };

    fn measure() -> Measure {
        Measure {
            line_rate_mhz: 15_500_000,
            field_rate_mhz: 55_000,
            interlace: false,
            lines: 250,
            h_front_porch: Span::ns(2_000),
            h_sync: Span::ns(4_000),
            h_back_porch: Span::ns(8_000),
            v_front_porch: Span::ns(100_000),
            v_sync: Span::ns(200_000),
            v_back_porch: Span::ns(1_000_000),
            line: Span::ns(64_000),
        }
    }

    fn lines(n: u64) -> u16 {
        u16::try_from(n).unwrap()
    }

    type Set = fn(&mut Measure, u64);

    /// (check, limit, bounds reported in the fault, how to set the value)
    fn cases() -> Vec<(Check, u64, (u64, u64), Set)> {
        vec![
            (
                Check::LineRateMin,
                15_000_000,
                (15_000_000, 16_000_000),
                |m, v| m.line_rate_mhz = v,
            ),
            (
                Check::LineRateMax,
                16_000_000,
                (15_000_000, 16_000_000),
                |m, v| m.line_rate_mhz = v,
            ),
            (Check::FieldRateMin, 50_000, (50_000, 60_000), |m, v| {
                m.field_rate_mhz = v
            }),
            (Check::FieldRateMax, 60_000, (50_000, 60_000), |m, v| {
                m.field_rate_mhz = v
            }),
            (Check::ProgressiveLinesMin, 200, (200, 300), |m, v| {
                m.lines = lines(v)
            }),
            (Check::ProgressiveLinesMax, 300, (200, 300), |m, v| {
                m.lines = lines(v)
            }),
            (Check::InterlacedLinesMin, 400, (400, 600), |m, v| {
                m.interlace = true;
                m.lines = lines(v);
            }),
            (Check::InterlacedLinesMax, 600, (400, 600), |m, v| {
                m.interlace = true;
                m.lines = lines(v);
            }),
            (Check::HorizontalSyncMin, 3_600, (3_600, 6_000), |m, v| {
                m.h_sync = Span::ns(v)
            }),
            (Check::HorizontalSyncMax, 6_000, (3_600, 6_000), |m, v| {
                m.h_sync = Span::ns(v)
            }),
            (
                Check::VerticalSyncMin,
                136_000,
                (136_000, 300_000),
                |m, v| m.v_sync = Span::ns(v),
            ),
            (
                Check::VerticalSyncMax,
                300_000,
                (136_000, 300_000),
                |m, v| m.v_sync = Span::ns(v),
            ),
            (
                Check::HorizontalFrontPorchMin,
                1_800,
                (1_800, u64::MAX),
                |m, v| {
                    m.h_front_porch = Span::ns(v);
                },
            ),
            (
                Check::HorizontalBackPorchMin,
                7_200,
                (7_200, u64::MAX),
                |m, v| {
                    m.h_back_porch = Span::ns(v);
                },
            ),
            (
                Check::VerticalFrontPorchMin,
                36_000,
                (36_000, u64::MAX),
                |m, v| {
                    m.v_front_porch = Span::ns(v);
                },
            ),
            (
                Check::VerticalBackPorchMin,
                900_000,
                (900_000, u64::MAX),
                |m, v| {
                    m.v_back_porch = Span::ns(v);
                },
            ),
        ]
    }

    fn at(set: Set, value: u64) -> Result<(), Fault> {
        let mut m = measure();
        set(&mut m, value);
        LIMITS.check(&m)
    }

    #[test]
    fn the_base_measure_passes() {
        assert_eq!(LIMITS.check(&measure()), Ok(()));
    }

    #[test]
    fn every_bound_passes_at_its_limit_and_fails_one_unit_beyond() {
        let cases = cases();
        assert_eq!(cases.len(), 16, "one case per check");
        for (check, limit, (min, max), set) in cases {
            let (inside, outside) = if is_upper(check) {
                (limit - 1, limit + 1)
            } else {
                (limit + 1, limit - 1)
            };
            assert_eq!(at(set, limit), Ok(()), "{check:?} at the limit");
            assert_eq!(at(set, inside), Ok(()), "{check:?} one unit inside");
            assert_eq!(
                at(set, outside),
                Err(Fault {
                    check,
                    value: outside,
                    min,
                    max
                }),
                "{check:?} one unit outside"
            );
        }
    }

    fn is_upper(check: Check) -> bool {
        matches!(
            check,
            Check::LineRateMax
                | Check::FieldRateMax
                | Check::ProgressiveLinesMax
                | Check::InterlacedLinesMax
                | Check::HorizontalSyncMax
                | Check::VerticalSyncMax
        )
    }

    #[test]
    fn far_out_values_are_rejected() {
        assert_eq!(
            at(|m, v| m.line_rate_mhz = v, 31_000_000)
                .unwrap_err()
                .check,
            Check::LineRateMax
        );
        assert_eq!(
            at(|m, v| m.line_rate_mhz = v, 0).unwrap_err().check,
            Check::LineRateMin
        );
        assert_eq!(
            at(|m, v| m.h_sync = Span::ns(v), 0).unwrap_err().check,
            Check::HorizontalSyncMin
        );
        assert_eq!(
            at(|m, v| m.v_back_porch = Span::ns(v), 0)
                .unwrap_err()
                .check,
            Check::VerticalBackPorchMin
        );
    }

    #[test]
    fn a_fraction_of_a_nanosecond_beyond_a_limit_fails() {
        // 3599.5 ns is below 3600 and reports 3599.
        let short = Measure {
            h_sync: Span {
                scaled: 7_199,
                den: 2,
            },
            ..measure()
        };
        assert_eq!(
            LIMITS.check(&short),
            Err(Fault {
                check: Check::HorizontalSyncMin,
                value: 3_599,
                min: 3_600,
                max: 6_000
            })
        );
        // 6000.5 ns is above 6000 and reports 6001.
        let long = Measure {
            h_sync: Span {
                scaled: 12_001,
                den: 2,
            },
            ..measure()
        };
        assert_eq!(
            LIMITS.check(&long),
            Err(Fault {
                check: Check::HorizontalSyncMax,
                value: 6_001,
                min: 3_600,
                max: 6_000
            })
        );
        // 1799.5 ns of front porch is below 1800.
        let porch = Measure {
            h_front_porch: Span {
                scaled: 3_599,
                den: 2,
            },
            ..measure()
        };
        assert_eq!(LIMITS.check(&porch).unwrap_err().value, 1_799);
    }

    #[test]
    fn no_interlaced_lines_refuse_every_interlaced_mode() {
        let progressive_only = Limits {
            interlaced_lines_min: 0,
            interlaced_lines_max: 0,
            ..LIMITS
        };
        let interlaced = Measure {
            interlace: true,
            lines: 480,
            ..measure()
        };
        assert_eq!(
            progressive_only.check(&interlaced),
            Err(Fault {
                check: Check::InterlacedLinesMax,
                value: 480,
                min: 0,
                max: 0
            })
        );
        assert_eq!(progressive_only.check(&measure()), Ok(()));
    }

    #[test]
    fn the_first_broken_bound_is_reported() {
        let both = Measure {
            field_rate_mhz: 70_000,
            h_sync: Span::ns(0),
            ..measure()
        };
        assert_eq!(LIMITS.check(&both).unwrap_err().check, Check::FieldRateMax);
    }

    #[test]
    fn a_range_gives_one_nominal_sync_per_axis() {
        let range = CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 15_750_000,
            v_min_mhz: 49_500,
            v_max_mhz: 65_000,
            h_front_porch_ns: 2_000,
            h_sync_ns: 4_700,
            h_back_porch_ns: 8_000,
            v_front_porch_ns: 64_000,
            v_sync_ns: 192_000,
            v_back_porch_ns: 1_024_000,
            hsync_positive: false,
            vsync_positive: false,
            progressive_lines_min: 192,
            progressive_lines_max: 288,
            interlaced_lines_min: 448,
            interlaced_lines_max: 576,
        };
        let l = Limits::of_range(&range, 60_000);
        assert_eq!(
            (l.h_sync_nominal_min_ns, l.h_sync_nominal_max_ns),
            (4_700, 4_700)
        );
        assert_eq!(
            (l.v_sync_nominal_min_ns, l.v_sync_nominal_max_ns),
            (192_000, 192_000)
        );
        assert_eq!(
            (l.line_rate_min_mhz, l.line_rate_max_mhz),
            (15_625_000, 15_750_000)
        );
        let fixed = CrtRange {
            h_max_mhz: 15_625_000,
            ..range
        };
        let f = Limits::of_range(&fixed, 50_000);
        assert_eq!(
            (f.line_rate_min_mhz, f.line_rate_max_mhz),
            (15_575_000, 15_625_000)
        );
        assert_eq!(
            (l.field_rate_min_mhz, l.field_rate_max_mhz),
            (49_500, 65_000)
        );
        assert_eq!(
            (l.progressive_lines_min, l.progressive_lines_max),
            (192, 288)
        );
        assert_eq!((l.interlaced_lines_min, l.interlaced_lines_max), (448, 576));
        assert_eq!(
            (
                l.h_front_porch_nominal_ns,
                l.h_back_porch_nominal_ns,
                l.v_front_porch_nominal_ns,
                l.v_back_porch_nominal_ns
            ),
            (2_000, 8_000, 64_000, 1_024_000)
        );
    }
}
