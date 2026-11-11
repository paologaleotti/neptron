use neptron_types::Scanlines;

/// The percent of a channel that a line keeps when it is not dark.
pub(crate) const FULL_PERCENT: u32 = 100;

/// The dark lines of the game destination: in each group of `period`
/// destination lines, the lines from `first_dark` on keep `keep_percent`
/// of each channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pattern {
    pub(crate) period: u32,
    pub(crate) first_dark: u32,
    pub(crate) keep_percent: u32,
}

impl Pattern {
    /// The pattern of `scanlines` for a crop of `crop_h` lines in a
    /// destination of `dest_h` lines, or `None` when no line is dark: a
    /// scale that is not a whole multiple of at least 2, a kind with no dark
    /// line at that multiple, or a strength of 0. The caller has checked
    /// the strength.
    pub(crate) fn of(scanlines: Scanlines, crop_h: u32, dest_h: u32) -> Option<Pattern> {
        if crop_h == 0 || !dest_h.is_multiple_of(crop_h) {
            return None;
        }
        let period = dest_h / crop_h;
        let dark = scanlines.kind.dark_lines(period);
        let keep_percent = FULL_PERCENT.saturating_sub(u32::from(scanlines.strength_percent));
        if period < 2 || dark == 0 || keep_percent == FULL_PERCENT {
            return None;
        }
        Some(Pattern {
            period,
            first_dark: period - dark.min(period),
            keep_percent,
        })
    }

    /// The percent that destination line `j` keeps.
    pub(crate) fn keep(&self, j: u32) -> u32 {
        if j % self.period >= self.first_dark {
            self.keep_percent
        } else {
            FULL_PERCENT
        }
    }
}

#[cfg(test)]
mod tests {
    use neptron_types::ScanlineKind;

    use super::*;

    fn black(strength_percent: u8) -> Scanlines {
        Scanlines {
            kind: ScanlineKind::Black,
            strength_percent,
        }
    }

    fn keeps(p: Option<Pattern>, lines: u32) -> Vec<u32> {
        (0..lines)
            .map(|j| p.map_or(FULL_PERCENT, |p| p.keep(j)))
            .collect()
    }

    #[test]
    fn two_x_darkens_the_second_line_of_each_pair() {
        let p = Pattern::of(black(100), 3, 6);
        assert_eq!(keeps(p, 6), [100, 0, 100, 0, 100, 0]);
        let p = Pattern::of(black(30), 3, 6);
        assert_eq!(keeps(p, 4), [100, 70, 100, 70]);
    }

    #[test]
    fn three_x_darkens_one_line_in_three() {
        let p = Pattern::of(black(100), 2, 6);
        assert_eq!(keeps(p, 6), [100, 100, 0, 100, 100, 0]);
    }

    #[test]
    fn four_x_darkens_the_last_two_lines_of_four() {
        let p = Pattern::of(black(100), 2, 8);
        assert_eq!(keeps(p, 8), [100, 100, 0, 0, 100, 100, 0, 0]);
    }

    #[test]
    fn no_pattern_without_a_whole_multiple_of_two() {
        assert_eq!(Pattern::of(black(100), 2, 5), None);
        assert_eq!(Pattern::of(black(100), 4, 4), None);
        assert_eq!(Pattern::of(black(100), 4, 2), None);
        assert_eq!(Pattern::of(black(100), 0, 4), None);
        assert_eq!(Pattern::of(black(100), 3, 0), None);
    }

    #[test]
    fn no_pattern_for_none_or_zero_strength() {
        assert_eq!(Pattern::of(Scanlines::NONE, 2, 4), None);
        assert_eq!(Pattern::of(black(0), 2, 4), None);
        let none_at_full = Scanlines {
            kind: ScanlineKind::None,
            strength_percent: 100,
        };
        assert_eq!(Pattern::of(none_at_full, 2, 4), None);
    }
}
