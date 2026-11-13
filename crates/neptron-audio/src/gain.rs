//! The user volume: a cubic gain curve in Q24 fixed point, and a ramp that
//! moves the gain to a new target with no click.

/// The highest volume, in percent: unity gain.
pub const VOLUME_MAX_PERCENT: u32 = 100;

/// Unity gain in Q24 fixed point: `2^24`.
pub const GAIN_UNITY_Q24: u32 = 1 << GAIN_SHIFT;

/// The time a volume change takes to reach its target, in milliseconds of
/// output frames.
pub const VOLUME_RAMP_MS: u32 = 20;

/// The fraction bits of a Q24 gain.
const GAIN_SHIFT: u32 = 24;
/// Half of one output step, for rounding: `2^23`.
const ROUND_Q24: i64 = 1 << (GAIN_SHIFT - 1);
/// `VOLUME_MAX_PERCENT` cubed.
const PERCENT_CUBED_MAX: u64 = 1_000_000;

/// The gain of `percent` in Q24: `floor(v^3 * 2^24 / 10^6)` for `v` the
/// percent up to [`VOLUME_MAX_PERCENT`]. 100 is [`GAIN_UNITY_Q24`], 0 is
/// silence, and in dB the curve is `60 * log10(v / 100)`.
pub fn gain_q24(percent: u32) -> u32 {
    let v = u64::from(percent.min(VOLUME_MAX_PERCENT));
    let gain = v * v * v * u64::from(GAIN_UNITY_Q24) / PERCENT_CUBED_MAX;
    // `gain` is at most `GAIN_UNITY_Q24`, a u32.
    u32::try_from(gain).unwrap_or(GAIN_UNITY_Q24)
}

/// `sample` times a gain of at most unity, rounded half up. The result
/// lies between 0 and `sample`, so it fits an `i16`.
pub fn scale(sample: i16, gain: u32) -> i16 {
    let product = (i64::from(sample) * i64::from(gain) + ROUND_Q24) >> GAIN_SHIFT;
    i16::try_from(product).unwrap_or(sample)
}

/// The gain of the pipeline's output: a target and a straight ramp to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gain {
    percent: u32,
    target: u32,
    /// The gain at the start of the ramp.
    from: u32,
    ramp_frames: u32,
    /// Frames of the ramp still to come; 0 when the gain is the target.
    left: u32,
}

impl Gain {
    /// Unity gain, with a ramp of `ramp_frames` frames for a change.
    pub(crate) fn new(ramp_frames: u32) -> Self {
        Self {
            percent: VOLUME_MAX_PERCENT,
            target: GAIN_UNITY_Q24,
            from: GAIN_UNITY_Q24,
            ramp_frames: ramp_frames.max(1),
            left: 0,
        }
    }

    /// The volume target, in percent.
    pub(crate) fn percent(&self) -> u32 {
        self.percent
    }

    /// Sets the volume at once, with no ramp.
    pub(crate) fn jump(&mut self, percent: u32) {
        self.percent = percent.min(VOLUME_MAX_PERCENT);
        self.target = gain_q24(self.percent);
        self.from = self.target;
        self.left = 0;
    }

    /// Ramps from the gain now to the gain of `percent`.
    pub(crate) fn ramp_to(&mut self, percent: u32) {
        let now = self.current();
        self.percent = percent.min(VOLUME_MAX_PERCENT);
        self.target = gain_q24(self.percent);
        self.from = now;
        self.left = if now == self.target {
            0
        } else {
            self.ramp_frames
        };
    }

    /// Scales interleaved stereo `samples` in place and moves the ramp on
    /// by their frames. At unity with no ramp the samples stay as they are.
    pub(crate) fn apply(&mut self, samples: &mut [i16]) {
        if self.left == 0 {
            if self.target != GAIN_UNITY_Q24 {
                for sample in samples.iter_mut() {
                    *sample = scale(*sample, self.target);
                }
            }
            return;
        }
        for frame in samples.chunks_exact_mut(2) {
            let gain = self.current();
            for sample in frame {
                *sample = scale(*sample, gain);
            }
            self.left = self.left.saturating_sub(1);
        }
    }

    /// The gain of the next frame.
    fn current(&self) -> u32 {
        if self.left == 0 {
            return self.target;
        }
        let span = i64::from(self.from) - i64::from(self.target);
        let gain =
            i64::from(self.target) + span * i64::from(self.left) / i64::from(self.ramp_frames);
        // `gain` lies between `from` and `target`, both u32.
        u32::try_from(gain).unwrap_or(self.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_is_unity_at_100_and_silence_at_0() {
        assert_eq!(gain_q24(100), GAIN_UNITY_Q24);
        assert_eq!(gain_q24(0), 0);
        assert_eq!(gain_q24(250), GAIN_UNITY_Q24);
        assert_eq!(gain_q24(u32::MAX), GAIN_UNITY_Q24);
    }

    #[test]
    fn the_curve_never_falls_and_rises_on_every_step_of_5() {
        for v in 1..=VOLUME_MAX_PERCENT {
            assert!(gain_q24(v) >= gain_q24(v - 1), "{v}");
            assert!(gain_q24(v) > 0, "{v} must not be silent");
        }
        for v in (5..=VOLUME_MAX_PERCENT).step_by(5) {
            assert!(gain_q24(v) > gain_q24(v - 5), "{v}");
        }
    }

    #[test]
    fn the_curve_is_60_db_over_the_decade() {
        for (v, db) in [
            (75, -7.50),
            (50, -18.06),
            (25, -36.12),
            (10, -60.0),
            (5, -78.06),
        ] {
            let gain = f64::from(gain_q24(v)) / f64::from(GAIN_UNITY_Q24);
            let measured = 20.0 * gain.log10();
            assert!((measured - db).abs() < 0.02, "{v}: {measured} dB");
        }
    }

    #[test]
    fn unity_is_bit_exact_and_no_gain_overflows_at_full_scale() {
        for sample in [i16::MIN, -12_345, -1, 0, 1, 12_345, i16::MAX] {
            assert_eq!(scale(sample, GAIN_UNITY_Q24), sample);
            for v in 0..=VOLUME_MAX_PERCENT {
                let out = scale(sample, gain_q24(v));
                assert!(
                    i32::from(out).abs() <= i32::from(sample).abs(),
                    "{sample} at {v}: {out}"
                );
                assert!(out == 0 || out.signum() == sample.signum());
            }
            assert_eq!(scale(sample, 0), 0);
        }
        assert_eq!(scale(i16::MIN, gain_q24(50)), -4_096);
        assert_eq!(scale(i16::MAX, gain_q24(50)), 4_096);
    }

    #[test]
    fn a_gain_at_unity_leaves_the_samples_as_they_are() {
        let mut gain = Gain::new(4);
        let mut samples = [i16::MIN, i16::MAX, 3, -3];
        gain.apply(&mut samples);
        assert_eq!(samples, [i16::MIN, i16::MAX, 3, -3]);
    }

    #[test]
    fn a_jump_has_no_ramp() {
        let mut gain = Gain::new(4);
        gain.jump(50);
        assert_eq!(gain.percent(), 50);
        let mut samples = [16_384i16; 4];
        gain.apply(&mut samples);
        assert_eq!(samples, [2_048; 4]);
        gain.jump(0);
        gain.apply(&mut samples);
        assert_eq!(samples, [0; 4]);
    }

    #[test]
    fn a_change_ramps_in_straight_steps_to_its_target() {
        let mut gain = Gain::new(4);
        gain.ramp_to(0);
        let mut samples = [16_000i16; 12];
        gain.apply(&mut samples);
        let lefts: Vec<i16> = samples.iter().step_by(2).copied().collect();
        assert_eq!(lefts, [16_000, 12_000, 8_000, 4_000, 0, 0]);
        assert_eq!(samples[1], 16_000, "both channels get the frame's gain");
        assert_eq!(gain.percent(), 0);
    }

    #[test]
    fn a_ramp_goes_on_across_calls_and_a_new_change_starts_where_it_is() {
        let mut gain = Gain::new(4);
        gain.ramp_to(0);
        let mut first = [16_000i16; 4];
        gain.apply(&mut first);
        assert_eq!(first, [16_000, 16_000, 12_000, 12_000]);
        gain.ramp_to(100);
        let mut second = [16_000i16; 10];
        gain.apply(&mut second);
        let lefts: Vec<i16> = second.iter().step_by(2).copied().collect();
        assert_eq!(lefts, [8_000, 10_000, 12_000, 14_000, 16_000]);
    }

    #[test]
    fn a_change_to_the_same_volume_has_no_ramp() {
        let mut gain = Gain::new(4);
        gain.ramp_to(100);
        let mut samples = [i16::MAX; 2];
        gain.apply(&mut samples);
        assert_eq!(samples, [i16::MAX; 2]);
    }
}
