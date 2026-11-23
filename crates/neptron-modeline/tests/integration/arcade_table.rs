//! The arcade policy table: real arcade boards, vertical and horizontal,
//! through the policy as it is, on every preset of data/monitors, on
//! profiles built from the EDID range limits of typical PC tubes, on flat
//! panels and on the composite TV standards.
//!
//! The test checks that every cell has a plan and that every CRT winner
//! has a whole-kHz pixel clock, and prints the table.
//! docs/spec/arcade-table.md holds a copy. To make it again:
//!
//! ```text
//! cargo xtask test -p neptron-modeline --no-capture arcade_table
//! ```
//!
//! The table is a report of the policy as it is, not a golden: a cell can
//! show a result that the arcade rule of the spec does not want. The cells
//! that follow the rule are goldens in tests/golden/policy_arcade.toml.

use std::collections::BTreeMap;

use neptron_modeline::policy::{
    Considered, EXACT_SPEED_MARGIN_PPM, Plan, PolicySettings, Proposal, RefreshKind, Rung, choose,
    decide,
};
use neptron_safety::validate;
use neptron_types::{
    CrtRange, Filter, GpuCaps, Modeline, MonitorKind, MonitorProfile, Rotation, SourceGeometry,
    TvStandard,
};

use crate::presets::{gpu_table, open_caps, panel_1080, pc_crt_30_71, preset};

/// An arcade board as FinalBurn Neo gives it: the frame as drawn, the rate
/// cut to 0.01 Hz, and the turn it asks for. The display aspect of every
/// board is 4:3 of the frame as drawn: FBNeo reports the upright 3:4 of a
/// vertical board, and the session inverts it (docs/spec/arcade.md,
/// section 11).
///
/// The wider table of every source class uses the same type: a console, a
/// handheld or a computer is a board with `arcade` false, its own display
/// aspect and its scan.
pub struct Board {
    pub name: &'static str,
    pub width: u16,
    pub height: u16,
    pub refresh_mhz: u32,
    pub rotation: Rotation,
    pub interlaced: bool,
    /// The display aspect of the frame as drawn, in thousandths.
    pub aspect_milli: u32,
    /// The source is an arcade board (PolicySettings::arcade).
    pub arcade: bool,
    /// The class of the summary in the spec.
    pub class: &'static str,
}

/// The reference boards, then the hard cases. Unless the comment names
/// MAME, the size is the one of the FBNeo driver entry and the rate is the
/// one FBNeo reports: its `BurnSetRefreshRate` value cut to 0.01 Hz, or
/// 60.00 Hz when the driver sets none (`nBurnFPS` defaults to 6000 in
/// `burn.cpp`). Read at libretro/FBNeo master on 2026-11-16.
pub const BOARDS: [Board; 18] = [
    // Cave 68000, DoDonPachi (d_dodonpachi.cpp): 240x320 upright,
    // CAVE_REFRESHRATE 15625 / 271.5 Hz.
    board("ddonpach", 320, 240, 57_550, Rotation::R90),
    // Toaplan 2, Batsugun (d_batsugun.cpp): 240x320, TOA_REFRESHRATE
    // 59.637405 Hz.
    board("batsugun", 320, 240, 59_630, Rotation::R90),
    // Psikyo, Gunbird (d_psikyo.cpp): 224x320, 15625 / 263.5 Hz.
    board("gunbird", 320, 224, 59_290, Rotation::R90),
    // Taito F3, Gunlock (d_taitof3.cpp): 224x320, vertical and flipped,
    // 58.943844 Hz.
    board("gunlock", 320, 224, 58_940, Rotation::R270),
    // Pac-Man (d_pacman.cpp): 224x288, vertical and flipped. FBNeo sets
    // no rate and reports 60.00 Hz; the board runs at 60.606 Hz.
    board("pacman", 288, 224, 60_000, Rotation::R270),
    // The same board at its own rate, 6.144 MHz / 384 / 264 (the MAME
    // timing of tests/golden/systems.toml), as a core that reports it
    // gives it.
    board("pacman_60606", 288, 224, 60_606, Rotation::R270),
    // Donkey Kong (d_dkong.cpp): 224x256, vertical and flipped, 60.00 Hz
    // reported. Galaga (d_galaga.cpp, 224x288) gives the same cells as
    // Pac-Man.
    board("dkong", 256, 224, 60_000, Rotation::R270),
    // Sega System 24, Gain Ground (MAME segas24.cpp, ROT270; no FBNeo
    // driver): 384x496 upright, 16 MHz / 656 / 424 = 57.524 Hz, 24.4 kHz.
    board("gground", 496, 384, 57_524, Rotation::R270),
    // NAOMI class, Ikaruga (MAME naomi.cpp, ROT270; no FBNeo driver):
    // 480x640 upright. 60.00 Hz is the nominal VGA rate. UNVERIFIED: the
    // MAME raw screen (26.917 MHz, 820 x 532) gives 61.70 Hz.
    board("ikaruga", 640, 480, 60_000, Rotation::R270),
    // Neo Geo: 304 wide in FBNeo (docs/spec/arcade.md, OP-099), 59.18 Hz.
    board("neogeo", 304, 224, 59_180, Rotation::R0),
    // CPS-1 and CPS-2: 384x224 at 59.63 Hz.
    board("cps2", 384, 224, 59_630, Rotation::R0),
    // Cave horizontal, Power Instinct 2 (d_pwrinst2.cpp): 320x240.
    board("pwrinst2", 320, 240, 57_550, Rotation::R0),
    // Taito F3, Darius Gaiden (d_taitof3.cpp): 320x232, 58.943844 Hz.
    board("dariusg", 320, 232, 58_940, Rotation::R0),
    // Irem M72, R-Type (d_m72.cpp): 384x256, 55.00 Hz.
    board("rtype", 384, 256, 55_000, Rotation::R0),
    // Midway Y Unit, Mortal Kombat (d_yunit.cpp): 400x256, 54.70 Hz.
    board("mk", 400, 256, 54_700, Rotation::R0),
    // CPS-3 wide mode, SFIII 3rd Strike widescreen (cps3run.cpp):
    // 496x224, 59.59949 Hz.
    board("sfiii3ws", 496, 224, 59_590, Rotation::R0),
    // Sega System 24, Bonanza Bros (MAME segas24.cpp, ROT0): 496x384.
    board("bnzabros", 496, 384, 57_524, Rotation::R0),
    // Little Robin (d_littlerb.cpp): 338x288 at 30.00 Hz.
    board("littlerb", 338, 288, 30_000, Rotation::R0),
];

const fn board(
    name: &'static str,
    width: u16,
    height: u16,
    refresh_mhz: u32,
    rotation: Rotation,
) -> Board {
    Board {
        name,
        width,
        height,
        refresh_mhz,
        rotation,
        interlaced: false,
        aspect_milli: 1_333,
        arcade: true,
        class: if matches!(rotation, Rotation::R90 | Rotation::R270) {
            "vertical arcade"
        } else {
            "horizontal arcade"
        },
    }
}

/// A source that is not an arcade board.
const fn other(
    class: &'static str,
    name: &'static str,
    (width, height, interlaced): (u16, u16, bool),
    refresh_mhz: u32,
    aspect_milli: u32,
) -> Board {
    Board {
        name,
        width,
        height,
        refresh_mhz,
        rotation: Rotation::R0,
        interlaced,
        aspect_milli,
        arcade: false,
        class,
    }
}

/// The consoles, handhelds and computers of the wider table, and the
/// extremes of the arcade rates. The systems of tests/golden/systems.toml
/// keep their geometry and 4:3 (the test
/// `the_wider_table_agrees_with_the_console_goldens` holds them to
/// policy.toml); a handheld also comes with its own display aspect. A
/// value marked UNVERIFIED is the common figure of the core, not checked
/// for this table.
pub const SOURCES: [Board; 44] = [
    other("console", "nes_ntsc", (256, 240, false), 60_099, 1_333),
    other("console", "snes_ntsc", (256, 224, false), 60_099, 1_333),
    // The SNES high-resolution modes: the SNES frame timing (snes9x and
    // bsnes give them as 512 wide, and 448 interlaced lines).
    other(
        "console hi-res",
        "snes_512",
        (512, 224, false),
        60_099,
        1_333,
    ),
    other(
        "console hi-res",
        "snes_512i",
        (512, 448, true),
        60_099,
        1_333,
    ),
    other("console", "md_ntsc_h32", (256, 224, false), 59_923, 1_333),
    other("console", "md_ntsc_h40", (320, 224, false), 59_923, 1_333),
    // Mega Drive interlace mode 2 (Sonic 2 two players): 448 lines at the
    // NTSC frame timing. UNVERIFIED: the rate the core reports.
    other("console hi-res", "md_448i", (320, 448, true), 59_923, 1_333),
    other("console", "sms_ntsc", (256, 192, false), 59_923, 1_333),
    other("console", "pc_engine", (256, 240, false), 59_826, 1_333),
    // PC Engine 336 and 512 wide: the same frame timing, other dot clocks.
    other("console", "pce_336", (336, 240, false), 59_826, 1_333),
    other(
        "console hi-res",
        "pce_512",
        (512, 240, false),
        59_826,
        1_333,
    ),
    // PlayStation NTSC: 263 lines at 59.826 Hz progressive, 59.940 Hz
    // interlaced. UNVERIFIED: the rates the core reports.
    other("console", "psx_256", (256, 240, false), 59_826, 1_333),
    other("console", "psx_320", (320, 240, false), 59_826, 1_333),
    other("console", "psx_368", (368, 240, false), 59_826, 1_333),
    other(
        "console hi-res",
        "psx_512",
        (512, 240, false),
        59_826,
        1_333,
    ),
    other(
        "console hi-res",
        "psx_640",
        (640, 240, false),
        59_826,
        1_333,
    ),
    other(
        "console hi-res",
        "psx_512i",
        (512, 480, true),
        59_940,
        1_333,
    ),
    other(
        "console hi-res",
        "psx_640i",
        (640, 480, true),
        59_940,
        1_333,
    ),
    // PlayStation PAL: 256 lines at 49.761 Hz, 512 interlaced at 50 Hz.
    // UNVERIFIED.
    other("PAL", "psx_pal_320", (320, 256, false), 49_761, 1_333),
    other("PAL", "psx_pal_640i", (640, 512, true), 50_000, 1_333),
    // Saturn NTSC. UNVERIFIED: the rates the core reports.
    other("console", "saturn_352", (352, 224, false), 59_826, 1_333),
    other(
        "console hi-res",
        "saturn_704i",
        (704, 480, true),
        59_940,
        1_333,
    ),
    // Nintendo 64 NTSC. UNVERIFIED: the rates the core reports.
    other("console", "n64_320", (320, 240, false), 60_000, 1_333),
    other(
        "console hi-res",
        "n64_640i",
        (640, 480, true),
        59_940,
        1_333,
    ),
    other("PAL", "nes_pal", (256, 240, false), 50_007, 1_333),
    other("PAL", "snes_pal", (256, 239, false), 50_007, 1_333),
    other("PAL", "md_pal_h40", (320, 240, false), 49_701, 1_333),
    // Handhelds with their own display aspect: Game Boy 10:9, Game Boy
    // Advance 3:2, WonderSwan 14:9 (the timings of systems.toml); Lynx
    // 160x102 at 75 Hz (UNVERIFIED: Handy's rate), Neo Geo Pocket 160x152
    // at 60.25 Hz (UNVERIFIED: the core's rate), Virtual Boy 384x224 at
    // 50.27 Hz (Mednafen's rate, UNVERIFIED), square pixels.
    other("handheld", "game_boy", (160, 144, false), 59_728, 1_111),
    other("handheld", "gba", (240, 160, false), 59_728, 1_500),
    other("handheld", "wonderswan", (224, 144, false), 75_472, 1_556),
    other("handheld", "lynx", (160, 102, false), 75_000, 1_569),
    other("handheld", "ngp", (160, 152, false), 60_253, 1_053),
    other("handheld", "virtual_boy", (384, 224, false), 50_270, 1_714),
    // Computers: C64 PAL with its border (VICE, 384x272, UNVERIFIED),
    // Amiga PAL 320x256 and 640x512 interlaced (UNVERIFIED: 49.920 Hz is
    // PUAE's progressive rate), DOS VGA 320x200 and 640x400 at 70.086 Hz
    // (25.175 MHz, 800 x 449), MSX and Atari 2600 (TMS9918 and TIA NTSC,
    // 59.923 Hz), ZX Spectrum.
    other(
        "computer",
        "c64_pal_border",
        (384, 272, false),
        50_125,
        1_333,
    ),
    other("computer", "amiga_pal", (320, 256, false), 49_920, 1_333),
    other("computer", "amiga_pal_i", (640, 512, true), 50_000, 1_333),
    other("computer", "vga_mode13h", (320, 200, false), 70_086, 1_333),
    other("computer", "dos_640x400", (640, 400, false), 70_086, 1_333),
    other("computer", "msx", (256, 192, false), 59_923, 1_333),
    other("computer", "atari_2600", (160, 192, false), 59_923, 1_333),
    other("computer", "zx_spectrum", (256, 192, false), 50_080, 1_333),
    // 24 kHz and the rate extremes of the arcade boards: System 24 (MAME
    // segas24.cpp, 57.524 Hz, 24.4 kHz) horizontal and vertical are in
    // BOARDS; Sky Fox (FBNeo d_skyfox.cpp: 224x320 upright, vertical and
    // flipped, 62.65 Hz) is the fastest board found; Little Robin
    // (30.00 Hz, in BOARDS) the slowest.
    Board {
        name: "skyfox",
        width: 320,
        height: 224,
        refresh_mhz: 62_650,
        rotation: Rotation::R270,
        interlaced: false,
        aspect_milli: 1_333,
        arcade: true,
        class: "vertical arcade",
    },
    other("computer", "vga_640x480", (640, 480, false), 59_940, 1_333),
    other(
        "console hi-res",
        "ntsc_480i",
        (640, 480, true),
        59_940,
        1_333,
    ),
];

impl Board {
    pub fn source(&self) -> SourceGeometry {
        SourceGeometry {
            width: self.width,
            height: self.height,
            refresh_mhz: self.refresh_mhz,
            aspect_milli: self.aspect_milli,
            interlaced: self.interlaced,
            rotation: self.rotation,
        }
    }
}

/// A display of the table: its profile and the GPU it is driven by.
pub struct Display {
    pub profile: MonitorProfile,
    pub caps: GpuCaps,
}

/// Every display of the table, by class: the 15 kHz presets, the
/// multi-range presets, the 31 kHz presets, the EDID tubes, the panels and
/// the TV standards. The CRT presets use a GPU with no pixel clock floor
/// that sends interlace, as the policy goldens do; a floor changes the
/// width of a mode, not its lines.
pub fn displays() -> Vec<Display> {
    let presets = [
        "generic_15",
        "arcade_15",
        "arcade_15ex",
        "h9110",
        "polo",
        "k7131",
        "k7000",
        "ntsc",
        "pal",
        "arcade_15_25",
        "arcade_15_25_31",
        "arcade_15_31",
        "m3129",
        "d9200",
        "d9400",
        "d9800",
        "ms929",
        "ms2930",
        "r666b",
        "pstar",
        "arcade_25",
        "arcade_31",
        "pc_31_120",
        "pc_70_120",
        "m2929",
    ];
    let mut displays: Vec<Display> = presets
        .iter()
        .map(|id| Display {
            profile: preset(id),
            caps: open_caps(),
        })
        .collect();
    displays.extend(edid_tubes());
    displays.extend(panels());
    let composite = gpu_table()
        .into_iter()
        .find(|(id, _)| id == "pi_vc4_composite")
        .map(|(_, caps)| caps)
        .expect("pi_vc4_composite is in data/hardware/matrix.toml");
    for standard in [TvStandard::Ntsc, TvStandard::Pal] {
        displays.push(Display {
            profile: standard.profile(),
            caps: composite,
        });
    }
    displays
}

/// Profiles that `neptron_platform::profile_from_edid` builds for typical
/// PC tubes, from their EDID range limits: line rates in kHz, field rates
/// in Hz, the pixel clock limit in MHz. The GPU's pixel clock is limited
/// to the EDID's, as `EdidProfile::limit_caps` does.
pub fn edid_tubes() -> Vec<Display> {
    let tubes = [
        // A 14 inch VGA tube that holds only 31 kHz.
        ("edid_31", (31, 32), (50, 75), 40),
        // A 15 inch tube, 1024x768 at 60 Hz.
        ("edid_30_54", (30, 54), (50, 120), 80),
        // A 17 inch tube, 1280x1024 at 60 Hz.
        ("edid_30_70", (30, 70), (50, 160), 110),
        // A 21 inch tube, 1600x1200 at 75 Hz.
        ("edid_30_96", (30, 96), (50, 160), 230),
    ];
    tubes
        .iter()
        .map(|&(id, h, v, pclock_mhz)| Display {
            profile: edid_tube(id, h, v),
            caps: GpuCaps {
                pclock_max_hz: pclock_mhz * 1_000_000,
                ..open_caps()
            },
        })
        .collect()
}

/// The CRT profile of `profile_from_edid` for range limits of `h_khz`
/// and `v_hz`. A copy of `crt_range` and the `gtf` module of
/// neptron-platform, which this crate may not depend on; the test
/// `the_edid_copy_gives_the_platform_profile` holds it to the platform's
/// own result for a 30 to 71 kHz tube.
pub fn edid_tube(id: &str, h_khz: (u64, u64), v_hz: (u64, u64)) -> MonitorProfile {
    const NS_MHZ: u128 = 1_000_000_000_000;
    let (lo, hi) = (
        u128::from(h_khz.0) * 1_000_000,
        u128::from(h_khz.1) * 1_000_000,
    );
    let (v_min, v_max) = (u128::from(v_hz.0) * 1_000, u128::from(v_hz.1) * 1_000);
    // GTF defaults: C' = 30 %, M' = 300, sync 8 % of the line.
    let back_a = 30 * NS_MHZ;
    let front_a = (30 - 16) * NS_MHZ;
    let b = 300 * 1_000_000_000_000_000_000_u128;
    let half_blank = |a: u128| {
        if a * lo <= 2 * b && 2 * b <= a * hi {
            (a * a).div_ceil(4 * b * 200)
        } else {
            let at = |h: u128| (a * h).saturating_sub(b).div_ceil(200 * h * h);
            at(lo).max(at(hi))
        }
    };
    let v_front = NS_MHZ.div_ceil(lo);
    let v_sync = (3 * NS_MHZ).div_ceil(lo);
    let v_back = 550_000_u128.saturating_sub(3 * NS_MHZ / hi);
    let blank = v_front + v_sync + v_back;
    let active = |v: u128, h: u128, up: bool| {
        let numerator = (NS_MHZ - blank * v) * h;
        let denominator = v * NS_MHZ;
        if up {
            numerator.div_ceil(denominator)
        } else {
            numerator / denominator
        }
    };
    let narrow = |x: u128| u32::try_from(x).unwrap();
    let lines = |x: u128| u16::try_from(x).unwrap();
    MonitorProfile {
        id: id.into(),
        kind: MonitorKind::Crt {
            ranges: vec![CrtRange {
                h_min_mhz: u64::try_from(lo).unwrap(),
                h_max_mhz: u64::try_from(hi).unwrap(),
                v_min_mhz: u64::try_from(v_min).unwrap(),
                v_max_mhz: u64::try_from(v_max).unwrap(),
                h_front_porch_ns: narrow(half_blank(front_a)),
                h_sync_ns: narrow((8 * NS_MHZ).div_ceil(100 * lo)),
                h_back_porch_ns: narrow(half_blank(back_a)),
                v_front_porch_ns: narrow(v_front),
                v_sync_ns: narrow(v_sync),
                v_back_porch_ns: narrow(v_back),
                hsync_positive: false,
                vsync_positive: true,
                progressive_lines_min: lines(active(v_max, lo, true).max(1)),
                progressive_lines_max: lines(active(v_min, hi, false)),
                interlaced_lines_min: 0,
                interlaced_lines_max: 0,
            }],
        },
    }
}

/// Flat panels with their preferred mode at 60 Hz (CEA-861 or CVT reduced
/// blanking timings), and the 1080p panel of the policy goldens.
pub fn panels() -> Vec<Display> {
    let mode = |pclock_hz, h: [u16; 4], v: [u16; 4]| Modeline {
        pclock_hz,
        hactive: h[0],
        hsync_start: h[1],
        hsync_end: h[2],
        htotal: h[3],
        vactive: v[0],
        vsync_start: v[1],
        vsync_end: v[2],
        vtotal: v[3],
        interlace: false,
        doublescan: false,
        hsync_positive: true,
        vsync_positive: true,
    };
    let panel = |id: &str, modes: Vec<Modeline>| Display {
        profile: MonitorProfile {
            id: id.into(),
            kind: MonitorKind::FlatPanel { modes },
        },
        caps: open_caps(),
    };
    vec![
        panel(
            "panel_720",
            vec![mode(
                74_250_000,
                [1280, 1390, 1430, 1650],
                [720, 725, 730, 750],
            )],
        ),
        panel(
            "panel_768",
            vec![mode(
                85_500_000,
                [1366, 1436, 1579, 1792],
                [768, 771, 774, 798],
            )],
        ),
        Display {
            profile: panel_1080(),
            caps: open_caps(),
        },
        panel(
            "panel_1440",
            vec![mode(
                241_500_000,
                [2560, 2608, 2640, 2720],
                [1440, 1443, 1448, 1481],
            )],
        ),
        panel(
            "panel_2160",
            vec![mode(
                594_000_000,
                [3840, 4016, 4104, 4400],
                [2160, 2168, 2178, 2250],
            )],
        ),
    ]
}

/// The largest shape error that a cell may have and still keep the game's
/// shape, in per mille: about one pixel of a 240-pixel side, and the
/// rounding of a mode width to whole pixels.
pub const SHAPE_ERROR_MAX_PERMILLE: i64 = 20;

/// One cell of the table.
pub struct Cell {
    /// The winner's rung, or "none".
    pub rung: String,
    /// Active size and scan, "240x480i".
    pub mode: String,
    /// Total lines of the mode (a frame of both fields when interlaced).
    pub total: u16,
    pub line_rate_mhz: u64,
    pub field_rate_mhz: u64,
    /// The game rate is kept: within 5 mHz on a CRT, or the speed change
    /// in percent on a panel or a TV.
    pub rate: String,
    /// The vertical scale: a whole factor, or a fraction for a fit.
    pub scale: String,
    /// The filter of the game picture: "nearest" or "sharp".
    pub filter: &'static str,
    /// The game's share of the active lines of the mode, in per mille.
    pub height_permille: u32,
    /// The black lines above and below the game, in lines of the mode.
    pub border_lines: u32,
    /// The shape on the screen against the game's own, in per mille: 0 is
    /// the right shape, above 0 is too wide.
    pub shape_error_permille: i64,
    /// The largest whole vertical scale of a proposal in the plan that
    /// keeps the game rate and that the validator accepts, as "3x 960p".
    pub best_whole: String,
    /// "ok" when the winner follows the arcade rule of the spec, "ok: "
    /// and the allowed reason, else the first rule it breaks.
    pub rule: &'static str,
}

/// The settings of an arcade board: the defaults of the schema, with the
/// source marked as an arcade board, as the systems `arcade` and `neogeo`
/// mark it.
pub fn settings() -> PolicySettings {
    PolicySettings {
        arcade: true,
        ..PolicySettings::default()
    }
}

/// The settings of `board`: [`settings`] for an arcade board, the
/// defaults for every other source.
pub fn settings_of(board: &Board) -> PolicySettings {
    PolicySettings {
        arcade: board.arcade,
        ..PolicySettings::default()
    }
}

/// The cell of one board on one display, with [`settings`].
pub fn cell(board: &Board, display: &Display) -> Cell {
    let (profile, caps) = (&display.profile, &display.caps);
    let plan = decide(&board.source(), &settings_of(board), profile, caps)
        .unwrap_or_else(|e| panic!("{} on {}: {e}", board.name, profile.id));
    let best = best_whole(&plan, profile, caps);
    let best_whole = best.map_or_else(
        || if is_crt(profile) { "none" } else { "-" }.to_owned(),
        |(k, progressive, h)| format!("{k}x {h}{}", if progressive { "p" } else { "i" }),
    );
    let choice = choose(&plan, |m| validate(m, profile, caps));
    let Some(winner) = choice.winner else {
        return Cell {
            rung: "none".into(),
            mode: "-".into(),
            total: 0,
            line_rate_mhz: 0,
            field_rate_mhz: 0,
            rate: "-".into(),
            scale: "-".into(),
            filter: "-",
            height_permille: 0,
            border_lines: 0,
            shape_error_permille: 0,
            best_whole,
            rule: no_mode_reason(profile, board),
        };
    };
    let p = winner.proposal;
    let m = &p.mode;
    if is_crt(profile) {
        assert!(
            m.pclock_is_whole_khz(),
            "{} on {}: pixel clock {} Hz",
            board.name,
            profile.id,
            m.pclock_hz
        );
    }
    let d = p.placement.dest;
    let game_lines = u32::from(plan.source.height) - 2 * crop_lines(p);
    let scale = match p.placement.scale.y {
        Some(k) => format!("{k}"),
        None => format!("{:.2}", f64::from(d.h) / f64::from(game_lines)),
    };
    let shape_error_permille = shape_error(&plan, profile, p);
    Cell {
        rung: winner.considered.rung.to_string(),
        mode: format!(
            "{}x{}{}",
            m.hactive,
            m.vactive,
            if m.interlace { "i" } else { "p" }
        ),
        total: m.vtotal,
        line_rate_mhz: p.timing.line_rate_mhz,
        field_rate_mhz: p.timing.field_rate_mhz,
        rate: rate(winner.considered, p),
        scale,
        filter: match p.placement.filter {
            Filter::Nearest => "nearest",
            Filter::SharpBilinear => "sharp",
        },
        height_permille: d.h * 1_000 / u32::from(m.vactive),
        border_lines: u32::from(m.vactive).saturating_sub(d.h),
        shape_error_permille,
        best_whole,
        rule: rule(
            board,
            profile,
            &plan,
            (winner.considered, p),
            shape_error_permille,
            caps,
        ),
    }
}

/// The verdict of the video rule of docs/spec/arcade.md (section 11) on a
/// winner: "ok", "ok: " and an
/// allowed reason, or the first rule that the winner breaks.
///
/// The allowed reasons: no exact mode exists on the profile; the profile
/// declares no interlace; a range has one line rate only; the overscan
/// crop hides edge lines (it keeps the width, so the visible lines lose
/// their shape by the share of the hidden lines).
fn rule(
    board: &Board,
    profile: &MonitorProfile,
    plan: &Plan,
    winner: (&Considered, &Proposal),
    shape_error_permille: i64,
    caps: &GpuCaps,
) -> &'static str {
    let (considered, p) = winner;
    let source = plan.source;
    if considered.rung.is_fitted()
        && let Err(broken) = fit_allowed(profile, plan, p, caps)
    {
        return broken;
    }
    let console_tv = !board.arcade && matches!(profile.kind, MonitorKind::Tv { .. });
    if console_tv && shape_error_permille.abs() > SHAPE_ERROR_MAX_PERMILLE {
        return "ok: console width rule";
    }
    if shape_error_permille.abs() > SHAPE_ERROR_MAX_PERMILLE {
        let crop = i64::from(crop_lines(p)) * 2 * 1_000 / i64::from(source.height);
        if crop == 0 || shape_error_permille.abs() > crop + SHAPE_ERROR_MAX_PERMILLE {
            return "shape";
        }
        return "ok: overscan crop";
    }
    if let MonitorKind::FlatPanel { .. } = profile.kind {
        return panel_rule(plan, p);
    }
    let MonitorKind::Crt { ranges } = &profile.kind else {
        return fit_reason(profile, plan, considered);
    };
    // A fit that no exact mode beats is the last resort, at the rate that
    // the profile reaches.
    if considered.rung.is_fitted() {
        return fit_reason(profile, plan, considered);
    }
    if p.timing.changes_speed() {
        let game = u64::from(board.refresh_mhz);
        let covered = |rate: u64| {
            ranges
                .iter()
                .any(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz)
        };
        if traded_for_window(plan, p, profile, caps) {
            return "ok: no window at the game's rate";
        }
        // Below every range, twice the rate keeps the speed.
        if covered(game) || covered(2 * game) {
            return "rate";
        }
        // A board below every range takes each frame twice at the rate
        // nearest twice its own when that changes its speed less.
        let nearest = |rate: u64| {
            ranges
                .iter()
                .map(|r| rate.clamp(r.v_min_mhz, r.v_max_mhz.max(r.v_min_mhz)))
                .min_by_key(|&c| c.abs_diff(rate))
                .unwrap_or(rate)
        };
        let error = |shown: u64, wanted: u64| shown.abs_diff(wanted) * 1_000_000 / wanted.max(1);
        let won = error(p.timing.field_rate_mhz, game * u64::from(p.timing.repeat()));
        let best = error(nearest(game), game).min(error(nearest(2 * game), 2 * game));
        if won > best + 5_000 {
            return "rate";
        }
    }
    crt_order(plan, p, profile, caps)
}

/// The fit wins on a CRT or a TV only when no exact mode exists: no
/// exact proposal of the plan validates, and the game's lines fit no
/// window of the profile at its rate. On a panel the fit is the panel
/// rule at a scale of 4.0 or more, or a game larger than the mode.
fn fit_allowed(
    profile: &MonitorProfile,
    plan: &Plan,
    p: &Proposal,
    caps: &GpuCaps,
) -> Result<(), &'static str> {
    if let MonitorKind::FlatPanel { .. } = profile.kind {
        let scale = panel_fill_scale_milli(plan, &p.mode);
        return if !(1_000..4_000).contains(&scale) {
            Ok(())
        } else {
            Err("panel scale")
        };
    }
    // An exact mode that changes the game's speed more than the fit by
    // more than the margin (and the 5 mHz match) does not beat it.
    let speed = |q: &Proposal| {
        let shown = q.timing.field_rate_mhz / u64::from(q.timing.repeat().max(1));
        shown.abs_diff(u64::from(q.timing.game_mhz))
    };
    let exact: Vec<&Proposal> = plan
        .proposals()
        .filter(|(_, c, q)| !c.rung.is_fitted() && validate(&q.mode, profile, caps).is_ok())
        .map(|(_, _, q)| q)
        .collect();
    if exact
        .iter()
        .any(|q| speed(q) < speed(p) + margin_mhz(q) + 5)
    {
        return Err("soft fit");
    }
    let lines = u32::from(plan.source.height);
    match &profile.kind {
        MonitorKind::Crt { ranges } => {
            let rates = [plan.source.refresh_mhz, plan.source.refresh_mhz * 2];
            let fits = ranges.iter().any(|r| {
                rates.iter().any(|&rate| {
                    // A rate that the range does not cover becomes the
                    // nearest one it covers, as the policy's `Nearest`.
                    let rate = u64::from(rate).clamp(r.v_min_mhz, r.v_max_mhz);
                    // A window holds the game when its most lines hold the
                    // game and its fewest lines fit the field at the rate.
                    let progressive = u32::from(r.progressive_lines_max).min(field_lines(r, rate));
                    let interlaced =
                        u32::from(r.interlaced_lines_max).min(2 * field_lines(r, rate));
                    let p_window = u32::from(r.progressive_lines_min) <= progressive;
                    let i_window = r.interlaced_lines_max > 0
                        && u32::from(r.interlaced_lines_min) <= interlaced;
                    (p_window && lines <= progressive)
                        || (caps.interlace && i_window && lines <= interlaced)
                })
            });
            if fits { Err("soft fit") } else { Ok(()) }
        }
        // A game taller than the progressive mode of a TV fits into the
        // interlaced mode, which keeps more of its lines.
        MonitorKind::Tv { .. } if !p.mode.interlace && lines > u32::from(p.mode.vactive) => {
            Err("tv fit")
        }
        _ => Ok(()),
    }
}

/// The panel rule: from a scale of 4.0 the game fills the
/// height (or the width) at its shape; under it, the largest whole
/// multiple that fits; a game larger than the mode fits.
fn panel_rule(plan: &Plan, p: &Proposal) -> &'static str {
    let scale = panel_fill_scale_milli(plan, &p.mode);
    let whole = p.placement.scale.y.is_some();
    let d = p.placement.dest;
    let fills = d.h == u32::from(p.mode.vactive) || d.w == u32::from(p.mode.hactive);
    match scale {
        // A fill that is a whole multiple takes the whole path.
        s if s >= 4_000 && !fills => "panel scale",
        s if (1_000..4_000).contains(&s) && !whole => "panel scale",
        _ => "ok",
    }
}

/// The allowed reason of a fitted winner, or "ok" for an exact one.
fn fit_reason(profile: &MonitorProfile, plan: &Plan, considered: &Considered) -> &'static str {
    if !considered.rung.is_fitted() {
        return "ok";
    }
    let exact = plan.proposals().any(|(_, c, _)| !c.rung.is_fitted());
    if exact && !matches!(profile.kind, MonitorKind::FlatPanel { .. }) {
        return "ok: the exact mode changes the speed beyond the margin";
    }
    match &profile.kind {
        MonitorKind::FlatPanel { .. } => "ok: panel fill",
        MonitorKind::Tv { .. } => "ok: no exact mode exists on this profile",
        MonitorKind::Crt { ranges } => {
            let interlace = ranges.iter().any(|r| r.interlaced_lines_max > 0);
            let tall = ranges
                .iter()
                .all(|r| u32::from(plan.source.height) > u32::from(r.progressive_lines_max));
            if !interlace && tall {
                "ok: the profile declares no interlace"
            } else {
                "ok: no exact mode exists on this profile"
            }
        }
    }
}

/// The active lines of one field that a range holds at `rate_mhz`, less
/// its vertical blanking at its highest line rate.
pub fn field_lines(range: &CrtRange, rate_mhz: u64) -> u32 {
    let total = range.h_max_mhz / rate_mhz.max(1);
    let blank_ns = u128::from(range.v_front_porch_ns)
        + u128::from(range.v_sync_ns)
        + u128::from(range.v_back_porch_ns);
    let blank = (blank_ns * u128::from(range.h_max_mhz)).div_ceil(1_000_000_000_000);
    u32::try_from(u128::from(total).saturating_sub(blank)).unwrap_or(u32::MAX)
}

/// The scale at which the game, at its shape, fills the height of a panel
/// mode, or its width when the width comes first, in thousandths.
pub fn panel_fill_scale_milli(plan: &Plan, mode: &Modeline) -> u64 {
    let s = plan.source;
    let aspect = if s.aspect_milli == 0 {
        u64::from(s.width) * 1_000 / u64::from(s.height)
    } else {
        u64::from(s.aspect_milli)
    };
    let by_height = u64::from(mode.vactive) * 1_000 / u64::from(s.height);
    let width_at_1 = aspect * u64::from(s.height) / 1_000;
    let by_width = u64::from(mode.hactive) * 1_000 / width_at_1.max(1);
    by_height.min(by_width)
}

/// The fill of an exact proposal in per mille of the picture lines: the
/// padding's share, or 1,000 when the mode is not padded.
fn fill_permille(p: &Proposal) -> u32 {
    p.padding.map_or(1_000, |pad| pad.fill_permille())
}

/// The order of the exact modes on a CRT at the winner's rate: the
/// smallest whole multiple that fills the screen (decision A7); a
/// progressive mode of at least 60 % before an interlaced one; below 60 %,
/// an interlaced 1:1 mode that fills more (decision A8).
fn crt_order(plan: &Plan, p: &Proposal, profile: &MonitorProfile, caps: &GpuCaps) -> &'static str {
    let rate = p.timing.target.rate_mhz;
    let others: Vec<&Proposal> = plan
        .proposals()
        .filter(|(_, c, q)| {
            !c.rung.is_fitted()
                && q.timing.target.rate_mhz == rate
                && validate(&q.mode, profile, caps).is_ok()
        })
        .map(|(_, _, q)| q)
        .collect();
    let fill = fill_permille(p);
    let k = p.placement.scale.y.unwrap_or(1);
    let full = |q: &Proposal| !q.mode.interlace && fill_permille(q) >= 750;
    if fill < 750 && others.iter().any(|q| full(q)) {
        return "smaller picture";
    }
    let smaller_full = others
        .iter()
        .filter(|q| full(q))
        .any(|q| q.overscan_crop.is_none() && q.placement.scale.y.unwrap_or(1) < k);
    if !p.mode.interlace && p.overscan_crop.is_none() && smaller_full {
        return "larger multiple";
    }
    // An interlaced source shows interlaced 1:1 as its own scan.
    if p.mode.interlace
        && !plan.source.interlaced
        && others
            .iter()
            .any(|q| !q.mode.interlace && fill_permille(q) >= 600)
    {
        return "interlace";
    }
    if fill < 600
        && others
            .iter()
            .any(|q| q.mode.interlace && fill_permille(q) > fill)
    {
        return "small picture";
    }
    "ok"
}

/// The allowed reason of a cell with no mode: a range of one line rate
/// covers the game's rate, so the whole-kHz pixel clock may find no mode
/// inside it; else no mode breaks the rule.
pub fn no_mode_reason(profile: &MonitorProfile, board: &Board) -> &'static str {
    let rate = u64::from(board.refresh_mhz);
    match &profile.kind {
        MonitorKind::Crt { ranges }
            if ranges.iter().any(|r| {
                r.h_min_mhz == r.h_max_mhz && r.v_min_mhz <= rate && rate <= r.v_max_mhz
            }) =>
        {
            "ok: one line rate only"
        }
        _ => "no mode",
    }
}

fn is_crt(profile: &MonitorProfile) -> bool {
    matches!(profile.kind, MonitorKind::Crt { .. })
}

pub fn crop_lines(p: &Proposal) -> u32 {
    p.overscan_crop.map_or(0, |c| u32::from(c.lines_per_edge))
}

fn rate(considered: &Considered, p: &Proposal) -> String {
    let kind = considered.target.kind;
    let shown = p.timing.field_rate_mhz as f64;
    let game = (u64::from(p.timing.game_mhz) * u64::from(p.timing.repeat())) as f64;
    match kind {
        RefreshKind::Panel | RefreshKind::Tv => {
            format!("{:+.2} %", (shown / game - 1.0) * 100.0)
        }
        _ if !p.timing.changes_speed() && p.timing.repeat() == 2 => "exact x2".into(),
        _ if !p.timing.changes_speed() => "exact".into(),
        _ => format!("{kind} {:+.2} %", (shown / game - 1.0) * 100.0),
    }
}

/// The shape on the screen over the game's own shape, less 1, in per
/// mille. A CRT shows its active area on a 4:3 face; a TV shows 704 of
/// its 720 samples as 4:3; a panel has square pixels.
pub fn shape_error(plan: &Plan, profile: &MonitorProfile, p: &Proposal) -> i64 {
    let d = p.placement.dest;
    let m = &p.mode;
    let height_share = f64::from(d.h) / f64::from(m.vactive);
    let shown = match profile.kind {
        MonitorKind::FlatPanel { .. } => f64::from(d.w) / f64::from(d.h),
        MonitorKind::Tv { .. } => f64::from(d.w) / 704.0 * 4.0 / 3.0 / height_share,
        MonitorKind::Crt { .. } => f64::from(d.w) / f64::from(m.hactive) * 4.0 / 3.0 / height_share,
    };
    let s = plan.source;
    let own = if s.aspect_milli == 0 {
        f64::from(s.width) / f64::from(s.height)
    } else {
        f64::from(s.aspect_milli) / 1_000.0
    };
    // The crop hides lines, so the game's own shape is that of the rest.
    let own = own * f64::from(s.height) / f64::from(u32::from(s.height) - 2 * crop_lines(p));
    ((shown / own - 1.0) * 1_000.0).round() as i64
}

/// The largest whole vertical scale among the exact proposals of a CRT
/// plan that keep the game rate and that the validator accepts: the
/// factor, whether the mode is progressive, and the game's lines.
fn best_whole(plan: &Plan, profile: &MonitorProfile, caps: &GpuCaps) -> Option<(u16, bool, u32)> {
    if !is_crt(profile) {
        return None;
    }
    plan.proposals()
        .filter(|(_, c, p)| {
            matches!(c.rung, Rung::Native | Rung::Multiple(_) | Rung::Interlaced)
                && !p.timing.changes_speed()
                && validate(&p.mode, profile, caps).is_ok()
        })
        .filter_map(|(_, _, p)| {
            p.placement
                .scale
                .y
                .map(|k| (k, !p.mode.interlace, p.placement.dest.h))
        })
        .max()
}

fn khz(mhz: u64) -> String {
    format!("{}.{:03}", mhz / 1_000_000, mhz % 1_000_000 / 1_000)
}

fn hz(mhz: u64) -> String {
    format!("{}.{:03}", mhz / 1_000, mhz % 1_000)
}

fn rotation(rotation: Rotation) -> &'static str {
    match rotation {
        Rotation::R0 => "R0",
        Rotation::R90 => "R90",
        Rotation::R180 => "R180",
        Rotation::R270 => "R270",
    }
}

/// The table of one board, as Markdown.
pub fn board_table(board: &Board, displays: &[Display]) -> String {
    let mut out = format!(
        "### `{}`: {}x{} frame, {}, {} Hz\n\n",
        board.name,
        board.width,
        board.height,
        rotation(board.rotation),
        hz(u64::from(board.refresh_mhz))
    );
    out.push_str(
        "| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |\n",
    );
    out.push_str(
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for display in displays {
        let c = cell(board, display);
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {}.{} % | {} | {}{}.{} % | {} | {} |\n",
            display.profile.id,
            c.rung,
            c.mode,
            c.total,
            khz(c.line_rate_mhz),
            hz(c.field_rate_mhz),
            c.rate,
            c.scale,
            c.filter,
            c.height_permille / 10,
            c.height_permille % 10,
            c.border_lines,
            if c.shape_error_permille < 0 { "-" } else { "+" },
            c.shape_error_permille.abs() / 10,
            c.shape_error_permille.abs() % 10,
            c.best_whole,
            c.rule,
        ));
    }
    out
}

#[test]
fn the_edid_copy_gives_the_platform_profile() {
    let mut built = edid_tube("edid_npt_0071", (30, 71), (50, 160));
    built.id = pc_crt_30_71().id;
    assert_eq!(built, pc_crt_30_71());
}

/// Every cell has a plan, and every CRT winner a whole-kHz pixel clock
/// (asserted in [`cell`]). The output is the table of docs/spec/arcade.md.
#[test]
fn arcade_table() {
    let displays = displays();
    let mut out = String::new();
    for board in &BOARDS {
        out.push_str(&board_table(board, &displays));
        out.push('\n');
    }
    println!("{out}");
}

/// The class of a display for the summary.
pub fn display_class(profile: &MonitorProfile) -> &'static str {
    match &profile.kind {
        MonitorKind::FlatPanel { .. } => "panel",
        MonitorKind::Tv { .. } => "TV",
        MonitorKind::Crt { ranges }
            if profile.id.starts_with("edid_3") && ranges[0].h_max_mhz > 32_000_000 =>
        {
            "multi-sync EDID"
        }
        MonitorKind::Crt { .. } if profile.id == "edid_31" => "31 kHz",
        MonitorKind::Crt { ranges } if ranges.len() > 1 => "multi-range preset",
        MonitorKind::Crt { ranges } => match ranges[0].h_max_mhz {
            h if h < 17_000_000 => "15 kHz",
            h if h < 26_000_000 => "25 kHz",
            h if h < 33_000_000 => "31 kHz",
            _ => "multi-sync preset",
        },
    }
}

/// The verdicts of the rule by source class and display class, as
/// Markdown: one row per pair, with the count of cells per verdict.
pub fn summary(boards: &[&Board], displays: &[Display]) -> String {
    let mut counts: BTreeMap<(&str, &str), BTreeMap<&str, usize>> = BTreeMap::new();
    for board in boards {
        for display in displays {
            let verdict = cell(board, display).rule;
            *counts
                .entry((board.class, display_class(&display.profile)))
                .or_default()
                .entry(verdict)
                .or_default() += 1;
        }
    }
    let mut out = String::from(
        "| Source class | Display class | Cells | Verdicts |\n| --- | --- | --- | --- |\n",
    );
    for ((source, display), verdicts) in &counts {
        let total: usize = verdicts.values().sum();
        let list: Vec<String> = verdicts.iter().map(|(v, n)| format!("{v}: {n}")).collect();
        out.push_str(&format!(
            "| {source} | {display} | {total} | {} |\n",
            list.join(", ")
        ));
    }
    out
}

/// The wider table: every source of [`SOURCES`] on every display, then
/// the summary by class of every source, the boards included.
#[test]
fn sources_table() {
    let displays = displays();
    let mut out = String::new();
    for board in &SOURCES {
        out.push_str(&board_table(board, &displays));
        out.push('\n');
    }
    let all: Vec<&Board> = BOARDS.iter().chain(SOURCES.iter()).collect();
    out.push_str(&summary(&all, &displays));
    println!("{out}");
}

/// How much more an exact mode may change the game's speed than a fit,
/// in millihertz of the game's rate of `p`.
pub fn margin_mhz(p: &Proposal) -> u64 {
    u64::from(p.timing.game_mhz) * EXACT_SPEED_MARGIN_PPM / 1_000_000
}

/// The winner trades the game's rate for an exact picture: no exact mode
/// at the game's rate validates, the winner is exact at the nearest rate
/// where a range holds the game, and no fit that validates changes the
/// speed less by more than the margin.
pub fn traded_for_window(
    plan: &Plan,
    p: &Proposal,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> bool {
    // The speed change in millihertz; within the 5 mHz match two changes
    // count as equal.
    let speed = |q: &Proposal| {
        let shown = q.timing.field_rate_mhz / u64::from(q.timing.repeat().max(1));
        shown.abs_diff(u64::from(q.timing.game_mhz))
    };
    let better_fit = plan.proposals().any(|(_, c, q)| {
        c.rung.is_fitted()
            && speed(q) + margin_mhz(p) + 5 <= speed(p)
            && validate(&q.mode, profile, caps).is_ok()
    });
    // The rate the walk aims for first: the game's own, or the nearest
    // covered one. No exact mode there validates.
    let base = plan
        .considered
        .iter()
        .filter(|c| matches!(c.target.kind, RefreshKind::Exact | RefreshKind::Nearest))
        .filter(|c| c.target.repeat == 1)
        .map(|c| c.target.rate_mhz)
        .max()
        .unwrap_or(0);
    let exact_at_base = plan.proposals().any(|(_, c, q)| {
        !c.rung.is_fitted()
            && c.target.rate_mhz == base
            && c.target.repeat == 1
            && validate(&q.mode, profile, caps).is_ok()
    });
    p.timing.target.kind == RefreshKind::Nearest
        && p.timing.target.rate_mhz < base
        && !exact_at_base
        && !better_fit
}
