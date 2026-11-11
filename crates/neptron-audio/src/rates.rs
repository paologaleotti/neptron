//! The rates of the core, the run and the device, and the nominal ratio
//! that joins them.

use crate::AudioError;

/// The lowest core sample rate the pipeline takes: 8 kHz, in millihertz.
pub const MIN_CORE_SAMPLE_RATE_MHZ: u64 = 8_000_000;
/// The highest core sample rate the pipeline takes: 384 kHz, in millihertz.
pub const MAX_CORE_SAMPLE_RATE_MHZ: u64 = 384_000_000;
/// The lowest core frame rate the pipeline takes: 10 Hz, in millihertz.
pub const MIN_CORE_FRAME_RATE_MHZ: u64 = 10_000;
/// The highest core frame rate the pipeline takes: 1 kHz, in millihertz.
pub const MAX_CORE_FRAME_RATE_MHZ: u64 = 1_000_000;
/// The lowest device rate the pipeline takes, in hertz.
pub const MIN_DEVICE_RATE_HZ: u32 = 8_000;
/// The highest device rate the pipeline takes, in hertz.
pub const MAX_DEVICE_RATE_HZ: u32 = 384_000;

/// The audio timing a core reports (libretro `retro_system_timing`), in
/// millihertz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreTiming {
    /// The core's sample rate, for example 32 040 500 for 32 040.5 Hz.
    pub sample_rate_mhz: u64,
    /// The core's frame rate, for example 60 099 for 60.0988 Hz, rounded.
    pub frame_rate_mhz: u64,
}

impl CoreTiming {
    /// Checks both rates against the limits of the pipeline.
    ///
    /// # Errors
    ///
    /// [`AudioError::CoreSampleRate`] or [`AudioError::CoreFrameRate`] for
    /// a rate outside its limits.
    pub fn check(&self) -> Result<(), AudioError> {
        if !(MIN_CORE_SAMPLE_RATE_MHZ..=MAX_CORE_SAMPLE_RATE_MHZ).contains(&self.sample_rate_mhz) {
            return Err(AudioError::CoreSampleRate {
                sample_rate_mhz: self.sample_rate_mhz,
                min_mhz: MIN_CORE_SAMPLE_RATE_MHZ,
                max_mhz: MAX_CORE_SAMPLE_RATE_MHZ,
            });
        }
        if !(MIN_CORE_FRAME_RATE_MHZ..=MAX_CORE_FRAME_RATE_MHZ).contains(&self.frame_rate_mhz) {
            return Err(AudioError::CoreFrameRate {
                frame_rate_mhz: self.frame_rate_mhz,
                min_mhz: MIN_CORE_FRAME_RATE_MHZ,
                max_mhz: MAX_CORE_FRAME_RATE_MHZ,
            });
        }
        Ok(())
    }

    /// Checks that `run_rate_mhz`, the rate at which the session runs the
    /// core, is between half and twice the core's frame rate.
    ///
    /// # Errors
    ///
    /// [`AudioError::RunRatio`] outside that range.
    pub fn check_run_rate(&self, run_rate_mhz: u64) -> Result<(), AudioError> {
        let run = u128::from(run_rate_mhz);
        let core = u128::from(self.frame_rate_mhz);
        if 2 * run < core || run > 2 * core {
            return Err(AudioError::RunRatio {
                run_rate_mhz,
                frame_rate_mhz: self.frame_rate_mhz,
            });
        }
        Ok(())
    }

    /// The nominal ratio of output frames to input frames for a device at
    /// `device_rate_hz` and the run rate `run_rate_mhz`:
    /// `f_dev * f_core / (f_in * f_run)`. The audio arrives at `f_in *
    /// f_run / f_core` in real time, as in RetroArch's
    /// `audio_driver_monitor_adjust_system_rates`.
    pub fn nominal_ratio(&self, device_rate_hz: u32, run_rate_mhz: u64) -> f64 {
        // Millihertz to hertz on the device side keeps the units equal.
        let output = f64::from(device_rate_hz) * 1_000.0 * self.frame_rate_mhz as f64;
        let input = self.sample_rate_mhz as f64 * run_rate_mhz as f64;
        output / input
    }
}

/// Checks the rate of a sink against the limits of the pipeline.
///
/// # Errors
///
/// [`AudioError::DeviceRate`] outside them.
pub fn check_device_rate(rate_hz: u32) -> Result<(), AudioError> {
    if !(MIN_DEVICE_RATE_HZ..=MAX_DEVICE_RATE_HZ).contains(&rate_hz) {
        return Err(AudioError::DeviceRate {
            rate_hz,
            min_hz: MIN_DEVICE_RATE_HZ,
            max_hz: MAX_DEVICE_RATE_HZ,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNES: CoreTiming = CoreTiming {
        sample_rate_mhz: 32_040_500,
        frame_rate_mhz: 60_099,
    };

    #[test]
    fn the_nominal_ratio_is_device_over_input_at_the_core_rate() {
        let ratio = SNES.nominal_ratio(48_000, 60_099);
        assert!((ratio - 48_000.0 / 32_040.5).abs() < 1e-12, "{ratio}");
    }

    #[test]
    fn a_faster_run_takes_fewer_output_frames_per_input_frame() {
        // A 50 Hz game run at 60 Hz: the audio arrives 1.2 times faster.
        let pal = CoreTiming {
            sample_rate_mhz: 44_100_000,
            frame_rate_mhz: 50_000,
        };
        let ratio = pal.nominal_ratio(48_000, 60_000);
        assert!(
            (ratio - 48_000.0 / (44_100.0 * 1.2)).abs() < 1e-12,
            "{ratio}"
        );
    }

    #[test]
    fn rates_outside_the_limits_are_refused_with_the_limit() {
        assert_eq!(SNES.check(), Ok(()));
        let slow = CoreTiming {
            sample_rate_mhz: 7_999_999,
            ..SNES
        };
        assert_eq!(
            slow.check(),
            Err(AudioError::CoreSampleRate {
                sample_rate_mhz: 7_999_999,
                min_mhz: MIN_CORE_SAMPLE_RATE_MHZ,
                max_mhz: MAX_CORE_SAMPLE_RATE_MHZ,
            })
        );
        let fast = CoreTiming {
            sample_rate_mhz: 384_000_001,
            ..SNES
        };
        assert!(matches!(
            fast.check(),
            Err(AudioError::CoreSampleRate { .. })
        ));
        let zero_fps = CoreTiming {
            frame_rate_mhz: 0,
            ..SNES
        };
        assert_eq!(
            zero_fps.check(),
            Err(AudioError::CoreFrameRate {
                frame_rate_mhz: 0,
                min_mhz: MIN_CORE_FRAME_RATE_MHZ,
                max_mhz: MAX_CORE_FRAME_RATE_MHZ,
            })
        );
        assert_eq!(check_device_rate(48_000), Ok(()));
        assert_eq!(
            check_device_rate(0),
            Err(AudioError::DeviceRate {
                rate_hz: 0,
                min_hz: MIN_DEVICE_RATE_HZ,
                max_hz: MAX_DEVICE_RATE_HZ,
            })
        );
        assert_eq!(
            AudioError::DeviceRate {
                rate_hz: 0,
                min_hz: 8_000,
                max_hz: 384_000
            }
            .to_string(),
            "device rate 0 Hz is outside 8000 to 384000 Hz"
        );
    }

    #[test]
    fn the_run_rate_is_between_half_and_twice_the_core_rate() {
        let core = CoreTiming {
            sample_rate_mhz: 48_000_000,
            frame_rate_mhz: 60_000,
        };
        assert_eq!(core.check_run_rate(30_000), Ok(()));
        assert_eq!(core.check_run_rate(120_000), Ok(()));
        assert_eq!(core.check_run_rate(50_000), Ok(()));
        assert_eq!(
            core.check_run_rate(29_999),
            Err(AudioError::RunRatio {
                run_rate_mhz: 29_999,
                frame_rate_mhz: 60_000
            })
        );
        assert!(core.check_run_rate(120_001).is_err());
        assert!(core.check_run_rate(u64::MAX).is_err());
    }
}
