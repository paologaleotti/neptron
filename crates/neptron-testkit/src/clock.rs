use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use neptron_backend::Clock;

/// The simulated clock: the only time source of the fakes.
///
/// Time moves only when a test or a fake moves it: [`SimClock::advance`],
/// [`SimClock::advance_to`], or a fake display that waits for a flip. No
/// call reads the wall clock or sleeps.
///
/// Clones share one time, so a test keeps a clone and gives others to the
/// fakes and to the code under test. The clock is `Send` and `Sync`, so it
/// fits code that asks for that, but the fakes start no thread.
#[derive(Debug, Clone, Default)]
pub struct SimClock {
    now_ns: Arc<AtomicU64>,
}

/// An error of [`SimClock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ClockError {
    /// The target time is before the time now. The clock never goes back.
    #[error("the clock is at {now_ns} ns and cannot go back to {target_ns} ns")]
    Backwards {
        /// The time now, in nanoseconds.
        now_ns: u64,
        /// The time that was asked for, in nanoseconds.
        target_ns: u64,
    },
}

impl SimClock {
    /// A clock at time 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// A clock at `ns`.
    pub fn starting_at(ns: u64) -> Self {
        Self {
            now_ns: Arc::new(AtomicU64::new(ns)),
        }
    }

    /// Moves the clock forward by `ns` and returns the new time. The time
    /// stops at `u64::MAX` (about 584 years) and does not wrap.
    pub fn advance(&self, ns: u64) -> u64 {
        let new = self.now().saturating_add(ns);
        self.now_ns.store(new, Ordering::Relaxed);
        new
    }

    /// Moves the clock forward to `target_ns`. A target equal to the time
    /// now changes nothing.
    ///
    /// # Errors
    ///
    /// [`ClockError::Backwards`] when `target_ns` is before the time now.
    /// The clock does not change.
    pub fn advance_to(&self, target_ns: u64) -> Result<(), ClockError> {
        let now_ns = self.now();
        if target_ns < now_ns {
            return Err(ClockError::Backwards { now_ns, target_ns });
        }
        self.now_ns.store(target_ns, Ordering::Relaxed);
        Ok(())
    }

    fn now(&self) -> u64 {
        self.now_ns.load(Ordering::Relaxed)
    }
}

impl Clock for SimClock {
    fn now_ns(&self) -> u64 {
        self.now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_one_time() {
        let clock = SimClock::new();
        let other = clock.clone();
        assert_eq!(clock.now_ns(), 0);
        assert_eq!(other.advance(16_683_333), 16_683_333);
        assert_eq!(clock.now_ns(), 16_683_333);
        assert_eq!(SimClock::starting_at(5).now_ns(), 5);
    }

    #[test]
    fn advance_to_never_goes_back() {
        let clock = SimClock::starting_at(100);
        assert_eq!(clock.advance_to(100), Ok(()));
        assert_eq!(clock.advance_to(250), Ok(()));
        assert_eq!(
            clock.advance_to(249),
            Err(ClockError::Backwards {
                now_ns: 250,
                target_ns: 249
            })
        );
        assert_eq!(clock.now_ns(), 250);
        assert_eq!(
            ClockError::Backwards {
                now_ns: 250,
                target_ns: 249
            }
            .to_string(),
            "the clock is at 250 ns and cannot go back to 249 ns"
        );
    }

    #[test]
    fn advance_saturates() {
        let clock = SimClock::starting_at(u64::MAX - 1);
        assert_eq!(clock.advance(10), u64::MAX);
        assert_eq!(clock.now_ns(), u64::MAX);
    }
}
