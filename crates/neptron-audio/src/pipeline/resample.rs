//! The rubato resampler, made once and driven without allocation.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Adjustable, Async, FixedAsync, PolynomialDegree, Resampler, Resizable,
    SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use crate::{AudioError, Resampling};

/// Taps of the sinc filter.
const SINC_TAPS: usize = 128;
/// Points between two input samples at which the sinc filter is computed.
const SINC_OVERSAMPLING: usize = 128;
/// How far the ratio may move from the ratio at construction, either way:
/// the run ratio range (1/2 to 2) times the deviation bound, rounded up.
const RATIO_RANGE: f64 = 2.02;

/// Interleaved stereo.
const CHANNELS: usize = 2;

/// A stereo `Async` resampler with fixed input, whose chunk size is set to
/// the input of each core frame.
pub(super) struct Stereo {
    inner: Async<f32>,
    max_input_frames: usize,
}

impl Stereo {
    /// A resampler for up to `max_input_frames` input frames per call, at
    /// `ratio` output frames per input frame. It allocates every buffer it
    /// needs.
    pub(super) fn new(
        resampling: Resampling,
        ratio: f64,
        max_input_frames: usize,
    ) -> Result<Self, AudioError> {
        let made = match resampling {
            Resampling::Sinc => {
                let parameters =
                    SincInterpolationParameters::new(SINC_TAPS, WindowFunction::BlackmanHarris2)
                        .interpolation(SincInterpolationType::Linear)
                        .oversampling_factor(SINC_OVERSAMPLING);
                Async::new_sinc(
                    ratio,
                    RATIO_RANGE,
                    &parameters,
                    max_input_frames,
                    CHANNELS,
                    FixedAsync::Input,
                )
            }
            Resampling::Polynomial => Async::new_poly(
                ratio,
                RATIO_RANGE,
                PolynomialDegree::Cubic,
                max_input_frames,
                CHANNELS,
                FixedAsync::Input,
            ),
        };
        let inner = made.map_err(|error| AudioError::Resampler {
            ratio: ratio.to_string(),
            reason: error.to_string(),
        })?;
        Ok(Self {
            inner,
            max_input_frames,
        })
    }

    /// The most output frames one call can give.
    pub(super) fn max_output_frames(&self) -> usize {
        self.inner.output_frames_max()
    }

    /// The delay the filter adds, in output frames.
    pub(super) fn delay_frames(&self) -> usize {
        self.inner.output_delay()
    }

    /// Sets the ratio for the next call. The ratio ramps from the old value
    /// over that call, so the pitch has no step.
    pub(super) fn set_ratio(&mut self, ratio: f64) -> Result<(), rubato::ResampleError> {
        self.inner.set_resample_ratio(ratio, true)
    }

    /// Resamples the interleaved `input` into `output` and returns the
    /// output frames written.
    pub(super) fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<usize, rubato::ResampleError> {
        let frames = (input.len() / CHANNELS).min(self.max_input_frames);
        if frames == 0 {
            return Ok(0);
        }
        self.inner.set_chunk_size(frames)?;
        let output_frames = output.len() / CHANNELS;
        let input =
            InterleavedSlice::new(&input[..frames * CHANNELS], CHANNELS, frames).map_err(|_| {
                rubato::ResampleError::InsufficientInputBufferSize {
                    expected: frames,
                    actual: input.len() / CHANNELS,
                }
            })?;
        let mut output =
            InterleavedSlice::new_mut(output, CHANNELS, output_frames).map_err(|_| {
                rubato::ResampleError::InsufficientOutputBufferSize {
                    expected: output_frames,
                    actual: output_frames,
                }
            })?;
        let (_, written) = self.inner.process_into_buffer(&input, &mut output, None)?;
        Ok(written)
    }
}
