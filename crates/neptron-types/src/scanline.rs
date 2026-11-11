use serde::{Deserialize, Serialize};

use crate::{Modeline, MonitorKind};

/// The scanlines that the renderer draws over the game picture of a
/// [`Scene`](crate::Scene).
///
/// # Pixel rule
///
/// The effect needs whole output lines. Let `k = dest.h / crop.h`. The
/// renderer draws scanlines only when `dest.h` is exactly `k * crop.h` and
/// `k` is at least 2. Then each source line covers `k` output lines, and
/// output line `j` of the destination (counted from `dest.y`, not from the
/// top of the mode) is line `r = j mod k` of its group. Line `r` is dark
/// when `r >= k - d`, with `d` from [`ScanlineKind::dark_lines`]: the dark
/// lines are the last `d` lines of each group. So the pattern follows the
/// source lines, wherever the destination sits in the mode.
///
/// Each channel of a game pixel on a dark line is the exact value of the
/// pixel times `(100 - strength_percent) / 100`, rounded to the nearest
/// integer, a half rounded up. For nearest scaling that is
/// `(c * (100 - strength_percent) + 50) / 100` in integer division, with
/// `c` the 8-bit channel. For sharp bilinear the factor applies to the
/// exact blend before its one rounding. The renderer works on the 8-bit
/// values as they are, which are gamma encoded, as a hardware line doubler
/// works on the signal.
///
/// Nothing else changes: the pixels of other lines, the black borders, a
/// pixel whose sample is outside the frame, and the UI canvas, which is
/// drawn over the game after the scanlines. With a fractional vertical
/// scale or a scale of 1 there are no scanlines, so a pattern can never
/// beat with the scale. [`Scanlines::NONE`] gives the same output, byte for
/// byte, as a renderer with no scanline support.
///
/// The renderer does not know whether the mode is interlaced, so the caller
/// gives [`Scanlines::NONE`] on an interlaced mode, as [`scanlines_for`]
/// does.
///
/// A renderer refuses a strength above [`Scanlines::MAX_STRENGTH_PERCENT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scanlines {
    /// How the dark lines are chosen.
    pub kind: ScanlineKind,
    /// How much a dark line loses, in whole percent: 100 is black, 0
    /// changes nothing.
    pub strength_percent: u8,
}

/// How the dark lines of [`Scanlines`] are chosen.
///
/// The names are snake_case on the wire, and the list is append-only. A
/// later kind (shaped scanlines, a pattern per multiple, a kind that
/// depends on the resolution) is a new variant. The enum is exhaustive on
/// purpose: a new variant then stops the build of every renderer and every
/// policy that does not handle it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanlineKind {
    /// No scanlines.
    None,
    /// Dark lines with a hard edge, as a hardware line doubler draws them:
    /// in each group of `k` output lines, the last `floor(k / 2)` are dark.
    /// 2x: line 2 of 2. 3x: line 3 of 3. 4x: lines 3 and 4 of 4.
    Black,
}

/// The scanline setting of the user, before the policy applies it.
///
/// The names are the choices of the setting `display.scanlines`, in the
/// same order. The list is append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanlineMode {
    /// Scanlines where the tube cannot draw real ones: a whole vertical
    /// multiple of at least 2 on a CRT at 20 kHz or more. None on a 15 kHz
    /// CRT, whose scanlines are real, and none on a flat panel.
    Auto,
    /// No scanlines.
    Off,
    /// Scanlines at every whole vertical multiple of at least 2, on any
    /// screen.
    On,
}

/// The screen as the scanline rule sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenClass {
    /// A CRT in a mode below [`CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ`]: each
    /// line of the mode is a real scanline with a dark gap below it.
    Crt15khz,
    /// A CRT in a mode at [`CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ`] or more: a
    /// 31 kHz VGA monitor, a multisync monitor in its 25 or 31 kHz range,
    /// or a PC CRT at a higher rate. Its lines are too close to show the
    /// gaps of a 240-line game.
    Crt31khz,
    /// A flat panel.
    FlatPanel,
}

/// The line rate that separates [`ScreenClass::Crt15khz`] from
/// [`ScreenClass::Crt31khz`], in millihertz. 15 kHz sets run from about
/// 15 to 16.5 kHz, and the next class of monitors starts at about 24 kHz.
pub const CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ: u64 = 20_000_000;

impl Scanlines {
    /// No scanlines: the output of a renderer with no scanline support.
    pub const NONE: Scanlines = Scanlines {
        kind: ScanlineKind::None,
        strength_percent: 0,
    };

    /// The largest valid strength: black lines.
    pub const MAX_STRENGTH_PERCENT: u8 = 100;
}

impl ScanlineKind {
    /// The number of dark lines in each group of `multiple` output lines
    /// that show one source line. 0 means no scanlines at that multiple.
    pub fn dark_lines(self, multiple: u32) -> u32 {
        match self {
            ScanlineKind::None => 0,
            ScanlineKind::Black => multiple / 2,
        }
    }
}

impl ScanlineMode {
    /// Every mode, in the order of the setting choices.
    pub const ALL: [ScanlineMode; 3] = [ScanlineMode::Auto, ScanlineMode::Off, ScanlineMode::On];

    /// The choice of the setting `display.scanlines`, and the wire name.
    pub fn key(self) -> &'static str {
        match self {
            ScanlineMode::Auto => "auto",
            ScanlineMode::Off => "off",
            ScanlineMode::On => "on",
        }
    }

    /// The mode of a setting choice, or `None` for an unknown choice.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
}

impl ScreenClass {
    /// The class of `mode` on a monitor of `kind`. A CRT mode is in the
    /// 15 kHz class when its line rate is below
    /// [`CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ`].
    pub fn of(kind: &MonitorKind, mode: &Modeline) -> Self {
        match kind {
            MonitorKind::FlatPanel { .. } => ScreenClass::FlatPanel,
            MonitorKind::Crt { .. } if mode.line_rate_mhz() < CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ => {
                ScreenClass::Crt15khz
            }
            MonitorKind::Crt { .. } => ScreenClass::Crt31khz,
        }
    }
}

/// The scanlines of the game picture: the one rule for the engine and for
/// the preview of the display panel.
///
/// - `mode` and `strength_percent` are the resolved settings
///   `display.scanlines` and `display.scanline_strength`. A strength above
///   100 counts as 100.
/// - `vertical_multiple` is the whole factor from the game lines to the
///   destination lines, or `None` for a fractional scale.
/// - `screen` is the class of the mode on the monitor, and `interlaced`
///   is true for an interlaced mode.
///
/// The result is [`Scanlines::NONE`] for an interlaced mode (a field shows
/// every other line, so a pattern would flicker), for a multiple that is
/// not a whole number of at least 2, for a strength of 0, for the mode
/// `off`, and for the mode `auto` on a 15 kHz CRT or a flat panel.
/// Otherwise it is [`ScanlineKind::Black`] at the strength.
pub fn scanlines_for(
    mode: ScanlineMode,
    strength_percent: u8,
    vertical_multiple: Option<u16>,
    screen: ScreenClass,
    interlaced: bool,
) -> Scanlines {
    let wanted = match mode {
        ScanlineMode::Off => false,
        ScanlineMode::On => true,
        ScanlineMode::Auto => screen == ScreenClass::Crt31khz,
    };
    let strength_percent = strength_percent.min(Scanlines::MAX_STRENGTH_PERCENT);
    let whole = vertical_multiple.is_some_and(|k| k >= 2);
    if !wanted || !whole || interlaced || strength_percent == 0 {
        return Scanlines::NONE;
    }
    Scanlines {
        kind: ScanlineKind::Black,
        strength_percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CrtRange;

    fn black(strength_percent: u8) -> Scanlines {
        Scanlines {
            kind: ScanlineKind::Black,
            strength_percent,
        }
    }

    #[test]
    fn black_darkens_the_last_half_of_each_group() {
        let table: Vec<u32> = (0..=6).map(|k| ScanlineKind::Black.dark_lines(k)).collect();
        assert_eq!(table, [0, 0, 1, 1, 2, 2, 3]);
        assert_eq!(ScanlineKind::None.dark_lines(4), 0);
    }

    #[test]
    fn mode_keys_round_trip() {
        let keys: Vec<_> = ScanlineMode::ALL.iter().map(|m| m.key()).collect();
        assert_eq!(keys, ["auto", "off", "on"]);
        for mode in ScanlineMode::ALL {
            assert_eq!(ScanlineMode::from_key(mode.key()), Some(mode));
        }
        assert_eq!(ScanlineMode::from_key("black"), None);
        assert_eq!(ScanlineMode::from_key("Auto"), None);
    }

    #[test]
    fn auto_draws_only_on_a_31khz_crt() {
        let at = |screen| scanlines_for(ScanlineMode::Auto, 100, Some(2), screen, false);
        assert_eq!(at(ScreenClass::Crt31khz), black(100));
        assert_eq!(at(ScreenClass::Crt15khz), Scanlines::NONE);
        assert_eq!(at(ScreenClass::FlatPanel), Scanlines::NONE);
    }

    #[test]
    fn on_and_off_ignore_the_screen() {
        for screen in [
            ScreenClass::Crt15khz,
            ScreenClass::Crt31khz,
            ScreenClass::FlatPanel,
        ] {
            let on = scanlines_for(ScanlineMode::On, 40, Some(3), screen, false);
            let off = scanlines_for(ScanlineMode::Off, 40, Some(3), screen, false);
            assert_eq!((on, off), (black(40), Scanlines::NONE), "{screen:?}");
        }
    }

    #[test]
    fn only_a_whole_multiple_of_two_or_more_draws() {
        let with = |k| scanlines_for(ScanlineMode::On, 100, k, ScreenClass::Crt31khz, false);
        assert_eq!(with(None), Scanlines::NONE);
        assert_eq!(with(Some(0)), Scanlines::NONE);
        assert_eq!(with(Some(1)), Scanlines::NONE);
        assert_eq!(with(Some(2)), black(100));
        assert_eq!(with(Some(4)), black(100));
    }

    #[test]
    fn interlace_and_zero_strength_draw_nothing() {
        let crt = ScreenClass::Crt31khz;
        assert_eq!(
            scanlines_for(ScanlineMode::On, 100, Some(2), crt, true),
            Scanlines::NONE
        );
        assert_eq!(
            scanlines_for(ScanlineMode::On, 0, Some(2), crt, false),
            Scanlines::NONE
        );
    }

    #[test]
    fn strength_above_100_counts_as_100() {
        let s = scanlines_for(ScanlineMode::On, 250, Some(2), ScreenClass::Crt31khz, false);
        assert_eq!(s, black(100));
    }

    fn mode(pclock_hz: u32, htotal: u16) -> Modeline {
        Modeline {
            pclock_hz,
            hactive: 0,
            hsync_start: 0,
            hsync_end: 0,
            htotal,
            vactive: 0,
            vsync_start: 0,
            vsync_end: 0,
            vtotal: 0,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn screen_class_follows_the_line_rate_of_the_mode() {
        let crt = MonitorKind::Crt {
            ranges: Vec::<CrtRange>::new(),
        };
        let panel = MonitorKind::FlatPanel { modes: Vec::new() };
        // 6.7 MHz over 426 clocks is 15.7 kHz; 25.175 MHz over 800 is 31.5.
        let low = mode(6_700_000, 426);
        let vga = mode(25_175_000, 800);
        assert_eq!(ScreenClass::of(&crt, &low), ScreenClass::Crt15khz);
        assert_eq!(ScreenClass::of(&crt, &vga), ScreenClass::Crt31khz);
        assert_eq!(ScreenClass::of(&panel, &low), ScreenClass::FlatPanel);
        // The limit itself is in the 31 kHz class: 20 MHz over 1000 clocks.
        let edge = mode(20_000_000, 1_000);
        assert_eq!(edge.line_rate_mhz(), CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ);
        assert_eq!(ScreenClass::of(&crt, &edge), ScreenClass::Crt31khz);
    }
}
