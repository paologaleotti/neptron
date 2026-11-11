//! Geometry and timing of a core, converted from the floating-point values
//! of libretro to the integer contract types, with every value checked.

/// The largest width or height of a frame, in pixels.
pub const MAX_DIMENSION_PX: u32 = 4096;
/// The largest display aspect ratio, in thousandths (100:1).
pub const MAX_ASPECT_MILLI: u32 = 100_000;
/// The lowest frame rate, in millihertz (1 Hz).
pub const MIN_FPS_MHZ: u32 = 1_000;
/// The highest frame rate, in millihertz (1,000 Hz).
pub const MAX_FPS_MHZ: u32 = 1_000_000;
/// The lowest audio sample rate, in millihertz (1 Hz).
pub const MIN_SAMPLE_RATE_MHZ: u32 = 1_000;
/// The highest audio sample rate, in millihertz (384 kHz).
pub const MAX_SAMPLE_RATE_MHZ: u32 = 384_000_000;

/// The picture size of a core: the base size of the frames, the largest
/// size the core can send, and the display aspect ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameGeometry {
    /// Width of the frames, in pixels.
    pub base_width: u16,
    /// Height of the frames, in lines.
    pub base_height: u16,
    /// The largest width of a frame.
    pub max_width: u16,
    /// The largest height of a frame.
    pub max_height: u16,
    /// Display aspect ratio, width over height, in thousandths.
    pub aspect_milli: u32,
}

/// The frame rate and the audio sample rate of a core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timing {
    /// Frames per second, in millihertz.
    pub fps_mhz: u32,
    /// Audio sample rate, in millihertz. Cores report fractions of a
    /// hertz.
    pub sample_rate_mhz: u32,
}

/// Geometry and timing together, as `retro_system_av_info` gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvInfo {
    /// The picture size.
    pub geometry: GameGeometry,
    /// The rates.
    pub timing: Timing,
}

/// Why a geometry or a timing from a core was refused.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum GeometryError {
    /// A width or a height is 0.
    #[error("size {width}x{height} has a zero side")]
    ZeroSize {
        /// The width given.
        width: u32,
        /// The height given.
        height: u32,
    },
    /// A width or a height is above [`MAX_DIMENSION_PX`].
    #[error("size {width}x{height} is above the limit of {limit} pixels")]
    TooLarge {
        /// The width given.
        width: u32,
        /// The height given.
        height: u32,
        /// The limit.
        limit: u32,
    },
    /// The base size is larger than the maximum size.
    #[error("base size {base_width}x{base_height} is above the maximum {max_width}x{max_height}")]
    BaseOverMax {
        /// The base width.
        base_width: u32,
        /// The base height.
        base_height: u32,
        /// The maximum width.
        max_width: u32,
        /// The maximum height.
        max_height: u32,
    },
    /// The aspect ratio rounds to 0 or is above [`MAX_ASPECT_MILLI`].
    #[error("aspect ratio {aspect} is outside 0.001 to {max}", max = MAX_ASPECT_MILLI / 1000)]
    Aspect {
        /// The aspect ratio given.
        aspect: f32,
    },
    /// The frame rate is not finite or outside the limits.
    #[error("frame rate {fps} Hz is outside {min} to {max} mHz", min = MIN_FPS_MHZ, max = MAX_FPS_MHZ)]
    FrameRate {
        /// The frame rate given, in hertz.
        fps: f64,
    },
    /// The sample rate is not finite or outside the limits.
    #[error("sample rate {rate} Hz is outside {min} to {max} mHz", min = MIN_SAMPLE_RATE_MHZ, max = MAX_SAMPLE_RATE_MHZ)]
    SampleRate {
        /// The sample rate given, in hertz.
        rate: f64,
    },
}

impl GameGeometry {
    /// Checks and converts the fields of a `retro_game_geometry`.
    ///
    /// An aspect ratio that is 0 or less, NaN or infinite means "width over
    /// height", as libretro.h says for 0 or less. Any other aspect is
    /// multiplied by 1000 and rounded half away from zero.
    pub fn from_core(
        base_width: u32,
        base_height: u32,
        max_width: u32,
        max_height: u32,
        aspect: f32,
    ) -> Result<Self, GeometryError> {
        let (base_width, base_height) = check_size(base_width, base_height)?;
        let (max_width, max_height) = check_size(max_width, max_height)?;
        if base_width > max_width || base_height > max_height {
            return Err(GeometryError::BaseOverMax {
                base_width: base_width.into(),
                base_height: base_height.into(),
                max_width: max_width.into(),
                max_height: max_height.into(),
            });
        }
        Ok(Self {
            base_width,
            base_height,
            max_width,
            max_height,
            aspect_milli: aspect_milli(aspect, base_width, base_height)?,
        })
    }

    /// The geometry after `SET_GEOMETRY`: a new base size and aspect, the
    /// same maximum. libretro.h says that call cannot change the maximum.
    pub fn resized(
        &self,
        base_width: u32,
        base_height: u32,
        aspect: f32,
    ) -> Result<Self, GeometryError> {
        Self::from_core(
            base_width,
            base_height,
            self.max_width.into(),
            self.max_height.into(),
            aspect,
        )
    }
}

impl Timing {
    /// Checks and converts the fields of a `retro_system_timing`. Each rate
    /// is multiplied by 1000 and rounded half away from zero.
    pub fn from_core(fps: f64, sample_rate: f64) -> Result<Self, GeometryError> {
        let fps_mhz =
            millihertz(fps, MIN_FPS_MHZ, MAX_FPS_MHZ).ok_or(GeometryError::FrameRate { fps })?;
        let sample_rate_mhz = millihertz(sample_rate, MIN_SAMPLE_RATE_MHZ, MAX_SAMPLE_RATE_MHZ)
            .ok_or(GeometryError::SampleRate { rate: sample_rate })?;
        Ok(Self {
            fps_mhz,
            sample_rate_mhz,
        })
    }

    /// Stereo frames of audio per video frame, rounded up.
    pub fn audio_frames_per_run(&self) -> usize {
        let frames = u64::from(self.sample_rate_mhz).div_ceil(u64::from(self.fps_mhz.max(1)));
        usize::try_from(frames).unwrap_or(usize::MAX)
    }
}

fn check_size(width: u32, height: u32) -> Result<(u16, u16), GeometryError> {
    if width == 0 || height == 0 {
        return Err(GeometryError::ZeroSize { width, height });
    }
    let too_large = GeometryError::TooLarge {
        width,
        height,
        limit: MAX_DIMENSION_PX,
    };
    if width > MAX_DIMENSION_PX || height > MAX_DIMENSION_PX {
        return Err(too_large);
    }
    // Both values are at most 4096 here, so they fit.
    Ok((
        u16::try_from(width).map_err(|_| too_large)?,
        u16::try_from(height).map_err(|_| too_large)?,
    ))
}

fn aspect_milli(aspect: f32, width: u16, height: u16) -> Result<u32, GeometryError> {
    if !aspect.is_finite() || aspect <= 0.0 {
        let (w, h) = (u32::from(width), u32::from(height));
        // `h` is at least 1 and `w` at most 4096: no overflow. A very tall
        // frame rounds to 0, which the contract cannot use, so 1 is the floor.
        return Ok(((w * 1000 + h / 2) / h).max(1));
    }
    let milli = (f64::from(aspect) * 1000.0).round();
    if milli < 1.0 || milli > f64::from(MAX_ASPECT_MILLI) {
        return Err(GeometryError::Aspect { aspect });
    }
    // In range, so the cast is exact.
    Ok(milli as u32)
}

fn millihertz(hz: f64, min: u32, max: u32) -> Option<u32> {
    if !hz.is_finite() {
        return None;
    }
    let mhz = (hz * 1000.0).round();
    // In range, so the cast is exact.
    (mhz >= f64::from(min) && mhz <= f64::from(max)).then_some(mhz as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normal_geometry_converts() {
        let g = GameGeometry::from_core(256, 224, 320, 240, 4.0 / 3.0).unwrap();
        assert_eq!(
            g,
            GameGeometry {
                base_width: 256,
                base_height: 224,
                max_width: 320,
                max_height: 240,
                aspect_milli: 1333,
            }
        );
    }

    #[test]
    fn a_missing_aspect_is_width_over_height() {
        for aspect in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let g = GameGeometry::from_core(320, 240, 320, 240, aspect).unwrap();
            assert_eq!(g.aspect_milli, 1333, "aspect {aspect}");
        }
        let g = GameGeometry::from_core(1, 4096, 1, 4096, 0.0).unwrap();
        assert_eq!(g.aspect_milli, 1, "the floor is one thousandth");
    }

    #[test]
    fn aspect_rounds_half_away_from_zero() {
        let g = |a| GameGeometry::from_core(8, 8, 8, 8, a).map(|g| g.aspect_milli);
        assert_eq!(g(1.0625), Ok(1063));
        assert_eq!(g(0.001), Ok(1));
        assert_eq!(g(100.0), Ok(100_000));
        assert_eq!(g(0.0004), Err(GeometryError::Aspect { aspect: 0.0004 }));
        assert_eq!(g(100.001), Err(GeometryError::Aspect { aspect: 100.001 }));
    }

    #[test]
    fn bad_sizes_are_refused() {
        assert_eq!(
            GameGeometry::from_core(0, 224, 320, 240, 0.0),
            Err(GeometryError::ZeroSize {
                width: 0,
                height: 224
            })
        );
        assert_eq!(
            GameGeometry::from_core(256, 224, 320, 0, 0.0),
            Err(GeometryError::ZeroSize {
                width: 320,
                height: 0
            })
        );
        assert_eq!(
            GameGeometry::from_core(256, 224, 4097, 240, 0.0),
            Err(GeometryError::TooLarge {
                width: 4097,
                height: 240,
                limit: 4096
            })
        );
        assert_eq!(
            GameGeometry::from_core(u32::MAX, 224, u32::MAX, 240, 0.0),
            Err(GeometryError::TooLarge {
                width: u32::MAX,
                height: 224,
                limit: 4096
            })
        );
        assert_eq!(
            GameGeometry::from_core(321, 224, 320, 240, 0.0),
            Err(GeometryError::BaseOverMax {
                base_width: 321,
                base_height: 224,
                max_width: 320,
                max_height: 240
            })
        );
    }

    #[test]
    fn resized_keeps_the_maximum() {
        let g = GameGeometry::from_core(256, 224, 320, 240, 0.0).unwrap();
        let r = g.resized(320, 240, 4.0 / 3.0).unwrap();
        assert_eq!((r.base_width, r.base_height), (320, 240));
        assert_eq!((r.max_width, r.max_height), (320, 240));
        assert!(matches!(
            g.resized(320, 241, 0.0),
            Err(GeometryError::BaseOverMax { .. })
        ));
    }

    #[test]
    fn timing_converts_to_millihertz() {
        assert_eq!(
            Timing::from_core(60.0988, 32_040.5),
            Ok(Timing {
                fps_mhz: 60_099,
                sample_rate_mhz: 32_040_500
            })
        );
        assert_eq!(
            Timing::from_core(59.9375, 44_100.0),
            Ok(Timing {
                fps_mhz: 59_938,
                sample_rate_mhz: 44_100_000
            })
        );
        assert_eq!(
            Timing::from_core(1.0, 384_000.0),
            Ok(Timing {
                fps_mhz: 1_000,
                sample_rate_mhz: 384_000_000
            })
        );
    }

    #[test]
    fn bad_rates_are_refused() {
        for fps in [0.0, -60.0, 0.9994, 1000.001, f64::NAN, f64::INFINITY] {
            assert!(
                matches!(
                    Timing::from_core(fps, 48_000.0),
                    Err(GeometryError::FrameRate { .. })
                ),
                "fps {fps}"
            );
        }
        for rate in [0.0, -1.0, 384_000.001, f64::NAN, f64::NEG_INFINITY, 1e300] {
            assert!(
                matches!(
                    Timing::from_core(60.0, rate),
                    Err(GeometryError::SampleRate { .. })
                ),
                "rate {rate}"
            );
        }
    }

    #[test]
    fn audio_frames_per_run_rounds_up() {
        let t = Timing::from_core(59.94, 44_100.0).unwrap();
        assert_eq!(t.audio_frames_per_run(), 736);
        let t = Timing::from_core(50.0, 48_000.0).unwrap();
        assert_eq!(t.audio_frames_per_run(), 960);
    }
}
