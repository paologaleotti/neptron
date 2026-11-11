//! Statistics of vblank and page-flip events: measured rate, jitter and
//! missed frames. Pure code; the KMS side collects the samples.

/// One event from the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// Kernel timestamp, in nanoseconds.
    pub time_ns: u64,
    /// The vblank sequence number the kernel reports.
    pub sequence: u32,
}

/// The statistics of a list of samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// How many samples.
    pub count: usize,
    /// Mean time between two vblanks, in nanoseconds: the time span over
    /// the vblanks the kernel counted in it.
    pub mean_interval_ns: u64,
    /// Shortest time between two samples, in nanoseconds.
    pub min_interval_ns: u64,
    /// Longest time between two samples, in nanoseconds.
    pub max_interval_ns: u64,
    /// Largest distance from the mean of one interval divided by the
    /// vblanks it spans, in nanoseconds.
    pub jitter_ns: u64,
    /// The vblank rate, in millihertz: the vblanks the kernel counted over
    /// the time span. A sample that the loop misses does not lower it.
    pub rate_mhz: u64,
    /// Vblanks the kernel counted between two samples beyond the first: a
    /// flip that took two vblanks adds one.
    pub skipped_vblanks: u64,
    /// The rate of the samples themselves, in millihertz. For page flips
    /// it is the flip rate, which tells one flip per field from one per
    /// frame.
    pub event_rate_mhz: u64,
}

/// How often page flips complete in an interlaced mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    /// One flip per field.
    PerField,
    /// One flip per frame (two fields).
    PerFrame,
    /// Neither, within the tolerance.
    Other,
}

/// The statistics of `samples`, in time order. `None` ("not measured")
/// with fewer than two samples, with no time or no vblank between the
/// first and the last, or with more than one vblank per millisecond: a
/// sequence that goes backwards wraps to about four billion vblanks.
pub fn stats(samples: &[Sample]) -> Option<Stats> {
    let first = samples.first()?;
    let last = samples.last()?;
    let intervals = samples.len().checked_sub(1).filter(|&n| n > 0)?;
    let span_ns = last.time_ns.checked_sub(first.time_ns).filter(|&t| t > 0)?;
    let vblanks = u64::from(last.sequence.wrapping_sub(first.sequence));
    if vblanks == 0 || vblanks > span_ns / VBLANK_INTERVAL_MIN_NS {
        return None;
    }
    let mean = span_ns / vblanks;
    let mut min = u64::MAX;
    let mut max = 0;
    let mut jitter = 0;
    let mut skipped = 0_u64;
    for pair in samples.windows(2) {
        let interval = pair[1].time_ns.saturating_sub(pair[0].time_ns);
        min = min.min(interval);
        max = max.max(interval);
        let steps = pair[1].sequence.wrapping_sub(pair[0].sequence);
        if steps > 0 {
            jitter = jitter.max((interval / u64::from(steps)).abs_diff(mean));
        }
        skipped += u64::from(steps.saturating_sub(1));
    }
    Some(Stats {
        count: samples.len(),
        mean_interval_ns: mean,
        min_interval_ns: min,
        max_interval_ns: max,
        jitter_ns: jitter,
        rate_mhz: per_second_mhz(vblanks, span_ns),
        skipped_vblanks: skipped,
        event_rate_mhz: per_second_mhz(u64::try_from(intervals).ok()?, span_ns),
    })
}

/// Compares a measured flip rate with the field rate of the mode.
pub fn cadence(flip_rate_mhz: u64, field_rate_mhz: u64) -> Cadence {
    let near =
        |target: u64| flip_rate_mhz.abs_diff(target) * 1_000 <= target * CADENCE_TOLERANCE_PERMILLE;
    if near(field_rate_mhz) {
        Cadence::PerField
    } else if near(field_rate_mhz / 2) {
        Cadence::PerFrame
    } else {
        Cadence::Other
    }
}

/// The pixel clock that a measured field rate implies for this mode, in
/// hertz. It shows the clock the hardware really runs at, which the
/// kernel's readback does not report. `None` when the product overflows.
pub fn implied_pclock_hz(
    field_rate_mhz: u64,
    htotal: u16,
    vtotal: u16,
    interlace: bool,
) -> Option<u64> {
    let fields_per_frame = if interlace { 2 } else { 1 };
    let product = (u64::from(htotal) * u64::from(vtotal)).checked_mul(field_rate_mhz)?;
    Some(product / (1_000 * fields_per_frame))
}

/// `count` events over `span_ns`, in millihertz, rounded to the nearest.
fn per_second_mhz(count: u64, span_ns: u64) -> u64 {
    let span = u128::from(span_ns);
    let rate = (u128::from(count) * NS_PER_S_MHZ + span / 2) / span;
    u64::try_from(rate).unwrap_or(u64::MAX)
}

impl Cadence {
    /// The name in the report.
    pub fn name(self) -> &'static str {
        match self {
            Cadence::PerField => "per_field",
            Cadence::PerFrame => "per_frame",
            Cadence::Other => "other",
        }
    }
}

/// The tolerance of the cadence match, in thousandths of the rate.
const CADENCE_TOLERANCE_PERMILLE: u64 = 20;
/// No display makes more than one vblank per millisecond (1 kHz).
const VBLANK_INTERVAL_MIN_NS: u64 = 1_000_000;
const NS_PER_S_MHZ: u128 = 1_000_000_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    fn even(count: u32, interval_ns: u64) -> Vec<Sample> {
        (0..count)
            .map(|i| Sample {
                time_ns: 1_000 + u64::from(i) * interval_ns,
                sequence: 100 + i,
            })
            .collect()
    }

    #[test]
    fn even_samples_give_the_rate_and_no_jitter() {
        let s = stats(&even(61, 16_683_350)).unwrap();
        assert_eq!(s.count, 61);
        assert_eq!(s.mean_interval_ns, 16_683_350);
        assert_eq!(s.jitter_ns, 0);
        assert_eq!(s.rate_mhz, 59_940);
        assert_eq!(s.skipped_vblanks, 0);
    }

    #[test]
    fn a_missed_sample_is_a_skipped_vblank_and_keeps_the_rate() {
        let mut samples = even(5, 10_000_000);
        for s in &mut samples[3..] {
            s.time_ns += 10_000_000;
            s.sequence += 1;
        }
        let s = stats(&samples).unwrap();
        assert_eq!(s.min_interval_ns, 10_000_000);
        assert_eq!(s.max_interval_ns, 20_000_000);
        assert_eq!(s.mean_interval_ns, 10_000_000);
        assert_eq!(s.jitter_ns, 0);
        assert_eq!(s.skipped_vblanks, 1);
        assert_eq!(s.rate_mhz, 100_000);
        assert_eq!(s.event_rate_mhz, 80_000);
    }

    #[test]
    fn a_sequence_that_wraps_counts_no_skip() {
        let samples = [
            Sample {
                time_ns: 0,
                sequence: u32::MAX,
            },
            Sample {
                time_ns: 16_666_667,
                sequence: 0,
            },
        ];
        assert_eq!(stats(&samples).unwrap().skipped_vblanks, 0);
    }

    #[test]
    fn a_late_vblank_shows_as_jitter() {
        let times = [0, 10_000_000, 21_000_000, 30_000_000];
        let samples: Vec<Sample> = (0_u32..)
            .zip(times)
            .map(|(i, time_ns)| Sample {
                time_ns,
                sequence: i,
            })
            .collect();
        let s = stats(&samples).unwrap();
        assert_eq!(s.mean_interval_ns, 10_000_000);
        assert_eq!(s.jitter_ns, 1_000_000);
        assert_eq!(s.rate_mhz, s.event_rate_mhz);
    }

    #[test]
    fn a_gap_in_the_sequence_keeps_the_vblank_rate() {
        // One sample every second vblank of a 120 Hz mode.
        let samples: Vec<Sample> = (0..121_u32)
            .map(|i| Sample {
                time_ns: 5_000 + u64::from(i) * 16_666_667,
                sequence: 1_000 + 2 * i,
            })
            .collect();
        let s = stats(&samples).unwrap();
        assert_eq!(s.rate_mhz, 120_000);
        assert_eq!(s.event_rate_mhz, 60_000);
        assert_eq!(s.mean_interval_ns, 8_333_333);
        assert_eq!(s.skipped_vblanks, 120);
        assert!(s.jitter_ns <= 1, "{s:?}");
    }

    #[test]
    fn the_rate_counts_vblanks_across_a_sequence_wrap() {
        let samples = [
            Sample {
                time_ns: 0,
                sequence: u32::MAX - 1,
            },
            Sample {
                time_ns: 30_000_000,
                sequence: 1,
            },
        ];
        let s = stats(&samples).unwrap();
        assert_eq!(s.rate_mhz, 100_000);
        assert_eq!(s.skipped_vblanks, 2);
    }

    #[test]
    fn a_sequence_that_goes_backwards_is_not_measured() {
        let samples = [
            Sample {
                time_ns: 0,
                sequence: 500,
            },
            Sample {
                time_ns: 1_000_000_000,
                sequence: 499,
            },
        ];
        assert_eq!(stats(&samples), None);
    }

    #[test]
    fn more_than_one_vblank_per_millisecond_is_not_measured() {
        let at = |time_ns, sequence| Sample { time_ns, sequence };
        assert_eq!(stats(&[at(0, 0), at(10_000_000, 11)]), None);
        assert!(stats(&[at(0, 0), at(10_000_000, 10)]).is_some());
    }

    #[test]
    fn an_implied_clock_that_overflows_is_not_measured() {
        assert_eq!(implied_pclock_hz(u64::MAX, u16::MAX, u16::MAX, false), None);
    }

    #[test]
    fn no_vblank_between_the_samples_is_not_measured() {
        let samples = [
            Sample {
                time_ns: 0,
                sequence: 7,
            },
            Sample {
                time_ns: 10,
                sequence: 7,
            },
        ];
        assert_eq!(stats(&samples), None);
    }

    #[test]
    fn too_few_samples_give_nothing() {
        assert_eq!(stats(&[]), None);
        assert_eq!(stats(&even(1, 10)), None);
        assert_eq!(stats(&even(3, 0)), None);
    }

    #[test]
    fn cadence_tells_field_from_frame() {
        assert_eq!(cadence(59_900, 59_940), Cadence::PerField);
        assert_eq!(cadence(29_970, 59_940), Cadence::PerFrame);
        assert_eq!(cadence(45_000, 59_940), Cadence::Other);
        assert_eq!(cadence(0, 59_940), Cadence::Other);
    }

    #[test]
    fn implied_clock_follows_the_totals() {
        assert_eq!(implied_pclock_hz(59_940, 800, 525, false), Some(25_174_800));
        assert_eq!(implied_pclock_hz(59_940, 858, 525, true), Some(13_499_986));
    }
}
