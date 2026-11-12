//! Devices that may be missing on a machine: a run goes on without them.

use neptron_backend::AudioSink;

/// The rate the silent sink reports, in hertz.
pub const NULL_SINK_RATE_HZ: u32 = 48_000;

/// An audio sink, or none: a machine with no usable sound device still
/// shows the picture and reads the input, so a run does not fail on it.
#[derive(Debug)]
pub enum MaybeSink<S> {
    /// The sound device.
    Device(S),
    /// No sound device: every sample is dropped, and the queue stays empty.
    Silent,
}

impl<S: AudioSink> AudioSink for MaybeSink<S> {
    fn rate_hz(&self) -> u32 {
        match self {
            Self::Device(sink) => sink.rate_hz(),
            Self::Silent => NULL_SINK_RATE_HZ,
        }
    }

    fn queued_frames(&self) -> usize {
        match self {
            Self::Device(sink) => sink.queued_frames(),
            Self::Silent => 0,
        }
    }

    fn write(&mut self, stereo: &[i16]) -> usize {
        match self {
            Self::Device(sink) => sink.write(stereo),
            Self::Silent => stereo.len() / 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;

    impl AudioSink for Fixed {
        fn rate_hz(&self) -> u32 {
            44_100
        }
        fn queued_frames(&self) -> usize {
            7
        }
        fn write(&mut self, _stereo: &[i16]) -> usize {
            1
        }
    }

    #[test]
    fn the_silent_sink_takes_every_frame_and_queues_none() {
        let mut sink = MaybeSink::<Fixed>::Silent;
        assert_eq!(sink.rate_hz(), NULL_SINK_RATE_HZ);
        assert_eq!(
            sink.write(&[1, 2, 3, 4, 5]),
            2,
            "an odd last sample is not taken"
        );
        assert_eq!(sink.queued_frames(), 0);
    }

    #[test]
    fn a_device_sink_passes_through() {
        let mut sink = MaybeSink::Device(Fixed);
        assert_eq!(sink.rate_hz(), 44_100);
        assert_eq!(sink.queued_frames(), 7);
        assert_eq!(sink.write(&[0; 8]), 1);
    }
}
