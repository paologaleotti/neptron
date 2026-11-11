//! The vertical blanks of one mode on the simulated clock.

use std::collections::BTreeMap;

use crate::rng;

/// Nanoseconds times millihertz in one second: `10^9 * 10^3`.
const NS_MHZ_PER_S: u128 = 1_000_000_000_000;

/// How far each vertical blank moves from its nominal time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Jitter {
    /// Every vertical blank is on its nominal time.
    #[default]
    None,
    /// A seeded offset in `[-max_ns, max_ns]`, from the vertical blank
    /// sequence number and the seed only.
    Seeded {
        /// The seed of the generator.
        seed: u64,
        /// The largest offset, in nanoseconds.
        max_ns: u64,
    },
    /// An offset in nanoseconds per vertical blank sequence number. A
    /// sequence number not in the map has no offset.
    Script(BTreeMap<u64, i64>),
}

impl Jitter {
    /// The largest offset this jitter can give, in nanoseconds.
    pub(crate) fn max_abs_ns(&self) -> u64 {
        match self {
            Jitter::None => 0,
            Jitter::Seeded { max_ns, .. } => *max_ns,
            Jitter::Script(offsets) => offsets
                .values()
                .map(|v| v.unsigned_abs())
                .max()
                .unwrap_or(0),
        }
    }

    fn offset_ns(&self, sequence: u64) -> i64 {
        match self {
            Jitter::None => 0,
            Jitter::Seeded { seed, max_ns } => rng::symmetric(*seed, sequence, *max_ns),
            Jitter::Script(offsets) => offsets.get(&sequence).copied().unwrap_or(0),
        }
    }
}

/// The vertical blanks of one mode. Blank `k` (from 1) comes `k` periods
/// after `start_ns`, plus its jitter, and has the sequence number
/// `sequence_base + k`.
///
/// The jitter of every blank is below half a period, so the blanks stay in
/// order: [`Grid::new`] checks it.
#[derive(Debug, Clone)]
pub(crate) struct Grid {
    start_ns: u64,
    rate_mhz: u64,
    sequence_base: u64,
    jitter: Jitter,
}

/// Why a [`Grid`] cannot be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GridError {
    /// The refresh rate is 0.
    NoRate,
    /// The jitter can reach half a period or more.
    JitterTooLarge { jitter_ns: u64, period_ns: u64 },
}

impl Grid {
    pub(crate) fn new(
        start_ns: u64,
        rate_mhz: u64,
        sequence_base: u64,
        jitter: Jitter,
    ) -> Result<Self, GridError> {
        if rate_mhz == 0 {
            return Err(GridError::NoRate);
        }
        let period_ns = u64::try_from(NS_MHZ_PER_S / u128::from(rate_mhz)).unwrap_or(u64::MAX);
        let jitter_ns = jitter.max_abs_ns();
        if jitter_ns.saturating_mul(2) >= period_ns {
            return Err(GridError::JitterTooLarge {
                jitter_ns,
                period_ns,
            });
        }
        Ok(Self {
            start_ns,
            rate_mhz,
            sequence_base,
            jitter,
        })
    }

    /// The sequence number of blank `k`.
    pub(crate) fn sequence(&self, k: u64) -> u64 {
        self.sequence_base.saturating_add(k)
    }

    /// The time of blank `k`, in nanoseconds. The nominal time has no
    /// cumulative rounding error: it is `k * 10^12 / rate_mhz`, rounded
    /// down, after the start.
    pub(crate) fn time_ns(&self, k: u64) -> u64 {
        let nominal = u128::from(k) * NS_MHZ_PER_S / u128::from(self.rate_mhz);
        let nominal = u64::try_from(nominal)
            .unwrap_or(u64::MAX)
            .saturating_add(self.start_ns);
        nominal.saturating_add_signed(self.jitter.offset_ns(self.sequence(k)))
    }

    /// The first blank from `from_k` on that comes after `time_ns`.
    pub(crate) fn first_after(&self, time_ns: u64, from_k: u64) -> u64 {
        let mut k = from_k
            .max(1)
            .max(self.nominal_index(time_ns).saturating_sub(1));
        while self.time_ns(k) <= time_ns {
            k += 1;
        }
        k
    }

    /// The count of blanks at or before `time_ns`.
    pub(crate) fn count_until(&self, time_ns: u64) -> u64 {
        let mut k = self.nominal_index(time_ns) + 1;
        while k > 0 && self.time_ns(k) > time_ns {
            k -= 1;
        }
        k
    }

    /// The index of the last nominal blank at or before `time_ns`, or 0.
    fn nominal_index(&self, time_ns: u64) -> u64 {
        let elapsed = u128::from(time_ns.saturating_sub(self.start_ns));
        u64::try_from(elapsed * u128::from(self.rate_mhz) / NS_MHZ_PER_S).unwrap_or(u64::MAX - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_times_have_no_drift() {
        let grid = Grid::new(1_000, 59_940, 10, Jitter::None).unwrap();
        assert_eq!(grid.time_ns(1), 1_000 + 16_683_350);
        // 59 940 periods of 1/59.94 s are exactly 1000 s.
        assert_eq!(grid.time_ns(59_940), 1_000 + 1_000_000_000_000);
        assert_eq!(grid.sequence(3), 13);
    }

    #[test]
    fn first_after_and_count_until_agree() {
        let jitter = Jitter::Seeded {
            seed: 9,
            max_ns: 2_000_000,
        };
        let grid = Grid::new(500, 60_000, 0, jitter).unwrap();
        for t in (0..2_000_000_000_u64).step_by(1_234_567) {
            let k = grid.first_after(t, 1);
            assert!(grid.time_ns(k) > t);
            assert!(k == 1 || grid.time_ns(k - 1) <= t, "t {t} k {k}");
            assert_eq!(grid.count_until(t), k - 1, "t {t}");
        }
        assert_eq!(grid.first_after(0, 7), 7);
        for k in 1..200 {
            assert!(grid.time_ns(k) < grid.time_ns(k + 1));
        }
    }

    #[test]
    fn scripted_jitter_moves_one_blank() {
        let grid = Grid::new(
            0,
            50_000,
            100,
            Jitter::Script(BTreeMap::from([(102, -3_000)])),
        )
        .unwrap();
        assert_eq!(grid.time_ns(1), 20_000_000);
        assert_eq!(grid.time_ns(2), 39_997_000);
        assert_eq!(grid.time_ns(3), 60_000_000);
    }

    #[test]
    fn the_grid_checks_its_inputs() {
        assert_eq!(
            Grid::new(0, 0, 0, Jitter::None).unwrap_err(),
            GridError::NoRate
        );
        let jitter = Jitter::Seeded {
            seed: 1,
            max_ns: 10_000_000,
        };
        assert_eq!(
            Grid::new(0, 50_000, 0, jitter).unwrap_err(),
            GridError::JitterTooLarge {
                jitter_ns: 10_000_000,
                period_ns: 20_000_000
            }
        );
        let script = Jitter::Script(BTreeMap::from([(1, -9_999_999)]));
        assert!(Grid::new(0, 50_000, 0, script).is_ok());
    }
}
