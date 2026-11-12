use serde::{Deserialize, Serialize};

/// A display mode as integer timings, in the form the hardware gets them.
///
/// The horizontal values count pixels and the vertical values count lines,
/// both from the start of the active area, as in a DRM mode or an X11
/// modeline. The order is `active <= sync_start <= sync_end <= total` on
/// each axis; this type does not enforce it, the safety validator does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modeline {
    /// Pixel clock, in hertz.
    pub pclock_hz: u32,
    /// Visible pixels per line.
    pub hactive: u16,
    /// Pixel at which the horizontal sync pulse starts.
    pub hsync_start: u16,
    /// Pixel at which the horizontal sync pulse ends.
    pub hsync_end: u16,
    /// Pixels per line, blanking included.
    pub htotal: u16,
    /// Visible lines per frame.
    pub vactive: u16,
    /// Line at which the vertical sync pulse starts.
    pub vsync_start: u16,
    /// Line at which the vertical sync pulse ends.
    pub vsync_end: u16,
    /// Lines per frame, blanking included.
    pub vtotal: u16,
    /// The mode is interlaced. The vertical values then count the lines of
    /// both fields together, as in DRM: 480i has `vactive` 480 and `vtotal`
    /// 525.
    pub interlace: bool,
    /// Each line is sent twice. The vertical values count each line once, as
    /// in DRM.
    pub doublescan: bool,
    /// The horizontal sync pulse is positive.
    pub hsync_positive: bool,
    /// The vertical sync pulse is positive.
    pub vsync_positive: bool,
}

impl Modeline {
    /// The step of a pixel clock that a display can take, in hertz. The
    /// kernel's mode record (`struct drm_mode_modeinfo`) holds the clock in
    /// kHz, so a display backend refuses any other clock and never rounds.
    pub const PCLOCK_STEP_HZ: u32 = 1_000;

    /// The pixel clock is a whole number of kHz ([`Self::PCLOCK_STEP_HZ`]),
    /// so a display can take it as it is.
    pub fn pclock_is_whole_khz(&self) -> bool {
        self.pclock_hz.is_multiple_of(Self::PCLOCK_STEP_HZ)
    }

    /// The horizontal line rate, in millihertz: `pclock_hz / htotal`.
    ///
    /// The result is rounded to the nearest millihertz, with a half rounded
    /// up. Doublescan does not change the line rate. A modeline with
    /// `htotal` 0 has no line rate, and the function returns 0 for it.
    pub fn line_rate_mhz(&self) -> u64 {
        div_round(u64::from(self.pclock_hz) * MILLI, u64::from(self.htotal))
    }

    /// The vertical field rate, in millihertz: the rate of vertical sync
    /// pulses on the wire.
    ///
    /// The value is `pclock_hz / (htotal * vtotal)`, times 2 for an
    /// interlaced mode (two fields per frame) and divided by 2 for a
    /// doublescan mode (each line sent twice). This is the refresh rule of
    /// DRM. The result is rounded to the nearest millihertz, with a half
    /// rounded up. A modeline with `htotal` or `vtotal` 0 has no field rate,
    /// and the function returns 0 for it.
    pub fn field_rate_mhz(&self) -> u64 {
        let fields_per_frame = if self.interlace { 2 } else { 1 };
        let scans_per_line = if self.doublescan { 2 } else { 1 };
        // The numerator is below 2^33 * 1000 and the denominator below
        // 2^33, so neither product can overflow a u64.
        let numerator = u64::from(self.pclock_hz) * MILLI * fields_per_frame;
        let denominator = u64::from(self.htotal) * u64::from(self.vtotal) * scans_per_line;
        div_round(numerator, denominator)
    }
}

const MILLI: u64 = 1_000;

/// `numerator / denominator`, rounded to the nearest integer with a half
/// rounded up; 0 when `denominator` is 0.
fn div_round(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    // The callers keep both values below 2^43, so the sum cannot overflow.
    (numerator + denominator / 2) / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(pclock_hz: u32, htotal: u16, vtotal: u16) -> Modeline {
        Modeline {
            pclock_hz,
            hactive: 0,
            hsync_start: 0,
            hsync_end: 0,
            htotal,
            vactive: 0,
            vsync_start: 0,
            vsync_end: 0,
            vtotal,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn vga_640x480_progressive() {
        let m = mode(25_175_000, 800, 525);
        // 25 175 000 Hz / 800 = 31 468.75 Hz, exact.
        assert_eq!(m.line_rate_mhz(), 31_468_750);
        // 25 175 000 000 mHz / 420 000 = 59 940.476..., rounds down.
        assert_eq!(m.field_rate_mhz(), 59_940);
    }

    #[test]
    fn ntsc_480i_counts_two_fields_per_frame() {
        let m = Modeline {
            interlace: true,
            ..mode(13_500_000, 858, 525)
        };
        // 13 500 000 000 mHz / 858 = 15 734 265.73..., rounds up.
        assert_eq!(m.line_rate_mhz(), 15_734_266);
        // 2 * 13 500 000 000 mHz / 450 450 = 59 940.059..., rounds down.
        assert_eq!(m.field_rate_mhz(), 59_940);
    }

    #[test]
    fn doublescan_halves_the_field_rate_and_keeps_the_line_rate() {
        let single = mode(12_587_500, 400, 525);
        let double = Modeline {
            doublescan: true,
            ..mode(12_587_500, 400, 525)
        };
        assert_eq!(single.line_rate_mhz(), 31_468_750);
        assert_eq!(double.line_rate_mhz(), 31_468_750);
        // 12 587 500 000 mHz / 210 000 = 59 940.476..., rounds down.
        assert_eq!(single.field_rate_mhz(), 59_940);
        // 12 587 500 000 mHz / 420 000 = 29 970.238..., rounds down.
        assert_eq!(double.field_rate_mhz(), 29_970);
    }

    #[test]
    fn interlace_and_doublescan_cancel() {
        let both = Modeline {
            interlace: true,
            doublescan: true,
            ..mode(12_000_000, 800, 500)
        };
        // 12 000 000 Hz / 400 000 = 30 Hz, exact.
        assert_eq!(both.field_rate_mhz(), 30_000);
    }

    #[test]
    fn half_rounds_up() {
        // 1 Hz / 2 000 = 0.5 mHz rounds up; 1 Hz / 3 000 = 0.33 mHz rounds down.
        assert_eq!(mode(1, 2000, 1).line_rate_mhz(), 1);
        assert_eq!(mode(1, 3000, 1).line_rate_mhz(), 0);
    }

    #[test]
    fn only_a_clock_in_whole_khz_is_whole_khz() {
        assert!(mode(25_175_000, 800, 525).pclock_is_whole_khz());
        assert!(mode(1_000, 800, 525).pclock_is_whole_khz());
        assert!(mode(0, 800, 525).pclock_is_whole_khz());
        assert!(!mode(25_466_400, 800, 525).pclock_is_whole_khz());
        assert!(!mode(25_175_001, 800, 525).pclock_is_whole_khz());
        assert!(!mode(999, 800, 525).pclock_is_whole_khz());
        assert!(!mode(u32::MAX, 800, 525).pclock_is_whole_khz());
    }

    #[test]
    fn zero_totals_give_zero() {
        assert_eq!(mode(25_175_000, 0, 525).line_rate_mhz(), 0);
        assert_eq!(mode(25_175_000, 0, 525).field_rate_mhz(), 0);
        assert_eq!(mode(25_175_000, 800, 0).field_rate_mhz(), 0);
    }

    #[test]
    fn largest_values_do_not_overflow() {
        let m = Modeline {
            interlace: true,
            ..mode(u32::MAX, 1, 1)
        };
        assert_eq!(m.line_rate_mhz(), u64::from(u32::MAX) * 1_000);
        assert_eq!(m.field_rate_mhz(), u64::from(u32::MAX) * 2_000);
        let slow = Modeline {
            doublescan: true,
            ..mode(1, u16::MAX, u16::MAX)
        };
        assert_eq!(slow.field_rate_mhz(), 0);
    }
}
