//! The measured speed of a late core, for the audio run rate, and the
//! bounded change of that run rate.

/// Late frames over which the session measures the rate of core frames:
/// 2 s at 60 Hz.
pub const SPEED_WINDOW_FRAMES: usize = 120;

/// Late frames in the window before the window rule can call the core
/// slow. Each late frame before the measure is used costs the audio queue
/// the part of a frame by which it is late; 8 frames of a core at 1.04
/// intervals keep the queue above zero at the entry.
pub const SPEED_MIN_FRAMES: usize = 8;

/// The margin, in permille of the interval, by which the mean cycle of
/// the window without its largest cycle must be above the interval for
/// the core to be slow. Below it, the error of the mode's run rate is
/// inside the 0.5% that the rate control corrects.
pub const SPEED_SLOW_MARGIN_PERMILLE: u64 = 5;

/// The recent cycles that the window compares with itself: 0.5 s at 60
/// Hz. Long enough to hold several long frames of a core that has one
/// every few frames, short enough that a change of scene shows within
/// half a second.
pub const SPEED_RECENT_FRAMES: usize = 30;

/// The difference, in permille of the mean of the older cycles of the
/// window, between the mean of the recent cycles and that mean at which
/// the speed of the core changed: the window keeps only the recent
/// cycles, so the measure follows the new speed within about twice
/// [`SPEED_RECENT_FRAMES`] frames, in both directions.
pub const SPEED_CHANGE_PERMILLE: u64 = 100;

/// The cycle, in permille of the interval, of a frame that is clearly
/// slow.
pub const SPEED_CLEAR_SLOW_PERMILLE: u64 = 1_200;

/// Clearly slow frames in a row after which the measure is used at once,
/// and clearly fast frames in a row after a clearly slow run after which
/// it stops at once. A single long frame never passes.
pub const SPEED_CLEAR_SLOW_FRAMES: u32 = 2;

/// The cycle, in permille of the interval, of a frame that is clearly
/// fast: the frame fit, or almost.
pub const SPEED_CLEAR_FAST_PERMILLE: u64 = 1_050;

/// The largest change of the audio run rate, in permille of the mode's
/// run rate per second. A change of `D` (as a fraction) over `D / s`
/// seconds puts the queue off by `D * D / (2 s)` seconds of audio. At 4
/// per second the largest change before the mute, 50%, costs at most
/// 31 ms, the queue's target level, half of its span; the change of a
/// scene of Killer Instinct (33%) costs 14 ms and glides over 83 ms.
pub const RUN_RATE_SLEW_PERMILLE_PER_S: u64 = 4_000;

/// The level of the audio queue, in permille of its target, inside which
/// the session leaves the run rate to the rate control alone: 16 ms either
/// way at 48 kHz. A core with a long frame every few frames swings the
/// level before a write inside it, so the correction does not follow those
/// swings.
pub const QUEUE_BAND_PERMILLE: u64 = 500;

/// The largest change of the run rate, in permille, that the session adds
/// to bring the queue back to its target when its level is outside
/// [`QUEUE_BAND_PERMILLE`]: in proportion to the distance beyond the band,
/// up to the whole target. At 60, the queue comes back towards its band
/// with a time constant of about 0.5 s (a queue one target too full is
/// back in the band in about 1.5 s); the rate control alone (0.5%) takes
/// about 10 s. The pitch moves by at most a semitone while the queue is
/// far off.
pub const QUEUE_TRIM_PERMILLE: u64 = 60;

/// The frames over which the session smooths the level of the queue that
/// it corrects: about a quarter of a second. The level before a write
/// swings with the cycle of each frame; a core with a long frame every
/// few frames swings it by a frame of audio, which the correction must not
/// follow.
pub const QUEUE_SMOOTH_FRAMES: u64 = 16;

/// The measured rate, in permille of the core's frame rate, below which
/// the sound is muted: the lower bound of the audio pipeline.
pub const SOUND_MUTE_PERMILLE: u64 = 500;

/// The measured rate, in permille of the core's frame rate, above which a
/// muted sound comes back. The gap to [`SOUND_MUTE_PERMILLE`] keeps a core
/// that hovers at half speed from chopping the sound.
pub const SOUND_RESUME_PERMILLE: u64 = 520;

/// Nanoseconds in one second times millihertz in one hertz.
const NS_MHZ: u64 = 1_000_000_000_000;

/// The cycle times of the last late frames, in a ring made at launch: the
/// time from the end of the frame before to the end of the frame.
#[derive(Debug, Clone)]
pub(crate) struct Speed {
    cycles: [u64; SPEED_WINDOW_FRAMES],
    first: usize,
    len: usize,
    sum_ns: u64,
    /// Clearly slow cycles in a row.
    clear_slow: u32,
    /// Clearly fast cycles in a row since the last clearly slow run, or
    /// `None` when no clearly slow run came since the window was cleared.
    clear_fast: Option<u32>,
}

impl Speed {
    /// An empty measure.
    pub(crate) fn new() -> Self {
        Self {
            cycles: [0; SPEED_WINDOW_FRAMES],
            first: 0,
            len: 0,
            sum_ns: 0,
            clear_slow: 0,
            clear_fast: None,
        }
    }

    /// Drops every cycle.
    pub(crate) fn clear(&mut self) {
        self.first = 0;
        self.len = 0;
        self.sum_ns = 0;
        self.clear_slow = 0;
        self.clear_fast = None;
    }

    /// Adds the cycle of a late frame in a mode whose flips come every
    /// `interval_ns`.
    pub(crate) fn push(&mut self, cycle_ns: u64, interval_ns: u64) {
        let permille = |p: u64| u128::from(interval_ns) * u128::from(p);
        let cycle = u128::from(cycle_ns) * 1_000;
        if self.turned_fast(cycle <= permille(SPEED_CLEAR_FAST_PERMILLE)) {
            self.clear();
            return;
        }
        let clearly = cycle >= permille(SPEED_CLEAR_SLOW_PERMILLE);
        self.clear_slow = if clearly {
            self.clear_slow.saturating_add(1)
        } else {
            0
        };
        let size = self.cycles.len();
        if self.len == size {
            self.sum_ns -= self.cycles[self.first];
            self.cycles[self.first] = cycle_ns;
            self.first = (self.first + 1) % size;
        } else {
            self.cycles[(self.first + self.len) % size] = cycle_ns;
            self.len += 1;
        }
        self.sum_ns += cycle_ns;
        if self.clear_slow >= SPEED_CLEAR_SLOW_FRAMES {
            self.clear_fast = Some(0);
        }
        self.follow_a_change();
    }

    /// Counts a clearly `fast` cycle after a clearly slow run: true when
    /// [`SPEED_CLEAR_SLOW_FRAMES`] of them came in a row, as when a heavy
    /// scene of the game ends. Long frames that come one by one between
    /// fast ones never make a clearly slow run.
    fn turned_fast(&mut self, fast: bool) -> bool {
        let Some(count) = self.clear_fast.as_mut() else {
            return false;
        };
        *count = if fast { *count + 1 } else { 0 };
        *count >= SPEED_CLEAR_SLOW_FRAMES
    }

    /// Keeps only the recent cycles when their mean differs from the mean
    /// of the older cycles of the window by [`SPEED_CHANGE_PERMILLE`]. The
    /// older part holds at least as many cycles as the recent one, so a
    /// core with a long frame every few frames compares like with like.
    fn follow_a_change(&mut self) {
        if self.len < 2 * SPEED_RECENT_FRAMES {
            return;
        }
        let size = self.cycles.len();
        let recent_ns: u64 = (self.len - SPEED_RECENT_FRAMES..self.len)
            .map(|i| self.cycles[(self.first + i) % size])
            .sum();
        let older_ns = self.sum_ns - recent_ns;
        let older_len = (self.len - SPEED_RECENT_FRAMES) as u128;
        let recent = u128::from(recent_ns) * older_len;
        let older = u128::from(older_ns) * SPEED_RECENT_FRAMES as u128;
        if recent.abs_diff(older) * 1_000 > older * u128::from(SPEED_CHANGE_PERMILLE) {
            self.first = (self.first + self.len - SPEED_RECENT_FRAMES) % size;
            self.len = SPEED_RECENT_FRAMES;
            self.sum_ns = recent_ns;
        }
    }

    /// The measured rate of core frames, in millihertz, when the core is
    /// slow: the last [`SPEED_CLEAR_SLOW_FRAMES`] cycles were clearly
    /// slow, or the window holds [`SPEED_MIN_FRAMES`] or more and its mean
    /// cycle without its largest one is above `interval_ns` by
    /// [`SPEED_SLOW_MARGIN_PERMILLE`]. So a single long frame of a fast
    /// core is not a slow core, and a core slow on average is.
    pub(crate) fn rate_mhz(&self, interval_ns: u64) -> Option<u64> {
        if self.len == 0 || self.sum_ns == 0 {
            return None;
        }
        let slow = self.clear_slow >= SPEED_CLEAR_SLOW_FRAMES || self.slow_on_average(interval_ns);
        slow.then(|| self.len as u64 * NS_MHZ / self.sum_ns)
    }

    fn slow_on_average(&self, interval_ns: u64) -> bool {
        if self.len < SPEED_MIN_FRAMES {
            return false;
        }
        let largest = (0..self.len)
            .map(|i| self.cycles[(self.first + i) % self.cycles.len()])
            .max()
            .unwrap_or(0);
        let rest = u128::from(self.sum_ns - largest);
        let count = (self.len - 1) as u128;
        rest * 1_000
            > count * u128::from(interval_ns) * u128::from(1_000 + SPEED_SLOW_MARGIN_PERMILLE)
    }
}

/// The level of the audio queue: the `last` level before a write, and the
/// level `smoothed` over [`QUEUE_SMOOTH_FRAMES`], in frames, with the
/// `target` level of the rate control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueLevel {
    pub(crate) last: u64,
    pub(crate) smoothed: u64,
    pub(crate) target: u64,
}

/// The run rate, in millihertz, that the audio should reach: the
/// `measured` rate of a slow core, or the mode's rate, corrected by the
/// level of the queue.
///
/// Inside [`QUEUE_BAND_PERMILLE`] of its target, the queue is left to the
/// rate control. Beyond the band, in proportion to the distance beyond it
/// (up to the whole target):
/// - a last level above twice the target (the whole span of the rate
///   control) means the core makes audio faster than measured, as when it
///   turned fast and the window has not followed yet: the rate moves from
///   the measure towards the mode's rate at once, in proportion to the
///   level above the span (all the way at three times the target), so
///   the queue does not fill up. A core with a long frame every few
///   frames swings the level by about a frame of audio around its target
///   and stays below the span;
/// - in both directions the smoothed level trims the rate by up to
///   [`QUEUE_TRIM_PERMILLE`], so that the queue returns to its band.
pub(crate) fn run_rate_target(mode_rate: u64, measured: Option<u64>, queue: QueueLevel) -> u64 {
    let pull = if queue.target == 0 {
        0
    } else {
        let over = i128::from(queue.last) - 2 * i128::from(queue.target);
        (over * 1_000 / i128::from(queue.target)).clamp(0, 1_000)
    };
    let beyond = queue_beyond_permille(queue.smoothed, queue.target);
    let base = match measured {
        Some(rate) if pull > 0 && rate < mode_rate => {
            let step = u128::from(mode_rate - rate) * pull.unsigned_abs() / 1_000;
            rate + u64::try_from(step).unwrap_or(0)
        }
        Some(rate) => rate,
        None => mode_rate,
    };
    let trim = i128::from(beyond) * i128::from(QUEUE_TRIM_PERMILLE) / 1_000;
    u64::try_from(i128::from(base) * (1_000 + trim) / 1_000).unwrap_or(base)
}

/// The queue at `level` frames is inside [`QUEUE_BAND_PERMILLE`] of its
/// `target`.
pub(crate) fn queue_in_band(level: u64, target: u64) -> bool {
    queue_beyond_permille(level, target) == 0
}

/// How far the queue is beyond its band, in permille of its target:
/// positive when too full, negative when too empty, 0 inside the band,
/// at most 1000 either way.
fn queue_beyond_permille(level: u64, target: u64) -> i64 {
    if target == 0 {
        return 0;
    }
    let off_permille = (i128::from(level) - i128::from(target)) * 1_000 / i128::from(target);
    let beyond = (off_permille.abs() - i128::from(QUEUE_BAND_PERMILLE)).clamp(0, 1_000);
    i64::try_from(beyond * off_permille.signum()).unwrap_or(0)
}

/// The smoothed level after a new `level`: an exponential mean over
/// [`QUEUE_SMOOTH_FRAMES`] frames. A smoothed level of 0 takes the new
/// level at once.
pub(crate) fn smooth_level(smoothed: u64, level: u64) -> u64 {
    if smoothed == 0 {
        return level;
    }
    let step = (i128::from(level) - i128::from(smoothed)) / i128::from(QUEUE_SMOOTH_FRAMES);
    u64::try_from(i128::from(smoothed) + step).unwrap_or(level)
}

/// The run rate one step from `current` towards `target`, both in
/// millihertz, after `elapsed_ns` of a mode whose run rate is
/// `mode_rate_mhz`: at most [`RUN_RATE_SLEW_PERMILLE_PER_S`] of the mode's
/// rate per second, and at least 1 mHz.
pub(crate) fn slew(current: u64, target: u64, mode_rate_mhz: u64, elapsed_ns: u64) -> u64 {
    let step = u128::from(mode_rate_mhz)
        * u128::from(RUN_RATE_SLEW_PERMILLE_PER_S)
        * u128::from(elapsed_ns)
        / 1_000_000_000_000;
    let step = u64::try_from(step).unwrap_or(u64::MAX).max(1);
    if target > current {
        current.saturating_add(step).min(target)
    } else {
        current.saturating_sub(step).max(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELD_NS: u64 = 16_666_667;

    fn speed(cycles: impl IntoIterator<Item = u64>) -> Speed {
        let mut speed = Speed::new();
        for cycle in cycles {
            speed.push(cycle, FIELD_NS);
        }
        speed
    }

    #[test]
    fn a_core_slow_on_average_is_measured() {
        let cycles = (0..200).map(|k| {
            if k % 2 == 0 {
                FIELD_NS * 97 / 100
            } else {
                FIELD_NS * 105 / 100
            }
        });
        let rate = speed(cycles).rate_mhz(FIELD_NS).unwrap();
        assert_eq!(
            rate,
            120 * NS_MHZ / (60 * (FIELD_NS * 97 / 100) + 60 * (FIELD_NS * 105 / 100))
        );
    }

    #[test]
    fn periodic_long_frames_are_measured() {
        let cycles = (0..200).map(|k| {
            if k % 4 == 3 {
                FIELD_NS * 7 / 2
            } else {
                FIELD_NS
            }
        });
        let rate = speed(cycles).rate_mhz(FIELD_NS).unwrap();
        assert_eq!(
            rate,
            120 * NS_MHZ / (90 * FIELD_NS + 30 * (FIELD_NS * 7 / 2))
        );
    }

    #[test]
    fn one_long_frame_of_a_fast_core_is_not_slow() {
        let mut cycles = vec![300_000_000];
        cycles.extend([FIELD_NS; 119]);
        assert_eq!(speed(cycles).rate_mhz(FIELD_NS), None);
        assert_eq!(speed([FIELD_NS * 3]).rate_mhz(FIELD_NS), None);
    }

    #[test]
    fn two_clearly_slow_frames_are_measured_at_once() {
        let rate = speed([FIELD_NS * 13 / 10; 2]).rate_mhz(FIELD_NS);
        assert_eq!(rate, Some(NS_MHZ / (FIELD_NS * 13 / 10)));
        let speed = speed([FIELD_NS * 13 / 10, FIELD_NS * 13 / 10, FIELD_NS]);
        assert_eq!(
            speed.rate_mhz(FIELD_NS),
            None,
            "too few for the window rule"
        );
    }

    #[test]
    fn a_core_slow_by_less_than_the_margin_is_not_measured() {
        let cycles = [FIELD_NS * 1_004 / 1_000; 120];
        assert_eq!(speed(cycles).rate_mhz(FIELD_NS), None);
        let cycles = [FIELD_NS * 1_010 / 1_000; 120];
        assert!(speed(cycles).rate_mhz(FIELD_NS).is_some());
    }

    #[test]
    fn the_window_follows_a_change_of_rate() {
        let mut speed = speed([20_000_000; 200]);
        assert_eq!(speed.rate_mhz(FIELD_NS), Some(50_000));
        for _ in 0..SPEED_WINDOW_FRAMES {
            speed.push(25_000_000, FIELD_NS);
        }
        assert_eq!(speed.rate_mhz(FIELD_NS), Some(40_000));
        speed.clear();
        assert_eq!(speed.rate_mhz(FIELD_NS), None);
    }

    #[test]
    fn a_slow_core_that_turns_fast_stops_the_measure_soon() {
        let mut speed = speed([FIELD_NS * 3 / 2; 120]);
        assert!(speed.rate_mhz(FIELD_NS).is_some());
        let frames = (1..=60)
            .find(|_| {
                speed.push(FIELD_NS, FIELD_NS);
                speed.rate_mhz(FIELD_NS).is_none()
            })
            .unwrap();
        assert!(frames <= 2 * SPEED_RECENT_FRAMES as u64, "{frames} frames");
    }

    #[test]
    fn a_change_of_speed_shows_within_twice_the_recent_frames() {
        let mut speed = speed([FIELD_NS; 120].map(|c| c * 11 / 10));
        for _ in 0..2 * SPEED_RECENT_FRAMES {
            speed.push(FIELD_NS * 3 / 2, FIELD_NS);
        }
        let rate = speed.rate_mhz(FIELD_NS).unwrap();
        let slow = NS_MHZ / (FIELD_NS * 3 / 2);
        assert!(rate.abs_diff(slow) * 100 <= slow, "{rate} for {slow}");
    }

    #[test]
    fn a_clearly_slow_core_that_turns_fast_stops_the_measure_at_once() {
        let mut speed = speed([FIELD_NS * 3 / 2; 60]);
        speed.push(FIELD_NS, FIELD_NS);
        assert!(speed.rate_mhz(FIELD_NS).is_some());
        speed.push(FIELD_NS * 1_040 / 1_000, FIELD_NS);
        assert_eq!(speed.rate_mhz(FIELD_NS), None, "two fast frames");
    }

    #[test]
    fn periodic_long_frames_keep_the_measure() {
        let cycles = (0..1_000).map(|k| if k % 10 == 9 { FIELD_NS * 4 } else { FIELD_NS });
        let rate = speed(cycles).rate_mhz(FIELD_NS).unwrap();
        let truth = 10 * NS_MHZ / (9 * FIELD_NS + 4 * FIELD_NS);
        assert!(rate.abs_diff(truth) * 100 <= truth, "{rate} for {truth}");
    }

    #[test]
    fn the_queue_corrects_the_rate_only_outside_its_band() {
        let target = 1_536;
        let at = |level, measured| {
            let queue = QueueLevel {
                last: level,
                smoothed: level,
                target,
            };
            run_rate_target(60_000, measured, queue)
        };
        assert_eq!(at(target, None), 60_000);
        assert_eq!(at(target * 3 / 2, Some(40_000)), 40_000, "inside the band");
        assert_eq!(at(target / 2, None), 60_000);
        assert_eq!(at(target * 2, None), 61_800, "too full: 3% faster");
        assert_eq!(at(0, None), 58_200, "too empty: 3% slower");
        assert_eq!(at(0, Some(40_000)), 38_800);
        assert_eq!(at(target * 5, None), 63_600, "at most 6%");
        assert_eq!(
            at(target * 2, Some(40_000)),
            41_200,
            "at the span: the measure"
        );
        assert_eq!(
            at(target * 3, Some(40_000)),
            63_600,
            "far above: the mode's rate"
        );
        let fresh = QueueLevel {
            last: target * 5 / 2,
            smoothed: target,
            target,
        };
        assert_eq!(
            run_rate_target(60_000, Some(40_000), fresh),
            50_000,
            "at once"
        );
        let none = QueueLevel {
            last: 5,
            smoothed: 5,
            target: 0,
        };
        assert_eq!(run_rate_target(60_000, None, none), 60_000);
    }

    #[test]
    fn the_smoothed_level_follows_slowly() {
        assert_eq!(smooth_level(0, 1_000), 1_000);
        assert_eq!(smooth_level(1_600, 0), 1_500);
        assert_eq!(smooth_level(1_500, 3_100), 1_600);
        assert_eq!(smooth_level(1_500, 1_500), 1_500);
    }

    #[test]
    fn the_slew_is_bounded_in_both_directions() {
        assert_eq!(slew(60_000, 40_000, 60_000, 10_000_000), 57_600);
        assert_eq!(slew(57_600, 60_000, 60_000, 10_000_000), 60_000);
        assert_eq!(slew(60_000, 59_990, 60_000, 1), 59_999, "at least 1 mHz");
        assert_eq!(slew(60_000, 60_000, 60_000, 10_000_000), 60_000);
    }
}
