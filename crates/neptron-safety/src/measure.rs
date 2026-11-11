use neptron_types::Modeline;

/// The values of a modeline that the range checks compare, in the units of
/// a `CrtRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Measure {
    pub(crate) line_rate_mhz: u64,
    pub(crate) field_rate_mhz: u64,
    pub(crate) interlace: bool,
    /// Active lines as the tube draws them: both fields of an interlaced
    /// mode, and twice `vactive` for a doublescan mode, as Switchres counts
    /// them against the line limits of a range.
    pub(crate) lines: u16,
    pub(crate) h_front_porch: Span,
    pub(crate) h_sync: Span,
    pub(crate) h_back_porch: Span,
    pub(crate) v_front_porch: Span,
    pub(crate) v_sync: Span,
    pub(crate) v_back_porch: Span,
    /// One line of the vertical values: one scan line, or two for
    /// doublescan. A vertical value of a mode can only change in steps of
    /// one line.
    pub(crate) line: Span,
}

/// A time of `scaled / den` nanoseconds, kept as a fraction so that no
/// rounding happens before a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) scaled: u128,
    pub(crate) den: u128,
}

const NS_PER_S: u128 = 1_000_000_000;

impl Measure {
    /// Measures a modeline that passed `check_structure`: the pixel clock is
    /// not 0 and the timings are in order. The subtractions saturate, so a
    /// modeline that did not pass gives wrong values but no panic.
    pub(crate) fn of(mode: &Modeline) -> Measure {
        let scans: u16 = if mode.doublescan { 2 } else { 1 };
        let fields: u128 = if mode.interlace { 2 } else { 1 };
        let pclock = u128::from(mode.pclock_hz);
        let htotal = u128::from(mode.htotal);
        let horizontal = |pixels: u16| Span {
            scaled: u128::from(pixels) * NS_PER_S,
            den: pclock,
        };
        // An interlaced mode counts the lines of both fields, so one unit
        // is half a scan line. A doublescan mode sends each unit twice.
        let vertical = |units: u16| Span {
            scaled: u128::from(units) * htotal * u128::from(scans) * NS_PER_S,
            den: pclock * fields,
        };
        Measure {
            line_rate_mhz: mode.line_rate_mhz(),
            field_rate_mhz: mode.field_rate_mhz(),
            interlace: mode.interlace,
            lines: mode.vactive.saturating_mul(scans),
            h_front_porch: horizontal(mode.hsync_start.saturating_sub(mode.hactive)),
            h_sync: horizontal(mode.hsync_end.saturating_sub(mode.hsync_start)),
            h_back_porch: horizontal(mode.htotal.saturating_sub(mode.hsync_end)),
            v_front_porch: vertical(mode.vsync_start.saturating_sub(mode.vactive)),
            v_sync: vertical(mode.vsync_end.saturating_sub(mode.vsync_start)),
            v_back_porch: vertical(mode.vtotal.saturating_sub(mode.vsync_end)),
            line: Span {
                scaled: htotal * u128::from(scans) * NS_PER_S,
                den: pclock,
            },
        }
    }
}

impl Span {
    /// A span of exactly `ns` nanoseconds.
    #[cfg(test)]
    pub(crate) fn ns(ns: u64) -> Span {
        Span {
            scaled: u128::from(ns),
            den: 1,
        }
    }

    /// The span rounded down to whole nanoseconds. A span is at least an
    /// integer limit exactly when this value is.
    pub(crate) fn floor_ns(self) -> u64 {
        match self.scaled.checked_div(self.den) {
            Some(ns) => u64::try_from(ns).unwrap_or(u64::MAX),
            None => u64::MAX,
        }
    }

    /// The span rounded up to whole nanoseconds. A span is at most an
    /// integer limit exactly when this value is.
    pub(crate) fn ceil_ns(self) -> u64 {
        if self.den == 0 {
            return u64::MAX;
        }
        u64::try_from(self.scaled.div_ceil(self.den)).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vga() -> Modeline {
        Modeline {
            pclock_hz: 25_175_000,
            hactive: 640,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 525,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn progressive_spans() {
        let m = Measure::of(&vga());
        assert_eq!(m.line_rate_mhz, 31_468_750);
        assert_eq!(m.field_rate_mhz, 59_940);
        assert_eq!(m.lines, 480);
        assert!(!m.interlace);
        // 16 px at 25.175 MHz is 635.55 ns.
        assert_eq!(
            (m.h_front_porch.floor_ns(), m.h_front_porch.ceil_ns()),
            (635, 636)
        );
        // 96 px is 3813.30 ns; 48 px is 1906.65 ns.
        assert_eq!(m.h_sync.floor_ns(), 3_813);
        assert_eq!(m.h_back_porch.floor_ns(), 1_906);
        // One line is 800 px, 31777.56 ns.
        assert_eq!(m.line.floor_ns(), 31_777);
        // 10 lines, 2 lines and 33 lines.
        assert_eq!(m.v_front_porch.floor_ns(), 317_775);
        assert_eq!(m.v_sync.floor_ns(), 63_555);
        assert_eq!(m.v_back_porch.floor_ns(), 1_048_659);
    }

    #[test]
    fn interlaced_units_are_half_lines() {
        let ntsc = Modeline {
            pclock_hz: 13_500_000,
            hactive: 720,
            hsync_start: 739,
            hsync_end: 801,
            htotal: 858,
            vactive: 480,
            vsync_start: 488,
            vsync_end: 494,
            vtotal: 525,
            interlace: true,
            ..vga()
        };
        let m = Measure::of(&ntsc);
        assert_eq!(m.lines, 480);
        assert!(m.interlace);
        // One scan line is 858 px at 13.5 MHz: 63555.55 ns.
        assert_eq!(m.line.floor_ns(), 63_555);
        // 6 units are 3 scan lines: 190666.66 ns.
        assert_eq!(m.v_sync.floor_ns(), 190_666);
        assert_eq!(m.v_sync.ceil_ns(), 190_667);
    }

    #[test]
    fn doublescan_units_are_two_lines() {
        let double = Modeline {
            pclock_hz: 12_587_500,
            hactive: 320,
            hsync_start: 328,
            hsync_end: 376,
            htotal: 400,
            vactive: 240,
            vsync_start: 245,
            vsync_end: 246,
            vtotal: 263,
            doublescan: true,
            ..vga()
        };
        let m = Measure::of(&double);
        assert_eq!(m.lines, 480);
        // One unit is two scan lines of 400 px: 63555.11 ns.
        assert_eq!(m.line.floor_ns(), 63_555);
        assert_eq!(m.v_sync.floor_ns(), 63_555);
        assert_eq!(m.v_front_porch.floor_ns(), 317_775);
    }

    #[test]
    fn largest_values_do_not_overflow() {
        let huge = Modeline {
            pclock_hz: 1,
            hactive: 1,
            hsync_start: 1,
            hsync_end: 1,
            htotal: u16::MAX,
            vactive: u16::MAX,
            vsync_start: u16::MAX,
            vsync_end: u16::MAX,
            vtotal: u16::MAX,
            interlace: false,
            doublescan: true,
            ..vga()
        };
        let m = Measure::of(&huge);
        assert_eq!(m.lines, u16::MAX);
        assert_eq!(m.h_back_porch.floor_ns(), 65_534 * 1_000_000_000);
        assert_eq!(m.line.floor_ns(), 65_535 * 2 * 1_000_000_000);
    }

    #[test]
    fn floor_and_ceil() {
        let exact = Span { scaled: 10, den: 5 };
        assert_eq!((exact.floor_ns(), exact.ceil_ns()), (2, 2));
        let between = Span { scaled: 11, den: 5 };
        assert_eq!((between.floor_ns(), between.ceil_ns()), (2, 3));
        let empty = Span { scaled: 7, den: 0 };
        assert_eq!((empty.floor_ns(), empty.ceil_ns()), (u64::MAX, u64::MAX));
        let beyond = Span {
            scaled: u128::from(u64::MAX) * 4,
            den: 2,
        };
        assert_eq!((beyond.floor_ns(), beyond.ceil_ns()), (u64::MAX, u64::MAX));
        assert_eq!(Span::ns(42).floor_ns(), 42);
    }
}
