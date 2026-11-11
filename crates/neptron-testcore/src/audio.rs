//! The audio of the test core: a known sample pattern and the number of
//! stereo frames per video frame.

#![forbid(unsafe_code)]

use crate::script::Timing;

/// The stereo sample with this index, counted from 0 at load or reset.
///
/// The left channel is the low 16 bits of the index, as a signed value: a
/// ramp that wraps every 65,536 samples. The right channel is its negation,
/// with wrapping, so a swap of the channels shows.
pub fn sample(index: u64) -> [i16; 2] {
    let left = index as u16 as i16;
    [left, left.wrapping_neg()]
}

/// The number of stereo frames the core has sent after `frames` video frames
/// at a constant `timing`: `frames * sample_rate / fps`, rounded down.
///
/// The core sends the difference of two such counts in each frame, so the
/// count never drifts from the exact rate.
pub fn frames_after(frames: u64, timing: Timing) -> u64 {
    let total = u128::from(frames) * u128::from(timing.sample_rate_hz) * MHZ_PER_HZ
        / u128::from(timing.fps_mhz.max(1));
    u64::try_from(total).unwrap_or(u64::MAX)
}

/// The largest number of stereo frames that one video frame at `timing` has.
pub fn max_frames_per_run(timing: Timing) -> usize {
    let per_frame = (u128::from(timing.sample_rate_hz) * MHZ_PER_HZ)
        .div_ceil(u128::from(timing.fps_mhz.max(1)));
    usize::try_from(per_frame).unwrap_or(usize::MAX)
}

const MHZ_PER_HZ: u128 = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    const NTSC: Timing = Timing {
        fps_mhz: 59_940,
        sample_rate_hz: 44_100,
    };

    #[test]
    fn samples_ramp_and_wrap() {
        assert_eq!(sample(0), [0, 0]);
        assert_eq!(sample(1), [1, -1]);
        assert_eq!(sample(0x7FFF), [i16::MAX, -i16::MAX]);
        assert_eq!(sample(0x8000), [i16::MIN, i16::MIN]);
        assert_eq!(sample(0xFFFF), [-1, 1]);
        assert_eq!(sample(0x1_0000), sample(0));
    }

    #[test]
    fn frame_count_follows_the_exact_rate() {
        let even = Timing {
            fps_mhz: 60_000,
            sample_rate_hz: 48_000,
        };
        assert_eq!(frames_after(1, even), 800);
        assert_eq!(frames_after(60, even), 48_000);
        // 44,100 / 59.94 = 735.735...: 59,940 frames take exactly 44,100 s.
        assert_eq!(frames_after(59_940, NTSC), 44_100 * 1000);
        assert_eq!(frames_after(1, NTSC), 735);
        assert_eq!(frames_after(4, NTSC), 2942);
    }

    #[test]
    fn per_frame_counts_stay_within_the_maximum() {
        let max = max_frames_per_run(NTSC);
        assert_eq!(max, 736);
        for n in 0..10_000 {
            let count = frames_after(n + 1, NTSC) - frames_after(n, NTSC);
            assert!(count == 735 || count == 736, "frame {n}: {count}");
            assert!(usize::try_from(count).unwrap() <= max);
        }
    }

    #[test]
    fn huge_frame_counts_saturate() {
        let fast = Timing {
            fps_mhz: 1,
            sample_rate_hz: 384_000,
        };
        assert_eq!(frames_after(u64::MAX, fast), u64::MAX);
    }
}
