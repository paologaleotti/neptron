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
    /// Mean time between two samples, in nanoseconds.
    pub mean_interval_ns: u64,
    /// Shortest time between two samples, in nanoseconds.
    pub min_interval_ns: u64,
    /// Longest time between two samples, in nanoseconds.
    pub max_interval_ns: u64,
    /// Largest distance of one interval from the mean, in nanoseconds.
    pub jitter_ns: u64,
    /// The event rate, in millihertz.
    pub rate_mhz: u64,
    /// Vblanks the kernel counted between two samples beyond the first: a
    /// flip that took two vblanks adds one.
    pub skipped_vblanks: u64,
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

/// The statistics of `samples`, in time order. `None` with fewer than two
/// samples or no time between the first and the last.
pub fn stats(samples: &[Sample]) -> Option<Stats> {
    let first = samples.first()?;
    let last = samples.last()?;
    let intervals = samples.len().checked_sub(1).filter(|&n| n > 0)?;
    let span_ns = last.time_ns.checked_sub(first.time_ns).filter(|&t| t > 0)?;
    let n = u64::try_from(intervals).ok()?;
    let mean = span_ns / n;
    let mut min = u64::MAX;
    let mut max = 0;
    let mut jitter = 0;
    let mut skipped = 0_u64;
    for pair in samples.windows(2) {
        let interval = pair[1].time_ns.saturating_sub(pair[0].time_ns);
        min = min.min(interval);
        max = max.max(interval);
        jitter = jitter.max(interval.abs_diff(mean));
        let steps = pair[1].sequence.wrapping_sub(pair[0].sequence);
        skipped += u64::from(steps.saturating_sub(1));
    }
    let rate = u128::from(n) * NS_PER_S_MHZ / u128::from(span_ns);
    Some(Stats {
        count: samples.len(),
        mean_interval_ns: mean,
        min_interval_ns: min,
        max_interval_ns: max,
        jitter_ns: jitter,
        rate_mhz: u64::try_from(rate).unwrap_or(u64::MAX),
        skipped_vblanks: skipped,
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
/// kernel's readback does not report.
pub fn implied_pclock_hz(field_rate_mhz: u64, htotal: u16, vtotal: u16, interlace: bool) -> u64 {
    let fields_per_frame = if interlace { 2 } else { 1 };
    u64::from(htotal) * u64::from(vtotal) * field_rate_mhz / (1_000 * fields_per_frame)
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
    fn a_late_sample_shows_as_jitter_and_a_skipped_vblank() {
        let mut samples = even(5, 10_000_000);
        for s in &mut samples[3..] {
            s.time_ns += 10_000_000;
            s.sequence += 1;
        }
        let s = stats(&samples).unwrap();
        assert_eq!(s.min_interval_ns, 10_000_000);
        assert_eq!(s.max_interval_ns, 20_000_000);
        assert_eq!(s.mean_interval_ns, 12_500_000);
        assert_eq!(s.jitter_ns, 7_500_000);
        assert_eq!(s.skipped_vblanks, 1);
    }

    #[test]
    fn a_sequence_that_wraps_counts_no_skip() {
        let samples = [
            Sample {
                time_ns: 0,
                sequence: u32::MAX,
            },
            Sample {
                time_ns: 10,
                sequence: 0,
            },
        ];
        assert_eq!(stats(&samples).unwrap().skipped_vblanks, 0);
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
        assert_eq!(implied_pclock_hz(59_940, 800, 525, false), 25_174_800);
        assert_eq!(implied_pclock_hz(59_940, 858, 525, true), 13_499_986);
    }
}
