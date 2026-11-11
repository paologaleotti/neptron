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
    use neptron_types::{Modeline, MonitorKind};

    use super::*;
    use crate::Settings;
    use crate::policy::{PictureStyle, RefreshKind, RefreshPolicy};

    #[test]
    fn a_multiple_of_the_width_stops_before_an_overflow() {
        assert_eq!(multiple_width(256, 5), Some(1_280));
        assert_eq!(multiple_width(u16::MAX, 65_537), Some(4_294_967_295));
        assert_eq!(multiple_width(u16::MAX, 65_538), None);
        assert_eq!(multiple_width(2, u32::MAX), None);
    }

    const SETTINGS: Effective = Effective {
        style: PictureStyle::Auto,
        refresh: RefreshPolicy::Exact,
        horizontal: HorizontalStrategy::Native,
        solver: Settings {
            doublescan: false,
            refresh_tolerance_mhz: 2_000,
            super_width: 2_560,
            monitor_aspect: crate::Aspect::STANDARD_CRT,
        },
    };

    const CAPS: GpuCaps = GpuCaps {
        pclock_min_hz: 25_000_000,
        pclock_max_hz: 165_000_000,
        interlace: true,
        flip_per_field: false,
        max_width: 1_920,
        max_height: 1_200,
    };

    fn profile() -> MonitorProfile {
        MonitorProfile {
            id: "none".into(),
            kind: MonitorKind::Crt { ranges: vec![] },
        }
    }

    fn shape(rung: Rung, lines: u16, interlace: bool) -> Shape {
        Shape {
            rung,
            game_lines: lines,
            factor: 1,
            lines,
            interlace,
            base_width: 256,
            bordered: false,
        }
    }

    /// A mode of 400 pixels per line and 250 lines per field: 6,000,000 Hz
    /// gives exactly 60.000 Hz, progressive or interlaced, and each extra
    /// 100 Hz of pixel clock adds 1 mHz.
    fn candidate(hactive: u16, vactive: u16, interlace: bool, pclock_hz: u32) -> Candidate {
        let vtotal = if interlace { 500 } else { 250 };
        Candidate {
            modeline: Modeline {
                pclock_hz,
                hactive,
                hsync_start: hactive + 10,
                hsync_end: hactive + 30,
                htotal: 400,
                vactive,
                vsync_start: vactive + 3,
                vsync_end: vactive + 6,
                vtotal,
                interlace,
                doublescan: false,
                hsync_positive: false,
                vsync_positive: false,
            },
            range_index: 0,
            stretched: false,
            refresh_off: false,
            x_scale_milli: 1_000,
            y_scale_milli: 1_000,
            refresh_diff_mhz: 0,
        }
    }

    fn ask<'a>(shape: &'a Shape, profile: &'a MonitorProfile, enforce: bool) -> Ask<'a> {
        Ask {
            shape,
            target: RefreshTarget {
                kind: RefreshKind::Exact,
                rate_mhz: 60_000,
                repeat: 1,
            },
            enforce_refresh: enforce,
            settings: &SETTINGS,
            profile,
            caps: &CAPS,
        }
    }

    #[test]
    fn an_empty_list_is_no_mode() {
        let (s, p) = (shape(Rung::Native, 224, false), profile());
        assert_eq!(ask(&s, &p, true).pick(&[], 256), Err(Dropped::NoMode));
    }

    #[test]
    fn the_first_mode_that_passes_wins_and_else_the_first_reason_stays() {
        let (s, p) = (shape(Rung::Native, 224, false), profile());
        let wrong_lines = candidate(256, 240, false, 6_000_000);
        let good = candidate(256, 224, false, 6_000_000);
        let wrong_width = candidate(320, 224, false, 6_000_000);
        let a = ask(&s, &p, true);
        assert_eq!(a.pick(&[wrong_lines, good], 256), Ok(good));
        assert_eq!(
            a.pick(&[wrong_lines, wrong_width], 256),
            Err(Dropped::LinesChanged {
                asked: 224,
                got: 240
            })
        );
    }

    #[test]
    fn each_check_names_its_rule() {
        let (p, a_shape) = (profile(), shape(Rung::Multiple(2), 448, false));
        let a = ask(&a_shape, &p, true);
        assert_eq!(
            a.check(&candidate(256, 448, true, 12_000_000), 256),
            Err(Dropped::ScanChanged {
                asked_interlace: false
            })
        );
        assert_eq!(
            a.check(&candidate(341, 448, false, 6_000_000), 256),
            Err(Dropped::WidthChanged {
                asked: 256,
                got: 341
            })
        );
    }

    #[test]
    fn the_refresh_matches_below_5_mhz_and_misses_at_5() {
        let (s, p) = (shape(Rung::Native, 224, false), profile());
        let a = ask(&s, &p, true);
        // 6,000,400 Hz gives 60.004 Hz, 6,000,500 Hz gives 60.005 Hz.
        assert_eq!(a.check(&candidate(256, 224, false, 6_000_400), 256), Ok(()));
        assert_eq!(
            a.check(&candidate(256, 224, false, 6_000_500), 256),
            Err(Dropped::RefreshMissed {
                target_mhz: 60_000,
                got_mhz: 60_005
            })
        );
        let last_resort = ask(&s, &p, false);
        assert_eq!(
            last_resort.check(&candidate(256, 224, false, 5_000_000), 256),
            Ok(())
        );
    }

    #[test]
    fn only_an_interlaced_native_source_may_come_back_progressive() {
        let p = profile();
        let native_480i = shape(Rung::Native, 480, true);
        let progressive = candidate(256, 480, false, 6_000_000);
        assert_eq!(ask(&native_480i, &p, true).check(&progressive, 256), Ok(()));
        let rung_3 = shape(Rung::Interlaced, 480, true);
        assert_eq!(
            ask(&rung_3, &p, true).check(&progressive, 256),
            Err(Dropped::ScanChanged {
                asked_interlace: true
            })
        );
    }

    #[test]
    fn the_fitted_rung_keeps_any_lines_scan_and_width_but_not_any_rate() {
        let (s, p) = (shape(Rung::Fitted, 144, false), profile());
        let a = ask(&s, &p, true);
        assert_eq!(a.check(&candidate(714, 536, true, 6_000_000), 160), Ok(()));
        assert!(matches!(
            a.check(&candidate(714, 536, false, 5_000_000), 160),
            Err(Dropped::RefreshMissed { .. })
        ));
    }

    #[test]
    fn a_mode_below_the_floor_is_dropped_with_its_numbers() {
        let (s, p) = (shape(Rung::Native, 224, false), profile());
        let a = ask(&s, &p, true);
        let low = candidate(256, 224, false, 6_000_000);
        assert_eq!(
            a.at_floor(Ok((low, HorizontalScale::Multiple(1)))),
            Err(Dropped::PixelClockFloor {
                width: 256,
                pclock_hz: 6_000_000,
                floor_hz: 25_000_000,
                max_width: 1_920
            })
        );
        let high = candidate(1_280, 224, false, 25_000_000);
        assert!(a.at_floor(Ok((high, HorizontalScale::Multiple(5)))).is_ok());
        assert_eq!(a.at_floor(Err(Dropped::NoMode)), Err(Dropped::NoMode));
    }
}
