use neptron_types::Modeline;

/// One modeline from Switchres, with the score that ranks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Candidate {
    /// The timings.
    pub modeline: Modeline,
    /// Index of the profile range that made the modeline.
    pub range_index: usize,
    /// The source does not scale by a whole number into the mode: the
    /// picture is stretched on at least one axis.
    pub stretched: bool,
    /// The refresh differs from the source by more than the tolerance of the
    /// settings.
    pub refresh_off: bool,
    /// Horizontal scale of the source into the mode, in thousandths.
    pub x_scale_milli: u32,
    /// Vertical scale of the source into the mode, in thousandths.
    pub y_scale_milli: u32,
    /// Refresh that Switchres aims for minus the refresh of the source, in
    /// millihertz. Switchres computes it before it rounds the timings to
    /// whole pixels and lines; `Modeline::field_rate_mhz` gives the rate of
    /// the timings.
    pub refresh_diff_mhz: i64,
}
