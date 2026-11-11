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
