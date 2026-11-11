//! The line-rate classes of the hard rails. The build script reads this
//! file too, so that both use the same class bounds.

/// Lowest line rate of the 25 kHz class, in millihertz. Lower rates are in
/// the 15 kHz class.
pub(crate) const CLASS_25_KHZ_MIN_LINE_RATE_MHZ: u64 = 19_000_000;

/// Lowest line rate of the 31 kHz class, in millihertz.
pub(crate) const CLASS_31_KHZ_MIN_LINE_RATE_MHZ: u64 = 28_000_000;

/// Lowest line rate above the 31 kHz class, in millihertz. From this rate
/// up the limits come from the EDID of the monitor, through its profile,
/// and no compiled rail applies.
pub(crate) const EDID_MIN_LINE_RATE_MHZ: u64 = 40_000_000;

/// Each class: its name, and its lowest and highest line rate in
/// millihertz, both inside the class.
pub(crate) const CLASSES: [(&str, u64, u64); 3] = [
    ("15 kHz class", 0, CLASS_25_KHZ_MIN_LINE_RATE_MHZ - 1),
    (
        "25 kHz class",
        CLASS_25_KHZ_MIN_LINE_RATE_MHZ,
        CLASS_31_KHZ_MIN_LINE_RATE_MHZ - 1,
    ),
    (
        "31 kHz class",
        CLASS_31_KHZ_MIN_LINE_RATE_MHZ,
        EDID_MIN_LINE_RATE_MHZ - 1,
    ),
];
