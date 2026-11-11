//! How far a mode can be from the values of a range: the lowest line rate,
//! and the porch and sync times. The range checks and the hard rails use the
//! same porch and sync functions.

use crate::Axis;

/// Share of a nominal porch or sync time that a horizontal value must
/// reach, in percent. Switchres fits the horizontal values to whole
/// characters and accepts the same share.
const NOMINAL_MIN_PERCENT: u64 = 90;

/// Longest sync pulse, in percent of the nominal time. Switchres output
/// stays below 130 % of the nominal time.
const SYNC_MAX_PERCENT: u64 = 150;

const PERCENT: u64 = 100;

/// Lowest line rate, in millihertz, that a range with line rates
/// `h_min_mhz` to `h_max_mhz` accepts for a mode with the field rate
/// `field_rate_mhz`.
///
/// Switchres picks the lines per field (`total_lines_for_yres` in
/// `modeline.cpp`): it adds one line while the line rate is below `h_min`
/// and one more line keeps the line rate below `h_max`. The line rate is
/// the field rate times the lines, so the loop ends with a line rate of at
/// least `h_min`, or of at least `h_max - field_rate` when the next line
/// would reach `h_max`. Switchres does not check the result against `h_min`
/// again. The lowest line rate it can give is therefore the lower of
/// `h_min` and `h_max - field_rate`. A fixed range (`h_min == h_max`) thus
/// accepts up to one field rate below `h_min`; a range wider than one field
/// rate accepts nothing below `h_min`.
pub(crate) fn line_rate_min_mhz(h_min_mhz: u64, h_max_mhz: u64, field_rate_mhz: u64) -> u64 {
    h_min_mhz.min(h_max_mhz.saturating_sub(field_rate_mhz))
}

/// Shortest porch or sync pulse, in nanoseconds, for a nominal time.
///
/// A horizontal value must reach 90 % of the nominal time. A vertical
/// value can only change in steps of one line, and Switchres rounds it to
/// the nearest line, so it may also be one line short of the nominal time.
/// The result is never 0: a porch or a pulse is never empty.
pub(crate) fn min_ns(axis: Axis, nominal_ns: u64, line_ns: u64) -> u64 {
    let share = nominal_ns
        .saturating_mul(NOMINAL_MIN_PERCENT)
        .div_ceil(PERCENT);
    let min = if axis == Axis::Vertical {
        share.min(nominal_ns.saturating_sub(line_ns))
    } else {
        share
    };
    min.max(1)
}

/// Longest sync pulse, in nanoseconds, for a nominal time: 150 % of the
/// nominal time, and for a vertical pulse at least one line more than the
/// nominal time.
pub(crate) fn sync_max_ns(axis: Axis, nominal_ns: u64, line_ns: u64) -> u64 {
    let share = nominal_ns.saturating_mul(SYNC_MAX_PERCENT) / PERCENT;
    if axis == Axis::Vertical {
        share.max(nominal_ns.saturating_add(line_ns))
    } else {
        share
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_min_is_ninety_percent_rounded_up() {
        assert_eq!(min_ns(Axis::Horizontal, 4_700, 64_000), 4_230);
        // 671 * 0.9 = 603.9
        assert_eq!(min_ns(Axis::Horizontal, 671, 31_000), 604);
        assert_eq!(min_ns(Axis::Horizontal, 1, 0), 1);
        assert_eq!(min_ns(Axis::Horizontal, 0, 0), 1);
    }

    #[test]
    fn vertical_min_allows_one_line_or_ten_percent() {
        // 192 us with 64 us lines: one line short is less than 90 %.
        assert_eq!(min_ns(Axis::Vertical, 192_000, 64_000), 128_000);
        // 1024 us with 64 us lines: 90 % is less than one line short.
        assert_eq!(min_ns(Axis::Vertical, 1_024_000, 64_000), 921_600);
        // A porch shorter than a line needs one nanosecond.
        assert_eq!(min_ns(Axis::Vertical, 64_000, 64_000), 1);
        assert_eq!(min_ns(Axis::Vertical, 40_000, 64_000), 1);
    }

    #[test]
    fn sync_max() {
        assert_eq!(sync_max_ns(Axis::Horizontal, 4_700, 64_000), 7_050);
        assert_eq!(sync_max_ns(Axis::Vertical, 192_000, 64_000), 288_000);
        // 1.5 * 100 us is less than 100 us and one 64 us line.
        assert_eq!(sync_max_ns(Axis::Vertical, 100_000, 64_000), 164_000);
        // A short vertical pulse may be one line longer than nominal.
        assert_eq!(sync_max_ns(Axis::Vertical, 32_000, 40_000), 72_000);
    }

    #[test]
    fn line_rate_min_follows_switchres() {
        // pal: one line rate, 15625 Hz, and a 50 Hz mode.
        assert_eq!(
            line_rate_min_mhz(15_625_000, 15_625_000, 50_000),
            15_575_000
        );
        // ntsc: one line rate, 15734.26 Hz, and a 59.94 Hz mode.
        assert_eq!(
            line_rate_min_mhz(15_734_260, 15_734_260, 59_940),
            15_674_320
        );
        // A range wider than the field rate keeps its minimum.
        assert_eq!(
            line_rate_min_mhz(15_625_000, 16_200_000, 60_000),
            15_625_000
        );
        // A range narrower than the field rate: h_max - field_rate.
        assert_eq!(
            line_rate_min_mhz(15_625_000, 15_650_000, 60_000),
            15_590_000
        );
        // Exactly one field rate wide: both bounds meet.
        assert_eq!(
            line_rate_min_mhz(15_625_000, 15_685_000, 60_000),
            15_625_000
        );
        assert_eq!(line_rate_min_mhz(0, 10, 60_000), 0);
    }

    #[test]
    fn large_values_saturate() {
        assert_eq!(
            min_ns(Axis::Horizontal, u64::MAX, 0),
            u64::MAX.div_ceil(PERCENT)
        );
        assert_eq!(
            sync_max_ns(Axis::Horizontal, u64::MAX, 0),
            u64::MAX / PERCENT
        );
        assert_eq!(sync_max_ns(Axis::Vertical, u64::MAX, u64::MAX), u64::MAX);
    }
}
