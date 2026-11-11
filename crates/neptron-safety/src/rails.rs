use crate::Rejection;
use crate::class::CLASSES;
use crate::limits::Limits;
use crate::measure::Measure;

/// The compiled-in limits of one line-rate class. No profile can widen
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClassRails {
    pub(crate) class: &'static str,
    pub(crate) limits: Limits,
    /// The name of each rail, at the index of its `Check`.
    pub(crate) names: [&'static str; 16],
}

// `RAILS` holds one `ClassRails` per entry of `CLASSES`, in the same order.
include!(concat!(env!("OUT_DIR"), "/rails.rs"));

/// The rails of the class that holds `line_rate_mhz`, or `None` above the
/// 31 kHz class, where the profile built from EDID is the limit.
pub(crate) fn rails_for(line_rate_mhz: u64) -> Option<&'static ClassRails> {
    CLASSES
        .iter()
        .zip(RAILS.iter())
        .find(|((_, low, high), _)| (*low..=*high).contains(&line_rate_mhz))
        .map(|(_, rails)| rails)
}

/// Checks a mode against the rails of its class, with the same tolerances
/// as the range checks.
pub(crate) fn check_rails(m: &Measure) -> Result<(), Rejection> {
    let Some(rails) = rails_for(m.line_rate_mhz) else {
        return Ok(());
    };
    rails.limits.check(m).map_err(|fault| Rejection::HardRail {
        // `index` is below 16, the length of `names`: `Check` has 16
        // variants, as the test of `Check::ALL` proves.
        rail: rails.names[fault.check.index()],
        value: fault.value,
        limit: if fault.check.is_max() {
            fault.max
        } else {
            fault.min
        },
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use neptron_types::{CrtRange, MonitorKind, MonitorProfile};

    use super::*;
    use crate::Axis;
    use crate::check::Check;
    use crate::class::EDID_MIN_LINE_RATE_MHZ;
    use crate::measure::Span;
    use crate::tolerance::{line_rate_min_mhz, min_ns, sync_max_ns};

    /// The rails, computed by hand from the `crt_range` lines of
    /// data/monitors. A change to a preset that moves a rail fails here.
    #[test]
    fn rails_match_the_presets() {
        let expected = [
            (
                "15 kHz class",
                Limits {
                    // 15250 Hz (d9200, d9400, d9800, m3129); 19000 Hz
                    // (d9400, d9800 18001-19000) clipped below 19 kHz.
                    line_rate_min_mhz: 15_250_000,
                    line_rate_max_mhz: 18_999_999,
                    // 40-80 Hz (d9200, d9400, d9800, m3129).
                    field_rate_min_mhz: 40_000,
                    field_rate_max_mhz: 80_000,
                    // 192 (generic_15 and most); 320 (d9400, d9800
                    // 18001-19000).
                    progressive_lines_min: 192,
                    progressive_lines_max: 320,
                    interlaced_lines_min: 448,
                    interlaced_lines_max: 576,
                    // 0.400 us (pstar); 4.750 us (ms2930, r666b).
                    h_sync_nominal_min_ns: 400,
                    h_sync_nominal_max_ns: 4_750,
                    // 0.040 ms (pstar); 0.192 ms (generic_15 and most).
                    v_sync_nominal_min_ns: 40_000,
                    v_sync_nominal_max_ns: 192_000,
                    // 0.200 us (pstar); 4.700 us (ntsc); 0.040 ms and
                    // 0.640 ms (pstar).
                    h_front_porch_nominal_ns: 200,
                    h_back_porch_nominal_ns: 4_700,
                    v_front_porch_nominal_ns: 40_000,
                    v_back_porch_nominal_ns: 640_000,
                },
            ),
            (
                "25 kHz class",
                Limits {
                    // 18001-19000 touches 19000 Hz, so the class starts at
                    // its own bound; 29000 Hz (d9400, d9800 20501-29000)
                    // clipped below 28 kHz.
                    line_rate_min_mhz: 19_000_000,
                    line_rate_max_mhz: 27_999_999,
                    field_rate_min_mhz: 40_000,
                    field_rate_max_mhz: 80_000,
                    // 288 (d9400, d9800 18001-19000); 400 (arcade_25 and
                    // others).
                    progressive_lines_min: 288,
                    progressive_lines_max: 400,
                    // 768-800 (arcade_25 24960 and pstar 25300-25400).
                    interlaced_lines_min: 768,
                    interlaced_lines_max: 800,
                    // 0.400 us (pstar); 4.688 us (d9400, d9800
                    // 18001-19000).
                    h_sync_nominal_min_ns: 400,
                    h_sync_nominal_max_ns: 4_688,
                    // 0.040 ms (pstar); 0.200 ms (arcade_25).
                    v_sync_nominal_min_ns: 40_000,
                    v_sync_nominal_max_ns: 200_000,
                    // 0.200 us (pstar); 3.200 us (arcade_25); 0.040 ms and
                    // 0.640 ms (pstar).
                    h_front_porch_nominal_ns: 200,
                    h_back_porch_nominal_ns: 3_200,
                    v_front_porch_nominal_ns: 40_000,
                    v_back_porch_nominal_ns: 640_000,
                },
            ),
            (
                "31 kHz class",
                Limits {
                    // 20501-29000 (d9400, d9800) reaches into the class
                    // from below and 30000-70000 (pc_70_120) beyond it:
                    // both clipped.
                    line_rate_min_mhz: 28_000_000,
                    line_rate_max_mhz: 39_999_999,
                    // 40 Hz (d9200, d9400, d9800, m3129); 130 Hz
                    // (pc_31_120, pc_70_120).
                    field_rate_min_mhz: 40_000,
                    field_rate_max_mhz: 130_000,
                    // 192 (pc_70_120 100-130 Hz); 1024 (pc_70_120 50-65 Hz).
                    progressive_lines_min: 192,
                    progressive_lines_max: 1_024,
                    // No range of the class allows interlace.
                    interlaced_lines_min: 0,
                    interlaced_lines_max: 0,
                    // 0.275 us (pc_70_120); 3.813 us (d9200, d9400, d9800,
                    // m3129).
                    h_sync_nominal_min_ns: 275,
                    h_sync_nominal_max_ns: 3_813,
                    // 0.032 ms (pc_70_120); 0.164 ms (d9400, d9800
                    // 20501-29000).
                    v_sync_nominal_min_ns: 32_000,
                    v_sync_nominal_max_ns: 164_000,
                    // 0.170 us (pstar); 1.750 us (ms2930, r666b); 0.020 ms
                    // (d9200, d9400, d9800); 0.436 ms (pc_31_120).
                    h_front_porch_nominal_ns: 170,
                    h_back_porch_nominal_ns: 1_750,
                    v_front_porch_nominal_ns: 20_000,
                    v_back_porch_nominal_ns: 436_000,
                },
            ),
        ];
        assert_eq!(RAILS.len(), expected.len());
        for (rails, (class, limits)) in RAILS.iter().zip(expected) {
            assert_eq!(rails.class, class);
            assert_eq!(rails.limits, limits, "{class}");
            for check in Check::ALL {
                assert_eq!(
                    rails.names[check.index()],
                    format!("{class} {}", check.label())
                );
            }
        }
    }

    #[test]
    fn classes_split_at_19_28_and_40_khz() {
        let class = |mhz| rails_for(mhz).map(|r| r.class);
        assert_eq!(class(0), Some("15 kHz class"));
        assert_eq!(class(18_999_999), Some("15 kHz class"));
        assert_eq!(class(19_000_000), Some("25 kHz class"));
        assert_eq!(class(27_999_999), Some("25 kHz class"));
        assert_eq!(class(28_000_000), Some("31 kHz class"));
        assert_eq!(class(39_999_999), Some("31 kHz class"));
        assert_eq!(class(40_000_000), None);
        assert_eq!(class(u64::MAX), None);
    }

    /// Inside every rail of the 15 kHz class, with 64 us lines.
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

    fn lines(n: u64) -> u16 {
        u16::try_from(n).unwrap()
    }

    type Set = fn(&mut Measure, u64);

    /// Each rail of the 15 kHz class with its limit for 64 us lines.
    fn cases() -> [(Check, u64, Set); 16] {
        [
            (Check::LineRateMin, 15_250_000, |m, v| m.line_rate_mhz = v),
            (Check::LineRateMax, 18_999_999, |m, v| m.line_rate_mhz = v),
            (Check::FieldRateMin, 40_000, |m, v| m.field_rate_mhz = v),
            (Check::FieldRateMax, 80_000, |m, v| m.field_rate_mhz = v),
            (Check::ProgressiveLinesMin, 192, |m, v| m.lines = lines(v)),
            (Check::ProgressiveLinesMax, 320, |m, v| m.lines = lines(v)),
            (Check::InterlacedLinesMin, 448, |m, v| {
                m.interlace = true;
                m.lines = lines(v);
            }),
            (Check::InterlacedLinesMax, 576, |m, v| {
                m.interlace = true;
                m.lines = lines(v);
            }),
            // 90 % of 400 ns; 150 % of 4750 ns.
            (Check::HorizontalSyncMin, 360, |m, v| m.h_sync = Span::ns(v)),
            (Check::HorizontalSyncMax, 7_125, |m, v| {
                m.h_sync = Span::ns(v);
            }),
            // 40 us is shorter than a line, so the minimum is 1 ns; 150 %
            // of 192 us.
            (Check::VerticalSyncMin, 1, |m, v| m.v_sync = Span::ns(v)),
            (Check::VerticalSyncMax, 288_000, |m, v| {
                m.v_sync = Span::ns(v);
            }),
            // 90 % of 200 ns and of 4700 ns.
            (Check::HorizontalFrontPorchMin, 180, |m, v| {
                m.h_front_porch = Span::ns(v);
            }),
            (Check::HorizontalBackPorchMin, 4_230, |m, v| {
                m.h_back_porch = Span::ns(v);
            }),
            // 40 us is shorter than a line; 640 us less one line.
            (Check::VerticalFrontPorchMin, 1, |m, v| {
                m.v_front_porch = Span::ns(v);
            }),
            (Check::VerticalBackPorchMin, 576_000, |m, v| {
                m.v_back_porch = Span::ns(v);
            }),
        ]
    }

    fn at(set: Set, value: u64) -> Result<(), Rejection> {
        let mut m = measure();
        set(&mut m, value);
        check_rails(&m)
    }

    #[test]
    fn every_rail_passes_at_its_limit_and_fails_one_unit_beyond() {
        assert_eq!(check_rails(&measure()), Ok(()));
        for (check, limit, set) in cases() {
            let (inside, outside) = if check.is_max() {
                (limit - 1, limit + 1)
            } else {
                (limit + 1, limit - 1)
            };
            assert_eq!(at(set, limit), Ok(()), "{check:?} at the limit");
            assert_eq!(at(set, inside), Ok(()), "{check:?} one unit inside");
            if check == Check::LineRateMax {
                // One unit above is the next class, with its own rails.
                continue;
            }
            let rail = RAILS[0].names[check.index()];
            assert_eq!(
                at(set, outside),
                Err(Rejection::HardRail {
                    rail,
                    value: outside,
                    limit
                }),
                "{check:?} one unit outside"
            );
        }
    }

    #[test]
    fn the_next_class_judges_a_line_rate_above_the_class() {
        let next = Measure {
            line_rate_mhz: 19_000_000,
            ..measure()
        };
        assert_eq!(
            check_rails(&next),
            Err(Rejection::HardRail {
                rail: "25 kHz class progressive lines min",
                value: 240,
                limit: 288
            })
        );
    }

    #[test]
    fn the_rail_names_carry_the_class_and_the_unit() {
        let fast = Measure {
            field_rate_mhz: 130_001,
            line_rate_mhz: 31_000_000,
            lines: 480,
            ..measure()
        };
        assert_eq!(
            check_rails(&fast),
            Err(Rejection::HardRail {
                rail: "31 kHz class field rate max mHz",
                value: 130_001,
                limit: 130_000
            })
        );
        let interlaced = Measure {
            line_rate_mhz: 31_000_000,
            interlace: true,
            lines: 480,
            ..measure()
        };
        assert_eq!(
            check_rails(&interlaced),
            Err(Rejection::HardRail {
                rail: "31 kHz class interlaced lines max",
                value: 480,
                limit: 0
            })
        );
        let tall = Measure {
            line_rate_mhz: 24_000_000,
            lines: 401,
            ..measure()
        };
        assert_eq!(
            check_rails(&tall),
            Err(Rejection::HardRail {
                rail: "25 kHz class progressive lines max",
                value: 401,
                limit: 400
            })
        );
    }

    #[test]
    fn no_rail_applies_from_40_khz() {
        let edid = Measure {
            line_rate_mhz: 40_000_000,
            field_rate_mhz: 200_000,
            lines: 2_000,
            ..measure()
        };
        assert_eq!(check_rails(&edid), Ok(()));
        let below = Measure {
            line_rate_mhz: 39_999_999,
            ..edid
        };
        assert!(check_rails(&below).is_err());
    }

    fn presets() -> Vec<(String, CrtRange)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors");
        let mut all = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "toml") {
                continue;
            }
            let profile: MonitorProfile =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let MonitorKind::Crt { ranges } = profile.kind else {
                panic!("{} is not a CRT", profile.id);
            };
            all.extend(ranges.into_iter().map(|r| (profile.id.clone(), r)));
        }
        all
    }

    /// Every shipped range lies inside the rails of each class it touches,
    /// so a mode that a shipped preset accepts never hits a rail. The
    /// tolerances grow with the nominal time, so nominal times inside the
    /// rails give limits inside the rails for any line time.
    #[test]
    fn every_preset_lies_inside_its_rails() {
        let ranges = presets();
        assert_eq!(ranges.len(), 54);
        for (id, r) in &ranges {
            let mut classes = 0;
            for ((name, low, high), rails) in CLASSES.iter().zip(RAILS.iter()) {
                if r.h_min_mhz > *high || r.h_max_mhz < *low {
                    continue;
                }
                classes += 1;
                let at = format!("{id} in the {name}");
                let l = &rails.limits;
                assert!(l.line_rate_min_mhz <= r.h_min_mhz.max(*low), "{at}");
                assert!(r.h_max_mhz.min(*high) <= l.line_rate_max_mhz, "{at}");
                assert!(l.field_rate_min_mhz <= r.v_min_mhz, "{at}");
                assert!(r.v_max_mhz <= l.field_rate_max_mhz, "{at}");
                assert!(l.progressive_lines_min <= r.progressive_lines_min, "{at}");
                assert!(r.progressive_lines_max <= l.progressive_lines_max, "{at}");
                if r.interlaced_lines_max > 0 {
                    assert!(l.interlaced_lines_min <= r.interlaced_lines_min, "{at}");
                    assert!(r.interlaced_lines_max <= l.interlaced_lines_max, "{at}");
                }
                let h_sync = u64::from(r.h_sync_ns);
                let v_sync = u64::from(r.v_sync_ns);
                assert!(l.h_sync_nominal_min_ns <= h_sync, "{at}");
                assert!(h_sync <= l.h_sync_nominal_max_ns, "{at}");
                assert!(l.v_sync_nominal_min_ns <= v_sync, "{at}");
                assert!(v_sync <= l.v_sync_nominal_max_ns, "{at}");
                assert!(
                    l.h_front_porch_nominal_ns <= u64::from(r.h_front_porch_ns),
                    "{at}"
                );
                assert!(
                    l.h_back_porch_nominal_ns <= u64::from(r.h_back_porch_ns),
                    "{at}"
                );
                assert!(
                    l.v_front_porch_nominal_ns <= u64::from(r.v_front_porch_ns),
                    "{at}"
                );
                assert!(
                    l.v_back_porch_nominal_ns <= u64::from(r.v_back_porch_ns),
                    "{at}"
                );
            }
            assert!(classes > 0 || r.h_min_mhz >= 40_000_000, "{id}: {r:?}");
        }
    }

    /// A fixed range accepts a line rate up to one field rate below its
    /// minimum. For every shipped range that lowest line rate stays in the
    /// class of the range minimum and inside its rail, so the rails need no
    /// widening for it.
    #[test]
    fn the_lowest_accepted_line_rate_stays_inside_the_rails() {
        for (id, r) in presets() {
            let lowest = line_rate_min_mhz(r.h_min_mhz, r.h_max_mhz, r.v_max_mhz);
            let Some(class) = rails_for(r.h_min_mhz) else {
                assert!(lowest >= EDID_MIN_LINE_RATE_MHZ, "{id}: {r:?}");
                continue;
            };
            assert_eq!(
                rails_for(lowest).map(|c| c.class),
                Some(class.class),
                "{id}: {r:?}"
            );
            assert!(lowest >= class.limits.line_rate_min_mhz, "{id}: {r:?}");
        }
    }

    #[test]
    fn tolerances_grow_with_the_nominal_time() {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            for line in [0, 1, 13_000, 64_000, 1_000_000] {
                let mut last = (0, 0);
                for nominal in (0..2_000_000).step_by(997) {
                    let now = (
                        min_ns(axis, nominal, line),
                        sync_max_ns(axis, nominal, line),
                    );
                    assert!(
                        now.0 >= last.0 && now.1 >= last.1,
                        "{axis:?} {nominal} {line}"
                    );
                    last = now;
                }
            }
        }
    }
}
