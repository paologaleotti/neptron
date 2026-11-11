use std::fmt;

use neptron_types::CrtRange;

use super::RefreshPolicy;

/// "Refresh matches the core to two decimals": the field rate of a mode
/// matches its target when they differ by less than this, in millihertz.
/// Two values that differ by less than half of 0.01 Hz can round to the
/// same two decimals; 5 mHz or more can never.
pub const REFRESH_MATCH_MHZ: u64 = 5;

/// The rate of `force_50`, in millihertz.
pub const FORCE_50_MHZ: u32 = 50_000;

/// The rate of `force_60`, in millihertz.
pub const FORCE_60_MHZ: u32 = 60_000;

/// Where the field rate of a candidate comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefreshKind {
    /// The game's own rate.
    Exact,
    /// Twice the game's rate; each game frame is shown twice.
    Double,
    /// Twice the game's rate, which the `native` style takes on its own
    /// when the refresh policy did not ask for it: each game frame is shown
    /// twice, and the game speed does not change. The notice says so.
    NativeDouble,
    /// `force_50`: the game speed changes by the ratio.
    Forced50,
    /// `force_60`: the game speed changes by the ratio.
    Forced60,
    /// No range covers the game's rate: the nearest rate a range covers.
    /// The game speed changes by the ratio.
    Nearest,
    /// The last resort: the rate that Switchres reaches, whatever it is.
    Unmatched,
    /// A flat panel: the rate of the panel mode. Audio rate control absorbs
    /// the difference.
    Panel,
}

/// The field rate a candidate aims for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefreshTarget {
    /// Where the rate comes from.
    pub kind: RefreshKind,
    /// The field rate, in millihertz.
    pub rate_mhz: u32,
    /// How many fields show one game frame: 2 for double refresh, else 1.
    pub repeat: u8,
}

/// A refresh that the policy asks for but the profile does not cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkippedTarget {
    /// The refresh that was asked for.
    pub kind: RefreshKind,
    /// Its field rate, in millihertz.
    pub rate_mhz: u64,
}

/// The refresh targets of a CRT profile, in the order the policy walks
/// them, and the ones it skips because no range covers them.
///
/// Double refresh and a forced rate are offered only where a range covers
/// the rate; when none does, the policy falls back to the game's rate, and
/// the skipped target is the reason. The exact target follows a double or
/// forced one, so a game whose lines cannot reach the doubled or forced
/// rate still gets its own. A game rate that no range covers becomes the
/// nearest rate that one covers.
pub(crate) fn refresh_targets(
    policy: RefreshPolicy,
    game_mhz: u32,
    ranges: &[CrtRange],
) -> (Vec<RefreshTarget>, Vec<SkippedTarget>) {
    let mut targets = Vec::new();
    let mut skipped = Vec::new();
    let asked = match policy {
        RefreshPolicy::Exact => None,
        RefreshPolicy::ExactX2 => Some((RefreshKind::Double, u64::from(game_mhz) * 2, 2)),
        RefreshPolicy::Force50 => Some((RefreshKind::Forced50, u64::from(FORCE_50_MHZ), 1)),
        RefreshPolicy::Force60 => Some((RefreshKind::Forced60, u64::from(FORCE_60_MHZ), 1)),
    };
    if let Some((kind, rate_mhz, repeat)) = asked {
        match u32::try_from(rate_mhz) {
            Ok(rate) if covers(ranges, rate_mhz) => targets.push(RefreshTarget {
                kind,
                rate_mhz: rate,
                repeat,
            }),
            _ => skipped.push(SkippedTarget { kind, rate_mhz }),
        }
    }
    targets.push(game_target(game_mhz, ranges));
    (targets, skipped)
}

/// The extra target of the `native` style: twice the game's rate, for a
/// monitor that cannot show the game's lines at its own rate. `None` when
/// the refresh policy asks for double refresh already, or when no range
/// covers the rate. The refresh policy did not ask for it, so a rate that
/// no range covers is not a skipped target.
pub(crate) fn native_double_target(
    policy: RefreshPolicy,
    game_mhz: u32,
    ranges: &[CrtRange],
) -> Option<RefreshTarget> {
    if policy == RefreshPolicy::ExactX2 {
        return None;
    }
    let rate_mhz = u64::from(game_mhz) * 2;
    let rate = u32::try_from(rate_mhz).ok()?;
    covers(ranges, rate_mhz).then_some(RefreshTarget {
        kind: RefreshKind::NativeDouble,
        rate_mhz: rate,
        repeat: 2,
    })
}

/// The target for the game's own rate: exact when a range covers it, else
/// the nearest rate that a range covers.
pub(crate) fn game_target(game_mhz: u32, ranges: &[CrtRange]) -> RefreshTarget {
    if covers(ranges, u64::from(game_mhz)) {
        return RefreshTarget {
            kind: RefreshKind::Exact,
            rate_mhz: game_mhz,
            repeat: 1,
        };
    }
    RefreshTarget {
        kind: RefreshKind::Nearest,
        rate_mhz: nearest_covered(game_mhz, ranges),
        repeat: 1,
    }
}

/// Some range has `rate_mhz` inside its field rate limits, both ends
/// included, as the validator checks them.
pub(crate) fn covers(ranges: &[CrtRange], rate_mhz: u64) -> bool {
    ranges
        .iter()
        .any(|r| r.v_min_mhz <= rate_mhz && rate_mhz <= r.v_max_mhz)
}

/// The covered rate nearest to `rate_mhz`; on a tie, the lower one. With
/// no range, the rate itself.
fn nearest_covered(rate_mhz: u32, ranges: &[CrtRange]) -> u32 {
    let rate = u64::from(rate_mhz);
    ranges
        .iter()
        .map(|r| rate.clamp(r.v_min_mhz, r.v_max_mhz.max(r.v_min_mhz)))
        .min_by_key(|&near| (near.abs_diff(rate), near))
        .and_then(|near| u32::try_from(near).ok())
        .unwrap_or(rate_mhz)
}

impl fmt::Display for RefreshKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exact => "exact",
            Self::Double => "double refresh",
            Self::NativeDouble => "double refresh for native",
            Self::Forced50 => "forced 50 Hz",
            Self::Forced60 => "forced 60 Hz",
            Self::Nearest => "nearest in range",
            Self::Unmatched => "unmatched",
            Self::Panel => "panel rate",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(v_min_mhz: u64, v_max_mhz: u64) -> CrtRange {
        CrtRange {
            h_min_mhz: 15_625_000,
            h_max_mhz: 15_750_000,
            v_min_mhz,
            v_max_mhz,
            h_front_porch_ns: 2_000,
            h_sync_ns: 4_700,
            h_back_porch_ns: 8_000,
            v_front_porch_ns: 64_000,
            v_sync_ns: 192_000,
            v_back_porch_ns: 1_024_000,
            hsync_positive: false,
            vsync_positive: false,
            progressive_lines_min: 192,
            progressive_lines_max: 288,
            interlaced_lines_min: 448,
            interlaced_lines_max: 576,
        }
    }

    fn target(kind: RefreshKind, rate_mhz: u32, repeat: u8) -> RefreshTarget {
        RefreshTarget {
            kind,
            rate_mhz,
            repeat,
        }
    }

    const NES: u32 = 60_099;

    #[test]
    fn exact_is_the_game_rate_when_a_range_covers_it() {
        let (targets, skipped) =
            refresh_targets(RefreshPolicy::Exact, NES, &[range(49_500, 65_000)]);
        assert_eq!(targets, [target(RefreshKind::Exact, NES, 1)]);
        assert!(skipped.is_empty());
    }

    #[test]
    fn exact_outside_every_range_is_the_nearest_covered_rate() {
        // 70 Hz DOS on a TV.
        let (targets, _) = refresh_targets(RefreshPolicy::Exact, 70_086, &[range(49_500, 65_000)]);
        assert_eq!(targets, [target(RefreshKind::Nearest, 65_000, 1)]);
        let (targets, _) = refresh_targets(RefreshPolicy::Exact, 30_000, &[range(49_500, 65_000)]);
        assert_eq!(targets, [target(RefreshKind::Nearest, 49_500, 1)]);
    }

    #[test]
    fn the_nearest_rate_takes_the_closest_range_and_the_lower_rate_on_a_tie() {
        let ranges = [range(100_000, 130_000), range(50_000, 65_000)];
        assert_eq!(nearest_covered(70_000, &ranges), 65_000);
        assert_eq!(nearest_covered(90_000, &ranges), 100_000);
        let tie = [range(100_000, 130_000), range(50_000, 80_000)];
        assert_eq!(nearest_covered(90_000, &tie), 80_000);
    }

    #[test]
    fn exact_x2_doubles_where_a_range_covers_it_then_falls_back_to_exact() {
        let pc_31_120 = [range(100_000, 130_000), range(50_000, 65_000)];
        let (targets, skipped) = refresh_targets(RefreshPolicy::ExactX2, NES, &pc_31_120);
        assert_eq!(
            targets,
            [
                target(RefreshKind::Double, 120_198, 2),
                target(RefreshKind::Exact, NES, 1)
            ]
        );
        assert!(skipped.is_empty());
    }

    #[test]
    fn exact_x2_that_no_range_covers_is_skipped_with_its_rate() {
        let (targets, skipped) =
            refresh_targets(RefreshPolicy::ExactX2, NES, &[range(49_500, 65_000)]);
        assert_eq!(targets, [target(RefreshKind::Exact, NES, 1)]);
        assert_eq!(
            skipped,
            [SkippedTarget {
                kind: RefreshKind::Double,
                rate_mhz: 120_198
            }]
        );
    }

    #[test]
    fn a_forced_rate_is_offered_only_where_a_range_covers_it() {
        let tv = [range(49_500, 65_000)];
        let (targets, _) = refresh_targets(RefreshPolicy::Force50, NES, &tv);
        assert_eq!(
            targets,
            [
                target(RefreshKind::Forced50, 50_000, 1),
                target(RefreshKind::Exact, NES, 1)
            ]
        );
        let (targets, _) = refresh_targets(RefreshPolicy::Force60, 50_007, &tv);
        assert_eq!(targets[0], target(RefreshKind::Forced60, 60_000, 1));

        let ntsc_only = [range(59_000, 61_000)];
        let (targets, skipped) = refresh_targets(RefreshPolicy::Force50, NES, &ntsc_only);
        assert_eq!(targets, [target(RefreshKind::Exact, NES, 1)]);
        assert_eq!(
            skipped,
            [SkippedTarget {
                kind: RefreshKind::Forced50,
                rate_mhz: 50_000
            }]
        );
    }

    #[test]
    fn a_rate_at_a_range_limit_is_covered() {
        assert!(covers(&[range(49_500, 65_000)], 49_500));
        assert!(covers(&[range(49_500, 65_000)], 65_000));
        assert!(!covers(&[range(49_500, 65_000)], 65_001));
    }

    #[test]
    fn native_gets_twice_the_game_rate_where_a_range_covers_it() {
        let pc_31_120 = [range(100_000, 130_000), range(50_000, 65_000)];
        assert_eq!(
            native_double_target(RefreshPolicy::Exact, NES, &pc_31_120),
            Some(target(RefreshKind::NativeDouble, 120_198, 2))
        );
        assert_eq!(
            native_double_target(RefreshPolicy::Force50, NES, &pc_31_120),
            Some(target(RefreshKind::NativeDouble, 120_198, 2))
        );
        assert_eq!(
            native_double_target(RefreshPolicy::Exact, NES, &[range(49_500, 65_000)]),
            None
        );
    }

    #[test]
    fn native_gets_no_second_double_target_with_exact_x2() {
        let pc_31_120 = [range(100_000, 130_000), range(50_000, 65_000)];
        assert_eq!(
            native_double_target(RefreshPolicy::ExactX2, NES, &pc_31_120),
            None
        );
        assert_eq!(
            native_double_target(RefreshPolicy::Exact, u32::MAX, &[range(0, u64::MAX)]),
            None
        );
    }

    #[test]
    fn a_doubled_rate_beyond_u32_is_skipped() {
        let (targets, skipped) =
            refresh_targets(RefreshPolicy::ExactX2, u32::MAX, &[range(0, u64::MAX)]);
        assert_eq!(targets, [target(RefreshKind::Exact, u32::MAX, 1)]);
        assert_eq!(skipped.len(), 1);
    }
}
