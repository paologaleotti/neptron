/// The time source. Real backends read a monotonic clock; tests use a
/// simulated one.
pub trait Clock {
    /// The time now, in nanoseconds from an origin that the clock chooses.
    /// It never goes back.
    fn now_ns(&self) -> u64;
}
