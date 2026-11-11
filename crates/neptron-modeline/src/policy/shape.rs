use neptron_types::{CrtRange, GpuCaps};

use super::padding::forced_picture_lines;
use super::source::div_round;
use super::{Dropped, OrientedSource, Rung};
use crate::Aspect;

/// The standard mode heights for a game with fewer lines than the profile
/// allows, smallest first. A whole multiple `k` of the game uses `k` times
/// each. The game is centred in the first one that a range holds at the
/// rate; when none fits, in the smallest height in range that holds it.
pub const BORDER_LINES: [u16; 2] = [224, 240];

/// The most source lines that the overscan crop hides at each edge, top
/// and bottom, in lines of the game.
///
/// A whole multiple that misses the lines a range holds by a few lines is
/// cropped, as the overscan of a CRT hides the edge lines, so the picture
/// stays pixel exact at nearly full height. 4 lines per edge cover an NES
/// at 2x on a 31 kHz monitor that holds 478 lines (1 per edge) and Donkey
/// Kong at 2x in 502 lines (3 per edge). 4 of 240 lines is 1.7 % of the
/// height at each edge, inside the 5 % or more that a consumer CRT hides.
pub const OVERSCAN_CROP_MAX_LINES_PER_EDGE: u16 = 4;

/// The source lines that the overscan crop hides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverscanCrop {
    /// Source lines hidden at the top and the same number at the bottom.
    pub lines_per_edge: u16,
    /// Lines of the game at its multiple before the crop.
    pub asked_lines: u16,
    /// The most progressive lines that a range holds at the target rate.
    pub capacity_lines: u16,
}

impl OverscanCrop {
    /// Source lines hidden in all: both edges.
    pub fn lines(&self) -> u16 {
        self.lines_per_edge.saturating_mul(2)
    }
}

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
    /// Source lines hidden at the edges, so the multiple fits the range.
    pub(crate) crop: Option<OverscanCrop>,
}

/// Step 4, lines and scan: the line count is the height times the factor
/// of the rung. An exact request with fewer lines than the ranges at the
/// target rate allow is centred in a larger mode: a height from
/// [`border_lines`] when progressive, the interlaced minimum when
/// interlaced (rung 3 of a short game, such as a vertical game on 15 kHz).
/// Its base width then keeps the game's aspect on the screen. A progressive
/// game that no mode height holds is dropped.
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
        crop: None,
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
            border_lines(game_lines, factor, ranges, rate_mhz).ok_or(Dropped::NoBorderMode {
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

/// The shape of a progressive whole multiple (the native rung is the
/// multiple 1) whose lines miss what the ranges hold at `rate_mhz`, with
/// the fewest source lines per edge hidden so that the multiple fits; or
/// `None` when no crop up to [`OVERSCAN_CROP_MAX_LINES_PER_EDGE`] fits, or
/// the shape is not such a multiple. The width is never cropped.
pub(crate) fn overscan_crop(
    shape: &Shape,
    source: &OrientedSource,
    ranges: &[CrtRange],
    rate_mhz: u32,
) -> Option<Shape> {
    let multiple = matches!(shape.rung, Rung::Native | Rung::Multiple(_));
    if !multiple || shape.interlace || shape.bordered || shape.factor == 0 {
        return None;
    }
    let capacity = progressive_capacity(ranges, rate_mhz);
    if shape.game_lines <= capacity {
        return None;
    }
    let held = capacity / shape.factor;
    let lines_per_edge = source.height.saturating_sub(held).div_ceil(2);
    let shown = source.height.checked_sub(lines_per_edge.checked_mul(2)?)?;
    if lines_per_edge > OVERSCAN_CROP_MAX_LINES_PER_EDGE || shown == 0 {
        return None;
    }
    let game_lines = shown * shape.factor;
    if progressive_min(ranges, rate_mhz).is_some_and(|min| game_lines < min) {
        return None;
    }
    Some(Shape {
        game_lines,
        lines: game_lines,
        crop: Some(OverscanCrop {
            lines_per_edge,
            asked_lines: shape.game_lines,
            capacity_lines: capacity,
        }),
        ..*shape
    })
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
    let rate = u64::from(rate_mhz).max(1);
    ranges
        .iter()
        .filter(|r| r.progressive_lines_min > 0 && r.v_min_mhz <= rate && rate <= r.v_max_mhz)
        .map(|r| range_capacity(r, rate_mhz))
        .max()
        .unwrap_or(0)
}

/// The most progressive lines of one range at `rate_mhz`, whatever its
/// field rate limits.
fn range_capacity(range: &CrtRange, rate_mhz: u32) -> u16 {
    let rate = u64::from(rate_mhz).max(1);
    let lines = (range.h_max_mhz / rate).saturating_sub(blank_lines(range, range.h_max_mhz));
    u16::try_from(lines)
        .unwrap_or(u16::MAX)
        .min(range.progressive_lines_max)
}

/// The vertical blanking of a range in whole lines at a line rate of
/// `line_rate_mhz`, rounded up. The product of a time in nanoseconds and a
/// rate in millihertz can pass `u64`, so it is taken in `u128` and
/// saturates.
pub(crate) fn blank_lines(range: &CrtRange, line_rate_mhz: u64) -> u64 {
    const NS_MHZ_PER_LINE: u128 = 1_000_000_000_000;
    let blank_ns = u128::from(range.v_front_porch_ns)
        + u128::from(range.v_sync_ns)
        + u128::from(range.v_back_porch_ns);
    let lines = (blank_ns * u128::from(line_rate_mhz)).div_ceil(NS_MHZ_PER_LINE);
    u64::try_from(lines).unwrap_or(u64::MAX)
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

/// The mode height for a progressive game of `game_lines` lines (its
/// height times `factor`) that has fewer lines than the ranges allow at
/// `rate_mhz`, or `None` when no range holds a height that holds it.
///
/// The standard heights come first: [`BORDER_LINES`] times the factor, when
/// a range holds them. Then the smallest height that a range holds and that
/// Switchres does not pad: at least the lines the range forces at the rate
/// (`forced_picture_lines`), so the black borders are the policy's own,
/// centred, and not a porch that moves the picture up.
pub(crate) fn border_lines(
    game_lines: u16,
    factor: u16,
    ranges: &[CrtRange],
    rate_mhz: u32,
) -> Option<u16> {
    let held = |lines: u16| {
        covering(ranges, rate_mhz)
            .filter_map(|r| progressive_window(r, rate_mhz))
            .any(|w| w.fewest <= lines && lines <= w.most)
    };
    let standard = BORDER_LINES
        .into_iter()
        .filter_map(|lines| lines.checked_mul(factor))
        .find(|&lines| lines >= game_lines && held(lines));
    standard.or_else(|| {
        covering(ranges, rate_mhz)
            .filter_map(|r| progressive_window(r, rate_mhz))
            .map(|w| (game_lines.max(w.unpadded), w.most))
            .filter(|&(lines, most)| lines <= most)
            .map(|(lines, _)| lines)
            .min()
    })
}

/// The progressive mode heights of one range at one field rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Window {
    fewest: u16,
    most: u16,
    /// The fewest active lines for which Switchres adds no padding: the
    /// lines the minimum line rate forces, inside the window.
    unpadded: u16,
}

/// The window of a range at `rate_mhz`, or `None` when it holds no
/// progressive height at that rate.
fn progressive_window(range: &CrtRange, rate_mhz: u32) -> Option<Window> {
    let fewest = range.progressive_lines_min;
    let most = range_capacity(range, rate_mhz);
    if fewest == 0 || fewest > most {
        return None;
    }
    let forced = forced_picture_lines(range, rate_mhz);
    let unpadded = u16::try_from(forced)
        .unwrap_or(u16::MAX)
        .clamp(fewest, most);
    Some(Window {
        fewest,
        most,
        unpadded,
    })
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
            turned: false,
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
    fn the_border_height_is_the_first_standard_height_that_a_range_holds() {
        // generic_15 at 60 Hz: 192 to 241 lines.
        let ranges = generic_15();
        assert_eq!(border_lines(144, 1, &ranges, 60_000), Some(224));
        assert_eq!(border_lines(230, 1, &ranges, 60_000), Some(240));
        // At 50 Hz the range holds 192 to 288 lines and forces 293: a 2x
        // request of 180 lines fits neither 448 nor 480, so it gets 288.
        assert_eq!(border_lines(180, 2, &ranges, 50_000), Some(288));
    }

    #[test]
    fn a_multiple_uses_the_standard_heights_times_its_factor() {
        // pc_31_120 at 59.923 Hz holds 400 to 508 lines: 2 x 224 = 448.
        let range = CrtRange {
            h_min_mhz: 31_400_000,
            h_max_mhz: 31_600_000,
            v_min_mhz: 50_000,
            v_max_mhz: 65_000,
            v_front_porch_ns: 34_000,
            v_sync_ns: 101_000,
            v_back_porch_ns: 436_000,
            ..range((400, 512), (0, 0))
        };
        assert_eq!(border_lines(384, 2, &[range], 59_923), Some(448));
        // At 1x neither 224 nor 240 is in the window: the height is the
        // 507 picture lines that the range forces, not the 400 minimum.
        assert_eq!(border_lines(192, 1, &[range], 59_923), Some(507));
        // At 50.08 Hz the range forces 609 lines, above its 512 limit.
        assert_eq!(border_lines(192, 1, &[range], 50_080), Some(512));
    }

    #[test]
    fn with_no_standard_height_the_border_is_the_smallest_unpadded_height() {
        // generic_15 at 65 Hz holds 192 to 221 lines and forces 221.
        let ranges = generic_15();
        assert_eq!(border_lines(144, 1, &ranges, 65_000), Some(221));
        let window = progressive_window(&ranges[0], 65_000).unwrap();
        assert_eq!(
            window,
            Window {
                fewest: 192,
                most: 221,
                unpadded: 221
            }
        );
    }

    #[test]
    fn a_range_that_holds_no_height_at_the_rate_gives_no_border() {
        // arcade_25 at 65 Hz: 24,960 / 65 = 384 lines less 32 of blanking.
        let range = CrtRange {
            h_min_mhz: 24_960_000,
            h_max_mhz: 24_960_000,
            v_front_porch_ns: 80_000,
            v_sync_ns: 200_000,
            v_back_porch_ns: 1_000_000,
            ..range((384, 400), (768, 800))
        };
        assert_eq!(progressive_window(&range, 65_000), None);
        assert_eq!(border_lines(192, 1, &[range], 65_000), None);
        // At 60 Hz it holds exactly 384.
        assert_eq!(border_lines(192, 1, &[range], 60_000), Some(384));
    }

    #[test]
    fn a_multiple_that_misses_the_range_is_cropped_by_the_fewest_lines_per_edge() {
        // generic_15 at 60 Hz holds 241 lines.
        let ranges = generic_15();
        let crop = |height: u16, rung: Rung| {
            let s = source(256, height, false);
            let shaped =
                shape(rung, &s, &ranges, &caps(true), Aspect::STANDARD_CRT, 60_000).unwrap();
            overscan_crop(&shaped, &s, &ranges, 60_000)
        };
        assert_eq!(crop(241, Rung::Native), None);
        // 242 lines: 1 too many, so 1 per edge and 240 lines shown.
        let s = crop(242, Rung::Native).unwrap();
        assert_eq!((s.game_lines, s.lines), (240, 240));
        assert_eq!(
            s.crop,
            Some(OverscanCrop {
                lines_per_edge: 1,
                asked_lines: 242,
                capacity_lines: 241
            })
        );
        // 249 lines: 8 too many, 4 per edge, the bound.
        assert_eq!(crop(249, Rung::Native).unwrap().game_lines, 241);
        // 250 lines: 9 too many, 5 per edge, above the bound.
        assert_eq!(crop(250, Rung::Native), None);
        // 2x of 122 lines: 244 in 241 holds 120 source lines, 1 per edge.
        let s = crop(122, Rung::Multiple(2)).unwrap();
        assert_eq!((s.game_lines, s.factor), (240, 2));
    }

    #[test]
    fn only_a_progressive_multiple_is_cropped() {
        let ranges = generic_15();
        let s = source(640, 242, false);
        for rung in [Rung::Interlaced, Rung::Fitted] {
            let shaped = Shape {
                rung,
                game_lines: 242,
                factor: 1,
                lines: 242,
                interlace: rung == Rung::Interlaced,
                base_width: 640,
                bordered: false,
                crop: None,
            };
            assert_eq!(overscan_crop(&shaped, &s, &ranges, 60_000), None);
        }
        let huge = source(256, u16::MAX, false);
        let shaped = Shape {
            rung: Rung::Native,
            game_lines: u16::MAX,
            factor: 1,
            lines: u16::MAX,
            interlace: false,
            base_width: 256,
            bordered: false,
            crop: None,
        };
        assert_eq!(overscan_crop(&shaped, &huge, &ranges, 60_000), None);
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

    #[test]
    fn the_border_width_holds_at_the_extremes() {
        let huge = OrientedSource {
            aspect_milli: u32::MAX,
            ..source(u16::MAX, u16::MAX, false)
        };
        let aspect = Aspect {
            width: u16::MAX,
            height: u16::MAX,
        };
        assert_eq!(border_width(&huge, u16::MAX, u16::MAX, aspect), u16::MAX);
        let thin = OrientedSource {
            aspect_milli: 1,
            ..source(u16::MAX, 1, false)
        };
        assert_eq!(border_width(&thin, 1, u16::MAX, aspect), u16::MAX);
    }

    #[test]
    fn the_progressive_capacity_holds_at_the_extremes() {
        let wide = CrtRange {
            h_max_mhz: u64::MAX,
            v_min_mhz: 0,
            v_max_mhz: u64::MAX,
            v_front_porch_ns: u32::MAX,
            v_sync_ns: u32::MAX,
            v_back_porch_ns: u32::MAX,
            ..range((1, u16::MAX), (0, 0))
        };
        assert_eq!(progressive_capacity(&[wide], 1), u16::MAX);
        // 2^64 mHz / 2^32 mHz is 2^32 lines, all of them blanking.
        assert_eq!(progressive_capacity(&[wide], u32::MAX), 0);
    }
}
