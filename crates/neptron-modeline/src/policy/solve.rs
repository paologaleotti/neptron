use neptron_types::{CrtRange, GpuCaps, MonitorProfile, Rotation, SourceGeometry};

use super::crt::Crt;
use super::padding::padding;
use super::placement::{Vertical, crt_placement};
use super::settings::Effective;
use super::shape::Shape;
use super::{
    Dropped, HorizontalScale, HorizontalStrategy, OrientedSource, Proposal, REFRESH_MATCH_MHZ,
    RefreshTarget, Rung, Timing,
};
use crate::{Candidate, ModelineError, modelines_with};

/// The most Switchres calls the dynamic super width makes for one
/// candidate. Each step at least adds one to the multiple, and the estimate
/// usually lands in one step, so the bound is never reached on a range
/// whose pixel clock grows with the width.
const DYNAMIC_STEPS_MAX: usize = 16;

/// The aspect sent with every request: Switchres takes every source as 4:3
/// and does not read it.
const REQUEST_ASPECT_MILLI: u32 = 1_333;

/// The outcome of step 5 for one candidate: a proposal, a reason to drop
/// it, or an error of the profile that stops the whole plan.
pub(crate) type Solved = Result<Result<Proposal, Dropped>, ModelineError>;

/// Step 5: ask Switchres for the modeline of the shape at the target rate,
/// with the width that `strategy` gives.
///
/// The policy computes the width, so Switchres gets no pixel clock floor
/// and never doubles the width itself: the reason for the width is then
/// the policy's own rule. Of the modes Switchres returns, the first that
/// keeps the requested lines, scan and width, and reaches the target rate,
/// is the proposal. `enforce_refresh` is false only for the last resort.
///
/// The pixel clock is known only once Switchres gives the line rate, so a
/// native or fixed width below the GPU floor is dropped right after the
/// call, before a proposal is built (`PixelClockFloor`). The same holds
/// for a source wider than the GPU, at the GPU's widest mode.
pub(crate) fn solve(
    shape: &Shape,
    target: RefreshTarget,
    enforce_refresh: bool,
    strategy: HorizontalStrategy,
    crt: &Crt<'_>,
) -> Solved {
    let caps = crt.caps;
    let ask = Ask {
        shape,
        target,
        enforce_refresh,
        settings: &crt.settings,
        profile: crt.profile,
        caps,
    };
    let picked = match strategy {
        _ if shape.base_width > caps.max_width => ask.at_floor(ask.fixed(caps.max_width)?),
        HorizontalStrategy::Native => ask.at_floor(ask.multiple(1)?),
        HorizontalStrategy::DynamicSuperWidth => ask.dynamic()?,
        HorizontalStrategy::FixedSuperWidth { width } => ask.at_floor(ask.fixed(width)?),
    };
    Ok(picked.map(|(candidate, horizontal)| {
        proposal(
            shape,
            target,
            &crt.source,
            crt.ranges,
            candidate,
            horizontal,
        )
    }))
}

struct Ask<'a> {
    shape: &'a Shape,
    target: RefreshTarget,
    enforce_refresh: bool,
    settings: &'a Effective,
    profile: &'a MonitorProfile,
    caps: &'a GpuCaps,
}

type Picked = Result<(Candidate, HorizontalScale), Dropped>;

impl Ask<'_> {
    fn multiple(&self, k: u16) -> Result<Picked, ModelineError> {
        let width = u32::from(self.shape.base_width) * u32::from(k);
        let width = u16::try_from(width).unwrap_or(u16::MAX);
        Ok(self.call(width)?.map(|c| (c, HorizontalScale::Multiple(k))))
    }

    fn fixed(&self, width: u16) -> Result<Picked, ModelineError> {
        Ok(self
            .call(width)?
            .map(|c| (c, HorizontalScale::Fixed(width))))
    }

    /// Drops a mode whose pixel clock is below the GPU floor, with the
    /// numbers, so the caller can go on with the dynamic super width.
    fn at_floor(&self, picked: Picked) -> Picked {
        let (candidate, horizontal) = picked?;
        let pclock_hz = candidate.modeline.pclock_hz;
        if pclock_hz < self.caps.pclock_min_hz {
            return Err(Dropped::PixelClockFloor {
                width: u32::from(candidate.modeline.hactive),
                pclock_hz,
                floor_hz: self.caps.pclock_min_hz,
                max_width: self.caps.max_width,
            });
        }
        Ok((candidate, horizontal))
    }

    /// The smallest multiple of the base width whose pixel clock is at
    /// least the GPU floor, the bound the validator checks. The pixel clock
    /// grows with the width at a fixed line rate, so the next multiple to
    /// try is estimated from the last one.
    fn dynamic(&self) -> Result<Picked, ModelineError> {
        let floor = self.caps.pclock_min_hz;
        let base = u32::from(self.shape.base_width);
        let mut k: u32 = 1;
        let mut last = (base, 0);
        for _ in 0..DYNAMIC_STEPS_MAX {
            let Some(width) = multiple_width(self.shape.base_width, k)
                .filter(|&w| w <= u32::from(self.caps.max_width))
            else {
                break;
            };
            let multiple = u16::try_from(k).unwrap_or(u16::MAX);
            let (candidate, horizontal) = match self.multiple(multiple)? {
                Ok(found) => found,
                Err(dropped) => return Ok(Err(dropped)),
            };
            let pclock = candidate.modeline.pclock_hz;
            if pclock >= floor {
                return Ok(Ok((candidate, horizontal)));
            }
            last = (width, pclock);
            let estimate = (u64::from(k) * u64::from(floor)).div_ceil(u64::from(pclock.max(1)));
            k = u32::try_from(estimate).unwrap_or(u32::MAX).max(k + 1);
        }
        Ok(Err(Dropped::PixelClockFloor {
            width: last.0,
            pclock_hz: last.1,
            floor_hz: floor,
            max_width: self.caps.max_width,
        }))
    }

    fn call(&self, width: u16) -> Result<Result<Candidate, Dropped>, ModelineError> {
        let request = SourceGeometry {
            width,
            height: self.shape.lines,
            refresh_mhz: self.target.rate_mhz,
            aspect_milli: REQUEST_ASPECT_MILLI,
            interlaced: self.shape.interlace,
            rotation: Rotation::R0,
        };
        let caps = GpuCaps {
            pclock_min_hz: 0,
            ..*self.caps
        };
        let list = modelines_with(&request, self.profile, &caps, &self.settings.solver)?;
        Ok(self.pick(&list, width))
    }

    /// The first mode that passes every check, or the reason the first one
    /// fails.
    fn pick(&self, list: &[Candidate], width: u16) -> Result<Candidate, Dropped> {
        let mut first = None;
        for candidate in list {
            match self.check(candidate, width) {
                Ok(()) => return Ok(*candidate),
                Err(dropped) => {
                    first.get_or_insert(dropped);
                }
            }
        }
        Err(first.unwrap_or(Dropped::NoMode))
    }

    fn check(&self, candidate: &Candidate, width: u16) -> Result<(), Dropped> {
        let mode = &candidate.modeline;
        let shape = self.shape;
        if shape.rung != Rung::Fitted {
            if mode.vactive != shape.lines {
                return Err(Dropped::LinesChanged {
                    asked: shape.lines,
                    got: mode.vactive,
                });
            }
            // An interlaced source on a monitor with no interlaced range is
            // shown progressive: 480i as 480p.
            let deinterlaced = shape.rung == Rung::Native && shape.interlace && !mode.interlace;
            if mode.interlace != shape.interlace && !deinterlaced {
                return Err(Dropped::ScanChanged {
                    asked_interlace: shape.interlace,
                });
            }
            if mode.hactive != width {
                return Err(Dropped::WidthChanged {
                    asked: width,
                    got: mode.hactive,
                });
            }
        }
        let field = mode.field_rate_mhz();
        let target = u64::from(self.target.rate_mhz);
        if self.enforce_refresh && field.abs_diff(target) >= REFRESH_MATCH_MHZ {
            return Err(Dropped::RefreshMissed {
                target_mhz: self.target.rate_mhz,
                got_mhz: field,
            });
        }
        Ok(())
    }
}

/// `base` times `k`, or `None` when the product does not fit a `u32`: the
/// search then stops, as for a width above the GPU's widest mode.
fn multiple_width(base: u16, k: u32) -> Option<u32> {
    u32::from(base).checked_mul(k)
}

fn proposal(
    shape: &Shape,
    target: RefreshTarget,
    source: &OrientedSource,
    ranges: &[CrtRange],
    candidate: Candidate,
    horizontal: HorizontalScale,
) -> Proposal {
    let mode = candidate.modeline;
    let vertical = if shape.rung == Rung::Fitted {
        Vertical::Fill
    } else {
        Vertical::Exact {
            lines: shape.game_lines,
            factor: shape.factor,
        }
    };
    let padded = match ranges.get(candidate.range_index) {
        Some(range) if shape.rung != Rung::Fitted => {
            padding(shape.game_lines, &mode, target.rate_mhz, range)
        }
        _ => None,
    };
    Proposal {
        mode,
        source_index: candidate.range_index,
        horizontal: Some(horizontal),
        placement: crt_placement(source, &mode, vertical, horizontal, shape.base_width),
        timing: Timing {
            game_mhz: source.refresh_mhz,
            target,
            line_rate_mhz: mode.line_rate_mhz(),
            field_rate_mhz: mode.field_rate_mhz(),
        },
        bordered: shape.bordered,
        strategy_replaced: None,
        padding: padded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_multiple_of_the_width_stops_before_an_overflow() {
        assert_eq!(multiple_width(256, 5), Some(1_280));
        assert_eq!(multiple_width(u16::MAX, 65_537), Some(4_294_967_295));
        assert_eq!(multiple_width(u16::MAX, 65_538), None);
        assert_eq!(multiple_width(2, u32::MAX), None);
    }
}
