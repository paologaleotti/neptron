//! A CRT monitor profile built from the range limits in an EDID.

use neptron_types::{CrtRange, GpuCaps, MonitorKind, MonitorProfile};
use thiserror::Error;

use crate::edid::{EdidError, RangeLimits, TimingFormula, parse_edid};
use crate::gtf;

/// A monitor profile built from an EDID, with the pixel clock limit that a
/// profile cannot hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdidProfile {
    /// A CRT profile with one range. Its id is `edid_` and the manufacturer
    /// id and the product code, for example `edid_npt_0071`.
    pub profile: MonitorProfile,
    /// The highest pixel clock the monitor accepts, in hertz. Apply it with
    /// [`EdidProfile::limit_caps`].
    pub pclock_max_hz: u32,
    /// The timing formula the EDID names. The profile does not use it.
    pub formula: TimingFormula,
}

/// Why no profile can be built from an EDID.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProfileError {
    /// The EDID does not parse.
    #[error("EDID refused: {0}")]
    Edid(#[from] EdidError),
    /// The display has a digital input, so it is not a CRT on an analog
    /// output.
    #[error("EDID of {id} describes a digital input, not an analog CRT")]
    NotAnalog {
        /// The id the profile would have.
        id: String,
    },
    /// Bit 0 of byte 0x18 is clear: EDID 1.3 does not claim default GTF
    /// support, or EDID 1.4 does not claim continuous frequency. The
    /// display then accepts only its listed timings, and the GTF porches of
    /// the profile have no basis.
    #[error(
        "EDID 1.{revision} of {id} clears bit 0 of byte 0x18 (default GTF in 1.3, continuous frequency in 1.4)"
    )]
    NotContinuous {
        /// The id the profile would have.
        id: String,
        /// The EDID revision: 3 or 4.
        revision: u8,
    },
    /// The EDID has no range limits descriptor.
    #[error("EDID of {id} has no range limits descriptor")]
    NoRangeLimits {
        /// The id the profile would have.
        id: String,
    },
    /// The rates leave no progressive line count after the vertical
    /// blanking.
    #[error("EDID of {id} leaves no line count: minimum {min}, maximum {max}")]
    NoLines {
        /// The id the profile would have.
        id: String,
        /// The fewest active lines the rates allow.
        min: u64,
        /// The most active lines the rates allow.
        max: u64,
    },
}

/// Builds a CRT monitor profile from the range limits in an EDID.
///
/// The profile has one range:
///
/// - The line and field rates are the EDID limits, converted exactly to
///   millihertz.
/// - The porches and syncs come from the default GTF parameters, as the
///   `gtf` module of this crate describes: EDID gives none.
/// - The sync polarities are the GTF ones: horizontal negative, vertical
///   positive.
/// - The progressive line limits are the active lines that the rates leave
///   after the vertical blanking: at most `(1 / v_min - blanking) * h_max`,
///   rounded down, and at least `(1 / v_max - blanking) * h_min`, rounded
///   up.
/// - The range accepts no interlaced mode: EDID range limits do not say
///   that the monitor accepts one.
///
/// # Errors
///
/// Returns a [`ProfileError`] when the EDID does not parse, has a digital
/// input, clears bit 0 of byte 0x18 (default GTF in EDID 1.3, continuous
/// frequency in EDID 1.4), has no range limits
/// descriptor, or leaves no line count.
pub fn profile_from_edid(bytes: &[u8]) -> Result<EdidProfile, ProfileError> {
    let edid = parse_edid(bytes)?;
    let id = format!(
        "edid_{}_{:04x}",
        edid.manufacturer.to_ascii_lowercase(),
        edid.product_code
    );
    if !edid.analog {
        return Err(ProfileError::NotAnalog { id });
    }
    if !edid.continuous_frequency {
        return Err(ProfileError::NotContinuous {
            id,
            revision: edid.revision,
        });
    }
    let Some(limits) = edid.range_limits else {
        return Err(ProfileError::NoRangeLimits { id });
    };
    let range = crt_range(&limits).map_err(|(min, max)| ProfileError::NoLines {
        id: id.clone(),
        min,
        max,
    })?;
    Ok(EdidProfile {
        profile: MonitorProfile {
            id,
            kind: MonitorKind::Crt {
                ranges: vec![range],
            },
        },
        pclock_max_hz: limits.pclock_max_hz,
        formula: limits.formula,
    })
}

impl EdidProfile {
    /// `caps` with its highest pixel clock lowered to the monitor's limit.
    ///
    /// The other values stay. If the GPU's lowest pixel clock is then above
    /// the highest, no mode passes the validator.
    pub fn limit_caps(&self, caps: &GpuCaps) -> GpuCaps {
        GpuCaps {
            pclock_max_hz: caps.pclock_max_hz.min(self.pclock_max_hz),
            ..*caps
        }
    }
}

const MHZ_PER_HZ: u64 = 1_000;
const MHZ_PER_KHZ: u64 = 1_000_000;
/// A time in nanoseconds times a rate in millihertz gives cycles times
/// this value.
const NS_TIMES_MHZ: u128 = 1_000_000_000_000;

/// The range for `limits`, or the line limits when they leave no line
/// count.
fn crt_range(limits: &RangeLimits) -> Result<CrtRange, (u64, u64)> {
    let h_min_mhz = u64::from(limits.h_min_khz) * MHZ_PER_KHZ;
    let h_max_mhz = u64::from(limits.h_max_khz) * MHZ_PER_KHZ;
    let v_min_mhz = u64::from(limits.v_min_hz) * MHZ_PER_HZ;
    let v_max_mhz = u64::from(limits.v_max_hz) * MHZ_PER_HZ;
    let blanking = gtf::blanking(h_min_mhz, h_max_mhz);
    let v_blank_ns = blanking.v_total_ns();
    let lines_max = active_lines(v_min_mhz, h_max_mhz, v_blank_ns, false);
    let lines_min = active_lines(v_max_mhz, h_min_mhz, v_blank_ns, true).max(1);
    if lines_max == 0 || lines_min > lines_max || lines_min > u64::from(u16::MAX) {
        return Err((lines_min, lines_max));
    }
    Ok(CrtRange {
        h_min_mhz,
        h_max_mhz,
        v_min_mhz,
        v_max_mhz,
        h_front_porch_ns: to_u32(blanking.h_front_porch_ns),
        h_sync_ns: to_u32(blanking.h_sync_ns),
        h_back_porch_ns: to_u32(blanking.h_back_porch_ns),
        v_front_porch_ns: to_u32(blanking.v_front_porch_ns),
        v_sync_ns: to_u32(blanking.v_sync_ns),
        v_back_porch_ns: to_u32(blanking.v_back_porch_ns),
        hsync_positive: false,
        vsync_positive: true,
        // Both values are checked against u16::MAX above, and a maximum
        // above it is lowered to it, which only narrows the range.
        progressive_lines_min: u16::try_from(lines_min).unwrap_or(u16::MAX),
        progressive_lines_max: u16::try_from(lines_max).unwrap_or(u16::MAX),
        interlaced_lines_min: 0,
        interlaced_lines_max: 0,
    })
}

/// The active lines of one field at field rate `v_mhz` and line rate
/// `h_mhz`, after `v_blank_ns` of vertical blanking; 0 when the blanking
/// fills the field.
fn active_lines(v_mhz: u64, h_mhz: u64, v_blank_ns: u64, round_up: bool) -> u64 {
    let v = u128::from(v_mhz);
    let blank = u128::from(v_blank_ns) * v;
    let Some(active) = NS_TIMES_MHZ.checked_sub(blank) else {
        return 0;
    };
    let numerator = active * u128::from(h_mhz);
    let denominator = v * NS_TIMES_MHZ;
    let lines = if round_up {
        numerator.div_ceil(denominator)
    } else {
        numerator / denominator
    };
    u64::try_from(lines).unwrap_or(u64::MAX)
}

/// The GTF times are at most 3e9 ns, below u32::MAX; the saturation only
/// guards that bound, and a larger porch is a stricter one.
fn to_u32(ns: u64) -> u32 {
    u32::try_from(ns).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(v: (u16, u16), h: (u16, u16)) -> RangeLimits {
        RangeLimits {
            v_min_hz: v.0,
            v_max_hz: v.1,
            h_min_khz: h.0,
            h_max_khz: h.1,
            pclock_max_hz: 100_000_000,
            formula: TimingFormula::DefaultGtf,
        }
    }

    #[test]
    fn line_limits_of_a_30_to_71_khz_crt() {
        let range = crt_range(&limits((50, 160), (30, 71))).unwrap();
        // (20 ms - 641 081 ns) * 71 kHz = 1 374.48 lines, rounded down.
        assert_eq!(range.progressive_lines_max, 1_374);
        // (6.25 ms - 641 081 ns) * 30 kHz = 168.27 lines, rounded up.
        assert_eq!(range.progressive_lines_min, 169);
    }

    #[test]
    fn blanking_that_fills_the_field_leaves_no_lines() {
        // At 1 kHz the vertical blanking is 1 ms + 3 ms + 0, more than a
        // 255 Hz field (3.9 ms).
        assert_eq!(crt_range(&limits((255, 255), (1, 1))), Err((1, 0)));
    }

    #[test]
    fn line_count_above_u16_is_lowered() {
        let range = crt_range(&limits((1, 255), (255, 255))).unwrap();
        assert_eq!(range.progressive_lines_max, u16::MAX);
    }

    #[test]
    fn active_lines_rounds_in_both_directions() {
        // 1 s at 1 Hz, no blanking, 1.5 lines per second.
        assert_eq!(active_lines(1_000, 1_500, 0, false), 1);
        assert_eq!(active_lines(1_000, 1_500, 0, true), 2);
    }
}
