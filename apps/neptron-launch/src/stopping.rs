//! The stop of a running frontend: SIGTERM once, then SIGKILL when the
//! frontend has not ended after a bounded wait. A frontend stuck inside a
//! core never reads its stop flag, and the launcher must still stop.

/// How long the launcher waits after its SIGTERM before it sends SIGKILL,
/// in nanoseconds. A frontend that reads its stop flag ends within a few
/// frames; this leaves time for the last save RAM write.
pub const STOP_WAIT_NS: u64 = 5_000_000_000;

/// What the launcher does to its running child at one look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopAction {
    /// Nothing: no stop, or the wait goes on.
    Wait,
    /// Send SIGTERM.
    Term,
    /// Send SIGKILL.
    Kill,
}

/// The progress of one stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopping {
    wait_ns: u64,
    term_at_ns: Option<u64>,
    killed: bool,
}

impl Stopping {
    /// A child that no stop has reached yet, with `wait_ns` between the
    /// SIGTERM and the SIGKILL.
    pub fn new(wait_ns: u64) -> Self {
        Self {
            wait_ns,
            term_at_ns: None,
            killed: false,
        }
    }

    /// The action at `now_ns`, when the stop flag is `stop`. Each signal is
    /// sent once.
    pub fn next(&mut self, stop: bool, now_ns: u64) -> StopAction {
        if !stop || self.killed {
            return StopAction::Wait;
        }
        match self.term_at_ns {
            None => {
                self.term_at_ns = Some(now_ns);
                StopAction::Term
            }
            Some(at) if now_ns.saturating_sub(at) >= self.wait_ns => {
                self.killed = true;
                StopAction::Kill
            }
            Some(_) => StopAction::Wait,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    #[test]
    fn no_stop_sends_nothing() {
        let mut s = Stopping::new(STOP_WAIT_NS);
        for now in [0, 10 * MS, 60_000 * MS] {
            assert_eq!(s.next(false, now), StopAction::Wait);
        }
    }

    #[test]
    fn a_stop_sends_sigterm_once_then_sigkill_after_the_wait() {
        let mut s = Stopping::new(STOP_WAIT_NS);
        assert_eq!(s.next(true, 1_000 * MS), StopAction::Term);
        assert_eq!(s.next(true, 1_020 * MS), StopAction::Wait);
        assert_eq!(s.next(true, 5_999 * MS), StopAction::Wait);
        assert_eq!(s.next(true, 6_000 * MS), StopAction::Kill);
        assert_eq!(s.next(true, 6_020 * MS), StopAction::Wait);
        assert_eq!(s.next(true, 60_000 * MS), StopAction::Wait);
    }

    #[test]
    fn the_wait_counts_from_the_sigterm_not_from_the_start() {
        let mut s = Stopping::new(STOP_WAIT_NS);
        assert_eq!(s.next(false, 0), StopAction::Wait);
        assert_eq!(s.next(true, 100_000 * MS), StopAction::Term);
        assert_eq!(s.next(true, 104_999 * MS), StopAction::Wait);
        assert_eq!(s.next(true, 105_000 * MS), StopAction::Kill);
    }

    #[test]
    fn the_limit_is_five_seconds() {
        assert_eq!(STOP_WAIT_NS, 5_000 * MS);
    }
}
