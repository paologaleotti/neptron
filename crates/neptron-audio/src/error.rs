/// An error of the audio pipeline. Each variant carries the value and the
/// limit it broke.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
    /// The core's sample rate is outside the range the pipeline takes.
    #[error("core sample rate {sample_rate_mhz} mHz is outside {min_mhz} to {max_mhz} mHz")]
    CoreSampleRate {
        /// The rate the core gave, in millihertz.
        sample_rate_mhz: u64,
        /// The lowest rate the pipeline takes, in millihertz.
        min_mhz: u64,
        /// The highest rate the pipeline takes, in millihertz.
        max_mhz: u64,
    },
    /// The core's frame rate is outside the range the pipeline takes.
    #[error("core frame rate {frame_rate_mhz} mHz is outside {min_mhz} to {max_mhz} mHz")]
    CoreFrameRate {
        /// The rate the core gave, in millihertz.
        frame_rate_mhz: u64,
        /// The lowest rate the pipeline takes, in millihertz.
        min_mhz: u64,
        /// The highest rate the pipeline takes, in millihertz.
        max_mhz: u64,
    },
    /// The sink's rate is outside the range the pipeline takes.
    #[error("device rate {rate_hz} Hz is outside {min_hz} to {max_hz} Hz")]
    DeviceRate {
        /// The rate of the sink, in hertz.
        rate_hz: u32,
        /// The lowest rate the pipeline takes, in hertz.
        min_hz: u32,
        /// The highest rate the pipeline takes, in hertz.
        max_hz: u32,
    },
    /// The run rate is less than half or more than twice the core's frame
    /// rate.
    #[error(
        "run rate {run_rate_mhz} mHz is outside half to twice the core frame rate {frame_rate_mhz} mHz"
    )]
    RunRatio {
        /// The run rate that was asked for, in millihertz.
        run_rate_mhz: u64,
        /// The core's frame rate, in millihertz.
        frame_rate_mhz: u64,
    },
    /// rubato refused to make the resampler.
    #[error("the resampler for ratio {ratio} cannot be made: {reason}")]
    Resampler {
        /// The ratio of output to input frames it was made for.
        ratio: String,
        /// rubato's reason.
        reason: String,
    },
}
