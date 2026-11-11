//! The run rate measured from flip times, for an output whose flip rate is
//! not the mode rate.

/// Flip intervals in the mean: 2 s at 60 Hz.
pub const FLIP_METER_INTERVALS: usize = 120;

/// Intervals the meter needs before it gives a rate.
pub const FLIP_METER_MIN_INTERVALS: usize = 30;

/// Picoseconds in one second: an interval in nanoseconds gives a rate in
/// millihertz as `10^12 / ns`.
const NS_MHZ_PER_S: u128 = 1_000_000_000_000;

/// The mean flip rate over the last [`FLIP_METER_INTERVALS`] flips.
///
/// The desktop window flips at the desktop rate, not at the mode rate, so
/// the session measures its run rate here and gives it to
/// [`AudioPipeline::set_run_rate_mhz`](crate::AudioPipeline::set_run_rate_mhz).
/// The meter keeps a fixed array and does not allocate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlipRateMeter {
    times_ns: [u64; FLIP_METER_INTERVALS + 1],
    /// Index of the next slot to write.
    next: usize,
    /// Times held, up to the array length.
    held: usize,
}

impl Default for FlipRateMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl FlipRateMeter {
    /// A meter with no flip yet.
    pub fn new() -> Self {
        Self {
            times_ns: [0; FLIP_METER_INTERVALS + 1],
            next: 0,
            held: 0,
        }
    }

    /// Records the time of a flip (`FlipEvent::time_ns`). A time that is not
    /// after the last one is ignored.
    pub fn push(&mut self, time_ns: u64) {
        if self.held > 0 && time_ns <= self.newest() {
            return;
        }
        self.times_ns[self.next] = time_ns;
        self.next = (self.next + 1) % self.times_ns.len();
        self.held = (self.held + 1).min(self.times_ns.len());
    }

    /// The mean flip rate, in millihertz, rounded. `None` before
    /// [`FLIP_METER_MIN_INTERVALS`] intervals.
    pub fn rate_mhz(&self) -> Option<u64> {
        let intervals = self.held.checked_sub(1)?;
        if intervals < FLIP_METER_MIN_INTERVALS {
            return None;
        }
        let oldest = (self.next + self.times_ns.len() - self.held) % self.times_ns.len();
        let span = u128::from(self.newest() - self.times_ns[oldest]);
        let rate = (intervals as u128 * NS_MHZ_PER_S + span / 2) / span;
        u64::try_from(rate).ok()
    }

    /// Forgets every flip, for example after a mode switch.
    pub fn reset(&mut self) {
        self.held = 0;
        self.next = 0;
    }

    fn newest(&self) -> u64 {
        self.times_ns[(self.next + self.times_ns.len() - 1) % self.times_ns.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(meter: &mut FlipRateMeter, period_ns: u64, count: u64) {
        for index in 0..count {
            meter.push(1_000 + index * period_ns);
        }
    }

    #[test]
    fn no_rate_before_the_minimum_intervals() {
        let mut meter = FlipRateMeter::new();
        assert_eq!(meter.rate_mhz(), None);
        feed(&mut meter, 16_666_667, 30);
        assert_eq!(meter.rate_mhz(), None);
        meter.push(1_000 + 30 * 16_666_667);
        assert_eq!(meter.rate_mhz(), Some(60_000));
    }

    #[test]
    fn the_rate_is_the_mean_over_the_last_intervals() {
        let mut meter = FlipRateMeter::new();
        // 50 Hz, then a desktop at 60 Hz: after 121 flips at 60 Hz only
        // those count.
        feed(&mut meter, 20_000_000, 200);
        assert_eq!(meter.rate_mhz(), Some(50_000));
        let start = 1_000 + 199 * 20_000_000;
        for index in 1..=121 {
            meter.push(start + index * 16_666_667);
        }
        assert_eq!(meter.rate_mhz(), Some(60_000));
    }

    #[test]
    fn jitter_averages_out() {
        let mut meter = FlipRateMeter::new();
        let mut time = 0;
        for index in 0..500u64 {
            let jitter = if index % 2 == 0 { 500_000 } else { 0 };
            meter.push(time + jitter);
            time += 16_639_267;
        }
        let rate = meter.rate_mhz().unwrap();
        assert!(rate.abs_diff(60_099) <= 1, "{rate}");
    }

    #[test]
    fn a_time_that_does_not_move_forward_is_ignored_and_reset_forgets() {
        let mut meter = FlipRateMeter::new();
        feed(&mut meter, 20_000_000, 40);
        let rate = meter.rate_mhz();
        meter.push(5);
        meter.push(1_000 + 39 * 20_000_000);
        assert_eq!(meter.rate_mhz(), rate);
        meter.reset();
        assert_eq!(meter.rate_mhz(), None);
        assert_eq!(FlipRateMeter::default(), FlipRateMeter::new());
    }
}
