use neptron_types::{CrtRange, Modeline};

use super::shape::blank_lines;

/// A mode is padded when the game fills less than this share of the lines
/// the range forces at the field rate, in percent.
///
/// A range with a minimum line rate forces at least `h_min / field rate`
/// lines per field. Less the vertical blanking, the rest is picture. A game
/// with fewer lines leaves black bands; Switchres puts them in the front
/// porch, so the picture sits high on the screen. Small bands are normal on
/// every CRT (224 lines in 240 at 15 kHz is 93 %, a 448-line doubled game
/// in 506 at 31.4 kHz is 89 %); half the screen is not (224 lines in 480
/// at 30 kHz is 47 %).
pub const PADDED_FILL_MIN_PERCENT: u64 = 75;

/// Why a mode counts as padded: the numbers of the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Padding {
    /// Lines of the game in one field.
    pub game_lines: u32,
    /// Lines per field that the range leaves for the picture at this field
    /// rate: the active lines of the mode, or more when the minimum line
    /// rate forces more.
    pub picture_lines: u32,
    /// The minimum line rate of the range, in millihertz.
    pub line_rate_min_mhz: u64,
    /// The field rate aimed for, in millihertz.
    pub field_rate_mhz: u32,
}

/// The padding rule: `Some` when the game fills less than
/// [`PADDED_FILL_MIN_PERCENT`] of the picture lines of the mode.
///
/// The picture lines are the larger of the mode's active lines per field
/// (a border mode) and the lines the range forces:
/// `ceil(h_min / field_rate) - ceil(h_min * vertical_blank)`. The rates and
/// the blanking are those of the range that made the mode, at the target
/// field rate, so the rule does not depend on how Switchres rounds.
pub(crate) fn padding(
    game_lines: u16,
    mode: &Modeline,
    field_rate_mhz: u32,
    range: &CrtRange,
) -> Option<Padding> {
    let scan = if mode.interlace { 2 } else { 1 };
    let game = u32::from(game_lines) / scan;
    let active = u32::from(mode.vactive) / scan;
    let forced = forced_picture_lines(range, field_rate_mhz);
    let picture = active.max(forced);
    if u64::from(game) * 100 >= PADDED_FILL_MIN_PERCENT * u64::from(picture) {
        return None;
    }
    Some(Padding {
        game_lines: game,
        picture_lines: picture,
        line_rate_min_mhz: range.h_min_mhz,
        field_rate_mhz,
    })
}

/// Lines per field that the minimum line rate forces, less the vertical
/// blanking at that line rate.
fn forced_picture_lines(range: &CrtRange, field_rate_mhz: u32) -> u32 {
    let rate = u64::from(field_rate_mhz).max(1);
    let total = range.h_min_mhz.div_ceil(rate);
    let blank = blank_lines(range, range.h_min_mhz);
    u32::try_from(total.saturating_sub(blank)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The range that neptron-platform builds from the EDID of a 30 to
    /// 71 kHz, 50 to 160 Hz PC CRT.
    fn edid_30_71() -> CrtRange {
        CrtRange {
            h_min_mhz: 30_000_000,
            h_max_mhz: 71_000_000,
            v_min_mhz: 50_000,
            v_max_mhz: 160_000,
            h_front_porch_ns: 817,
            h_sync_ns: 2_667,
            h_back_porch_ns: 3_334,
            v_front_porch_ns: 33_334,
            v_sync_ns: 100_000,
            v_back_porch_ns: 507_747,
            hsync_positive: false,
            vsync_positive: true,
            progressive_lines_min: 169,
            progressive_lines_max: 1_374,
            interlaced_lines_min: 0,
            interlaced_lines_max: 0,
        }
    }

    fn generic_15() -> CrtRange {
        CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 15_750_000,
            v_min_mhz: 49_500,
            v_max_mhz: 65_000,
            v_front_porch_ns: 64_000,
            v_sync_ns: 192_000,
            v_back_porch_ns: 1_024_000,
            progressive_lines_min: 192,
            progressive_lines_max: 288,
            interlaced_lines_min: 448,
            interlaced_lines_max: 576,
            ..edid_30_71()
        }
    }

    fn mode(vactive: u16, interlace: bool) -> Modeline {
        Modeline {
            pclock_hz: 10_000_000,
            hactive: 256,
            hsync_start: 264,
            hsync_end: 290,
            htotal: 322,
            vactive,
            vsync_start: vactive + 1,
            vsync_end: vactive + 4,
            vtotal: vactive + 20,
            interlace,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: true,
        }
    }

    #[test]
    fn the_edid_range_forces_480_picture_lines_at_60_hz() {
        // ceil(30000 / 60.099) = 500 lines, ceil(641.081 us x 30 kHz) = 20.
        assert_eq!(forced_picture_lines(&edid_30_71(), 60_099), 480);
        // ceil(30000 / 120.198) = 250, less 20.
        assert_eq!(forced_picture_lines(&edid_30_71(), 120_198), 230);
    }

    #[test]
    fn a_224_line_game_at_60_hz_on_a_30_khz_range_is_padded() {
        let padded = padding(224, &mode(224, false), 60_099, &edid_30_71());
        assert_eq!(
            padded,
            Some(Padding {
                game_lines: 224,
                picture_lines: 480,
                line_rate_min_mhz: 30_000_000,
                field_rate_mhz: 60_099
            })
        );
    }

    #[test]
    fn line_doubling_or_double_refresh_is_not_padded_on_a_30_khz_range() {
        assert_eq!(padding(448, &mode(448, false), 60_099, &edid_30_71()), None);
        assert_eq!(
            padding(224, &mode(224, false), 120_198, &edid_30_71()),
            None
        );
    }

    #[test]
    fn a_224_line_game_on_a_15_khz_range_is_not_padded() {
        // ceil(15625 / 60.099) = 260, ceil(1.28 ms x 15.625 kHz) = 20: 224 of 240.
        assert_eq!(forced_picture_lines(&generic_15(), 60_099), 240);
        assert_eq!(padding(224, &mode(224, false), 60_099, &generic_15()), None);
    }

    #[test]
    fn the_rule_flips_at_75_percent_of_the_picture_lines() {
        // 480 picture lines: 360 is 75 %, 359 is below.
        assert_eq!(padding(360, &mode(360, false), 60_099, &edid_30_71()), None);
        assert!(padding(359, &mode(359, false), 60_099, &edid_30_71()).is_some());
    }

    #[test]
    fn a_game_in_a_border_mode_counts_the_border_lines() {
        // A Game Boy in a 224-line mode at 15 kHz: 144 of 262 - 20 = 242.
        let padded = padding(144, &mode(224, false), 59_728, &generic_15()).unwrap();
        assert_eq!((padded.game_lines, padded.picture_lines), (144, 242));
        // In a 400-line mode the mode's own lines count.
        let padded = padding(288, &mode(400, false), 120_000, &edid_30_71()).unwrap();
        assert_eq!(padded.picture_lines, 400);
    }

    #[test]
    fn an_interlaced_mode_counts_lines_per_field() {
        // 480i on 15 kHz: 240 of 241 per field.
        assert_eq!(padding(480, &mode(480, true), 59_940, &generic_15()), None);
    }

    #[test]
    fn the_rule_holds_at_the_extremes() {
        let range = CrtRange {
            h_min_mhz: u64::MAX,
            v_front_porch_ns: u32::MAX,
            v_sync_ns: u32::MAX,
            v_back_porch_ns: u32::MAX,
            ..edid_30_71()
        };
        assert_eq!(forced_picture_lines(&range, 1), u32::MAX);
        assert_eq!(forced_picture_lines(&range, u32::MAX), 0);
        assert!(padding(1, &mode(1, false), 1, &range).is_some());
    }
}
