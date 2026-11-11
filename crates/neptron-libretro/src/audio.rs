//! The audio of one frame, in a buffer made at load and at a timing change.

use crate::geometry::Timing;

/// The smallest audio buffer, in stereo frames.
const MIN_AUDIO_FRAMES: usize = 4096;
/// The buffer holds this many frames' worth of audio. Cores send uneven
/// amounts per frame; four times the mean leaves room for that.
const AUDIO_HEADROOM: usize = 4;

/// Interleaved stereo samples of the current frame.
#[derive(Debug)]
pub(crate) struct AudioBuffer {
    samples: Vec<i16>,
    /// The most samples (not frames) that the buffer takes.
    limit: usize,
    dropped_frames: usize,
}

impl AudioBuffer {
    pub(crate) fn new() -> Self {
        Self {
            samples: Vec::new(),
            limit: 0,
            dropped_frames: 0,
        }
    }

    /// Makes room for the audio of one frame at `timing`. It never shrinks.
    /// The error is the number of bytes that could not be allocated.
    pub(crate) fn reserve_for(&mut self, timing: &Timing) -> Result<(), usize> {
        let frames = timing
            .audio_frames_per_run()
            .saturating_mul(AUDIO_HEADROOM)
            .max(MIN_AUDIO_FRAMES);
        let limit = frames.saturating_mul(2);
        if limit > self.limit {
            let more = limit - self.samples.len();
            self.samples
                .try_reserve_exact(more)
                .map_err(|_| limit.saturating_mul(size_of::<i16>()))?;
            self.limit = limit;
        }
        Ok(())
    }

    pub(crate) fn clear(&mut self) {
        self.samples.clear();
        self.dropped_frames = 0;
    }

    /// Stereo frames that still fit.
    pub(crate) fn room_frames(&self) -> usize {
        self.limit.saturating_sub(self.samples.len()) / 2
    }

    /// Appends whole stereo frames that fit, and drops the rest.
    pub(crate) fn push(&mut self, interleaved: &[i16]) {
        let frames = interleaved.len() / 2;
        let take = frames.min(self.room_frames());
        self.samples
            .extend_from_slice(interleaved.get(..take * 2).unwrap_or_default());
        self.drop_frames(frames - take);
    }

    pub(crate) fn drop_frames(&mut self, frames: usize) {
        self.dropped_frames = self.dropped_frames.saturating_add(frames);
    }

    pub(crate) fn samples(&self) -> &[i16] {
        &self.samples
    }

    pub(crate) fn dropped_frames(&self) -> usize {
        self.dropped_frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing(fps_mhz: u32, sample_rate_mhz: u32) -> Timing {
        Timing {
            fps_mhz,
            sample_rate_mhz,
        }
    }

    #[test]
    fn audio_fits_until_the_limit_without_growing() {
        let mut a = AudioBuffer::new();
        a.reserve_for(&timing(60_000, 48_000_000)).unwrap();
        assert_eq!(a.room_frames(), 4096);
        let capacity = a.samples.capacity();
        a.push(&[1, -1, 2, -2]);
        assert_eq!(a.samples(), [1, -1, 2, -2]);
        let big = vec![0i16; 2 * 5000];
        a.push(&big);
        assert_eq!(a.samples().len(), 2 * 4096);
        assert_eq!(a.dropped_frames(), 5000 - 4094);
        assert_eq!(a.samples.capacity(), capacity, "no allocation");
        a.clear();
        assert_eq!(a.dropped_frames(), 0);
        assert_eq!(a.room_frames(), 4096);
    }

    #[test]
    fn a_high_rate_makes_a_larger_buffer() {
        let mut a = AudioBuffer::new();
        a.reserve_for(&timing(1_000, 384_000_000)).unwrap();
        assert_eq!(a.room_frames(), 4 * 384_000);
        a.reserve_for(&timing(60_000, 48_000_000)).unwrap();
        assert_eq!(a.room_frames(), 4 * 384_000, "it never shrinks");
    }

    #[test]
    fn an_odd_sample_count_drops_the_half_frame() {
        let mut a = AudioBuffer::new();
        a.reserve_for(&timing(60_000, 48_000_000)).unwrap();
        a.push(&[1, 2, 3]);
        assert_eq!(a.samples(), [1, 2]);
    }
}
