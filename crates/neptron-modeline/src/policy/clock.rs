use neptron_types::{CrtRange, Modeline};

/// The most pixels that [`whole_khz`] adds to or takes from the horizontal
/// total of Switchres. Each pixel changes the time of the front porch, the
/// sync pulse and the active area by about one part in the total, so the
/// bound keeps the change near 2.5 % on a 15 kHz mode of 320 pixels per
/// line. The `ntsc` preset has a field rate range of one value, 59.940 Hz,
/// and needs the most choices: about one in ten pairs of a total and a
/// whole-kHz clock gives that rate to the millihertz.
const HTOTAL_CHANGE_MAX: u16 = 8;

/// The share of the nominal time, in percent, that each horizontal porch
/// and the horizontal sync pulse keep. Switchres lays out the line with the
/// same share (`get_line_params` in `modeline.cpp`).
const HORIZONTAL_MIN_PERCENT: u64 = 90;

/// The longest horizontal sync pulse, in percent of the nominal time, that
/// a changed total may give, unless the pulse of Switchres is longer
/// already. The validator allows 150 %; this bound keeps a changed total
/// near the pulse that Switchres aims for.
const HSYNC_MAX_PERCENT: u64 = 110;

const NS_PER_S: u128 = 1_000_000_000;
const PERCENT: u64 = 100;

/// The mode of Switchres with a pixel clock in whole kHz, which a display
/// takes as it is ([`Modeline::PCLOCK_STEP_HZ`]).
///
/// The line rate and the field rate follow from the clock and the totals,
/// so a changed clock changes both. A step of 1 kHz moves the field rate by
/// `1000 / (htotal * vtotal)` Hz per field: about 9 mHz for a 320-wide
/// 15 kHz mode, more than [`super::REFRESH_MATCH_MHZ`]. Rounding alone can
/// then miss the game's rate, and at the edge of a range it can cross the
/// edge. So the function tries Switchres's horizontal total, then one
/// pixel more, one pixel less, two more, and so on up to
/// [`HTOTAL_CHANGE_MAX`]; for each total, the whole-kHz clock below and
/// above the one that keeps the line time of Switchres, the nearer first.
/// The pixels go to or come from the back porch. It returns the first mode
/// whose field rate equals Switchres's to the millihertz, else the one
/// nearest it, among the modes that:
///
/// - keep the line rate and the field rate inside `range` where the mode
///   of Switchres has them inside. Switchres can give a line rate outside a
///   range of one line rate (`ntsc`, `pal`): the line rate is then the field
///   rate times the lines, and it follows the field rate;
/// - keep each horizontal porch and the sync pulse at least 90 % of the
///   nominal time of `range`, or no shorter than in the mode of Switchres;
/// - keep the sync pulse at most 110 % of the nominal time, or no longer
///   than in the mode of Switchres.
///
/// When no mode passes, or `range` is `None`, the result is the nearest
/// whole kHz at Switchres's total; the safety validator judges it as any
/// other mode. A mode whose clock is already whole kHz comes back as it is.
pub(crate) fn whole_khz(mode: Modeline, range: Option<&CrtRange>) -> Modeline {
    if mode.pclock_is_whole_khz() || mode.htotal == 0 {
        return mode;
    }
    let Some(range) = range else {
        return nearest(mode);
    };
    // A field rate outside the range is never the aim: the validator
    // refuses it, and a range of one field rate (`ntsc`) takes only that.
    let want = mode
        .field_rate_mhz()
        .clamp(range.v_min_mhz, range.v_max_mhz.max(range.v_min_mhz));
    let mut best: Option<((bool, u64), Modeline)> = None;
    for htotal in totals(mode.htotal) {
        for pclock_hz in clocks(mode.pclock_hz, mode.htotal, htotal) {
            let next = Modeline {
                pclock_hz,
                htotal,
                ..mode
            };
            if !keeps_range(&next, &mode, range) || !keeps_porches(&next, &mode, range) {
                continue;
            }
            let error = next.field_rate_mhz().abs_diff(want);
            // A mode inside the range ranks before one outside it.
            let key = (!inside(&next, range), error);
            if key == (false, 0) {
                return next;
            }
            if best.is_none_or(|(k, _)| key < k) {
                best = Some((key, next));
            }
        }
    }
    best.map_or_else(|| nearest(mode), |(_, m)| m)
}

/// The mode of Switchres moved into the line rates that the validator
/// takes from `range`: from the lower of `h_min` and `h_max` less one field
/// rate, up to `h_max`.
///
/// Switchres picks the lines per field and then the clock, and it can end
/// one line per field off: 24.989 kHz on `arcade_25` (24.960 kHz only),
/// 16.217 kHz at the top of `arcade_15` (up to 16.200 kHz). The validator
/// refuses such a mode. This function gives the mode the total of lines
/// per field whose line rate lies in that band at the same field rate: the
/// most lines at or under `h_max`, or the fewest at or over the low end.
/// It adds lines to or takes them from the vertical back porch, and scales
/// the clock so the field rate stays. The active lines, the sync and the
/// front porch do not move. A mode that is inside already, or a change that
/// would end the frame at or before the end of the sync, comes back as it
/// is. [`whole_khz`] then sets the clock to whole kHz inside the range.
pub(crate) fn onto_line_band(mode: Modeline, range: &CrtRange) -> Modeline {
    if mode.vtotal == 0 {
        return mode;
    }
    let field = mode.field_rate_mhz().max(1);
    let line = mode.line_rate_mhz();
    let (low, high) = line_band(range, field);
    if (low..=high).contains(&line) {
        return mode;
    }
    if line < low {
        return onto_line_floor(mode, low, field);
    }
    // The line rate is the field rate times the total over the fields per
    // frame. The band is one field rate wide, so the largest total at or
    // under `h_max` lies in it; an interlaced total keeps its parity, which
    // gives the half line of the second field.
    let most = if mode.interlace {
        let most = 2 * high / field;
        most - (most + u64::from(mode.vtotal)) % 2
    } else {
        high / field
    };
    let Ok(vtotal) = u16::try_from(most) else {
        return mode;
    };
    if vtotal <= mode.vsync_end {
        return mode;
    }
    // Below 2^32 * 2^16, so the product fits a u64.
    let scaled = u64::from(mode.pclock_hz) * u64::from(vtotal) / u64::from(mode.vtotal);
    let Ok(pclock_hz) = u32::try_from(scaled) else {
        return mode;
    };
    Modeline {
        pclock_hz,
        vtotal,
        ..mode
    }
}

/// The mode with the fewest lines whose line rate reaches `low` at the
/// same field rate: lines added to the vertical back porch, the clock
/// scaled. A total that does not fit comes back as it is.
fn onto_line_floor(mode: Modeline, low: u64, field: u64) -> Modeline {
    let fewest = if mode.interlace {
        let fewest = (2 * low).div_ceil(field);
        fewest + (fewest + u64::from(mode.vtotal)) % 2
    } else {
        low.div_ceil(field)
    };
    let Ok(vtotal) = u16::try_from(fewest) else {
        return mode;
    };
    let scaled = u64::from(mode.pclock_hz) * u64::from(vtotal) / u64::from(mode.vtotal);
    match u32::try_from(scaled) {
        Ok(pclock_hz) => Modeline {
            pclock_hz,
            vtotal,
            ..mode
        },
        Err(_) => mode,
    }
}

/// Switchres's total, then one pixel more and one less, two more and two
/// less, and so on, each total that fits a `u16` once.
fn totals(htotal: u16) -> impl Iterator<Item = u16> {
    let changed = (1..=HTOTAL_CHANGE_MAX)
        .flat_map(move |d| [htotal.checked_add(d), htotal.checked_sub(d)])
        .flatten();
    std::iter::once(htotal).chain(changed)
}

/// The mode with its clock at the nearest whole kHz, a half rounded up, or
/// the step below when that does not fit a `u32`.
fn nearest(mode: Modeline) -> Modeline {
    let step = u64::from(Modeline::PCLOCK_STEP_HZ);
    let up = (u64::from(mode.pclock_hz) + step / 2) / step * step;
    let pclock_hz = u32::try_from(up).unwrap_or(floor_step(u64::from(mode.pclock_hz)));
    Modeline { pclock_hz, ..mode }
}

/// The whole-kHz clocks next to `pclock_hz * htotal / htotal_before`, the
/// clock that keeps the line time: the nearer first, and only those that
/// fit a `u32` and are above 0.
fn clocks(pclock_hz: u32, htotal_before: u16, htotal: u16) -> Vec<u32> {
    let step = u64::from(Modeline::PCLOCK_STEP_HZ);
    // Below 2^32 * 2^16, so the product fits a u64.
    let scaled = u64::from(pclock_hz) * u64::from(htotal);
    let before = u64::from(htotal_before);
    let below = scaled / (before * step) * step;
    let above = below + step;
    let below_gap = scaled - below * before;
    let above_gap = above * before - scaled;
    let order = if above_gap < below_gap {
        [above, below]
    } else {
        [below, above]
    };
    order
        .into_iter()
        .filter(|&c| c > 0)
        .filter_map(|c| u32::try_from(c).ok())
        .collect()
}

fn floor_step(pclock_hz: u64) -> u32 {
    let step = u64::from(Modeline::PCLOCK_STEP_HZ);
    // At most the input, which came from a u32.
    u32::try_from(pclock_hz / step * step).unwrap_or(0)
}

/// The line rate and the field rate of `mode` lie where the validator
/// takes them for `range`: the field rate inside its limits, the line rate
/// from the lower of `h_min` and `h_max` less one field rate up to `h_max`.
fn inside(mode: &Modeline, range: &CrtRange) -> bool {
    let (low, high) = line_band(range, mode.field_rate_mhz());
    (low..=high).contains(&mode.line_rate_mhz())
        && (range.v_min_mhz..=range.v_max_mhz).contains(&mode.field_rate_mhz())
}

/// The line rates the validator takes from `range` at `field_mhz`.
fn line_band(range: &CrtRange, field_mhz: u64) -> (u64, u64) {
    let high = range.h_max_mhz;
    (range.h_min_mhz.min(high.saturating_sub(field_mhz)), high)
}

fn keeps_range(next: &Modeline, before: &Modeline, range: &CrtRange) -> bool {
    let keeps = |rate: fn(&Modeline) -> u64, min: u64, max: u64| {
        let inside = |m: &Modeline| (min..=max).contains(&rate(m));
        inside(next) || !inside(before)
    };
    // The band of the validator: `onto_line_band` put the mode
    // inside it where it can, and the clock must keep it there.
    let (low, high) = line_band(range, next.field_rate_mhz());
    let line_inside = |m: &Modeline| (low..=high).contains(&m.line_rate_mhz());
    if !line_inside(next) && line_inside(before) {
        return false;
    }
    keeps(Modeline::field_rate_mhz, range.v_min_mhz, range.v_max_mhz)
}

fn keeps_porches(next: &Modeline, before: &Modeline, range: &CrtRange) -> bool {
    let front = |m: &Modeline| m.hsync_start.saturating_sub(m.hactive);
    let sync = |m: &Modeline| m.hsync_end.saturating_sub(m.hsync_start);
    let back = |m: &Modeline| m.htotal.saturating_sub(m.hsync_end);
    let long_enough = [
        (front(next), front(before), range.h_front_porch_ns),
        (sync(next), sync(before), range.h_sync_ns),
        (back(next), back(before), range.h_back_porch_ns),
    ]
    .into_iter()
    .all(|(pixels, pixels_before, nominal_ns)| {
        let min_ns = (u64::from(nominal_ns) * HORIZONTAL_MIN_PERCENT)
            .div_ceil(PERCENT)
            .max(1);
        Time::of(pixels, next) >= Time::ns(min_ns)
            || Time::of(pixels, next) >= Time::of(pixels_before, before)
    });
    let max_ns = u64::from(range.h_sync_ns) * HSYNC_MAX_PERCENT / PERCENT;
    let sync_short_enough = Time::of(sync(next), next) <= Time::ns(max_ns)
        || Time::of(sync(next), next) <= Time::of(sync(before), before);
    long_enough && sync_short_enough
}

/// A time as the fraction `pixels * 1e9 / pclock_hz` nanoseconds, compared
/// by cross products: every product stays below 2^16 * 2^30 * 2^32, far
/// inside a `u128`.
#[derive(Clone, Copy)]
struct Time {
    numerator: u128,
    denominator: u128,
}

impl Time {
    fn of(pixels: u16, mode: &Modeline) -> Self {
        Self {
            numerator: u128::from(pixels) * NS_PER_S,
            denominator: u128::from(mode.pclock_hz).max(1),
        }
    }

    fn ns(ns: u64) -> Self {
        Self {
            numerator: u128::from(ns),
            denominator: 1,
        }
    }
}

impl PartialEq for Time {
    fn eq(&self, other: &Self) -> bool {
        self.numerator * other.denominator == other.numerator * self.denominator
    }
}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (self.numerator * other.denominator).partial_cmp(&(other.numerator * self.denominator))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `ntsc` preset: one line rate, one field rate.
    fn ntsc() -> CrtRange {
        CrtRange {
            h_min_mhz: 15_734_260,
            h_max_mhz: 15_734_260,
            v_min_mhz: 59_940,
            v_max_mhz: 59_940,
            h_front_porch_ns: 1_500,
            h_sync_ns: 4_700,
            h_back_porch_ns: 4_700,
            v_front_porch_ns: 191_000,
            v_sync_ns: 191_000,
            v_back_porch_ns: 953_000,
            hsync_positive: false,
            vsync_positive: false,
            progressive_lines_min: 192,
            progressive_lines_max: 240,
            interlaced_lines_min: 448,
            interlaced_lines_max: 480,
        }
    }

    /// The mode Switchres gives for 256x224 at 59.94 Hz on `ntsc`.
    fn ntsc_256() -> Modeline {
        Modeline {
            pclock_hz: 4_852_622,
            hactive: 256,
            hsync_start: 263,
            hsync_end: 286,
            htotal: 309,
            vactive: 224,
            vsync_start: 235,
            vsync_end: 238,
            vtotal: 262,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    /// The `arcade_25` preset: 24.960 kHz only.
    fn arcade_25() -> CrtRange {
        CrtRange {
            h_min_mhz: 24_960_000,
            h_max_mhz: 24_960_000,
            v_min_mhz: 49_500,
            v_max_mhz: 65_000,
            interlaced_lines_min: 768,
            interlaced_lines_max: 800,
            progressive_lines_min: 384,
            progressive_lines_max: 400,
            ..ntsc()
        }
    }

    /// An interlaced mode one line per field above 24.960 kHz at 60 Hz:
    /// 60 Hz x 833 / 2 = 24.990 kHz.
    fn one_line_high() -> Modeline {
        Modeline {
            pclock_hz: 33_236_700,
            hactive: 1_024,
            hsync_start: 1_050,
            hsync_end: 1_150,
            htotal: 1_330,
            vactive: 768,
            vsync_start: 773,
            vsync_end: 783,
            vtotal: 833,
            interlace: true,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn a_mode_above_a_single_line_rate_gets_one_line_per_field_less() {
        let m = one_line_high();
        assert_eq!(m.field_rate_mhz(), 60_000);
        let moved = onto_line_band(m, &arcade_25());
        // 831 lines: 60 Hz x 831 / 2 = 24.930 kHz, inside 24.900 to 24.960.
        assert_eq!(moved.vtotal, 831);
        assert_eq!(moved.field_rate_mhz(), 60_000);
        assert!((24_900_000..=24_960_000).contains(&moved.line_rate_mhz()));
        assert_eq!(
            (moved.vactive, moved.vsync_start, moved.vsync_end),
            (768, 773, 783)
        );
        let clocked = whole_khz(moved, Some(&arcade_25()));
        assert!(clocked.pclock_is_whole_khz());
        assert!((24_900_000..=24_960_000).contains(&clocked.line_rate_mhz()));
        assert!(clocked.field_rate_mhz().abs_diff(60_000) < 5);
    }

    #[test]
    fn a_move_that_would_end_the_frame_in_the_sync_keeps_the_mode() {
        // The sync ends on line 831: the 831-line total that the band asks
        // for leaves no back porch, so the mode stays as it is.
        let tight = Modeline {
            vsync_start: 826,
            vsync_end: 831,
            ..one_line_high()
        };
        assert_eq!(onto_line_band(tight, &arcade_25()), tight);
        // One line earlier the total of 831 stays above the sync.
        let room = Modeline {
            vsync_start: 825,
            vsync_end: 830,
            ..one_line_high()
        };
        assert_eq!(onto_line_band(room, &arcade_25()).vtotal, 831);
    }

    #[test]
    fn a_mode_above_the_top_of_a_band_comes_down_into_it() {
        let band = CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 16_200_000,
            ..arcade_25()
        };
        // 65 Hz x 499 / 2 = 16.2175 kHz: above the band.
        let high = Modeline {
            vtotal: 499,
            vactive: 456,
            vsync_start: 460,
            vsync_end: 466,
            pclock_hz: 12_000_000,
            htotal: 740,
            ..one_line_high()
        };
        let moved = onto_line_band(high, &band);
        assert!(
            moved.line_rate_mhz() <= 16_200_000,
            "{}",
            moved.line_rate_mhz()
        );
        assert!(moved.vtotal % 2 == 1 && moved.vtotal > moved.vsync_end);
    }

    #[test]
    fn a_mode_inside_the_band_stays() {
        let inside = Modeline {
            vtotal: 831,
            pclock_hz: 33_156_900,
            ..one_line_high()
        };
        assert_eq!(onto_line_band(inside, &arcade_25()), inside);
        // 24.99 kHz on a 15 kHz band needs 540 lines, fewer than the 783
        // that end the sync: the mode stays, and the validator judges it.
        let band = CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 16_200_000,
            ..arcade_25()
        };
        assert_eq!(onto_line_band(one_line_high(), &band), one_line_high());
    }

    #[test]
    fn a_clock_in_whole_khz_stays_as_it_is() {
        let m = Modeline {
            pclock_hz: 4_852_000,
            ..ntsc_256()
        };
        assert_eq!(whole_khz(m, Some(&ntsc())), m);
        assert_eq!(whole_khz(m, None), m);
    }

    #[test]
    fn with_no_range_the_clock_takes_the_nearest_khz() {
        let at = |pclock_hz| {
            whole_khz(
                Modeline {
                    pclock_hz,
                    ..ntsc_256()
                },
                None,
            )
            .pclock_hz
        };
        assert_eq!(at(4_852_499), 4_852_000);
        assert_eq!(at(4_852_500), 4_853_000);
        assert_eq!(at(u32::MAX), 4_294_967_000);
        assert_eq!(at(1), 0);
    }

    #[test]
    fn a_range_of_one_field_rate_gets_it_by_a_changed_total() {
        let m = ntsc_256();
        assert_eq!(m.field_rate_mhz(), 59_940);
        // At 309 pixels, 4 852 000 Hz gives 59.932 Hz and 4 853 000 Hz
        // gives 59.945 Hz. At 311 pixels, 4 884 000 Hz gives
        // 4 884 000 / (311 * 262) = 59.9396 Hz, 59.940 to the millihertz.
        let got = whole_khz(m, Some(&ntsc()));
        assert_eq!(
            got,
            Modeline {
                pclock_hz: 4_884_000,
                htotal: 311,
                ..m
            }
        );
        assert_eq!(got.field_rate_mhz(), 59_940);
        assert!(got.pclock_is_whole_khz());
    }

    #[test]
    fn the_first_total_that_keeps_the_field_rate_wins() {
        // 6 700 400 Hz at 426 x 262 gives 60.0306 Hz; 6 700 000 Hz gives
        // 60.0270 Hz, 6 701 000 Hz 60.0360 Hz: no clock at 426 keeps 60.031.
        let m = Modeline {
            pclock_hz: 6_700_400,
            hactive: 320,
            hsync_start: 334,
            hsync_end: 366,
            htotal: 426,
            vactive: 240,
            vsync_start: 241,
            vsync_end: 244,
            vtotal: 262,
            ..ntsc_256()
        };
        let wide = CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 15_750_000,
            v_min_mhz: 49_500,
            v_max_mhz: 65_000,
            h_front_porch_ns: 2_000,
            h_back_porch_ns: 8_000,
            ..ntsc()
        };
        let got = whole_khz(m, Some(&wide));
        assert_eq!(got.field_rate_mhz(), m.field_rate_mhz());
        assert!(got.pclock_is_whole_khz());
        assert!(got.htotal.abs_diff(m.htotal) <= HTOTAL_CHANGE_MAX);
        assert_eq!(
            (got.hactive, got.hsync_start, got.hsync_end, got.vtotal),
            (m.hactive, m.hsync_start, m.hsync_end, m.vtotal)
        );
    }

    #[test]
    fn a_rate_at_the_top_of_its_range_never_crosses_it() {
        let m = ntsc_256();
        let top = CrtRange {
            h_min_mhz: 15_000_000,
            h_max_mhz: m.line_rate_mhz(),
            v_min_mhz: 50_000,
            v_max_mhz: m.field_rate_mhz(),
            ..ntsc()
        };
        let got = whole_khz(m, Some(&top));
        assert!(got.line_rate_mhz() <= top.h_max_mhz);
        assert!(got.field_rate_mhz() <= top.v_max_mhz);
        assert!(got.pclock_is_whole_khz());
    }

    #[test]
    fn a_porch_at_ninety_percent_does_not_get_shorter() {
        // A front porch of 7 pixels at 4 852 622 Hz is 1 442.5 ns. With a
        // nominal time of 1 603 ns its floor is 1 443 ns, above the porch,
        // so no mode may give a shorter front porch than Switchres's.
        let range = CrtRange {
            h_min_mhz: 15_000_000,
            h_max_mhz: 16_000_000,
            v_min_mhz: 50_000,
            v_max_mhz: 65_000,
            h_front_porch_ns: 1_603,
            ..ntsc()
        };
        let m = ntsc_256();
        let got = whole_khz(m, Some(&range));
        let porch = |x: &Modeline| Time::of(x.hsync_start - x.hactive, x);
        assert!(porch(&got) >= porch(&m));
        assert!(got.pclock_is_whole_khz());
    }

    #[test]
    fn with_no_mode_that_keeps_the_rules_the_clock_takes_the_nearest_khz() {
        // Long nominal porches: a higher clock shortens the front porch and
        // a smaller total shortens the back porch below Switchres's, and the
        // lower clock at 309 pixels misses the one field rate.
        let range = CrtRange {
            h_front_porch_ns: 10_000,
            h_back_porch_ns: 100_000,
            ..ntsc()
        };
        let got = whole_khz(ntsc_256(), Some(&range));
        assert_eq!(
            got,
            Modeline {
                pclock_hz: 4_853_000,
                ..ntsc_256()
            }
        );
    }

    #[test]
    fn the_totals_go_out_from_switchres_one_pixel_at_a_time() {
        let t: Vec<u16> = totals(300).take(5).collect();
        assert_eq!(t, [300, 301, 299, 302, 298]);
        assert_eq!(totals(300).count(), 17);
        assert_eq!(
            totals(u16::MAX).take(3).collect::<Vec<_>>(),
            [u16::MAX, u16::MAX - 1, u16::MAX - 2]
        );
        assert_eq!(
            totals(1).collect::<Vec<_>>(),
            [1, 2, 0, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn the_clocks_next_to_the_line_time_come_nearer_first() {
        // 4 852 622 * 311 / 309 = 4 884 030.4: 4 884 000 is nearer.
        assert_eq!(clocks(4_852_622, 309, 311), [4_884_000, 4_885_000]);
        // 4 852 622 at 309: 4 853 000 is nearer than 4 852 000.
        assert_eq!(clocks(4_852_622, 309, 309), [4_853_000, 4_852_000]);
        assert_eq!(clocks(400, 309, 309), [1_000]);
        assert_eq!(clocks(u32::MAX, 1, 1), [4_294_967_000]);
    }
}
