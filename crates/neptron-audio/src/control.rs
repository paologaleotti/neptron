//! The dynamic rate control law of RetroArch (H.-K. Arntzen, "Dynamic Rate
//! Control for Retro Game Emulators", 2012, equation 2).

/// The largest relative change of the resample ratio, in parts per million:
/// `d` in the law. 0.5%.
pub const MAX_RATE_DEVIATION_PPM: u32 = 5_000;

/// The control span `A_B`, in milliseconds of output frames. The target
/// queue level is half of it.
pub const CONTROL_SPAN_MS: u32 = 64;

/// The law that turns the queue level into a correction of the nominal
/// resample ratio.
///
/// Once per core frame, before the write: `direction = clamp((A_B - 2 q) /
/// A_B, -1, 1)` and `factor = 1 + d * direction`. The resample ratio is
/// the nominal ratio times `factor`. A queue below half the span gives a
/// factor above 1, so more output frames per input frame, and the queue
/// fills; a queue above half the span drains. The clamp keeps the factor
/// inside `[1 - d, 1 + d]` when the queue is beyond the span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateControl {
    span_frames: usize,
}

impl RateControl {
    /// The law for a device at `device_rate_hz`: a span of
    /// [`CONTROL_SPAN_MS`] of its frames, and `d` =
    /// [`MAX_RATE_DEVIATION_PPM`].
    pub fn new(device_rate_hz: u32) -> Self {
        let span = u64::from(device_rate_hz) * u64::from(CONTROL_SPAN_MS) / 1_000;
        // Two frames at least, so that the target is not zero and the
        // division below has a divisor.
        let span_frames = usize::try_from(span).unwrap_or(usize::MAX).max(2);
        Self { span_frames }
    }

    /// The control span `A_B`, in output frames.
    pub fn span_frames(&self) -> usize {
        self.span_frames
    }

    /// The target queue level, in output frames: half the span.
    pub fn target_frames(&self) -> usize {
        self.span_frames / 2
    }

    /// The factor for the nominal ratio at the queue level `queued_frames`,
    /// read just before the write. It is always inside `[1 - d, 1 + d]`.
    pub fn factor(&self, queued_frames: usize) -> f64 {
        let span = self.span_frames as f64;
        let direction = ((span - 2.0 * queued_frames as f64) / span).clamp(-1.0, 1.0);
        1.0 + max_deviation() * direction
    }
}

/// `d` as a fraction.
pub(crate) fn max_deviation() -> f64 {
    f64::from(MAX_RATE_DEVIATION_PPM) / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_span_is_64_ms_of_device_frames() {
        assert_eq!(RateControl::new(48_000).span_frames(), 3_072);
        assert_eq!(RateControl::new(48_000).target_frames(), 1_536);
        assert_eq!(RateControl::new(44_100).span_frames(), 2_822);
        assert_eq!(RateControl::new(0).span_frames(), 2);
    }

    #[test]
    fn the_factor_follows_the_distance_from_half_full() {
        let law = RateControl::new(48_000);
        assert_eq!(law.factor(1_536), 1.0);
        assert_eq!(law.factor(0), 1.005);
        assert_eq!(law.factor(3_072), 0.995);
        // A quarter of the span below the target: half of d.
        assert_eq!(law.factor(768), 1.0025);
        assert_eq!(law.factor(2_304), 0.9975);
    }

    #[test]
    fn the_factor_never_leaves_the_deviation_bound() {
        let law = RateControl::new(48_000);
        assert_eq!(law.factor(10_000), 0.995);
        assert_eq!(law.factor(usize::MAX), 0.995);
        for queued in (0..8_000).step_by(7) {
            let factor = law.factor(queued);
            assert!((0.995..=1.005).contains(&factor), "{queued}: {factor}");
        }
    }

    #[test]
    fn the_factor_falls_as_the_queue_rises() {
        let law = RateControl::new(48_000);
        let factors: Vec<f64> = (0..=3_072).map(|q| law.factor(q)).collect();
        assert!(factors.windows(2).all(|w| w[1] < w[0]));
    }
}
