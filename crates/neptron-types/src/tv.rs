use serde::{Deserialize, Serialize};

use crate::{Modeline, MonitorKind, MonitorProfile};

/// The pixel clock of every TV mode, in hertz: the 13.5 MHz sample clock
/// of ITU-R BT.601. The composite encoder of the Raspberry Pi runs its
/// clock at 8 times this rate and reads no other.
pub const TV_PCLOCK_HZ: u32 = 13_500_000;

/// 720x240 progressive on the 525-line system, 60.054 Hz.
///
/// The values are `drm_mode_240p` of `drivers/gpu/drm/vc4/vc4_vec.c` in
/// the Raspberry Pi kernel: the encoder takes 262 lines per field at most,
/// so this is the slowest progressive 525-line mode it accepts.
pub const TV_525_240P: Modeline = tv_mode([720, 734, 798, 858], [240, 243, 246, 262], false);

/// 720x480 interlaced on the 525-line system, 59.940 Hz.
///
/// The values are what `drm_analog_tv_mode()` of
/// `drivers/gpu/drm/drm_modes.c` generates for NTSC at 13.5 MHz, the mode
/// the kernel lists for the encoder: BT.601 front porch 16, sync
/// `DIV_ROUND_UP(4700 ns, 74 ns)` = 64, vertical porches 6 and 33 and sync
/// 6 for both fields together.
pub const TV_525_480I: Modeline = tv_mode([720, 736, 800, 858], [480, 486, 492, 525], true);

/// 720x288 progressive on the 625-line system, 50.080 Hz.
///
/// The values are `drm_mode_288p` of `drivers/gpu/drm/vc4/vc4_vec.c`: the
/// encoder takes 312 lines per field at most.
pub const TV_625_288P: Modeline = tv_mode([720, 740, 804, 864], [288, 290, 293, 312], false);

/// 720x576 interlaced on the 625-line system, 50.000 Hz.
///
/// The values are what `drm_analog_tv_mode()` generates for PAL at
/// 13.5 MHz: BT.601 front porch 12, sync 64, vertical porches 5 and 38
/// and sync 6 for both fields together. The kernel shares 43 blank lines
/// with a remainder of -1, and C integer division makes half of it 0.
pub const TV_625_576I: Modeline = tv_mode([720, 732, 796, 864], [576, 581, 587, 625], true);

/// A broadcast standard of a composite encoder: a value of the connector
/// property `TV mode` together with a line system.
///
/// The ten standards are the rows of the encoder table of the Raspberry Pi
/// kernel (`vc4_vec_tv_modes`). No other pair of a `TV mode` value and a
/// line system exists there. On the wire a standard is its [`Self::key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TvStandard {
    /// NTSC with the 7.5 IRE setup, 525 lines.
    #[serde(rename = "ntsc")]
    Ntsc,
    /// NTSC with no setup, the Japanese standard, 525 lines.
    #[serde(rename = "ntsc_j")]
    NtscJ,
    /// NTSC with a 4.43 MHz subcarrier, 525 lines.
    #[serde(rename = "ntsc_443")]
    Ntsc443,
    /// PAL colour on 525 lines (PAL-60).
    #[serde(rename = "pal_60")]
    Pal60,
    /// PAL-M, 525 lines.
    #[serde(rename = "pal_m")]
    PalM,
    /// PAL B, D, G, H and I, 625 lines.
    #[serde(rename = "pal")]
    Pal,
    /// PAL-N, 625 lines.
    #[serde(rename = "pal_n")]
    PalN,
    /// SECAM, 625 lines.
    #[serde(rename = "secam")]
    Secam,
    /// Black and white, no colour burst, 525 lines.
    #[serde(rename = "mono_60")]
    Mono60,
    /// Black and white, no colour burst, 625 lines.
    #[serde(rename = "mono_50")]
    Mono50,
}

impl TvStandard {
    /// Every standard, in the order of the declaration.
    pub const ALL: [TvStandard; 10] = [
        TvStandard::Ntsc,
        TvStandard::NtscJ,
        TvStandard::Ntsc443,
        TvStandard::Pal60,
        TvStandard::PalM,
        TvStandard::Pal,
        TvStandard::PalN,
        TvStandard::Secam,
        TvStandard::Mono60,
        TvStandard::Mono50,
    ];

    /// The lines of a frame of the line system: 525 or 625.
    pub fn lines(self) -> u16 {
        match self {
            TvStandard::Ntsc
            | TvStandard::NtscJ
            | TvStandard::Ntsc443
            | TvStandard::Pal60
            | TvStandard::PalM
            | TvStandard::Mono60 => 525,
            TvStandard::Pal | TvStandard::PalN | TvStandard::Secam | TvStandard::Mono50 => 625,
        }
    }

    /// The name of the value of the connector property `TV mode`, as the
    /// kernel lists it (`drm_tv_mode_enum_list`). PAL-60 and PAL share
    /// `PAL`, and both monochrome standards share `Mono`: the line system
    /// of the mode tells them apart.
    pub fn kms_name(self) -> &'static str {
        match self {
            TvStandard::Ntsc => "NTSC",
            TvStandard::NtscJ => "NTSC-J",
            TvStandard::Ntsc443 => "NTSC-443",
            TvStandard::Pal60 | TvStandard::Pal => "PAL",
            TvStandard::PalM => "PAL-M",
            TvStandard::PalN => "PAL-N",
            TvStandard::Secam => "SECAM",
            TvStandard::Mono60 | TvStandard::Mono50 => "Mono",
        }
    }

    /// The name of the standard in the settings and in files, the same as
    /// its serde name.
    pub fn key(self) -> &'static str {
        match self {
            TvStandard::Ntsc => "ntsc",
            TvStandard::NtscJ => "ntsc_j",
            TvStandard::Ntsc443 => "ntsc_443",
            TvStandard::Pal60 => "pal_60",
            TvStandard::PalM => "pal_m",
            TvStandard::Pal => "pal",
            TvStandard::PalN => "pal_n",
            TvStandard::Secam => "secam",
            TvStandard::Mono60 => "mono_60",
            TvStandard::Mono50 => "mono_50",
        }
    }

    /// The standard whose [`Self::key`] is `key`.
    pub fn from_key(key: &str) -> Option<TvStandard> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }

    /// The two modes of the line system: progressive first, then
    /// interlaced.
    pub fn modes(self) -> [Modeline; 2] {
        if self.lines() == 525 {
            [TV_525_240P, TV_525_480I]
        } else {
            [TV_625_288P, TV_625_576I]
        }
    }

    /// The monitor profile of a TV with this standard. Its id is
    /// `tv_<key>`, so that a record that keeps only the id keeps the
    /// standard too.
    pub fn profile(self) -> MonitorProfile {
        MonitorProfile {
            id: format!("{TV_PROFILE_ID_PREFIX}{}", self.key()),
            kind: MonitorKind::Tv { standard: self },
        }
    }

    /// The standard of a profile id that [`Self::profile`] made.
    pub fn from_profile_id(id: &str) -> Option<TvStandard> {
        id.strip_prefix(TV_PROFILE_ID_PREFIX)
            .and_then(Self::from_key)
    }
}

const TV_PROFILE_ID_PREFIX: &str = "tv_";

const fn tv_mode(h: [u16; 4], v: [u16; 4], interlace: bool) -> Modeline {
    Modeline {
        pclock_hz: TV_PCLOCK_HZ,
        hactive: h[0],
        hsync_start: h[1],
        hsync_end: h[2],
        htotal: h[3],
        vactive: v[0],
        vsync_start: v[1],
        vsync_end: v[2],
        vtotal: v[3],
        interlace,
        doublescan: false,
        hsync_positive: false,
        vsync_positive: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_525_line_modes_hold_the_kernel_values() {
        let p = TV_525_240P;
        assert_eq!(p.pclock_hz, 13_500_000);
        assert_eq!(
            [p.hactive, p.hsync_start, p.hsync_end, p.htotal],
            [720, 734, 798, 858]
        );
        assert_eq!(
            [p.vactive, p.vsync_start, p.vsync_end, p.vtotal],
            [240, 243, 246, 262]
        );
        assert!(!p.interlace);
        let i = TV_525_480I;
        assert_eq!(i.pclock_hz, 13_500_000);
        // hfp 16 (bt601_hfp), sync 64, back porch 858 - 720 - 16 - 64 = 58.
        assert_eq!(
            [i.hactive, i.hsync_start, i.hsync_end, i.htotal],
            [720, 736, 800, 858]
        );
        // vfp 3 + 3, sync 3 + 3, vbp 16 + 17; 525 - 480 - 6 - 6 - 33 = 0.
        assert_eq!(
            [i.vactive, i.vsync_start, i.vsync_end, i.vtotal],
            [480, 486, 492, 525]
        );
        assert!(i.interlace);
    }

    #[test]
    fn the_625_line_modes_hold_the_kernel_values() {
        let p = TV_625_288P;
        assert_eq!(p.pclock_hz, 13_500_000);
        assert_eq!(
            [p.hactive, p.hsync_start, p.hsync_end, p.htotal],
            [720, 740, 804, 864]
        );
        assert_eq!(
            [p.vactive, p.vsync_start, p.vsync_end, p.vtotal],
            [288, 290, 293, 312]
        );
        assert!(!p.interlace);
        let i = TV_625_576I;
        // hfp 12 (bt601_hfp), sync 64, back porch 864 - 720 - 12 - 64 = 68.
        assert_eq!(
            [i.hactive, i.hsync_start, i.hsync_end, i.htotal],
            [720, 732, 796, 864]
        );
        // vfp 3 + 2, sync 3 + 3, vbp 625 - 576 - 5 - 6 = 38.
        assert_eq!(
            [i.vactive, i.vsync_start, i.vsync_end, i.vtotal],
            [576, 581, 587, 625]
        );
        assert!(i.interlace);
    }

    #[test]
    fn every_mode_has_negative_sync_and_no_doublescan() {
        for m in [TV_525_240P, TV_525_480I, TV_625_288P, TV_625_576I] {
            assert!(!m.hsync_positive && !m.vsync_positive && !m.doublescan);
            assert_eq!(m.hactive % 4, 0, "the encoder takes a multiple of 4");
        }
    }

    #[test]
    fn the_pixel_clock_is_a_whole_number_of_khz() {
        // 13 500 000 Hz = 13 500 x 1 000 Hz.
        assert_eq!(TV_PCLOCK_HZ % Modeline::PCLOCK_STEP_HZ, 0);
        for m in [TV_525_240P, TV_525_480I, TV_625_288P, TV_625_576I] {
            assert!(m.pclock_is_whole_khz());
        }
    }

    #[test]
    fn the_field_rates_are_exact_millihertz() {
        // 13 500 000 000 mHz / (858 x 262) = 13 500 000 000 / 224 796
        // = 60 054.45..., rounds down.
        assert_eq!(858 * 262, 224_796);
        assert_eq!(13_500_000_000_u64 / 224_796, 60_054);
        assert_eq!(TV_525_240P.field_rate_mhz(), 60_054);
        // 2 x 13 500 000 000 mHz / (858 x 525) = 27 000 000 000 / 450 450
        // = 59 940.05..., rounds down.
        assert_eq!(858 * 525, 450_450);
        assert_eq!(27_000_000_000_u64 / 450_450, 59_940);
        assert_eq!(TV_525_480I.field_rate_mhz(), 59_940);
        // 13 500 000 000 mHz / (864 x 312) = 13 500 000 000 / 269 568
        // = 50 080.13..., rounds down.
        assert_eq!(864 * 312, 269_568);
        assert_eq!(13_500_000_000_u64 / 269_568, 50_080);
        assert_eq!(TV_625_288P.field_rate_mhz(), 50_080);
        // 2 x 13 500 000 000 mHz / (864 x 625) = 27 000 000 000 / 540 000
        // = 50 000, exact.
        assert_eq!(864 * 625, 540_000);
        assert_eq!(27_000_000_000_u64 % 540_000, 0);
        assert_eq!(TV_625_576I.field_rate_mhz(), 50_000);
    }

    #[test]
    fn the_line_rates_follow_the_line_system() {
        // 13 500 000 000 mHz / 858 = 15 734 265.73..., rounds up.
        assert_eq!(TV_525_240P.line_rate_mhz(), 15_734_266);
        assert_eq!(TV_525_480I.line_rate_mhz(), 15_734_266);
        // 13 500 000 000 mHz / 864 = 15 625 000, exact.
        assert_eq!(TV_625_288P.line_rate_mhz(), 15_625_000);
        assert_eq!(TV_625_576I.line_rate_mhz(), 15_625_000);
    }

    #[test]
    fn each_interlaced_field_matches_the_progressive_field() {
        // DRM halves the vertical values of an interlaced mode for the
        // encoder check, which then sees the same field as the
        // progressive mode.
        let half = |m: Modeline| [m.vactive, m.vsync_start, m.vsync_end, m.vtotal].map(|v| v / 2);
        let field = |m: Modeline| [m.vactive, m.vsync_start, m.vsync_end, m.vtotal];
        assert_eq!(half(TV_525_480I), field(TV_525_240P));
        assert_eq!(half(TV_625_576I), field(TV_625_288P));
    }

    #[test]
    fn the_standards_map_to_their_line_system_and_modes() {
        let lines: Vec<u16> = TvStandard::ALL.iter().map(|s| s.lines()).collect();
        assert_eq!(lines, [525, 525, 525, 525, 525, 625, 625, 625, 525, 625]);
        for s in TvStandard::ALL {
            let [progressive, interlaced] = s.modes();
            assert!(!progressive.interlace && interlaced.interlace);
            let htotal = if s.lines() == 525 { 858 } else { 864 };
            assert_eq!(progressive.htotal, htotal, "{s:?}");
            assert_eq!(interlaced.htotal, htotal, "{s:?}");
            assert_eq!(interlaced.vtotal, s.lines(), "{s:?}");
        }
    }

    #[test]
    fn kms_names_are_the_kernel_enum_names() {
        let names: Vec<&str> = TvStandard::ALL.iter().map(|s| s.kms_name()).collect();
        assert_eq!(
            names,
            [
                "NTSC", "NTSC-J", "NTSC-443", "PAL", "PAL-M", "PAL", "PAL-N", "SECAM", "Mono",
                "Mono"
            ]
        );
    }

    #[test]
    fn keys_are_unique_and_read_back() {
        for s in TvStandard::ALL {
            assert_eq!(TvStandard::from_key(s.key()), Some(s));
        }
        assert_eq!(TvStandard::from_key("auto"), None);
        assert_eq!(TvStandard::from_key("NTSC"), None);
        assert_eq!(TvStandard::from_key(""), None);
    }

    #[test]
    fn a_profile_id_carries_the_standard() {
        let profile = TvStandard::Pal60.profile();
        assert_eq!(profile.id, "tv_pal_60");
        assert_eq!(
            profile.kind,
            MonitorKind::Tv {
                standard: TvStandard::Pal60
            }
        );
        for s in TvStandard::ALL {
            assert_eq!(TvStandard::from_profile_id(&s.profile().id), Some(s));
        }
        assert_eq!(TvStandard::from_profile_id("ntsc"), None);
        assert_eq!(TvStandard::from_profile_id("tv_"), None);
        assert_eq!(TvStandard::from_profile_id("tv_auto"), None);
        assert_eq!(TvStandard::from_profile_id("generic_15"), None);
    }
}
