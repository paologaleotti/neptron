//! How the window paces flips.
//!
//! The window presents with the desktop's vsync. The desktop refresh rate
//! (often 60 or 120 Hz) does not match a 59.94 Hz or 50 Hz mode, so the
//! window cannot keep the mode rate exactly. It shows each frame for a
//! whole number of desktop refreshes, the number that brings the flip rate
//! nearest to the mode rate, and reports the real time of each flip.

/// The most desktop refreshes that one frame can take.
pub(crate) const MAX_REFRESHES_PER_FLIP: u32 = 8;

/// The desktop refresh rate in millihertz, from the rational rate that SDL
/// reports. It is 0 when the rate is unknown, that is when either part is
/// not positive.
pub(crate) fn refresh_rate_mhz(numerator: i32, denominator: i32) -> u64 {
    match (u64::try_from(numerator), u64::try_from(denominator)) {
        (Ok(num), Ok(den)) if num > 0 && den > 0 => (2 * num * 1_000 + den) / (2 * den),
        _ => 0,
    }
}

/// The desktop refreshes that one flip takes: `desktop_mhz / mode_mhz`,
/// rounded to the nearest whole number with a half rounded up, from 1 to
/// [`MAX_REFRESHES_PER_FLIP`]. An unknown rate (0) gives 1.
pub(crate) fn refreshes_per_flip(desktop_mhz: u64, mode_mhz: u64) -> u32 {
    if desktop_mhz == 0 || mode_mhz == 0 {
        return 1;
    }
    let refreshes = (2 * desktop_mhz + mode_mhz) / (2 * mode_mhz);
    u32::try_from(refreshes)
        .unwrap_or(MAX_REFRESHES_PER_FLIP)
        .clamp(1, MAX_REFRESHES_PER_FLIP)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NTSC_MHZ: u64 = 59_940;
    const PAL_MHZ: u64 = 50_000;

    #[test]
    fn the_rational_rate_becomes_millihertz() {
        assert_eq!(refresh_rate_mhz(60, 1), 60_000);
        assert_eq!(refresh_rate_mhz(60_000, 1_001), 59_940);
        assert_eq!(refresh_rate_mhz(120, 1), 120_000);
    }

    #[test]
    fn an_unknown_rate_is_0() {
        assert_eq!(refresh_rate_mhz(0, 1), 0);
        assert_eq!(refresh_rate_mhz(60, 0), 0);
        assert_eq!(refresh_rate_mhz(-60, 1), 0);
        assert_eq!(refresh_rate_mhz(60, -1), 0);
    }

    #[test]
    fn a_60_hz_desktop_shows_every_frame_once() {
        assert_eq!(refreshes_per_flip(60_000, NTSC_MHZ), 1);
        assert_eq!(refreshes_per_flip(60_000, PAL_MHZ), 1);
    }

    #[test]
    fn a_fast_desktop_shows_each_frame_for_several_refreshes() {
        assert_eq!(refreshes_per_flip(120_000, NTSC_MHZ), 2);
        assert_eq!(refreshes_per_flip(100_000, PAL_MHZ), 2);
        assert_eq!(refreshes_per_flip(144_000, NTSC_MHZ), 2);
        assert_eq!(refreshes_per_flip(165_000, NTSC_MHZ), 3);
        assert_eq!(refreshes_per_flip(240_000, 30_000), MAX_REFRESHES_PER_FLIP);
    }

    #[test]
    fn a_half_rounds_up() {
        assert_eq!(refreshes_per_flip(150_000, 60_000), 3);
        assert_eq!(refreshes_per_flip(149_999, 60_000), 2);
    }

    #[test]
    fn a_slow_desktop_still_takes_one_refresh_per_flip() {
        assert_eq!(refreshes_per_flip(30_000, NTSC_MHZ), 1);
    }

    #[test]
    fn an_unknown_rate_takes_one_refresh_per_flip() {
        assert_eq!(refreshes_per_flip(0, NTSC_MHZ), 1);
        assert_eq!(refreshes_per_flip(60_000, 0), 1);
    }

    #[test]
    fn a_huge_ratio_stops_at_the_limit() {
        assert_eq!(refreshes_per_flip(u64::MAX / 4, 1), MAX_REFRESHES_PER_FLIP);
    }
}
