use neptron_types::{CrtRange, GpuCaps};

use super::source::div_round;
use super::{Dropped, OrientedSource, Rung};
use crate::Aspect;

/// The mode heights that a source with fewer lines than the profile allows
/// is centred in, smallest first.
pub const BORDER_LINES: [u16; 2] = [224, 240];

/// What step 4 asks Switchres for, before the horizontal strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    pub(crate) rung: Rung,
    /// Lines of the game in the mode: its height times `factor`.
    pub(crate) game_lines: u16,
    /// Whole vertical scale of the game.
    pub(crate) factor: u16,
    /// Active lines of the mode.
    pub(crate) lines: u16,
    pub(crate) interlace: bool,
    /// The width that the horizontal strategy multiplies.
    pub(crate) base_width: u16,
    /// The game is centred in more lines than it has.
    pub(crate) bordered: bool,
}

/// Step 4, lines and scan: the line count is the height times the factor
/// of the rung. An exact request with fewer lines than the ranges at the
/// target rate allow is centred in a larger mode: a [`BORDER_LINES`] mode
/// when progressive, the interlaced minimum when interlaced (rung 3 of a
/// short game, such as a vertical game on 15 kHz). Its base width then
/// keeps the game's aspect on the screen. A progressive game that no border
/// height holds is dropped.
pub(crate) fn shape(
    rung: Rung,
    source: &OrientedSource,
    ranges: &[CrtRange],
    caps: &GpuCaps,
    monitor_aspect: Aspect,
    rate_mhz: u32,
) -> Result<Shape, Dropped> {
    let (factor, interlace) = match rung {
        Rung::Native | Rung::Fitted | Rung::Panel => (1, source.interlaced),
        Rung::Multiple(n) => (u16::from(n), false),
        Rung::Interlaced => {
            interlaced_rung(source, ranges, caps, rate_mhz)?;
            (1, true)
        }
    };
    let game_lines = fit_height(u32::from(source.height) * u32::from(factor), caps)?;
    let mut shape = Shape {
        rung,
        game_lines,
        factor,
        lines: game_lines,
        interlace,
        base_width: source.width,
        bordered: false,
    };
    let floor = match rung {
        Rung::Fitted => None,
        _ if interlace => interlaced_min(ranges, rate_mhz),
        _ => progressive_min(ranges, rate_mhz),
    };
    if let Some(min) = floor.filter(|&min| game_lines < min) {
        let lines = if interlace {
            min
        } else {
            border_lines(game_lines, min).ok_or(Dropped::NoBorderMode {
                lines: game_lines,
                line_min: min,
            })?
        };
        shape.lines = fit_height(u32::from(lines), caps)?;
        shape.base_width = border_width(source, game_lines, shape.lines, monitor_aspect);
        shape.bordered = true;
    }
    Ok(shape)
}

/// Rung 3 is for a progressive source whose lines fit no progressive range
/// at the target rate ("when the lines only fit that way"), on a GPU that
/// sends interlace.
fn interlaced_rung(
    source: &OrientedSource,
    ranges: &[CrtRange],
    caps: &GpuCaps,
    rate_mhz: u32,
) -> Result<(), Dropped> {
    if source.interlaced {
        return Err(Dropped::SameAsNative);
    }
    let progressive_max = progressive_capacity(ranges, rate_mhz);
    if source.height <= progressive_max {
        return Err(Dropped::FitsProgressive {
            lines: source.height,
            progressive_max,
        });
    }
    if !caps.interlace {
        return Err(Dropped::NoInterlace);
    }
    Ok(())
}

/// The most progressive lines that a range covering `rate_mhz` can show at
/// that rate: its line limit, or fewer when the highest line rate allows
/// only `floor(h_max / rate)` lines per field, less the vertical blanking
/// at that line rate. 0 when no range covers the rate.
pub(crate) fn progressive_capacity(ranges: &[CrtRange], rate_mhz: u32) -> u16 {
    const NS_MHZ_PER_LINE: u64 = 1_000_000_000_000;
    let rate = u64::from(rate_mhz).max(1);
    ranges
        .iter()
        .filter(|r| r.progressive_lines_min > 0 && r.v_min_mhz <= rate && rate <= r.v_max_mhz)
        .map(|r| {
            let blank_ns = u64::from(r.v_front_porch_ns)
                + u64::from(r.v_sync_ns)
                + u64::from(r.v_back_porch_ns);
            let blank = (blank_ns * r.h_max_mhz).div_ceil(NS_MHZ_PER_LINE);
            let lines = (r.h_max_mhz / rate).saturating_sub(blank);
            u16::try_from(lines)
                .unwrap_or(u16::MAX)
                .min(r.progressive_lines_max)
        })
        .max()
        .unwrap_or(0)
}

fn fit_height(lines: u32, caps: &GpuCaps) -> Result<u16, Dropped> {
    if lines > u32::from(caps.max_height) {
        return Err(Dropped::TallerThanGpu {
            lines,
            max_lines: caps.max_height,
        });
    }
    // At most max_height, a u16.
    Ok(lines as u16)
}

/// The ranges whose field rate limits hold `rate_mhz`; every range when
/// none does, because the request then goes to the nearest one.
fn covering(ranges: &[CrtRange], rate_mhz: u32) -> impl Iterator<Item = &CrtRange> {
    let rate = u64::from(rate_mhz);
    let any = ranges
        .iter()
        .any(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz);
    ranges
        .iter()
        .filter(move |r| !any || (r.v_min_mhz <= rate && rate <= r.v_max_mhz))
}

/// The fewest progressive lines of a range at the rate that has a
/// progressive window, or `None` when none has one.
fn progressive_min(ranges: &[CrtRange], rate_mhz: u32) -> Option<u16> {
    covering(ranges, rate_mhz)
        .filter(|r| r.progressive_lines_min > 0)
        .map(|r| r.progressive_lines_min)
        .min()
}

/// The fewest interlaced lines of a range at the rate that has an
/// interlaced window, or `None` when none has one.
fn interlaced_min(ranges: &[CrtRange], rate_mhz: u32) -> Option<u16> {
    covering(ranges, rate_mhz)
        .filter(|r| r.interlaced_lines_min > 0)
        .map(|r| r.interlaced_lines_min)
        .min()
}

/// The first border height that holds the game and that the ranges allow,
/// or `None` when no border height does.
pub(crate) fn border_lines(game_lines: u16, line_min: u16) -> Option<u16> {
    let floor = game_lines.max(line_min);
    BORDER_LINES.into_iter().find(|&lines| lines >= floor)
}

/// The width of a mode with `mode_lines` lines in which a game of
/// `game_lines` lines keeps its display aspect at one pixel per pixel.
///
/// The mode covers the whole screen, of shape `monitor_aspect`, so the
/// game's share of the width must be its share of the height times its
/// aspect over the screen's aspect. A core with no aspect gets square
/// pixels. The width is never below the game width.
fn border_width(
    source: &OrientedSource,
    game_lines: u16,
    mode_lines: u16,
    monitor_aspect: Aspect,
) -> u16 {
    let game_aspect_milli = match source.aspect_milli {
        0 => u64::from(div_round(
            u64::from(source.width) * 1_000,
            u64::from(source.height),
        )),
        a => u64::from(a),
    };
    let numerator =
        u64::from(source.width) * u64::from(mode_lines) * u64::from(monitor_aspect.width) * 1_000;
    let denominator = u64::from(monitor_aspect.height) * u64::from(game_lines) * game_aspect_milli;
    let width = div_round(numerator, denominator.max(1));
    u16::try_from(width).unwrap_or(u16::MAX).max(source.width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(prog: (u16, u16), inter: (u16, u16)) -> CrtRange {
        CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 15_750_000,
            v_min_mhz: 49_500,
            v_max_mhz: 65_000,
            h_front_porch_ns: 2_000,
            h_sync_ns: 4_700,
            h_back_porch_ns: 8_000,
            v_front_porch_ns: 64_000,
            v_sync_ns: 192_000,
            v_back_porch_ns: 1_024_000,
            hsync_positive: false,
            vsync_positive: false,
            progressive_lines_min: prog.0,
            progressive_lines_max: prog.1,
            interlaced_lines_min: inter.0,
            interlaced_lines_max: inter.1,
        }
    }

    fn generic_15() -> Vec<CrtRange> {
        vec![range((192, 288), (448, 576))]
    }

    fn caps(interlace: bool) -> GpuCaps {
        GpuCaps {
            pclock_min_hz: 0,
            pclock_max_hz: 165_000_000,
            interlace,
            flip_per_field: true,
            max_width: 1_920,
            max_height: 1_200,
        }
    }

    fn source(width: u16, height: u16, interlaced: bool) -> OrientedSource {
        OrientedSource {
            width,
            height,
            refresh_mhz: 60_000,
            interlaced,
            aspect_milli: 1_333,
            frame_width: width,
            frame_height: height,
        }
    }

    fn shape_of(rung: Rung, s: &OrientedSource, interlace: bool) -> Result<Shape, Dropped> {
        shape(
            rung,
            s,
            &generic_15(),
            &caps(interlace),
            Aspect::STANDARD_CRT,
            60_000,
        )
    }

    #[test]
    fn native_asks_for_the_game_lines() {
        let s = shape_of(Rung::Native, &source(256, 224, false), true).unwrap();
        assert_eq!(
            (s.lines, s.game_lines, s.factor, s.interlace),
            (224, 224, 1, false)
        );
        assert_eq!((s.base_width, s.bordered), (256, false));
    }

    #[test]
    fn native_of_an_interlaced_source_asks_for_interlace() {
        let s = shape_of(Rung::Native, &source(640, 480, true), true).unwrap();
        assert_eq!((s.lines, s.interlace), (480, true));
    }

    #[test]
    fn a_multiple_asks_for_the_lines_times_the_factor_progressive() {
        let s = shape_of(Rung::Multiple(3), &source(320, 240, false), true).unwrap();
        assert_eq!(
            (s.lines, s.game_lines, s.factor, s.interlace),
            (720, 720, 3, false)
        );
        assert_eq!(s.base_width, 320);
    }

    #[test]
    fn a_source_below_the_line_minimum_is_centred_in_a_border_mode() {
        // Game Boy: 144 lines, below the 192 of generic_15.
        let gb = OrientedSource {
            aspect_milli: 1_111,
            ..source(160, 144, false)
        };
        let s = shape_of(Rung::Native, &gb, true).unwrap();
        assert_eq!((s.lines, s.game_lines, s.bordered), (224, 144, true));
        // 160 x 224 x 4000 / (3 x 144 x 1111) = 298.7.
        assert_eq!(s.base_width, 299);
    }

    #[test]
    fn the_border_height_is_the_first_that_holds_the_game_and_the_profile() {
        assert_eq!(border_lines(144, 192), Some(224));
        assert_eq!(border_lines(230, 192), Some(240));
        assert_eq!(border_lines(144, 230), Some(240));
        assert_eq!(border_lines(144, 400), None);
        assert_eq!(border_lines(288, 400), None);
    }

    #[test]
    fn rung_three_is_only_for_lines_that_fit_no_progressive_range_at_the_rate() {
        // At 60 Hz, 15.75 kHz holds 262 lines less 21 of blanking.
        assert_eq!(
            shape_of(Rung::Interlaced, &source(256, 241, false), true),
            Err(Dropped::FitsProgressive {
                lines: 241,
                progressive_max: 241
            })
        );
        let s = shape_of(Rung::Interlaced, &source(640, 480, false), true).unwrap();
        assert_eq!((s.lines, s.interlace, s.bordered), (480, true, false));
    }

    #[test]
    fn a_vertical_game_on_15_khz_gets_rung_three_with_a_border() {
        // 224x288 turned: 288 lines do not fit progressive at 60 Hz.
        let tate = OrientedSource {
            aspect_milli: 750,
            ..source(224, 288, false)
        };
        let s = shape_of(Rung::Interlaced, &tate, true).unwrap();
        assert_eq!(
            (s.lines, s.game_lines, s.interlace, s.bordered),
            (448, 288, true, true)
        );
        // 224 x 448 x 4000 / (3 x 288 x 750) = 619.5.
        assert_eq!(s.base_width, 619);
    }

    #[test]
    fn the_progressive_capacity_follows_the_rate() {
        let ranges = generic_15();
        assert_eq!(progressive_capacity(&ranges, 60_000), 241);
        assert_eq!(progressive_capacity(&ranges, 50_000), 288);
        assert_eq!(progressive_capacity(&ranges, 70_000), 0);
    }

    #[test]
    fn rung_three_of_an_interlaced_source_is_rung_one() {
        assert_eq!(
            shape_of(Rung::Interlaced, &source(640, 480, true), true),
            Err(Dropped::SameAsNative)
        );
    }

    #[test]
    fn rung_three_needs_a_gpu_that_sends_interlace() {
        assert_eq!(
            shape_of(Rung::Interlaced, &source(640, 480, false), false),
            Err(Dropped::NoInterlace)
        );
    }

    #[test]
    fn a_mode_taller_than_the_gpu_is_dropped() {
        assert_eq!(
            shape_of(Rung::Multiple(4), &source(320, 480, false), true),
            Err(Dropped::TallerThanGpu {
                lines: 1_920,
                max_lines: 1_200
            })
        );
    }

    #[test]
    fn the_fitted_rung_asks_for_the_source_as_it_is() {
        let s = shape_of(Rung::Fitted, &source(160, 144, false), true).unwrap();
        assert_eq!((s.lines, s.bordered, s.base_width), (144, false, 160));
    }

    #[test]
    fn a_source_with_no_aspect_keeps_square_pixels_in_its_border() {
        let square = OrientedSource {
            aspect_milli: 0,
            ..source(160, 120, false)
        };
        let s = shape_of(Rung::Native, &square, true).unwrap();
        // A 4:3 game with square pixels in 224 lines: 4/3 x 224 = 298.7.
        assert_eq!((s.lines, s.base_width), (224, 299));
    }
}
