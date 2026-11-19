//! When a core frame waits for its flip: the wait pacing and the late
//! pacing of a core or a GPU that does not fit in one flip interval, and
//! the time of a flip on the session clock.

/// Flips in a row of wait pacing that land after their blank although the
/// work of their frame fit (the GPU was not done) before the loop goes to
/// late pacing. One such flip can be a fault of the display; two in a row
/// are a frame that does not fit.
pub const LATE_FLIPS_TO_ENTER: u32 = 2;

/// The shortest hold, in core frames: 0.5 s at 60 Hz. Late pacing ends
/// after a hold of frames in a row whose work and flip fit.
pub const LATE_HOLD_FRAMES: u32 = 30;

/// The longest hold, in core frames: 32 s at 60 Hz. Each entry into late
/// pacing doubles the hold of the next stay, up to this bound.
pub const LATE_HOLD_MAX_FRAMES: u32 = 1_920;

/// Frames of wait pacing with no late frame after which the hold halves,
/// down to [`LATE_HOLD_FRAMES`]: 10 s at 60 Hz. Long frames closer than
/// this lengthen each stay until the loop stays in late pacing; rarer
/// ones each cost one short stay.
pub const HOLD_DECAY_FRAMES: u32 = 600;

/// The margin, in permille of the work at which the display made flips
/// late, by which the work of a frame must be lower to count towards the
/// hold. A core that the GPU makes late stays in late pacing, with no
/// try of wait pacing that would fail.
pub const TRY_MARGIN_PERMILLE: u64 = 30;

/// Nanoseconds in one second times millihertz in one hertz.
const NS_MHZ: u64 = 1_000_000_000_000;

/// The pacing state of the loop. Pure: the session gives it the work of
/// each frame and the outcome of each flip.
#[derive(Debug, Clone)]
pub(crate) struct Pacing {
    late: bool,
    /// The work of the last presented frame, in nanoseconds.
    work_ns: u64,
    /// That work ended before the interval.
    work_fit: bool,
    /// Wait pacing: flips in a row that the display made late.
    late_flips: u32,
    enter_due: bool,
    /// Late pacing: frames in a row whose work and flip fit.
    fitting: u32,
    /// The hold of this stay in late pacing.
    stay_hold: u32,
    /// The hold of the next stay.
    hold: u32,
    /// Wait pacing: frames since the last halving of the hold.
    wait_frames: u32,
    /// The lowest work at which the display made two flips in a row late.
    late_work_ns: Option<u64>,
}

impl Pacing {
    /// Wait pacing, with the shortest hold and nothing learned.
    pub(crate) fn new() -> Self {
        Self {
            late: false,
            work_ns: 0,
            work_fit: true,
            late_flips: 0,
            enter_due: false,
            fitting: 0,
            stay_hold: LATE_HOLD_FRAMES,
            hold: LATE_HOLD_FRAMES,
            wait_frames: 0,
            late_work_ns: None,
        }
    }

    /// Goes back to wait pacing, with the shortest hold and nothing
    /// learned.
    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    /// The loop is in late pacing.
    pub(crate) fn late(&self) -> bool {
        self.late
    }

    /// Decides, at the first present of a core frame whose `work_ns` is
    /// known, in a mode whose flips come every `interval_ns`: true when the
    /// loop does not wait for the frame's last flip.
    pub(crate) fn on_present(&mut self, work_ns: u64, interval_ns: u64) -> bool {
        self.work_ns = work_ns;
        self.work_fit = work_ns < interval_ns;
        if self.late {
            if self.fitting >= self.stay_hold {
                self.late = false;
                self.wait_frames = 0;
                return false;
            }
            return true;
        }
        self.wait_frames = self.wait_frames.saturating_add(1);
        if self.wait_frames >= HOLD_DECAY_FRAMES {
            self.wait_frames = 0;
            self.hold = (self.hold / 2).max(LATE_HOLD_FRAMES);
        }
        if !self.work_fit || self.enter_due {
            self.enter();
            return true;
        }
        false
    }

    /// Takes the flip of a paced core frame: `late_flip` when it landed
    /// after the blank it aimed at. True when the display made it late: the
    /// work of the frame fit.
    pub(crate) fn on_flip(&mut self, late_flip: bool) -> bool {
        let display_late = late_flip && self.work_fit;
        if self.late {
            if late_flip || !self.work_fit || !self.below_late_work() {
                self.fitting = 0;
            } else {
                self.fitting = self.fitting.saturating_add(1);
            }
        } else if display_late {
            self.wait_frames = 0;
            self.late_flips += 1;
            if self.late_flips >= LATE_FLIPS_TO_ENTER {
                self.enter_due = true;
                let work_ns = self.work_ns;
                self.late_work_ns = Some(self.late_work_ns.map_or(work_ns, |w| w.min(work_ns)));
            }
        } else {
            self.late_flips = 0;
        }
        display_late
    }

    /// The work of the last frame is below the learned late work by
    /// [`TRY_MARGIN_PERMILLE`], or nothing is learned.
    fn below_late_work(&self) -> bool {
        self.late_work_ns.is_none_or(|late| {
            u128::from(self.work_ns) * 1_000
                <= u128::from(late) * u128::from(1_000 - TRY_MARGIN_PERMILLE)
        })
    }

    fn enter(&mut self) {
        self.late = true;
        self.enter_due = false;
        self.late_flips = 0;
        self.fitting = 0;
        self.stay_hold = self.hold;
        self.hold = (self.hold * 2).min(LATE_HOLD_MAX_FRAMES);
    }
}

/// The offset between the session clock and the clock of the flip events,
/// which can have another origin (the kernel's on KMS). Both clocks are
/// monotonic, so the offset is constant. Its lowest estimate, the session
/// time after a wait less the flip time, comes from a wait that returned
/// at the flip; a wait for a flip that completed earlier gives a higher
/// estimate and changes nothing.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FlipClock {
    offset_ns: Option<i128>,
}

impl FlipClock {
    /// The time on the session clock of a flip at `flip_ns` on its own
    /// clock, seen when the session clock reads `now_ns` after the wait.
    pub(crate) fn flip_time(&mut self, now_ns: u64, flip_ns: u64) -> u64 {
        let seen = i128::from(now_ns) - i128::from(flip_ns);
        let offset = self.offset_ns.map_or(seen, |o| o.min(seen));
        self.offset_ns = Some(offset);
        u64::try_from((i128::from(flip_ns) + offset).max(0)).unwrap_or(u64::MAX)
    }
}

/// The flip interval, in nanoseconds, of a mode with field rate
/// `field_rate_mhz` whose flips take `fields_per_flip` blanks. `None` for
/// a rate of 0.
pub(crate) fn interval_ns(field_rate_mhz: u64, fields_per_flip: u64) -> Option<u64> {
    (field_rate_mhz > 0).then(|| fields_per_flip * NS_MHZ / field_rate_mhz)
}

/// The blanks between two flips: from their vertical blank counts, or,
/// when the count does not advance (a driver that does not count blanks),
/// from their times rounded to whole fields of `field_ns`.
pub(crate) fn flip_gap(sequences: (u64, u64), times_ns: (u64, u64), field_ns: u64) -> u64 {
    let (last, now) = sequences;
    if now > last {
        return now - last;
    }
    let span_ns = times_ns.1.saturating_sub(times_ns.0);
    (span_ns + field_ns / 2) / field_ns.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELD_NS: u64 = 16_666_667;

    /// One frame of `work_ns` whose flip is late or not.
    fn frame(pacing: &mut Pacing, work_ns: u64, late_flip: bool) -> bool {
        let defer = pacing.on_present(work_ns, FIELD_NS);
        pacing.on_flip(late_flip);
        defer
    }

    /// Frames until the loop leaves late pacing, at most `max`.
    fn stay(pacing: &mut Pacing, work_ns: u64, max: u32) -> u32 {
        (1..=max)
            .find(|_| !frame(pacing, work_ns, false))
            .unwrap_or(max + 1)
    }

    #[test]
    fn a_frame_that_fits_waits_and_a_late_one_does_not() {
        let mut pacing = Pacing::new();
        assert!(!frame(&mut pacing, FIELD_NS - 1, false));
        assert!(frame(&mut pacing, FIELD_NS, true));
        assert!(pacing.late());
        assert!(frame(&mut pacing, FIELD_NS / 2, false), "late pacing holds");
        pacing.reset();
        assert!(!frame(&mut pacing, FIELD_NS / 2, false));
    }

    #[test]
    fn two_flips_late_by_the_display_enter_late_pacing() {
        let mut pacing = Pacing::new();
        assert!(
            !frame(&mut pacing, FIELD_NS / 2, true),
            "one can be a fault"
        );
        assert!(!frame(&mut pacing, FIELD_NS / 2, false));
        assert!(!frame(&mut pacing, FIELD_NS / 2, true));
        assert!(!frame(&mut pacing, FIELD_NS / 2, true));
        assert!(pacing.on_present(FIELD_NS / 2, FIELD_NS), "the next frame");
    }

    #[test]
    fn each_entry_doubles_the_next_hold_and_wait_pacing_halves_it() {
        let mut pacing = Pacing::new();
        let mut hold = LATE_HOLD_FRAMES;
        while hold < LATE_HOLD_MAX_FRAMES {
            assert!(frame(&mut pacing, FIELD_NS * 2, true), "entry");
            assert_eq!(stay(&mut pacing, FIELD_NS / 2, 4_000), hold + 1);
            hold *= 2;
        }
        assert!(frame(&mut pacing, FIELD_NS * 2, true));
        assert_eq!(
            stay(&mut pacing, FIELD_NS / 2, 4_000),
            LATE_HOLD_MAX_FRAMES + 1
        );
        for _ in 0..HOLD_DECAY_FRAMES * 2 {
            assert!(!frame(&mut pacing, FIELD_NS / 2, false));
        }
        assert!(frame(&mut pacing, FIELD_NS * 2, true));
        assert_eq!(
            stay(&mut pacing, FIELD_NS / 2, 4_000),
            LATE_HOLD_MAX_FRAMES / 4 + 1
        );
    }

    #[test]
    fn a_late_flip_or_a_late_work_starts_the_hold_again() {
        let mut pacing = Pacing::new();
        frame(&mut pacing, FIELD_NS * 2, true);
        for _ in 0..LATE_HOLD_FRAMES - 1 {
            frame(&mut pacing, FIELD_NS / 2, false);
        }
        frame(&mut pacing, FIELD_NS / 2, true);
        for _ in 0..LATE_HOLD_FRAMES - 1 {
            frame(&mut pacing, FIELD_NS / 2, false);
        }
        frame(&mut pacing, FIELD_NS, false);
        assert_eq!(stay(&mut pacing, FIELD_NS / 2, 100), LATE_HOLD_FRAMES + 1);
    }

    #[test]
    fn a_core_late_by_its_gpu_does_not_try_again_until_its_work_drops() {
        let work = FIELD_NS * 95 / 100;
        let mut pacing = Pacing::new();
        frame(&mut pacing, work, true);
        frame(&mut pacing, work, true);
        assert!(pacing.on_present(work, FIELD_NS));
        pacing.on_flip(true);
        assert_eq!(stay(&mut pacing, work, 4_000), 4_001, "no try at that work");
        let lower = work * (1_000 - TRY_MARGIN_PERMILLE) / 1_000;
        assert_eq!(stay(&mut pacing, lower, 100), LATE_HOLD_FRAMES + 1);
        pacing.reset();
        assert!(!frame(&mut pacing, work, false), "forgotten at the reset");
    }

    #[test]
    fn the_flip_clock_keeps_the_lowest_offset() {
        const AHEAD_NS: u64 = 1_000_000_000_000_000;
        let mut clock = FlipClock::default();
        let flip = AHEAD_NS + 5_000_000;
        assert_eq!(clock.flip_time(5_050_000, flip), 5_050_000);
        let next = flip + FIELD_NS;
        assert_eq!(
            clock.flip_time(5_000_000 + FIELD_NS + 30_000, next),
            5_000_000 + FIELD_NS + 30_000
        );
        let late = next + FIELD_NS;
        let seen = 5_000_000 + 2 * FIELD_NS + 10_000_000;
        assert_eq!(
            clock.flip_time(seen, late),
            5_000_000 + 2 * FIELD_NS + 30_000,
            "a wait after the flip keeps the offset"
        );
        let mut behind = FlipClock::default();
        assert_eq!(behind.flip_time(AHEAD_NS, 7), AHEAD_NS);
        assert_eq!(behind.flip_time(AHEAD_NS + 20, 17), AHEAD_NS + 10);
    }

    #[test]
    fn the_gap_comes_from_the_times_when_the_count_does_not_advance() {
        assert_eq!(flip_gap((5, 7), (0, 0), FIELD_NS), 2);
        assert_eq!(flip_gap((0, 0), (0, FIELD_NS + 1_000), FIELD_NS), 1);
        assert_eq!(flip_gap((0, 0), (0, 2 * FIELD_NS - 1_000), FIELD_NS), 2);
        assert_eq!(flip_gap((0, 0), (0, FIELD_NS * 3 / 2 - 1), FIELD_NS), 1);
    }

    #[test]
    fn the_interval_follows_the_rate_and_the_blanks_per_flip() {
        assert_eq!(interval_ns(60_000, 1), Some(16_666_666));
        assert_eq!(interval_ns(59_940, 2), Some(33_366_700));
        assert_eq!(interval_ns(0, 1), None);
    }
}
