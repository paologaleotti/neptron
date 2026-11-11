/// An audio output that plays 16-bit stereo samples.
pub trait AudioSink {
    /// The output sample rate, in hertz.
    fn rate_hz(&self) -> u32;

    /// Stereo frames (one left and one right sample each) that are queued
    /// and not played yet.
    fn queued_frames(&self) -> usize;

    /// Queues interleaved samples, left first, and returns how many stereo
    /// frames it took. It never blocks: with a full queue it takes fewer
    /// frames than it got, or none. An odd last sample is not taken.
    fn write(&mut self, stereo: &[i16]) -> usize;
}
