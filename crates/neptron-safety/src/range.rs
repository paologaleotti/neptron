use neptron_types::CrtRange;

use crate::check::Check;
use crate::limits::{Fault, Limits};
use crate::measure::Measure;
use crate::{Axis, InterlaceLimit, PorchEdge, Rejection};

/// Accepts a mode when one range of the profile takes it whole. Otherwise
/// it reports the range that is the closest to the mode: the range that
/// failed at the latest stage, and among those the range whose broken
/// limit is nearest to the value of the mode, the first one on a tie.
pub(crate) fn check_ranges(m: &Measure, ranges: &[CrtRange]) -> Result<(), Rejection> {
    let mut closest: Option<(Fault, Limits)> = None;
    for range in ranges {
        let limits = Limits::of_range(range, m.field_rate_mhz);
        match limits.check(m) {
            Ok(()) => return Ok(()),
            Err(fault) => {
                let closer = closest.is_none_or(|(best, _)| {
                    let (now, then) = (stage(fault.check), stage(best.check));
                    now > then || (now == then && distance(&fault) < distance(&best))
                });
                if closer {
                    closest = Some((fault, limits));
                }
            }
        }
    }
    match closest {
        Some((fault, limits)) => Err(rejection(m, &fault, &limits)),
        // A profile with no range accepts no line rate.
        None => Err(Rejection::LineRate {
            value_mhz: m.line_rate_mhz,
            min_mhz: 0,
            max_mhz: 0,
        }),
    }
}

/// The checks of one value share a stage, so that a range that fails low
/// and a range that fails high on the same value compare by distance.
fn stage(check: Check) -> u8 {
    match check {
        Check::LineRateMin | Check::LineRateMax => 0,
        Check::FieldRateMin | Check::FieldRateMax => 1,
        Check::ProgressiveLinesMin
        | Check::ProgressiveLinesMax
        | Check::InterlacedLinesMin
        | Check::InterlacedLinesMax => 2,
        Check::HorizontalSyncMin | Check::HorizontalSyncMax => 3,
        Check::VerticalSyncMin | Check::VerticalSyncMax => 4,
        Check::HorizontalFrontPorchMin => 5,
        Check::HorizontalBackPorchMin => 6,
        Check::VerticalFrontPorchMin => 7,
        Check::VerticalBackPorchMin => 8,
    }
}

/// How far the value of the mode is beyond the broken limit.
fn distance(fault: &Fault) -> u64 {
    if fault.value < fault.min {
        fault.min - fault.value
    } else {
        fault.value.saturating_sub(fault.max)
    }
}

fn rejection(m: &Measure, fault: &Fault, limits: &Limits) -> Rejection {
    let sync = |axis| Rejection::SyncWidth {
        axis,
        value_ns: fault.value,
        min_ns: fault.min,
        max_ns: fault.max,
    };
    let porch = |axis, edge| Rejection::Porch {
        axis,
        edge,
        value_ns: fault.value,
        min_ns: fault.min,
    };
    match fault.check {
        Check::LineRateMin | Check::LineRateMax => Rejection::LineRate {
            value_mhz: fault.value,
            min_mhz: fault.min,
            max_mhz: fault.max,
        },
        Check::FieldRateMin | Check::FieldRateMax => Rejection::FieldRate {
            value_mhz: fault.value,
            min_mhz: fault.min,
            max_mhz: fault.max,
        },
        Check::ProgressiveLinesMin | Check::ProgressiveLinesMax => Rejection::LineCount {
            value: m.lines,
            min: limits.progressive_lines_min,
            max: limits.progressive_lines_max,
            interlaced: false,
        },
        Check::InterlacedLinesMin | Check::InterlacedLinesMax
            if limits.interlaced_lines_max == 0 =>
        {
            Rejection::Interlace {
                forbidden_by: InterlaceLimit::Profile,
            }
        }
        Check::InterlacedLinesMin | Check::InterlacedLinesMax => Rejection::LineCount {
            value: m.lines,
            min: limits.interlaced_lines_min,
            max: limits.interlaced_lines_max,
            interlaced: true,
        },
        Check::HorizontalSyncMin | Check::HorizontalSyncMax => sync(Axis::Horizontal),
        Check::VerticalSyncMin | Check::VerticalSyncMax => sync(Axis::Vertical),
        Check::HorizontalFrontPorchMin => porch(Axis::Horizontal, PorchEdge::Front),
        Check::HorizontalBackPorchMin => porch(Axis::Horizontal, PorchEdge::Back),
        Check::VerticalFrontPorchMin => porch(Axis::Vertical, PorchEdge::Front),
        Check::VerticalBackPorchMin => porch(Axis::Vertical, PorchEdge::Back),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Span;

    fn range() -> CrtRange {
        CrtRange {
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
        }
    }

    /// A mode at the nominal values of `range()`, with 64 us lines.
    fn measure() -> Measure {
        Measure {
            line_rate_mhz: 15_700_000,
            field_rate_mhz: 60_000,
            interlace: false,
            lines: 240,
            h_front_porch: Span::ns(2_000),
            h_sync: Span::ns(4_700),
            h_back_porch: Span::ns(8_000),
            v_front_porch: Span::ns(64_000),
            v_sync: Span::ns(192_000),
            v_back_porch: Span::ns(1_024_000),
            line: Span::ns(64_000),
        }
    }

    #[test]
    fn a_mode_inside_one_range_passes() {
        assert_eq!(check_ranges(&measure(), &[range()]), Ok(()));
        let far = CrtRange {
            h_min_mhz: 31_000_000,
            h_max_mhz: 32_000_000,
            ..range()
        };
        assert_eq!(check_ranges(&measure(), &[far, range()]), Ok(()));
    }

    #[test]
    fn no_range_accepts_nothing() {
        assert_eq!(
            check_ranges(&measure(), &[]),
            Err(Rejection::LineRate {
                value_mhz: 15_700_000,
                min_mhz: 0,
                max_mhz: 0
            })
        );
    }

    #[test]
    fn each_check_maps_to_its_rejection() {
        let cases: [(Measure, Rejection); 12] = [
            (
                Measure {
                    line_rate_mhz: 15_750_001,
                    ..measure()
                },
                Rejection::LineRate {
                    value_mhz: 15_750_001,
                    min_mhz: 15_625_000,
                    max_mhz: 15_750_000,
                },
            ),
            (
                Measure {
                    field_rate_mhz: 49_499,
                    ..measure()
                },
                Rejection::FieldRate {
                    value_mhz: 49_499,
                    min_mhz: 49_500,
                    max_mhz: 65_000,
                },
            ),
            (
                Measure {
                    lines: 289,
                    ..measure()
                },
                Rejection::LineCount {
                    value: 289,
                    min: 192,
                    max: 288,
                    interlaced: false,
                },
            ),
            (
                Measure {
                    interlace: true,
                    lines: 447,
                    ..measure()
                },
                Rejection::LineCount {
                    value: 447,
                    min: 448,
                    max: 576,
                    interlaced: true,
                },
            ),
            (
                Measure {
                    h_sync: Span::ns(4_229),
                    ..measure()
                },
                Rejection::SyncWidth {
                    axis: Axis::Horizontal,
                    value_ns: 4_229,
                    min_ns: 4_230,
                    max_ns: 7_050,
                },
            ),
            (
                Measure {
                    h_sync: Span::ns(7_051),
                    ..measure()
                },
                Rejection::SyncWidth {
                    axis: Axis::Horizontal,
                    value_ns: 7_051,
                    min_ns: 4_230,
                    max_ns: 7_050,
                },
            ),
            (
                Measure {
                    v_sync: Span::ns(127_999),
                    ..measure()
                },
                Rejection::SyncWidth {
                    axis: Axis::Vertical,
                    value_ns: 127_999,
                    min_ns: 128_000,
                    max_ns: 288_000,
                },
            ),
            (
                Measure {
                    v_sync: Span::ns(288_001),
                    ..measure()
                },
                Rejection::SyncWidth {
                    axis: Axis::Vertical,
                    value_ns: 288_001,
                    min_ns: 128_000,
                    max_ns: 288_000,
                },
            ),
            (
                Measure {
                    h_front_porch: Span::ns(1_799),
                    ..measure()
                },
                Rejection::Porch {
                    axis: Axis::Horizontal,
                    edge: PorchEdge::Front,
                    value_ns: 1_799,
                    min_ns: 1_800,
                },
            ),
            (
                Measure {
                    h_back_porch: Span::ns(7_199),
                    ..measure()
                },
                Rejection::Porch {
                    axis: Axis::Horizontal,
                    edge: PorchEdge::Back,
                    value_ns: 7_199,
                    min_ns: 7_200,
                },
            ),
            (
                Measure {
                    v_front_porch: Span::ns(0),
                    ..measure()
                },
                Rejection::Porch {
                    axis: Axis::Vertical,
                    edge: PorchEdge::Front,
                    value_ns: 0,
                    min_ns: 1,
                },
            ),
            (
                Measure {
                    v_back_porch: Span::ns(921_599),
                    ..measure()
                },
                Rejection::Porch {
                    axis: Axis::Vertical,
                    edge: PorchEdge::Back,
                    value_ns: 921_599,
                    min_ns: 921_600,
                },
            ),
        ];
        for (m, expected) in cases {
            assert_eq!(check_ranges(&m, &[range()]), Err(expected));
        }
    }

    #[test]
    fn an_interlaced_mode_needs_an_interlaced_range() {
        let progressive_only = CrtRange {
            interlaced_lines_min: 0,
            interlaced_lines_max: 0,
            ..range()
        };
        let interlaced = Measure {
            interlace: true,
            lines: 480,
            ..measure()
        };
        assert_eq!(check_ranges(&interlaced, &[range()]), Ok(()));
        assert_eq!(
            check_ranges(&interlaced, &[progressive_only]),
            Err(Rejection::Interlace {
                forbidden_by: InterlaceLimit::Profile
            })
        );
    }

    #[test]
    fn the_closest_range_is_reported() {
        let far = CrtRange {
            h_min_mhz: 31_000_000,
            h_max_mhz: 32_000_000,
            ..range()
        };
        let short_lines = CrtRange {
            progressive_lines_max: 224,
            ..range()
        };
        // `far` fails at the line rate, `short_lines` at the line count.
        assert_eq!(
            check_ranges(&measure(), &[far, short_lines]),
            Err(Rejection::LineCount {
                value: 240,
                min: 192,
                max: 224,
                interlaced: false
            })
        );
        // Both fail low on the line rate: the nearer minimum wins.
        let higher = CrtRange {
            h_min_mhz: 16_000_000,
            h_max_mhz: 17_000_000,
            ..range()
        };
        let higher_rejection = Err(Rejection::LineRate {
            value_mhz: 15_700_000,
            min_mhz: 16_000_000,
            max_mhz: 17_000_000,
        });
        assert_eq!(check_ranges(&measure(), &[far, higher]), higher_rejection);
        assert_eq!(check_ranges(&measure(), &[higher, far]), higher_rejection);
        // At the same distance the first range is reported.
        let lower = CrtRange {
            h_min_mhz: 15_000_000,
            h_max_mhz: 15_400_000,
            ..range()
        };
        assert_eq!(check_ranges(&measure(), &[higher, lower]), higher_rejection);
    }

    #[test]
    fn failing_high_and_failing_low_compare_by_distance() {
        // arcade_15_25: a 15 kHz range and a fixed 24960 Hz range. A
        // 24.8 kHz mode is far above the first and just below the second.
        let fixed_25 = CrtRange {
            h_min_mhz: 24_960_000,
            h_max_mhz: 24_960_000,
            ..range()
        };
        let mode = Measure {
            line_rate_mhz: 24_800_000,
            ..measure()
        };
        assert_eq!(
            check_ranges(&mode, &[range(), fixed_25]),
            Err(Rejection::LineRate {
                value_mhz: 24_800_000,
                min_mhz: 24_900_000,
                max_mhz: 24_960_000
            })
        );
    }

    #[test]
    fn a_fixed_line_rate_accepts_one_field_rate_below_it() {
        let pal = CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 15_625_000,
            ..range()
        };
        let at = |line_rate_mhz| {
            let m = Measure {
                line_rate_mhz,
                field_rate_mhz: 50_000,
                ..measure()
            };
            check_ranges(&m, &[pal])
        };
        let rejected = |value_mhz| {
            Err(Rejection::LineRate {
                value_mhz,
                min_mhz: 15_575_000,
                max_mhz: 15_625_000,
            })
        };
        // 15625 Hz less one 50 Hz field rate.
        assert_eq!(at(15_575_000), Ok(()));
        assert_eq!(at(15_575_001), Ok(()));
        assert_eq!(at(15_574_999), rejected(15_574_999));
        // No tolerance above the range.
        assert_eq!(at(15_625_000), Ok(()));
        assert_eq!(at(15_625_001), rejected(15_625_001));
        // The bound moves with the field rate of the mode.
        let fast = Measure {
            line_rate_mhz: 15_565_000,
            field_rate_mhz: 60_000,
            ..measure()
        };
        assert_eq!(check_ranges(&fast, &[pal]), Ok(()));
    }

    #[test]
    fn a_wide_range_keeps_its_minimum() {
        let low = Measure {
            line_rate_mhz: 15_624_999,
            ..measure()
        };
        assert_eq!(
            check_ranges(&low, &[range()]),
            Err(Rejection::LineRate {
                value_mhz: 15_624_999,
                min_mhz: 15_625_000,
                max_mhz: 15_750_000
            })
        );
    }
}
