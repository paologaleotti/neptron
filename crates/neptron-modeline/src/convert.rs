//! The one place where integer contract values become the floating-point
//! values of Switchres, and back.
//!
//! Into Switchres: each integer becomes an `f64` exactly (every value of a
//! valid profile is far below 2^53), and one IEEE division by a power of ten
//! then gives the unit of `switchres.ini`, rounded to the nearest `f64`. A
//! correctly rounded parser gives the same `f64` for the decimal text of a
//! Switchres preset, so a profile from `data/monitors` reaches Switchres bit
//! for bit as the preset text does.
//!
//! Out of Switchres: the timings are already integers; each is checked
//! against the width of its contract field. The pixel clock is whole hertz,
//! because Switchres truncates it toward zero. A score is multiplied by
//! 1,000 and rounded to the nearest integer, a half away from zero. A
//! blanking time in an error is rounded to the nearest nanosecond, a half
//! away from zero.

use neptron_switchres_sys::{self as sys, BlankingFault};
use neptron_types::{CrtRange, GpuCaps, Modeline, Rotation, SourceGeometry};

use crate::{Candidate, ConvertError, Settings};

const PER_KILO: f64 = 1_000.0;
const PER_MEGA: f64 = 1_000_000.0;

/// A profile range in the units of a Switchres `crt_range` line.
pub(crate) fn range(range: &CrtRange) -> sys::Range {
    sys::Range {
        hfreq_min_hz: millihertz_to_hertz(range.h_min_mhz),
        hfreq_max_hz: millihertz_to_hertz(range.h_max_mhz),
        vfreq_min_hz: millihertz_to_hertz(range.v_min_mhz),
        vfreq_max_hz: millihertz_to_hertz(range.v_max_mhz),
        hfront_porch_us: nanoseconds_to_microseconds(range.h_front_porch_ns),
        hsync_pulse_us: nanoseconds_to_microseconds(range.h_sync_ns),
        hback_porch_us: nanoseconds_to_microseconds(range.h_back_porch_ns),
        vfront_porch_ms: nanoseconds_to_milliseconds(range.v_front_porch_ns),
        vsync_pulse_ms: nanoseconds_to_milliseconds(range.v_sync_ns),
        vback_porch_ms: nanoseconds_to_milliseconds(range.v_back_porch_ns),
        hsync_positive: range.hsync_positive,
        vsync_positive: range.vsync_positive,
        progressive_lines_min: range.progressive_lines_min,
        progressive_lines_max: range.progressive_lines_max,
        interlaced_lines_min: range.interlaced_lines_min,
        interlaced_lines_max: range.interlaced_lines_max,
    }
}

/// The arguments of `get_mode` for a source.
///
/// A rotation of 90 or 270 degrees is the "rotated" flag of Switchres. The
/// aspect of the source is not passed: Switchres takes every source as 4:3.
pub(crate) fn request(source: &SourceGeometry) -> sys::Request {
    sys::Request {
        refresh_hz: f64::from(source.refresh_mhz) / PER_KILO,
        width: source.width,
        height: source.height,
        interlaced: source.interlaced,
        rotated: matches!(source.rotation, Rotation::R90 | Rotation::R270),
    }
}

/// The generator settings for the settings and the GPU.
///
/// The GPU gives the pixel clock floor and whether interlace is allowed.
/// The fixed values are the defaults of `switchres_manager`; the shim keeps
/// the vertical shift correction off, as its default is.
pub(crate) fn settings(settings: &Settings, caps: &GpuCaps) -> sys::Settings {
    sys::Settings {
        pclock_min_hz: caps.pclock_min_hz,
        monitor_aspect: f64::from(settings.monitor_aspect.width)
            / f64::from(settings.monitor_aspect.height),
        refresh_tolerance_hz: f64::from(settings.refresh_tolerance_mhz) / PER_KILO,
        interlace: caps.interlace,
        doublescan: settings.doublescan,
        super_width: settings.super_width,
        pixel_precision: true,
        interlace_force_even: false,
        scale_proportional: true,
    }
}

/// A Switchres mode as a candidate with integer fields.
pub(crate) fn candidate(mode: &sys::Mode) -> Result<Candidate, ConvertError> {
    let modeline = Modeline {
        pclock_hz: pixel_clock(mode.pclock_hz)?,
        hactive: timing("hactive", mode.hactive)?,
        hsync_start: timing("hsync_start", mode.hbegin)?,
        hsync_end: timing("hsync_end", mode.hend)?,
        htotal: timing("htotal", mode.htotal)?,
        vactive: timing("vactive", mode.vactive)?,
        vsync_start: timing("vsync_start", mode.vbegin)?,
        vsync_end: timing("vsync_end", mode.vend)?,
        vtotal: timing("vtotal", mode.vtotal)?,
        interlace: mode.interlace != 0,
        doublescan: mode.doublescan != 0,
        hsync_positive: mode.hsync_positive != 0,
        vsync_positive: mode.vsync_positive != 0,
    };
    Ok(Candidate {
        modeline,
        range_index: usize::try_from(mode.range_index).map_err(|_| ConvertError::Timing {
            field: "range_index",
            value: i64::from(mode.range_index),
            max: sys::MAX_RANGES as u64,
        })?,
        stretched: mode.weight & sys::WEIGHT_STRETCHED != 0,
        refresh_off: mode.weight & sys::WEIGHT_REFRESH_OFF != 0,
        x_scale_milli: scale_milli("x_scale_milli", mode.x_scale)?,
        y_scale_milli: scale_milli("y_scale_milli", mode.y_scale)?,
        refresh_diff_mhz: hertz_to_millihertz("refresh_diff_mhz", mode.v_diff_hz)?,
    })
}

/// A blanking time of a refused range in nanoseconds: Switchres gives the
/// horizontal one in microseconds and the vertical one in milliseconds. A
/// value that is not finite or does not fit becomes 0 or `u64::MAX`; the
/// error only reports it.
pub(crate) fn blanking_ns(fault: BlankingFault, value: f64) -> u64 {
    let per_unit = match fault {
        BlankingFault::HorizontalTooLong => PER_KILO,
        BlankingFault::VerticalTooLong | BlankingFault::VerticalTooShort => PER_MEGA,
    };
    // `as` saturates: NaN gives 0, and a value out of range gives a bound.
    (value * per_unit).round() as u64
}

fn millihertz_to_hertz(value: u64) -> f64 {
    // Each value below 2^53 converts without loss, so the division is the
    // only rounding. A larger value is far outside every range that
    // Switchres accepts.
    value as f64 / PER_KILO
}

fn nanoseconds_to_microseconds(value: u32) -> f64 {
    f64::from(value) / PER_KILO
}

fn nanoseconds_to_milliseconds(value: u32) -> f64 {
    f64::from(value) / PER_MEGA
}

fn pixel_clock(hz: u64) -> Result<u32, ConvertError> {
    u32::try_from(hz).map_err(|_| ConvertError::Timing {
        field: "pclock_hz",
        value: i64::try_from(hz).unwrap_or(i64::MAX),
        max: u64::from(u32::MAX),
    })
}

fn timing(field: &'static str, value: i32) -> Result<u16, ConvertError> {
    u16::try_from(value).map_err(|_| ConvertError::Timing {
        field,
        value: i64::from(value),
        max: u64::from(u16::MAX),
    })
}

fn scale_milli(field: &'static str, value: f64) -> Result<u32, ConvertError> {
    let milli = (value * PER_KILO).round();
    if milli.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&milli) {
        // In range and integral after the check above, so the cast is exact.
        Ok(milli as u32)
    } else {
        Err(ConvertError::Score { field, value })
    }
}

fn hertz_to_millihertz(field: &'static str, value: f64) -> Result<i64, ConvertError> {
    // 2^53: every integer up to it is exact in both f64 and i64.
    const LIMIT: f64 = 9_007_199_254_740_992.0;
    let milli = (value * PER_KILO).round();
    if milli.is_finite() && (-LIMIT..=LIMIT).contains(&milli) {
        // In range and integral after the check above, so the cast is exact.
        Ok(milli as i64)
    } else {
        Err(ConvertError::Score { field, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Aspect;

    fn generic_15() -> CrtRange {
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

    fn parse(text: &str) -> f64 {
        text.parse().unwrap()
    }

    #[test]
    fn a_range_gets_the_doubles_of_the_preset_text() {
        let r = range(&generic_15());
        // Bit for bit the value that the decimal text of the preset parses to.
        assert_eq!(r.hfreq_min_hz.to_bits(), parse("15625").to_bits());
        assert_eq!(r.vfreq_min_hz.to_bits(), parse("49.50").to_bits());
        assert_eq!(r.hsync_pulse_us.to_bits(), parse("4.700").to_bits());
        assert_eq!(r.vfront_porch_ms.to_bits(), parse("0.064").to_bits());
        assert_eq!(r.vsync_pulse_ms.to_bits(), parse("0.192").to_bits());
        assert_eq!(r.vback_porch_ms.to_bits(), parse("1.024").to_bits());
        assert_eq!((r.hsync_positive, r.vsync_positive), (false, false));
        assert_eq!(
            (
                r.progressive_lines_min,
                r.progressive_lines_max,
                r.interlaced_lines_min,
                r.interlaced_lines_max
            ),
            (192, 288, 448, 576)
        );
    }

    #[test]
    fn each_sync_polarity_converts_on_its_own() {
        let mixed = CrtRange {
            hsync_positive: true,
            ..generic_15()
        };
        let r = range(&mixed);
        assert_eq!((r.hsync_positive, r.vsync_positive), (true, false));
        let mixed = CrtRange {
            vsync_positive: true,
            ..generic_15()
        };
        let r = range(&mixed);
        assert_eq!((r.hsync_positive, r.vsync_positive), (false, true));
    }

    #[test]
    fn a_fractional_line_rate_keeps_its_decimals() {
        let ntsc = CrtRange {
            h_min_mhz: 15_734_260,
            ..generic_15()
        };
        assert_eq!(
            range(&ntsc).hfreq_min_hz.to_bits(),
            parse("15734.26").to_bits()
        );
    }

    #[test]
    fn a_request_carries_size_refresh_and_flags() {
        let source = SourceGeometry {
            width: 288,
            height: 224,
            refresh_mhz: 60_606,
            aspect_milli: 1_333,
            interlaced: true,
            rotation: Rotation::R90,
        };
        let r = request(&source);
        assert_eq!(
            (r.width, r.height, r.interlaced, r.rotated),
            (288, 224, true, true)
        );
        assert_eq!(r.refresh_hz.to_bits(), parse("60.606").to_bits());
        for (rotation, rotated) in [
            (Rotation::R0, false),
            (Rotation::R90, true),
            (Rotation::R180, false),
            (Rotation::R270, true),
        ] {
            assert_eq!(
                request(&SourceGeometry { rotation, ..source }).rotated,
                rotated
            );
        }
    }

    #[test]
    fn settings_take_the_floor_and_interlace_from_the_gpu() {
        let caps = GpuCaps {
            pclock_min_hz: 25_000_000,
            pclock_max_hz: 400_000_000,
            interlace: false,
            flip_per_field: false,
            max_width: 4_096,
            max_height: 4_096,
        };
        let s = settings(&Settings::default(), &caps);
        assert_eq!(s.pclock_min_hz, 25_000_000);
        assert!(!s.interlace);
        assert!(!s.doublescan);
        assert_eq!(s.super_width, 2_560);
        assert_eq!(s.refresh_tolerance_hz.to_bits(), 2.0f64.to_bits());
        // STANDARD_CRT_ASPECT of Switchres is the expression 4.0/3.0.
        assert_eq!(s.monitor_aspect.to_bits(), (4.0f64 / 3.0).to_bits());
        let wide = Settings {
            monitor_aspect: Aspect {
                width: 16,
                height: 9,
            },
            doublescan: true,
            ..Settings::default()
        };
        let s = settings(
            &wide,
            &GpuCaps {
                interlace: true,
                ..caps
            },
        );
        assert_eq!(s.monitor_aspect.to_bits(), (16.0f64 / 9.0).to_bits());
        assert_eq!((s.interlace, s.doublescan), (true, true));
    }

    fn mode() -> sys::Mode {
        sys::Mode {
            pclock_hz: 6_514_560,
            x_scale: 1.0,
            y_scale: 1.0,
            v_scale: 1.0,
            x_diff: 0.0,
            y_diff: 0.0,
            v_diff_hz: 0.0,
            hactive: 320,
            hbegin: 333,
            hend: 364,
            htotal: 416,
            vactive: 240,
            vbegin: 242,
            vend: 245,
            vtotal: 261,
            interlace: 0,
            doublescan: 0,
            hsync_positive: 0,
            vsync_positive: 1,
            weight: 0,
            range_index: 2,
        }
    }

    #[test]
    fn a_blanking_time_converts_to_nanoseconds() {
        assert_eq!(blanking_ns(BlankingFault::HorizontalTooLong, 14.7), 14_700);
        assert_eq!(
            blanking_ns(BlankingFault::HorizontalTooLong, 10.714_285_7),
            10_714
        );
        assert_eq!(
            blanking_ns(BlankingFault::VerticalTooLong, 7.696),
            7_696_000
        );
        assert_eq!(blanking_ns(BlankingFault::VerticalTooShort, 0.000_002), 2);
        assert_eq!(blanking_ns(BlankingFault::VerticalTooShort, f64::NAN), 0);
        assert_eq!(
            blanking_ns(BlankingFault::HorizontalTooLong, f64::INFINITY),
            u64::MAX
        );
    }

    #[test]
    fn a_mode_becomes_a_candidate() {
        let c = candidate(&mode()).unwrap();
        assert_eq!(
            c.modeline,
            Modeline {
                pclock_hz: 6_514_560,
                hactive: 320,
                hsync_start: 333,
                hsync_end: 364,
                htotal: 416,
                vactive: 240,
                vsync_start: 242,
                vsync_end: 245,
                vtotal: 261,
                interlace: false,
                doublescan: false,
                hsync_positive: false,
                vsync_positive: true,
            }
        );
        assert_eq!(c.range_index, 2);
        assert!(!c.stretched && !c.refresh_off);
        assert_eq!(
            (c.x_scale_milli, c.y_scale_milli, c.refresh_diff_mhz),
            (1_000, 1_000, 0)
        );
    }

    #[test]
    fn flags_and_scores_convert() {
        let c = candidate(&sys::Mode {
            interlace: 1,
            doublescan: 1,
            hsync_positive: 1,
            weight: sys::WEIGHT_STRETCHED | sys::WEIGHT_REFRESH_OFF,
            x_scale: 2.0625,
            y_scale: 1.0004,
            v_diff_hz: -0.0625,
            ..mode()
        })
        .unwrap();
        assert!(c.modeline.interlace && c.modeline.doublescan && c.modeline.hsync_positive);
        assert!(c.stretched && c.refresh_off);
        // 2,062.5 and -62.5 are exact in binary: a half rounds away from zero.
        assert_eq!(c.x_scale_milli, 2_063);
        assert_eq!(c.y_scale_milli, 1_000);
        assert_eq!(c.refresh_diff_mhz, -63);
    }

    #[test]
    fn a_timing_outside_its_field_is_an_error() {
        assert_eq!(
            candidate(&sys::Mode {
                htotal: 70_000,
                ..mode()
            }),
            Err(ConvertError::Timing {
                field: "htotal",
                value: 70_000,
                max: 65_535
            })
        );
        assert_eq!(
            candidate(&sys::Mode {
                vbegin: -1,
                ..mode()
            }),
            Err(ConvertError::Timing {
                field: "vsync_start",
                value: -1,
                max: 65_535
            })
        );
        assert_eq!(
            candidate(&sys::Mode {
                pclock_hz: 5_000_000_000,
                ..mode()
            }),
            Err(ConvertError::Timing {
                field: "pclock_hz",
                value: 5_000_000_000,
                max: u64::from(u32::MAX)
            })
        );
        assert!(
            candidate(&sys::Mode {
                range_index: -1,
                ..mode()
            })
            .is_err()
        );
    }

    #[test]
    fn a_score_that_does_not_fit_is_an_error() {
        for (bad, field) in [
            (
                sys::Mode {
                    x_scale: f64::NAN,
                    ..mode()
                },
                "x_scale_milli",
            ),
            (
                sys::Mode {
                    y_scale: -1.0,
                    ..mode()
                },
                "y_scale_milli",
            ),
            (
                sys::Mode {
                    v_diff_hz: f64::INFINITY,
                    ..mode()
                },
                "refresh_diff_mhz",
            ),
        ] {
            match candidate(&bad) {
                Err(ConvertError::Score { field: f, .. }) => assert_eq!(f, field),
                other => panic!("{other:?}"),
            }
        }
    }
}
