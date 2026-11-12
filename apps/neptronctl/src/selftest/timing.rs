//! The flip and vertical blank statistics of one mode, from the flip
//! events of the display. Integers only.

use neptron_types::{FlipEvent, Modeline};

use super::report::TimingReport;

/// The statistics of `flips` in `mode`. The kernel's vblank counter and
/// time stamps give the period, so a flip that the loop misses does not
/// change it.
///
/// An interlaced mode can give one vertical blank per field or per frame;
/// the period is compared with the nearer of the two, and `vblank_unit`
/// says which.
pub fn stats(flips: &[FlipEvent], mode: &Modeline) -> TimingReport {
    let count = flips.len() as u64;
    let mut report = TimingReport {
        flips: count,
        vblanks: 0,
        missed_vblanks: 0,
        flips_per_second_mhz: 0,
        vblank_period_ns: None,
        expected_period_ns: None,
        vblank_unit: None,
        period_error_ppm: None,
        implied_pclock_hz: None,
    };
    let (Some(first), Some(last)) = (flips.first(), flips.last()) else {
        return report;
    };
    let vblanks = last.sequence.saturating_sub(first.sequence);
    let span_ns = last.time_ns.saturating_sub(first.time_ns);
    report.vblanks = vblanks;
    report.missed_vblanks = vblanks.saturating_sub(count - 1);
    if vblanks == 0 || span_ns == 0 {
        return report;
    }
    report.flips_per_second_mhz = div_round(u128::from(count - 1) * NS_TIMES_MHZ, span_ns.into());
    report.vblank_period_ns = Some(div_round(u128::from(span_ns), u128::from(vblanks)));
    let units = units(mode);
    // The nearer unit: the smaller distance between `span` and
    // `vblanks * expected`.
    let Some((unit, pixels_num, pixels_den)) = units.iter().copied().min_by_key(|(_, num, den)| {
        let expected = u128::from(vblanks) * period_ns_times(*num, *den, mode.pclock_hz);
        (u128::from(span_ns) * PERIOD_SCALE).abs_diff(expected)
    }) else {
        return report;
    };
    let expected_scaled = period_ns_times(pixels_num, pixels_den, mode.pclock_hz);
    report.vblank_unit = Some(unit.to_owned());
    report.expected_period_ns = Some(div_round(expected_scaled, PERIOD_SCALE));
    let measured_scaled = u128::from(span_ns) * PERIOD_SCALE;
    let expected_total = expected_scaled * u128::from(vblanks);
    if expected_total > 0 {
        report.period_error_ppm = Some(div_round(
            measured_scaled.abs_diff(expected_total) * PPM,
            expected_total,
        ));
    }
    report.implied_pclock_hz = Some(div_round(
        pixels_num * NS_PER_S * u128::from(vblanks),
        pixels_den * u128::from(span_ns),
    ));
    report
}

/// Nanoseconds per second.
const NS_PER_S: u128 = 1_000_000_000;
/// Millihertz times nanoseconds per second squared: a count per
/// nanosecond times this gives millihertz.
const NS_TIMES_MHZ: u128 = 1_000_000_000_000;
/// Parts per million.
const PPM: u128 = 1_000_000;
/// The expected period is kept in units of 1/`PERIOD_SCALE` ns, so a
/// period that is not a whole nanosecond keeps its precision.
const PERIOD_SCALE: u128 = 1_000_000;

/// The pixels of one vertical blank as a fraction, for each unit a blank
/// can be: (unit, numerator, denominator).
fn units(mode: &Modeline) -> Vec<(&'static str, u128, u128)> {
    let frame =
        u128::from(mode.htotal) * u128::from(mode.vtotal) * if mode.doublescan { 2 } else { 1 };
    if mode.interlace {
        vec![("field", frame, 2), ("frame", frame, 1)]
    } else {
        vec![("field", frame, 1)]
    }
}

/// The time of `num / den` pixels at `pclock_hz`, in 1/`PERIOD_SCALE` ns.
fn period_ns_times(num: u128, den: u128, pclock_hz: u32) -> u128 {
    if pclock_hz == 0 || den == 0 {
        return 0;
    }
    u128::from(div_round(
        num * NS_PER_S * PERIOD_SCALE,
        den * u128::from(pclock_hz),
    ))
}

/// `num / den` rounded to the nearest, a half up; 0 for `den` 0, and
/// `u64::MAX` for a quotient that does not fit.
fn div_round(num: u128, den: u128) -> u64 {
    if den == 0 {
        return 0;
    }
    u64::try_from((num + den / 2) / den).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 640x480 at 60 Hz: 800 x 500 pixels at 24 MHz, a period of
    /// 16 666 667 ns (16.666 666 ms).
    fn vga(interlace: bool) -> Modeline {
        Modeline {
            pclock_hz: 24_000_000,
            hactive: 640,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 500,
            interlace,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    fn flips(times_and_sequences: &[(u64, u64)]) -> Vec<FlipEvent> {
        times_and_sequences
            .iter()
            .map(|&(time_ns, sequence)| FlipEvent { time_ns, sequence })
            .collect()
    }

    #[test]
    fn a_flip_on_every_blank_gives_the_field_period_and_the_clock() {
        let period = 16_666_667;
        let events: Vec<(u64, u64)> = (0..61).map(|i| (1_000 + i * period, 7 + i)).collect();
        let t = stats(&flips(&events), &vga(false));
        assert_eq!(t.flips, 61);
        assert_eq!(t.vblanks, 60);
        assert_eq!(t.missed_vblanks, 0);
        assert_eq!(t.vblank_period_ns, Some(period));
        assert_eq!(t.expected_period_ns, Some(16_666_667));
        assert_eq!(t.vblank_unit.as_deref(), Some("field"));
        assert_eq!(t.period_error_ppm, Some(0));
        assert_eq!(t.implied_pclock_hz, Some(24_000_000));
        assert_eq!(t.flips_per_second_mhz, 60_000);
    }

    #[test]
    fn missed_blanks_do_not_change_the_period() {
        let period = 16_666_667;
        let events = [
            (0, 100),
            (2 * period, 102),
            (5 * period, 105),
            (6 * period, 106),
        ];
        let t = stats(&flips(&events), &vga(false));
        assert_eq!(t.vblanks, 6);
        assert_eq!(t.missed_vblanks, 3);
        assert_eq!(t.vblank_period_ns, Some(period));
        assert_eq!(t.period_error_ppm, Some(0));
        assert_eq!(t.flips_per_second_mhz, 30_000);
    }

    #[test]
    fn a_slower_clock_shows_in_the_error_and_the_implied_clock() {
        // 1 % longer blanks: the hardware makes a 1 % lower clock.
        let period = 16_833_334;
        let events: Vec<(u64, u64)> = (0..11).map(|i| (i * period, i)).collect();
        let t = stats(&flips(&events), &vga(false));
        assert_eq!(t.period_error_ppm, Some(10_000));
        assert_eq!(t.implied_pclock_hz, Some(23_762_375));
    }

    #[test]
    fn an_interlaced_mode_takes_the_nearer_unit() {
        let field = 8_333_333;
        let per_field: Vec<(u64, u64)> = (0..21).map(|i| (i * field, i)).collect();
        let t = stats(&flips(&per_field), &vga(true));
        assert_eq!(t.vblank_unit.as_deref(), Some("field"));
        assert_eq!(t.expected_period_ns, Some(8_333_333));
        assert_eq!(t.implied_pclock_hz, Some(24_000_001));
        let per_frame: Vec<(u64, u64)> = (0..21).map(|i| (i * 2 * field, i)).collect();
        let t = stats(&flips(&per_frame), &vga(true));
        assert_eq!(t.vblank_unit.as_deref(), Some("frame"));
        assert_eq!(t.expected_period_ns, Some(16_666_667));
    }

    #[test]
    fn too_few_flips_give_no_period() {
        let t = stats(&[], &vga(false));
        assert_eq!((t.flips, t.vblank_period_ns), (0, None));
        let t = stats(&flips(&[(5, 1)]), &vga(false));
        assert_eq!((t.flips, t.vblanks, t.vblank_period_ns), (1, 0, None));
        let t = stats(&flips(&[(5, 1), (5, 1)]), &vga(false));
        assert_eq!(t.vblank_period_ns, None, "no blank between the two");
    }
}
