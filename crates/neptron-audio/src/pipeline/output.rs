//! Sample conversion and fades.

/// Full scale of a 16-bit sample.
const FULL_SCALE: f32 = 32_768.0;

/// A 16-bit sample as a float in `[-1, 1)`.
pub(super) fn to_float(sample: i16) -> f32 {
    f32::from(sample) / FULL_SCALE
}

/// A float as a 16-bit sample, rounded and clipped.
pub(super) fn to_i16(value: f32) -> i16 {
    // `as` saturates at the i16 limits and maps NaN to 0.
    (value * FULL_SCALE).round() as i16
}

/// Converts interleaved stereo `output` to `samples`, with a linear fade-in
/// over `fade_frames` frames of which `fade_in_left` are still to come.
/// Returns the fade frames left after these frames.
pub(super) fn convert(
    output: &[f32],
    samples: &mut [i16],
    fade_frames: usize,
    mut fade_in_left: usize,
) -> usize {
    for (from, to) in output.chunks_exact(2).zip(samples.chunks_exact_mut(2)) {
        let gain = if fade_in_left > 0 {
            let done = fade_frames - fade_in_left;
            fade_in_left -= 1;
            done as f32 / fade_frames as f32
        } else {
            1.0
        };
        to[0] = to_i16(from[0] * gain);
        to[1] = to_i16(from[1] * gain);
    }
    fade_in_left
}

/// Writes into `samples` a linear fade from `last` to zero over
/// `fade_frames` frames, the last of them zero. Returns the frames written.
pub(super) fn fade_out(last: [i16; 2], samples: &mut [i16], fade_frames: usize) -> usize {
    let frames = fade_frames.min(samples.len() / 2);
    for (index, to) in samples.chunks_exact_mut(2).take(frames).enumerate() {
        let left = frames - 1 - index;
        for (channel, value) in to.iter_mut().zip(last) {
            let scaled = i32::from(value) * left as i32 / frames as i32;
            // `scaled` lies between 0 and `value`, so it fits an i16.
            *channel = i16::try_from(scaled).unwrap_or(0);
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_round_trip_through_float() {
        for sample in [i16::MIN, -12_345, -1, 0, 1, 12_345, i16::MAX] {
            assert_eq!(to_i16(to_float(sample)), sample);
        }
        assert_eq!(to_i16(2.0), i16::MAX);
        assert_eq!(to_i16(-2.0), i16::MIN);
        assert_eq!(to_i16(f32::NAN), 0);
    }

    #[test]
    fn the_fade_in_rises_linearly_from_zero() {
        let output = [0.5f32; 16];
        let mut samples = [0i16; 16];
        let left = convert(&output, &mut samples, 4, 4);
        assert_eq!(left, 0);
        let lefts: Vec<i16> = samples.iter().step_by(2).copied().collect();
        assert_eq!(
            lefts,
            [0, 4_096, 8_192, 12_288, 16_384, 16_384, 16_384, 16_384]
        );
        assert_eq!(samples[1], 0);
        assert_eq!(samples[3], 4_096);
    }

    #[test]
    fn a_fade_in_continues_across_calls() {
        let output = [0.5f32; 4];
        let mut samples = [0i16; 4];
        assert_eq!(convert(&output, &mut samples, 4, 4), 2);
        assert_eq!(samples, [0, 0, 4_096, 4_096]);
        assert_eq!(convert(&output, &mut samples, 4, 2), 0);
        assert_eq!(samples, [8_192, 8_192, 12_288, 12_288]);
    }

    #[test]
    fn the_fade_out_falls_linearly_to_zero() {
        let mut samples = [7i16; 10];
        assert_eq!(fade_out([1_000, -1_000], &mut samples, 4), 4);
        assert_eq!(&samples[..8], [750, -750, 500, -500, 250, -250, 0, 0]);
        assert_eq!(&samples[8..], [7, 7]);
    }

    #[test]
    fn the_fade_out_fits_the_buffer() {
        let mut samples = [7i16; 4];
        assert_eq!(fade_out([i16::MIN, i16::MAX], &mut samples, 100), 2);
        assert_eq!(samples, [-16_384, 16_383, 0, 0]);
    }
}
