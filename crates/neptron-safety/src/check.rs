/// One bound that a mode must keep, in the order `Limits::check` tests
/// them. When no range of a profile accepts a mode, the rejection comes from
/// the range that failed at the latest check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Check {
    LineRateMin,
    LineRateMax,
    FieldRateMin,
    FieldRateMax,
    ProgressiveLinesMin,
    ProgressiveLinesMax,
    InterlacedLinesMin,
    InterlacedLinesMax,
    HorizontalSyncMin,
    HorizontalSyncMax,
    VerticalSyncMin,
    VerticalSyncMax,
    HorizontalFrontPorchMin,
    HorizontalBackPorchMin,
    VerticalFrontPorchMin,
    VerticalBackPorchMin,
}

impl Check {
    /// Every check, in declaration order: `ALL[c.index()] == c`.
    #[allow(dead_code, reason = "only the build script and the tests use it")]
    pub(crate) const ALL: [Check; 16] = [
        Check::LineRateMin,
        Check::LineRateMax,
        Check::FieldRateMin,
        Check::FieldRateMax,
        Check::ProgressiveLinesMin,
        Check::ProgressiveLinesMax,
        Check::InterlacedLinesMin,
        Check::InterlacedLinesMax,
        Check::HorizontalSyncMin,
        Check::HorizontalSyncMax,
        Check::VerticalSyncMin,
        Check::VerticalSyncMax,
        Check::HorizontalFrontPorchMin,
        Check::HorizontalBackPorchMin,
        Check::VerticalFrontPorchMin,
        Check::VerticalBackPorchMin,
    ];

    /// The position of the check in `ALL`.
    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// The check refuses values above a limit, not below one.
    pub(crate) fn is_max(self) -> bool {
        matches!(
            self,
            Check::LineRateMax
                | Check::FieldRateMax
                | Check::ProgressiveLinesMax
                | Check::InterlacedLinesMax
                | Check::HorizontalSyncMax
                | Check::VerticalSyncMax
        )
    }

    /// The name of the bound with its unit.
    #[allow(dead_code, reason = "only the build script and the tests use it")]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Check::LineRateMin => "line rate min mHz",
            Check::LineRateMax => "line rate max mHz",
            Check::FieldRateMin => "field rate min mHz",
            Check::FieldRateMax => "field rate max mHz",
            Check::ProgressiveLinesMin => "progressive lines min",
            Check::ProgressiveLinesMax => "progressive lines max",
            Check::InterlacedLinesMin => "interlaced lines min",
            Check::InterlacedLinesMax => "interlaced lines max",
            Check::HorizontalSyncMin => "horizontal sync min ns",
            Check::HorizontalSyncMax => "horizontal sync max ns",
            Check::VerticalSyncMin => "vertical sync min ns",
            Check::VerticalSyncMax => "vertical sync max ns",
            Check::HorizontalFrontPorchMin => "horizontal front porch min ns",
            Check::HorizontalBackPorchMin => "horizontal back porch min ns",
            Check::VerticalFrontPorchMin => "vertical front porch min ns",
            Check::VerticalBackPorchMin => "vertical back porch min ns",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_every_check_in_order() {
        for (i, check) in Check::ALL.into_iter().enumerate() {
            assert_eq!(check.index(), i);
        }
        assert!(Check::ALL.is_sorted());
    }

    #[test]
    fn upper_bounds() {
        let upper: Vec<Check> = Check::ALL.into_iter().filter(|c| c.is_max()).collect();
        assert_eq!(
            upper,
            [
                Check::LineRateMax,
                Check::FieldRateMax,
                Check::ProgressiveLinesMax,
                Check::InterlacedLinesMax,
                Check::HorizontalSyncMax,
                Check::VerticalSyncMax,
            ]
        );
    }

    #[test]
    fn labels_are_unique_and_name_a_bound() {
        let mut labels: Vec<&str> = Check::ALL.into_iter().map(Check::label).collect();
        for (check, label) in Check::ALL.iter().zip(&labels) {
            let bound = if check.is_max() { "max" } else { "min" };
            assert!(label.contains(bound), "{check:?}: {label}");
        }
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), Check::ALL.len());
    }
}
